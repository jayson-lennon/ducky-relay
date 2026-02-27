//! Ducky Relay Shared Library
//!
//! Common types and constants for the ducky-relay varlink service and client.

use serde::{Deserialize, Serialize};
use zlink::{ReplyError, introspect};

// ============================================================================
// Constants
// ============================================================================

/// Default varlink socket path
pub const VARLINK_SOCKET: &str = "/run/duckycap.varlink";

// ============================================================================
// Message Types
// ============================================================================

/// Response for `SendKeys` method
#[derive(Debug, Clone, Serialize, Deserialize, introspect::Type)]
pub struct SendKeysResponse {
    pub success: bool,
    pub keys: Vec<String>,
}

// ============================================================================
// Error Types
// ============================================================================

/// Error types for the keystroke service
#[derive(Debug, Clone, PartialEq, ReplyError, introspect::ReplyError)]
#[zlink(interface = "io.ducky.Keystroke")]
pub enum KeystrokeError {
    InvalidKey { message: String },
}

// ============================================================================
// Client Proxy
// ============================================================================

// Proxy trait for the client
#[zlink::proxy("io.ducky.Keystroke")]
pub trait KeystrokeProxy {
    async fn send_keys(
        &mut self,
        keys: &[&str],
    ) -> zlink::Result<Result<SendKeysResponse, KeystrokeError>>;
}
