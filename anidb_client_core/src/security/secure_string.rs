//! Secure string implementation with automatic memory zeroing
//!
//! This module provides a SecureString type that automatically zeros its memory
//! when dropped and provides constant-time comparison.

use std::fmt;
use zeroize::Zeroize;

/// A string that zeros its memory when dropped
///
/// This type is designed to hold sensitive data like passwords and API keys.
/// It provides the following security features:
/// - Automatic memory zeroing on drop
/// - No Debug implementation that could leak data
/// - Constant-time comparison
/// - Memory locking on supported platforms
#[derive(Clone, Zeroize)]
pub struct SecureString {
    inner: Vec<u8>,
}

impl SecureString {
    /// Create a new SecureString from a regular string
    pub fn new(s: impl Into<String>) -> Self {
        let string = s.into();
        let inner = string.into_bytes();

        // Try to lock memory if supported
        #[cfg(unix)]
        Self::try_lock_memory(&inner);

        Self { inner }
    }

    /// Create a SecureString from bytes
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        #[cfg(unix)]
        Self::try_lock_memory(&bytes);

        Self { inner: bytes }
    }

    /// Get the string as a byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.inner
    }

    /// Get the string as a str reference
    ///
    /// # Panics
    /// Panics if the bytes are not valid UTF-8
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.inner).expect("SecureString contains invalid UTF-8")
    }

    /// Try to get the string as a str reference
    pub fn to_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.inner)
    }

    /// Convert to a regular String (use with caution)
    ///
    /// This creates a copy that is NOT automatically zeroed.
    /// Only use this when absolutely necessary.
    pub fn expose_secret(&self) -> String {
        String::from_utf8_lossy(&self.inner).into_owned()
    }

    /// Constant-time comparison
    pub fn constant_time_eq(&self, other: &Self) -> bool {
        if self.inner.len() != other.inner.len() {
            return false;
        }

        let mut result = 0u8;
        for (a, b) in self.inner.iter().zip(other.inner.iter()) {
            result |= a ^ b;
        }
        result == 0
    }

    /// Try to lock memory pages containing the secure data
    #[cfg(unix)]
    fn try_lock_memory(data: &[u8]) {
        use libc::{_SC_PAGESIZE, mlock, sysconf};

        unsafe {
            let page_size = sysconf(_SC_PAGESIZE) as usize;
            let ptr = data.as_ptr() as *const libc::c_void;
            let len = data.len();

            // Align to page boundary
            let aligned_ptr = (ptr as usize & !(page_size - 1)) as *const libc::c_void;
            let offset = ptr as usize - aligned_ptr as usize;
            let aligned_len = offset + len;

            // Ignore errors - memory locking is best-effort
            let _ = mlock(aligned_ptr, aligned_len);
        }
    }

    /// Try to lock memory pages containing the secure data
    #[cfg(windows)]
    fn try_lock_memory(data: &[u8]) {
        // TODO: Implement Windows memory locking when winapi is added
        // For now, this is a no-op on Windows
        let _ = data;
    }

    /// Unlock memory pages when dropping
    #[cfg(unix)]
    fn try_unlock_memory(&self) {
        use libc::{_SC_PAGESIZE, munlock, sysconf};

        unsafe {
            let page_size = sysconf(_SC_PAGESIZE) as usize;
            let ptr = self.inner.as_ptr() as *const libc::c_void;
            let len = self.inner.len();

            // Align to page boundary
            let aligned_ptr = (ptr as usize & !(page_size - 1)) as *const libc::c_void;
            let offset = ptr as usize - aligned_ptr as usize;
            let aligned_len = offset + len;

            // Ignore errors
            let _ = munlock(aligned_ptr, aligned_len);
        }
    }

    /// Unlock memory pages when dropping
    #[cfg(windows)]
    fn try_unlock_memory(&self) {
        // TODO: Implement Windows memory unlocking when winapi is added
        // For now, this is a no-op on Windows
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        // Try to unlock memory before zeroing
        #[cfg(any(unix, windows))]
        self.try_unlock_memory();

        // Manually zero the memory
        self.inner.zeroize();
    }
}

// Implement Debug to prevent accidental credential logging
impl fmt::Debug for SecureString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecureString(***)")
    }
}

// Implement Display to prevent accidental credential logging
impl fmt::Display for SecureString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

impl PartialEq for SecureString {
    fn eq(&self, other: &Self) -> bool {
        self.constant_time_eq(other)
    }
}

impl Eq for SecureString {}

// Implement From traits for convenience
impl From<String> for SecureString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SecureString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<Vec<u8>> for SecureString {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_string_creation() {
        let secure = SecureString::new("test password");
        assert_eq!(secure.as_str(), "test password");
    }

    #[test]
    fn test_secure_string_debug() {
        let secure = SecureString::new("secret password");
        let debug_str = format!("{secure:?}");
        assert_eq!(debug_str, "SecureString(***)");
        assert!(!debug_str.contains("secret"));
    }

    #[test]
    fn test_secure_string_display() {
        let secure = SecureString::new("secret password");
        let display_str = format!("{secure}");
        assert_eq!(display_str, "***");
        assert!(!display_str.contains("secret"));
    }

    #[test]
    fn test_constant_time_comparison() {
        let secure1 = SecureString::new("password123");
        let secure2 = SecureString::new("password123");
        let secure3 = SecureString::new("different");

        assert!(secure1.constant_time_eq(&secure2));
        assert!(!secure1.constant_time_eq(&secure3));

        // Test PartialEq implementation
        assert_eq!(secure1, secure2);
        assert_ne!(secure1, secure3);
    }

    #[test]
    fn test_from_bytes() {
        let bytes = b"test data".to_vec();
        let secure = SecureString::from_bytes(bytes);
        assert_eq!(secure.as_bytes(), b"test data");
    }

    #[test]
    fn test_expose_secret() {
        let secure = SecureString::new("sensitive data");
        let exposed = secure.expose_secret();
        assert_eq!(exposed, "sensitive data");
    }

    #[test]
    fn test_clone() {
        let secure1 = SecureString::new("cloneable");
        let secure2 = secure1.clone();
        assert_eq!(secure1, secure2);
    }
}
