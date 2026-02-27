//! Ducky Relay Varlink Service
//!
//! A varlink service that listens for keystroke messages.

use serde::{Deserialize, Serialize};
use zlink::{introspect, unix, ReplyError, Server, service};

const SOCKET_PATH: &str = "/run/duckycap.varlink";

#[tokio::main]
async fn main() {
    println!("Starting ducky-relay varlink server");
    run_server().await;
}

pub async fn run_server() {
    // Clean up any existing socket file
    let _ = tokio::fs::remove_file(SOCKET_PATH).await;
    
    println!("Binding to socket: {}", SOCKET_PATH);
    let listener = unix::bind(SOCKET_PATH).expect("Failed to bind to socket");

    // Create our service and server
    let service = KeystrokeService::new();
    let server = Server::new(listener, service);

    match server.run().await {
        Ok(_) => println!("Server done."),
        Err(e) => eprintln!("Server error: {:?}", e),
    }
}

// ============================================================================
// Message Types
// ============================================================================

/// Response for SendKeys method
#[derive(Debug, Clone, Serialize, Deserialize, introspect::Type)]
pub struct SendKeysResponse {
    success: bool,
    keys: Vec<String>,
}

/// Error types for the service
#[derive(Debug, ReplyError, introspect::ReplyError)]
#[zlink(interface = "io.ducky.Keystroke")]
enum KeystrokeError {
    InvalidKey { message: String },
}

// ============================================================================
// Service Implementation
// ============================================================================

struct KeystrokeService {
    keystroke_count: u64,
}

impl KeystrokeService {
    fn new() -> Self {
        Self { keystroke_count: 0 }
    }
}

#[service(interface = "io.ducky.Keystroke")]
impl KeystrokeService {
    async fn send_keys(&mut self, keys: Vec<String>) -> Result<SendKeysResponse, KeystrokeError> {
        let keys: Vec<String> = keys
            .into_iter()
            .filter(|k| !k.trim().is_empty())
            .collect();
        
        if keys.is_empty() {
            return Err(KeystrokeError::InvalidKey {
                message: "Keys list cannot be empty".to_string(),
            });
        }
        
        self.keystroke_count += 1;
        println!("Received key combination #{}: {:?}", self.keystroke_count, keys);
        
        Ok(SendKeysResponse {
            success: true,
            keys,
        })
    }
}
