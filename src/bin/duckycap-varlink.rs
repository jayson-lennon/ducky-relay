//! Ducky Relay Varlink Service
//!
//! A varlink service that listens for keystroke messages and executes
//! configured commands as a specific user based on a TOML config file.
//!
//! # Service Behavior
//!
//! - Uses systemd's `Type=notify` for proper service readiness signaling
//! - Self-terminates after 5 minutes of inactivity (no keystroke messages)
//! - Uses a monotonic clock to avoid issues with system time changes

use clap::Parser;
use ducky_relay::{KeystrokeError, SendKeysResponse, VARLINK_SOCKET};
use sd_notify::NotifyState;
use serde::Deserialize;
use std::collections::HashMap;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use zlink::{Server, service, unix};

// ============================================================================
// Constants
// ============================================================================

/// Debounce duration to prevent rapid-fire command execution
/// The duckyPad sends continuous press/release events even when key is held
const DEBOUNCE_DURATION: Duration = Duration::from_millis(50);

/// Idle timeout before self-termination (5 minutes)
const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Interval for checking idle timeout
const IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(30);

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
    /// Command to execute - if it starts with '/' it's treated as a script path,
    /// otherwise it's run as a shell command
    cmd: String,
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
    fn build_command_map(&self) -> HashMap<Vec<String>, String> {
        self.commands
            .iter()
            .map(|cmd| {
                let keys = parse_key_combination(&cmd.keys);
                (keys, cmd.cmd.clone())
            })
            .collect()
    }
}

// ============================================================================
// Key Combination Parsing
// ============================================================================

/// Parse a key combination string into a normalized vector of keys
fn parse_key_combination(input: &str) -> Vec<String> {
    let mut keys: Vec<String> = input.split('+').map(|s| s.trim().to_lowercase()).collect();
    keys.sort();
    keys
}

// ============================================================================
// Command Execution
// ============================================================================

/// Execute a command as a specific user with a login shell
///
/// If `cmd` starts with '/', it's treated as an absolute path to a script,
/// optionally followed by arguments.
/// Otherwise, it's run as a shell command via `bash -c`.
fn execute_as_user(user: &str, cmd: &str) -> Result<(), String> {
    let status = if cmd.starts_with('/') {
        // Absolute path - split script path from arguments
        let mut parts = cmd.split_whitespace();
        let script_path = parts.next().unwrap_or(cmd);
        let args: Vec<&str> = parts.collect();

        let mut command = Command::new("runuser");
        command.args([
            "-u",
            user,
            "--",
            "/bin/bash",
            "-l", // Login shell - loads user's profile
            "-c",
            // Use exec "$0" "$@" pattern to safely pass script path as $0
            // and forward any additional arguments via $@
            "exec \"$0\" \"$@\"",
            script_path,
        ]);

        // Add script arguments if any
        command.args(&args);

        command
            .status()
            .map_err(|e| format!("Failed to execute runuser: {e}"))?
    } else {
        // Shell command - run via bash -c
        Command::new("runuser")
            .args(["-u", user, "--", "/bin/bash", "-l", "-c", cmd])
            .status()
            .map_err(|e| format!("Failed to execute runuser: {e}"))?
    };

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

    for (keys, cmd) in &commands {
        println!("  {} -> {}", keys.join("+"), cmd);
    }

    run_server(config.user, commands).await;
}

// ============================================================================
// Server
// ============================================================================

/// Check for systemd socket activation (LISTEN_FDS environment variable)
/// Returns an OwnedFd if systemd passed us a socket
fn get_systemd_socket() -> Option<OwnedFd> {
    let listen_fds = std::env::var("LISTEN_FDS").ok()?;
    let count: i32 = listen_fds.parse().ok()?;

    if count >= 1 {
        // SD_LISTEN_FDS_START is always 3 (first fd after stdin/stdout/stderr)
        // SAFETY: systemd guarantees the fd is valid and is a Unix socket.
        // We take ownership of the fd which will be closed when OwnedFd is dropped.
        use std::os::unix::io::FromRawFd;
        Some(unsafe { OwnedFd::from_raw_fd(3) })
    } else {
        None
    }
}

#[allow(clippy::missing_panics_doc)]
pub async fn run_server(user: String, commands: HashMap<Vec<String>, String>) {
    let listener = match get_systemd_socket() {
        Some(fd) => {
            println!("Using socket from systemd (fd {})", fd.as_raw_fd());
            unix::Listener::try_from(fd).expect("Failed to convert systemd socket to listener")
        }
        None => {
            println!("No systemd socket, binding directly to: {VARLINK_SOCKET}");
            let _ = tokio::fs::remove_file(VARLINK_SOCKET).await;
            unix::bind(VARLINK_SOCKET).expect("Failed to bind to socket")
        }
    };

    let start_time = Arc::new(Instant::now());
    let last_activity = Arc::new(AtomicU64::new(0));

    spawn_idle_watchdog(Arc::clone(&start_time), Arc::clone(&last_activity));

    let service = KeystrokeService::new(user, commands, start_time, last_activity);
    let server = Server::new(listener, service);

    notify_systemd_ready();

    match server.run().await {
        Ok(()) => println!("Server done."),
        Err(e) => eprintln!("Server error: {e:?}"),
    }
}

