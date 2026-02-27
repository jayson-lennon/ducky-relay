//! Ducky Relay Varlink Service
//!
//! A varlink service that listens for keystroke messages.

use ducky_relay::{KeystrokeError, SendKeysResponse, VARLINK_SOCKET};
use wherror::Error;
use zlink::{Server, service, unix};

#[derive(Debug, Error)]
#[error(debug)]
pub struct DuckycapVarlinkError;

#[tokio::main]
async fn main() {
    println!("Starting ducky-relay varlink server");
    run_server().await;
}

#[allow(clippy::missing_panics_doc)]
pub async fn run_server() {
    // Clean up any existing socket file
    let _ = tokio::fs::remove_file(VARLINK_SOCKET).await;

    println!("Binding to socket: {VARLINK_SOCKET}");
    let listener = unix::bind(VARLINK_SOCKET).expect("Failed to bind to socket");

    // Create our service and server
    let service = KeystrokeService::new();
    let server = Server::new(listener, service);

    match server.run().await {
        Ok(()) => println!("Server done."),
        Err(e) => eprintln!("Server error: {e:?}"),
    }
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
    #[allow(clippy::unused_async)]
    async fn send_keys(&mut self, keys: Vec<String>) -> Result<SendKeysResponse, KeystrokeError> {
        let keys: Vec<String> = keys.into_iter().filter(|k| !k.trim().is_empty()).collect();

        if keys.is_empty() {
            return Err(KeystrokeError::InvalidKey {
                message: "Keys list cannot be empty".to_string(),
            });
        }

        self.keystroke_count += 1;
        println!(
            "Received key combination #{}: {:?}",
            self.keystroke_count, keys
        );

        Ok(SendKeysResponse {
            success: true,
            keys,
        })
    }
}
