//! Ducky Relay Varlink Service
//!
//! A varlink service that listens for keystroke messages.
//! Message format: { "key": "a" }

use serde::{Deserialize, Serialize};
use std::os::fd::OwnedFd;
use std::os::unix::io::FromRawFd;
use zlink::{
    introspect, unix, ReplyError, Server, service,
};

const SOCKET_PATH: &str = "/run/duckycap.varlink";

#[tokio::main]
async fn main() {
    println!("Starting ducky-relay varlink server");
    run_server().await;
}

pub async fn run_server() {
    // Check if we're running under systemd socket activation
    // If so, use the inherited socket; otherwise bind to the socket path
    let listener = if let Some(fd) = systemd_socket() {
        println!("Using systemd socket activation");
        // SAFETY: The file descriptor is provided by systemd and is valid
        let std_listener = unsafe { std::os::unix::net::UnixListener::from_raw_fd(fd) };
        std_listener.set_nonblocking(true).expect("Failed to set non-blocking");
        let owned_fd: OwnedFd = std_listener.into();
        unix::Listener::try_from(owned_fd).expect("Failed to create listener from systemd socket")
    } else {
        // Clean up any existing socket file
        let _ = tokio::fs::remove_file(SOCKET_PATH).await;
        println!("Binding to socket: {}", SOCKET_PATH);
        unix::bind(SOCKET_PATH).expect("Failed to bind to socket")
    };

    // Create our service and server
    let service = KeystrokeService::new();
    let server = Server::new(listener, service);

    match server.run().await {
        Ok(_) => println!("Server done."),
        Err(e) => eprintln!("Server error: {:?}", e),
    }
}

/// Check for systemd socket activation
/// Returns the file descriptor if running under socket activation
fn systemd_socket() -> Option<i32> {
    // Check for LISTEN_PID environment variable
    if let Ok(pid_str) = std::env::var("LISTEN_PID") {
        if let Ok(pid) = pid_str.parse::<i32>() {
            if pid == std::process::id() as i32 {
                // We're being socket-activated
                if let Ok(fds_str) = std::env::var("LISTEN_FDS") {
                    if let Ok(fds) = fds_str.parse::<i32>() {
                        if fds >= 1 {
                            // Return the first inherited socket (FD 3, which is SD_LISTEN_FDS_START)
                            return Some(3);
                        }
                    }
                }
            }
        }
    }
    None
}

// ============================================================================
// Message Types
// ============================================================================

/// Parameters for SendKey method
#[derive(Debug, Clone, Serialize, Deserialize, introspect::Type)]
pub struct SendKeyParameters {
    key: String,
}

/// Response for SendKey method
#[derive(Debug, Clone, Serialize, Deserialize, introspect::Type)]
pub struct SendKeyResponse {
    success: bool,
    key: String,
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

struct KeystrokeService {}

impl KeystrokeService {
    fn new() -> Self {
        Self {}
    }
}

#[service(interface = "io.ducky.Keystroke")]
impl KeystrokeService {
    async fn send_key(&mut self, parameters: SendKeyParameters) -> Result<SendKeyResponse, KeystrokeError> {
        let key = parameters.key.trim();
        
        if key.is_empty() {
            return Err(KeystrokeError::InvalidKey {
                message: "Key parameter cannot be empty".to_string(),
            });
        }
        
        println!("Received keystroke: '{}'", key);
        
        // TODO: Add your keystroke handling logic here
        // For example: forward to a device, store in buffer, etc.
        
        Ok(SendKeyResponse {
            success: true,
            key: key.to_string(),
        })
    }
}
