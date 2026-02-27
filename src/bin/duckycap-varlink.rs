//! Ducky Relay Varlink Service
//!
//! A varlink service that listens for keystroke messages and executes
//! configured commands as a specific user based on a TOML config file.

use clap::Parser;
use ducky_relay::{KeystrokeError, SendKeysResponse, VARLINK_SOCKET};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use zlink::{Server, service, unix};

// ============================================================================
// Constants
// ============================================================================

/// Debounce duration to prevent rapid-fire command execution
/// The duckyPad sends continuous press/release events even when key is held
const DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

// ============================================================================
// CLI Arguments
// ============================================================================

/// DuckyPad varlink service - executes commands based on key combinations
#[derive(Parser)]
#[command(name = "duckycap-varlink")]
struct Args {
    /// Path to TOML configuration file
    #[arg(short, long)]
    config: PathBuf,
}

// ============================================================================
// TOML Configuration
// ============================================================================

/// Root configuration structure
#[derive(Debug, Deserialize)]
struct Config {
    /// User to run commands as
    user: String,
    /// List of command mappings
    #[serde(default)]
    commands: Vec<CommandMapping>,
}

/// A single key combination to command mapping
#[derive(Debug, Deserialize)]
struct CommandMapping {
    /// Key combination string (e.g., "meta+f1", "a", "ctrl+shift+b")
    keys: String,
    /// Absolute path to the script to execute
    path: PathBuf,
}

impl Config {
    /// Load configuration from a TOML file
    fn load(path: &PathBuf) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file '{}': {e}", path.display()))?;

        toml::from_str(&content)
            .map_err(|e| format!("Failed to parse config file '{}': {e}", path.display()))
    }

    /// Convert commands to a HashMap for efficient lookup
    fn build_command_map(&self) -> HashMap<Vec<String>, PathBuf> {
        self.commands
            .iter()
            .map(|cmd| {
                let keys = parse_key_combination(&cmd.keys);
                (keys, cmd.path.clone())
            })
            .collect()
    }
}

// ============================================================================
// Key Combination Parsing
// ============================================================================

/// Parse a key combination string into a normalized vector of keys
fn parse_key_combination(input: &str) -> Vec<String> {
    let mut keys: Vec<String> = input
        .split('+')
        .map(|s| s.trim().to_lowercase())
        .collect();
    keys.sort();
    keys
}

// ============================================================================
// Command Execution
// ============================================================================

/// Execute a script as a specific user with a login shell
fn execute_as_user(user: &str, script_path: &PathBuf) -> Result<(), String> {
    let script_str = script_path.to_string_lossy();

    let status = Command::new("runuser")
        .args([
            "-u", user,
            "--",
            "/bin/bash",
            "-l",  // Login shell - loads user's profile
            "-c",
            // Use exec "$0" pattern to safely pass script path as $0,
            // avoiding shell interpretation of special characters in the path
            "exec \"$0\"",
            &script_str,
        ])
        .status()
        .map_err(|e| format!("Failed to execute runuser: {e}"))?;

    if !status.success() {
        return Err(format!(
            "Command failed with exit code: {:?}",
            status.code()
        ));
    }

    Ok(())
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Load configuration
    let config = match Config::load(&args.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading config: {e}");
            std::process::exit(1);
        }
    };

    // Build command mapping
    let commands = config.build_command_map();

    println!("Starting ducky-relay varlink server");
    println!("Config file: {}", args.config.display());
    println!("Running commands as user: {}", config.user);
    println!("Loaded {} command mappings", commands.len());

    for (keys, path) in &commands {
        println!("  {} -> {}", keys.join("+"), path.display());
    }

    run_server(config.user, commands).await;
}

// ============================================================================
// Server
// ============================================================================

