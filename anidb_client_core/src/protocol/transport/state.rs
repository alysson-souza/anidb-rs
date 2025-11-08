//! Connection state management
//!
//! This module implements a state machine for managing the UDP connection lifecycle.

use std::fmt;

/// Connection state enum representing the current state of the UDP connection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected to the server
    Disconnected,
    /// Attempting to establish connection
    Connecting,
    /// Connected but not authenticated
    Connected { session: Option<String> },
    /// Authenticated with valid session
    Authenticated { session: String, username: String },
    /// Connection is being closed
    Disconnecting,
    /// Connection failed with error
    Failed,
}

impl ConnectionState {
    /// Check if the connection can send packets
    pub fn can_send(&self) -> bool {
        matches!(
            self,
            ConnectionState::Connected { .. } | ConnectionState::Authenticated { .. }
        )
    }

    /// Check if the connection can receive packets
    pub fn can_receive(&self) -> bool {
        matches!(
            self,
            ConnectionState::Connected { .. } | ConnectionState::Authenticated { .. }
        )
    }

    /// Check if the connection is authenticated
    pub fn is_authenticated(&self) -> bool {
        matches!(self, ConnectionState::Authenticated { .. })
    }

    /// Check if the connection is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ConnectionState::Disconnected | ConnectionState::Failed
        )
    }

    /// Get the session tag if authenticated
    pub fn session(&self) -> Option<&str> {
        match self {
            ConnectionState::Authenticated { session, .. } => Some(session),
            ConnectionState::Connected { session: Some(s) } => Some(s),
            _ => None,
        }
    }
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "Disconnected"),
            ConnectionState::Connecting => write!(f, "Connecting"),
            ConnectionState::Connected { session: None } => write!(f, "Connected (no session)"),
            ConnectionState::Connected { session: Some(_) } => {
                write!(f, "Connected (with session)")
            }
            ConnectionState::Authenticated { username, .. } => {
                write!(f, "Authenticated (user: {username})")
            }
            ConnectionState::Disconnecting => write!(f, "Disconnecting"),
            ConnectionState::Failed => write!(f, "Failed"),
        }
    }
}

/// State transition validator
pub struct StateTransition {
    from: ConnectionState,
    to: ConnectionState,
}

impl StateTransition {
    /// Create a new state transition
    pub fn new(from: ConnectionState, to: ConnectionState) -> Self {
        Self { from, to }
    }

    /// Check if the transition is valid according to the state machine rules
    pub fn is_valid(&self) -> bool {
        use ConnectionState::*;

        match (self.from.clone(), self.to.clone()) {
            // From Disconnected
            (Disconnected, Connecting) => true,
            (Disconnected, Failed) => true,

            // From Connecting
            (Connecting, Connected { .. }) => true,
            (Connecting, Failed) => true,
            (Connecting, Disconnected) => true,

            // From Connected
            (Connected { .. }, Authenticated { .. }) => true,
            (Connected { .. }, Disconnecting) => true,
            (Connected { .. }, Failed) => true,
            (Connected { .. }, Disconnected) => true,

            // From Authenticated
            (Authenticated { .. }, Disconnecting) => true,
            (Authenticated { .. }, Failed) => true,
            (Authenticated { .. }, Connected { .. }) => true, // Session expired
            (Authenticated { .. }, Disconnected) => true,

            // From Disconnecting
            (Disconnecting, Disconnected) => true,
            (Disconnecting, Failed) => true,

            // From Failed
            (Failed, Disconnected) => true,
            (Failed, Connecting) => true, // Retry

            // All other transitions are invalid
            _ => false,
        }
    }

