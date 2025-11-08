//! Packet fragmentation handling
//!
//! This module handles reassembly of fragmented UDP responses from AniDB.

use crate::protocol::error::{ProtocolError, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Header information for fragmented packets
#[derive(Debug, Clone, PartialEq)]
pub struct FragmentHeader {
    /// Unique identifier for this fragmented message
    pub message_id: u32,
    /// Current fragment number (0-based)
    pub fragment_num: u16,
    /// Total number of fragments
    pub total_fragments: u16,
}

impl FragmentHeader {
    /// Parse fragment header from response
    /// Format: "7xx FRAGMENT {message_id} {fragment_num}/{total_fragments}"
    pub fn parse(response: &str) -> Option<Self> {
        let parts: Vec<&str> = response.split_whitespace().collect();

        if parts.len() < 4 || !parts[0].starts_with('7') {
            return None;
        }

        let message_id = parts[2].parse().ok()?;

        let frag_info: Vec<&str> = parts[3].split('/').collect();
        if frag_info.len() != 2 {
            return None;
        }

        let fragment_num = frag_info[0].parse().ok()?;
        let total_fragments = frag_info[1].parse().ok()?;

        Some(FragmentHeader {
            message_id,
            fragment_num,
            total_fragments,
        })
    }

    /// Check if this is the last fragment
    pub fn is_last(&self) -> bool {
        self.fragment_num == self.total_fragments - 1
    }

    /// Check if this is the first fragment
    pub fn is_first(&self) -> bool {
        self.fragment_num == 0
    }
}

/// Fragment assembly state
#[derive(Debug)]
struct AssemblyState {
    /// Fragments received so far
    fragments: HashMap<u16, String>,
    /// Total expected fragments
    total_fragments: u16,
    /// When assembly started
    started_at: Instant,
    /// Original command that triggered the response
    #[allow(dead_code)]
    command: Option<String>,
}

impl AssemblyState {
    fn new(total_fragments: u16) -> Self {
        Self {
            fragments: HashMap::new(),
            total_fragments,
            started_at: Instant::now(),
            command: None,
        }
    }

    /// Add a fragment
    fn add_fragment(&mut self, num: u16, data: String) -> Result<()> {
        if num >= self.total_fragments {
            return Err(ProtocolError::fragmentation(format!(
                "Fragment number {num} exceeds total {}",
                self.total_fragments
            )));
        }

        // Extract the data part (skip the fragment header line)
        // All fragments include a fragment header that needs to be stripped
        let lines: Vec<&str> = data.lines().collect();
        let data_part = if lines.len() > 1
            && (lines[0].starts_with("701 FRAGMENT") || lines[0].starts_with("702 FRAGMENT"))
        {
            // Skip the fragment header line
            lines[1..].join("\n")
        } else {
            data
        };

        self.fragments.insert(num, data_part);
        Ok(())
    }

    /// Check if all fragments have been received
    fn is_complete(&self) -> bool {
        self.fragments.len() == self.total_fragments as usize
    }

    /// Assemble all fragments into complete response
    fn assemble(&self) -> Result<String> {
        if !self.is_complete() {
            return Err(ProtocolError::fragmentation(format!(
                "Missing fragments: have {}, need {}",
                self.fragments.len(),
                self.total_fragments
            )));
        }

        let mut result = String::new();

        for i in 0..self.total_fragments {
            let fragment = self
                .fragments
                .get(&i)
                .ok_or_else(|| ProtocolError::fragmentation(format!("Missing fragment {i}")))?;

            // Fragments have already been cleaned up in add_fragment
            result.push_str(fragment);

            if i < self.total_fragments - 1 {
                result.push('\n');
            }
        }

        Ok(result)
    }
}

/// Fragment assembler for managing fragmented responses
pub struct FragmentAssembler {
    /// Active assembly states by message ID
    states: HashMap<u32, AssemblyState>,
    /// Timeout for fragment assembly
    timeout: Duration,
    /// Maximum concurrent assemblies
    max_assemblies: usize,
}

impl FragmentAssembler {
    /// Create a new fragment assembler
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            timeout: Duration::from_secs(30),
            max_assemblies: 10,
        }
    }

    /// Process a potentially fragmented response
    pub fn process(&mut self, response: &str) -> Result<Option<String>> {
        // First check if this is a fragmented response
        if let Some(header) = FragmentHeader::parse(response) {
            self.handle_fragment(header, response)
        } else {
            // Not fragmented, return as-is
            Ok(Some(response.to_string()))
        }
    }

    /// Handle a fragmented response
    fn handle_fragment(
        &mut self,
        header: FragmentHeader,
        response: &str,
    ) -> Result<Option<String>> {
        // Clean up old assemblies
        self.cleanup_expired();

        // Check assembly limit
        if !self.states.contains_key(&header.message_id) && self.states.len() >= self.max_assemblies
        {
            return Err(ProtocolError::fragmentation(
                "Too many concurrent fragment assemblies",
            ));
        }

        // Get or create assembly state
        let state = self
            .states
            .entry(header.message_id)
            .or_insert_with(|| AssemblyState::new(header.total_fragments));

        // Validate consistency
        if state.total_fragments != header.total_fragments {
            return Err(ProtocolError::fragmentation(format!(
                "Inconsistent total fragments: expected {}, got {}",
                state.total_fragments, header.total_fragments
            )));
        }

        // Add the fragment
        state.add_fragment(header.fragment_num, response.to_string())?;

        // Check if complete
        if state.is_complete() {
            let assembled = state.assemble()?;
            self.states.remove(&header.message_id);
            Ok(Some(assembled))
        } else {
            // Still waiting for more fragments
            Ok(None)
        }
    }

    /// Clean up expired assemblies
    fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.states
            .retain(|_, state| now.duration_since(state.started_at) < self.timeout);
    }

    /// Get the number of active assemblies
    pub fn active_assemblies(&self) -> usize {
        self.states.len()
    }

    /// Set assembly timeout
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Set maximum concurrent assemblies
    pub fn set_max_assemblies(&mut self, max: usize) {
        self.max_assemblies = max;
    }

    /// Clear all assembly states
    pub fn clear(&mut self) {
        self.states.clear();
    }
}