#[allow(clippy::missing_panics_doc)]
pub async fn run_server(user: String, commands: HashMap<Vec<String>, PathBuf>) {
    // Clean up any existing socket file
    let _ = tokio::fs::remove_file(VARLINK_SOCKET).await;

    println!("Binding to socket: {VARLINK_SOCKET}");
    let listener = unix::bind(VARLINK_SOCKET).expect("Failed to bind to socket");

    // Create our service and server
    let service = KeystrokeService::new(user, commands);
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
    user: String,
    commands: HashMap<Vec<String>, PathBuf>,
    /// Track last trigger time for each key combination (debounce)
    /// The duckyPad sends continuous press/release events, so we use
    /// time-based debouncing instead of tracking key state
    last_triggered: HashMap<Vec<String>, Instant>,
}

impl KeystrokeService {
    fn new(user: String, commands: HashMap<Vec<String>, PathBuf>) -> Self {
        Self {
            keystroke_count: 0,
            user,
            commands,
            last_triggered: HashMap::new(),
        }
    }
}

#[service(interface = "io.ducky.Keystroke")]
impl KeystrokeService {
    #[allow(clippy::unused_async)]
    async fn send_keys(&mut self, keys: Vec<String>, pressed: bool) -> Result<SendKeysResponse, KeystrokeError> {
        let keys: Vec<String> = keys.into_iter().filter(|k| !k.trim().is_empty()).collect();

        if keys.is_empty() {
            return Err(KeystrokeError::InvalidKey {
                message: "Keys list cannot be empty".to_string(),
            });
        }

        // Normalize keys for lookup (same as parse_key_combination)
        let mut normalized: Vec<String> = keys.iter().map(|k| k.to_lowercase()).collect();
        normalized.sort();

        self.keystroke_count += 1;
        println!(
            "Received key combination #{}: {:?} (pressed={})",
            self.keystroke_count, normalized, pressed
        );

        // The duckyPad sends continuous press/release events even when key is held,
        // so we ignore release events and use time-based debouncing for presses
        if !pressed {
            println!("Ignoring key release event (spurious from duckyPad): {:?}", normalized);
            return Ok(SendKeysResponse {
                success: true,
                keys: normalized,
                pressed: false,
            });
        }

        // Key press event - check debounce
        let now = Instant::now();

        // Clean up stale debounce entries (older than DEBOUNCE_DURATION)
        self.last_triggered.retain(|_, last_time| {
            now.duration_since(*last_time) < DEBOUNCE_DURATION
        });

        let should_trigger = match self.last_triggered.get(&normalized) {
            Some(last_time) => {
                let elapsed = now.duration_since(*last_time);
                if elapsed >= DEBOUNCE_DURATION {
                    println!("Debounce window passed ({:?} >= {:?}), allowing trigger", elapsed, DEBOUNCE_DURATION);
                    true
                } else {
                    println!("Ignoring key press within debounce window ({:?} < {:?}): {:?}", elapsed, DEBOUNCE_DURATION, normalized);
                    false
                }
            }
            None => {
                println!("First press for this key combination: {:?}", normalized);
                true
            }
        };

        if !should_trigger {
            return Ok(SendKeysResponse {
                success: true,
                keys: normalized,
                pressed: false, // Indicates no action taken due to debounce
            });
        }

        // Update last triggered time
        self.last_triggered.insert(normalized.clone(), now);

        // Look up and execute command if found
        if let Some(script_path) = self.commands.get(&normalized) {
            let user = self.user.clone();
            let path = script_path.clone();
            let key_desc = normalized.join("+");

            println!("Executing '{}' as user '{}'", path.display(), user);

            // Spawn command in background to avoid blocking
            tokio::spawn(async move {
                match execute_as_user(&user, &path) {
                    Ok(()) => println!("Command '{}' completed successfully for keys [{}]", path.display(), key_desc),
                    Err(e) => eprintln!("Command '{}' failed for keys [{}]: {}", path.display(), key_desc, e),
                }
            });
        } else {
            println!("No command mapped for keys: {:?}", normalized);
        }

        Ok(SendKeysResponse {
            success: true,
            keys: normalized,
            pressed,
        })
    }
}