fn notify_systemd_ready() {
    if std::env::var("NOTIFY_SOCKET").is_ok() {
        if let Err(e) = sd_notify::notify(false, &[NotifyState::Ready]) {
            eprintln!("Failed to notify systemd: {e}");
        }
    } else {
        let _ = sd_notify::notify(false, &[NotifyState::Ready]);
    }
}

fn spawn_idle_watchdog(start_time: Arc<Instant>, last_activity: Arc<AtomicU64>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(IDLE_CHECK_INTERVAL).await;

            let last_elapsed = last_activity.load(Ordering::Relaxed);
            let current_elapsed = start_time.elapsed().as_secs();

            if current_elapsed.saturating_sub(last_elapsed) >= IDLE_TIMEOUT.as_secs() {
                println!(
                    "No activity for {} seconds, terminating.",
                    IDLE_TIMEOUT.as_secs()
                );
                std::process::exit(0);
            }
        }
    });
}

// ============================================================================
// Service Implementation
// ============================================================================

struct KeystrokeService {
    user: String,
    commands: HashMap<Vec<String>, String>,
    /// Track last trigger time for each key combination (debounce)
    /// The duckyPad sends continuous press/release events, so we use
    /// time-based debouncing instead of tracking key state
    last_triggered: HashMap<Vec<String>, Instant>,
    /// Reference start time for monotonic clock (for idle timeout)
    start_time: Arc<Instant>,
    /// Shared elapsed seconds since start_time at last activity (for idle timeout)
    last_activity: Arc<AtomicU64>,
}

impl KeystrokeService {
    fn new(
        user: String,
        commands: HashMap<Vec<String>, String>,
        start_time: Arc<Instant>,
        last_activity: Arc<AtomicU64>,
    ) -> Self {
        Self {
            user,
            commands,
            last_triggered: HashMap::new(),
            start_time,
            last_activity,
        }
    }
}

#[service(interface = "io.ducky.Keystroke")]
impl KeystrokeService {
    #[allow(clippy::unused_async)]
    async fn send_keys(
        &mut self,
        keys: Vec<String>,
        pressed: bool,
    ) -> Result<SendKeysResponse, KeystrokeError> {
        self.last_activity
            .store(self.start_time.elapsed().as_secs(), Ordering::Relaxed);

        let keys: Vec<String> = keys.into_iter().filter(|k| !k.trim().is_empty()).collect();

        if keys.is_empty() {
            return Err(KeystrokeError::InvalidKey {
                message: "Keys list cannot be empty".to_string(),
            });
        }

        // Normalize keys for lookup (same as parse_key_combination)
        let mut normalized: Vec<String> = keys.iter().map(|k| k.to_lowercase()).collect();
        normalized.sort();

        println!(
            "Received key combination: {:?} (pressed={})",
            normalized, pressed
        );

        // The duckyPad sends continuous press/release events even when key is held,
        // so we ignore release events and use time-based debouncing for presses
        if !pressed {
            println!(
                "Ignoring key release event (spurious from duckyPad): {:?}",
                normalized
            );
            return Ok(SendKeysResponse {
                success: true,
                keys: normalized,
                pressed: false,
            });
        }

        // Key press event - check debounce
        let now = Instant::now();

        // Clean up stale debounce entries (older than DEBOUNCE_DURATION)
        self.last_triggered
            .retain(|_, last_time| now.duration_since(*last_time) < DEBOUNCE_DURATION);

        let should_trigger = match self.last_triggered.get(&normalized) {
            Some(last_time) => {
                let elapsed = now.duration_since(*last_time);
                if elapsed >= DEBOUNCE_DURATION {
                    println!(
                        "Debounce window passed ({:?} >= {:?}), allowing trigger",
                        elapsed, DEBOUNCE_DURATION
                    );
                    true
                } else {
                    println!(
                        "Ignoring key press within debounce window ({:?} < {:?}): {:?}",
                        elapsed, DEBOUNCE_DURATION, normalized
                    );
                    false
                }
            }
            None => {
                println!("First press for this key combination: {:?}", normalized);
                true
            }
        };

        // Always update the timer on every press - this resets the debounce window
        // so holding a key won't trigger again until 500ms after the last press
        self.last_triggered.insert(normalized.clone(), now);

        if !should_trigger {
            return Ok(SendKeysResponse {
                success: true,
                keys: normalized,
                pressed: false, // Indicates no action taken due to debounce
            });
        }

        // Look up and execute command if found
        if let Some(cmd) = self.commands.get(&normalized) {
            let user = self.user.clone();
            let cmd = cmd.clone();
            let key_desc = normalized.join("+");

            println!("Executing '{}' as user '{}'", cmd, user);

            // Spawn command in background to avoid blocking
            tokio::spawn(async move {
                match execute_as_user(&user, &cmd) {
                    Ok(()) => println!(
                        "Command '{}' completed successfully for keys [{}]",
                        cmd, key_desc
                    ),
                    Err(e) => eprintln!("Command '{}' failed for keys [{}]: {}", cmd, key_desc, e),
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