impl Default for FragmentAssembler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fragment_header_parse() {
        let response = "701 FRAGMENT 12345 0/3";
        let header = FragmentHeader::parse(response).unwrap();
        assert_eq!(header.message_id, 12345);
        assert_eq!(header.fragment_num, 0);
        assert_eq!(header.total_fragments, 3);
        assert!(header.is_first());
        assert!(!header.is_last());

        let response = "702 FRAGMENT 12345 2/3";
        let header = FragmentHeader::parse(response).unwrap();
        assert_eq!(header.fragment_num, 2);
        assert!(header.is_last());
        assert!(!header.is_first());
    }

    #[test]
    fn test_fragment_header_parse_invalid() {
        assert!(FragmentHeader::parse("200 OK").is_none());
        assert!(FragmentHeader::parse("701 FRAGMENT").is_none());
        assert!(FragmentHeader::parse("701 FRAGMENT abc 0/3").is_none());
        assert!(FragmentHeader::parse("701 FRAGMENT 123 0-3").is_none());
    }

    #[test]
    fn test_fragment_assembler_non_fragmented() {
        let mut assembler = FragmentAssembler::new();

        let response = "200 LOGIN ACCEPTED\nabc123";
        let result = assembler.process(response).unwrap();
        assert_eq!(result, Some(response.to_string()));
    }

    #[test]
    fn test_fragment_assembler_simple() {
        let mut assembler = FragmentAssembler::new();

        // First fragment
        let frag1 = "701 FRAGMENT 100 0/2\n220 FILE\npart1_data";
        let result = assembler.process(frag1).unwrap();
        assert_eq!(result, None); // Not complete yet

        // Second fragment
        let frag2 = "701 FRAGMENT 100 1/2\npart2_data";
        let result = assembler.process(frag2).unwrap();
        assert_eq!(result, Some("220 FILE\npart1_data\npart2_data".to_string()));

        // Assembly should be cleaned up
        assert_eq!(assembler.active_assemblies(), 0);
    }

    #[test]
    fn test_fragment_assembler_out_of_order() {
        let mut assembler = FragmentAssembler::new();

        // Second fragment first
        let frag2 = "701 FRAGMENT 200 1/3\npart2";
        let result = assembler.process(frag2).unwrap();
        assert_eq!(result, None);

        // Third fragment
        let frag3 = "701 FRAGMENT 200 2/3\npart3";
        let result = assembler.process(frag3).unwrap();
        assert_eq!(result, None);

        // First fragment last
        let frag1 = "701 FRAGMENT 200 0/3\n230 ANIME\npart1";
        let result = assembler.process(frag1).unwrap();
        assert_eq!(result, Some("230 ANIME\npart1\npart2\npart3".to_string()));
    }

    #[test]
    fn test_fragment_assembler_inconsistent() {
        let mut assembler = FragmentAssembler::new();

        let frag1 = "701 FRAGMENT 300 0/2\ndata1";
        assembler.process(frag1).unwrap();

        // Different total fragments for same message ID
        let frag2 = "701 FRAGMENT 300 1/3\ndata2";
        let result = assembler.process(frag2);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProtocolError::Fragmentation { .. }
        ));
    }

    #[test]
    fn test_fragment_assembler_duplicate() {
        let mut assembler = FragmentAssembler::new();

        let frag1 = "701 FRAGMENT 400 0/2\ndata1";
        assembler.process(frag1).unwrap();

        // Duplicate fragment (same number)
        let frag1_dup = "701 FRAGMENT 400 0/2\ndata1_modified";
        assembler.process(frag1_dup).unwrap();

        let frag2 = "701 FRAGMENT 400 1/2\ndata2";
        let result = assembler.process(frag2).unwrap();

        // Should use the latest version of fragment 0
        assert_eq!(result, Some("data1_modified\ndata2".to_string()));
    }

    #[test]
    fn test_fragment_assembler_max_assemblies() {
        let mut assembler = FragmentAssembler::new();
        assembler.set_max_assemblies(2);

        // Start two assemblies
        assembler.process("701 FRAGMENT 1 0/2\ndata").unwrap();
        assembler.process("701 FRAGMENT 2 0/2\ndata").unwrap();

        // Third should fail
        let result = assembler.process("701 FRAGMENT 3 0/2\ndata");
        assert!(result.is_err());
    }

    #[test]
    fn test_assembly_state() {
        let mut state = AssemblyState::new(3);

        assert!(!state.is_complete());

        state.add_fragment(0, "frag0".to_string()).unwrap();
        state.add_fragment(1, "frag1".to_string()).unwrap();
        assert!(!state.is_complete());

        state.add_fragment(2, "frag2".to_string()).unwrap();
        assert!(state.is_complete());

        // Invalid fragment number
        let result = state.add_fragment(3, "invalid".to_string());
        assert!(result.is_err());
    }
}