    /// Get a description of why a transition might be invalid
    pub fn validation_error(&self) -> Option<String> {
        if self.is_valid() {
            None
        } else {
            Some(format!(
                "Invalid transition from {} to {}",
                self.from, self.to
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_capabilities() {
        // Disconnected state
        let state = ConnectionState::Disconnected;
        assert!(!state.can_send());
        assert!(!state.can_receive());
        assert!(!state.is_authenticated());
        assert!(state.is_terminal());
        assert_eq!(state.session(), None);

        // Connected state
        let state = ConnectionState::Connected { session: None };
        assert!(state.can_send());
        assert!(state.can_receive());
        assert!(!state.is_authenticated());
        assert!(!state.is_terminal());
        assert_eq!(state.session(), None);

        // Authenticated state
        let state = ConnectionState::Authenticated {
            session: "abc123".to_string(),
            username: "testuser".to_string(),
        };
        assert!(state.can_send());
        assert!(state.can_receive());
        assert!(state.is_authenticated());
        assert!(!state.is_terminal());
        assert_eq!(state.session(), Some("abc123"));
    }

    #[test]
    fn test_valid_transitions() {
        let valid_transitions = vec![
            (ConnectionState::Disconnected, ConnectionState::Connecting),
            (
                ConnectionState::Connecting,
                ConnectionState::Connected { session: None },
            ),
            (
                ConnectionState::Connected { session: None },
                ConnectionState::Authenticated {
                    session: "abc123".to_string(),
                    username: "user".to_string(),
                },
            ),
            (
                ConnectionState::Authenticated {
                    session: "abc123".to_string(),
                    username: "user".to_string(),
                },
                ConnectionState::Disconnecting,
            ),
            (
                ConnectionState::Disconnecting,
                ConnectionState::Disconnected,
            ),
            (ConnectionState::Failed, ConnectionState::Connecting),
        ];

        for (from, to) in valid_transitions {
            let transition = StateTransition::new(from.clone(), to.clone());
            assert!(
                transition.is_valid(),
                "Transition from {from:?} to {to:?} should be valid"
            );
            assert_eq!(transition.validation_error(), None);
        }
    }

    #[test]
    fn test_invalid_transitions() {
        let invalid_transitions = vec![
            (
                ConnectionState::Disconnected,
                ConnectionState::Authenticated {
                    session: "abc".to_string(),
                    username: "user".to_string(),
                },
            ),
            (
                ConnectionState::Connecting,
                ConnectionState::Authenticated {
                    session: "abc".to_string(),
                    username: "user".to_string(),
                },
            ),
            (ConnectionState::Disconnecting, ConnectionState::Connecting),
            (
                ConnectionState::Failed,
                ConnectionState::Authenticated {
                    session: "abc".to_string(),
                    username: "user".to_string(),
                },
            ),
        ];

        for (from, to) in invalid_transitions {
            let transition = StateTransition::new(from.clone(), to.clone());
            assert!(
                !transition.is_valid(),
                "Transition from {from:?} to {to:?} should be invalid"
            );
            assert!(transition.validation_error().is_some());
        }
    }

    #[test]
    fn test_state_display() {
        assert_eq!(ConnectionState::Disconnected.to_string(), "Disconnected");
        assert_eq!(ConnectionState::Connecting.to_string(), "Connecting");
        assert_eq!(
            ConnectionState::Connected { session: None }.to_string(),
            "Connected (no session)"
        );
        assert_eq!(
            ConnectionState::Authenticated {
                session: "abc".to_string(),
                username: "testuser".to_string(),
            }
            .to_string(),
            "Authenticated (user: testuser)"
        );
    }

    #[test]
    fn test_terminal_states() {
        assert!(ConnectionState::Disconnected.is_terminal());
        assert!(ConnectionState::Failed.is_terminal());
        assert!(!ConnectionState::Connecting.is_terminal());
        assert!(!ConnectionState::Connected { session: None }.is_terminal());
        assert!(
            !ConnectionState::Authenticated {
                session: "abc".to_string(),
                username: "user".to_string(),
            }
            .is_terminal()
        );
        assert!(!ConnectionState::Disconnecting.is_terminal());
    }

    #[test]
    fn test_session_recovery_transition() {
        // Test that we can go from Authenticated back to Connected (session expired)
        let transition = StateTransition::new(
            ConnectionState::Authenticated {
                session: "old".to_string(),
                username: "user".to_string(),
            },
            ConnectionState::Connected { session: None },
        );
        assert!(transition.is_valid());
    }
}
