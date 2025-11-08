//! Callback registration and invocation for FFI
//!
//! This module handles the registration, unregistration, and management
//! of callbacks for the FFI layer.

use crate::ffi::handles::{CLIENTS, CallbackRegistration};
use crate::ffi::helpers::validate_mut_ptr;
use crate::ffi::types::{AniDBCallbackType, AniDBResult};
use crate::ffi_catch_panic;
use std::ffi::c_void;
use std::sync::atomic::Ordering;

/// Register a callback with the client
#[unsafe(no_mangle)]
pub extern "C" fn anidb_register_callback(
    handle: *mut c_void,
    callback_type: AniDBCallbackType,
    callback: *mut c_void,
    user_data: *mut c_void,
) -> u64 {
    if !validate_mut_ptr(handle) || !validate_mut_ptr(callback) {
        return 0;
    }

    let handle_id = handle as usize;
    if handle_id == 0 || handle_id > usize::MAX / 2 {
        return 0;
    }

    let clients = match CLIENTS.read() {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let client_arc = match clients.get(&handle_id) {
        Some(c) => c.clone(),
        None => return 0,
    };

    drop(clients);

    let client = match client_arc.lock() {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let callback_id = client.next_callback_id.fetch_add(1, Ordering::SeqCst);

    let registration = CallbackRegistration {
        callback_type,
        callback_ptr: callback,
        user_data,
    };

    if let Ok(mut callbacks) = client.callbacks.lock() {
        callbacks.insert(callback_id, registration);
        callback_id
    } else {
        0
    }
}

/// Unregister a callback
#[unsafe(no_mangle)]
pub extern "C" fn anidb_unregister_callback(handle: *mut c_void, callback_id: u64) -> AniDBResult {
    ffi_catch_panic!({
        if !validate_mut_ptr(handle) || callback_id == 0 {
            return AniDBResult::ErrorInvalidParameter;
        }

        let handle_id = handle as usize;
        if handle_id == 0 || handle_id > usize::MAX / 2 {
            return AniDBResult::ErrorInvalidHandle;
        }

        let clients = match CLIENTS.read() {
            Ok(c) => c,
            Err(_) => return AniDBResult::ErrorBusy,
        };

        let client_arc = match clients.get(&handle_id) {
            Some(c) => c.clone(),
            None => return AniDBResult::ErrorInvalidHandle,
        };

        drop(clients);

        let client = match client_arc.lock() {
            Ok(c) => c,
            Err(_) => return AniDBResult::ErrorBusy,
        };

        if let Ok(mut callbacks) = client.callbacks.lock() {
            if callbacks.remove(&callback_id).is_some() {
                AniDBResult::Success
            } else {
                AniDBResult::ErrorInvalidParameter
            }
        } else {
            AniDBResult::ErrorBusy
        }
    })
}
