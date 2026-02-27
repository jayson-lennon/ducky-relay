//! DuckyPad Capture Daemon
//!
//! Captures input from duckyPad keyboard using evdev with exclusive grab,
//! blocking input from reaching the system and forwarding key combinations
//! to the varlink service.

use evdev::{Device, EventSummary, EventType, KeyCode};
use std::collections::HashSet;
use std::path::Path;
use zlink::{proxy, unix};

const VARLINK_SOCKET: &str = "/run/duckycap.varlink";
const DUCKYPAD_SYMLINK: &str = "/dev/input/duckypad";
const DUCKYPAD_VENDOR_ID: u16 = 0x0483;
const DUCKYPAD_PRODUCT_ID: u16 = 0xD11C;

// Proxy trait for the client
#[proxy("io.ducky.Keystroke")]
trait KeystrokeProxy {
    async fn send_keys(
        &mut self,
        keys: &[&str],
    ) -> zlink::Result<Result<SendKeysOutput, KeystrokeError>>;
}

// Output type for send_keys (owned, not borrowed)
#[derive(Debug, Clone, serde::Deserialize)]
struct SendKeysOutput {
    success: bool,
    keys: Vec<String>,
}

// Error type
#[derive(Debug, Clone, PartialEq, zlink::ReplyError)]
#[zlink(interface = "io.ducky.Keystroke")]
enum KeystrokeError {
    InvalidKey { message: String },
}

#[tokio::main]
async fn main() {
    println!("Starting duckyPad capture daemon");

    // Find and open the duckyPad device
    let device = match find_duckypad_device() {
        Some(dev) => dev,
        None => {
            eprintln!("duckyPad device not found. Exiting.");
            std::process::exit(1);
        }
    };

    println!("Found device: {}", device.name().unwrap_or("unknown"));

    // Run the capture loop
    if let Err(e) = run_capture(device).await {
        eprintln!("Capture error: {:?}", e);
        std::process::exit(1);
    }
}

/// Find the duckyPad device, preferring the udev symlink if available
fn find_duckypad_device() -> Option<Device> {
    // First, try the udev symlink
    if Path::new(DUCKYPAD_SYMLINK).exists() {
        if let Ok(device) = Device::open(DUCKYPAD_SYMLINK) {
            // Verify it's the correct device
            if is_duckypad(&device) {
                println!("Using device via udev symlink: {}", DUCKYPAD_SYMLINK);
                return Some(device);
            } else {
                println!("Warning: Symlink exists but device doesn't match expected VID:PID");
            }
        }
    }

    // Fall back to scanning all devices
    println!("Scanning for duckyPad device by VID:PID...");

    for (_path, device) in evdev::enumerate() {
        if is_duckypad(&device) {
            println!("Found duckyPad at {:?}", device.physical_path());
            return Some(device);
        }
    }

    None
}

/// Check if a device matches the duckyPad VID:PID
fn is_duckypad(device: &Device) -> bool {
    let id = device.input_id();
    id.vendor() == DUCKYPAD_VENDOR_ID && id.product() == DUCKYPAD_PRODUCT_ID
}

/// Main capture loop
async fn run_capture(mut device: Device) -> Result<(), Box<dyn std::error::Error>> {
    // Grab the device exclusively - this blocks input from reaching other applications
    device.grab()?;
    println!("Device grabbed exclusively. Input will be blocked from the system.");

    // Track currently held keys
    let mut held_keys: HashSet<KeyCode> = HashSet::new();

    println!("Listening for key events...");

    // Event loop
    loop {
        let events = match device.fetch_events() {
            Ok(events) => events.collect::<Vec<_>>(),
            Err(e) => {
                eprintln!("Error reading events: {:?}", e);
                // Device was likely disconnected
                println!("Device may have been disconnected. Exiting.");
                return Err(Box::new(e));
            }
        };

        for event in events {
            println!("event {event:?}");
            // Only process key events
            if event.event_type() != EventType::KEY {
                continue;
            }

            // Destructure the event to get key details
            match event.destructure() {
                EventSummary::Key(_key_event, key, value) => {
                    // Handle key press (value == 1) and release (value == 0)
                    // Ignore key repeat (value == 2)
                    match value {
                        1 => {
                            // Key press
                            if held_keys.insert(key) {
                                // Key was newly pressed, send update
                                let key_names = get_key_names(&held_keys);
                                println!("Key press: {:?}", key_names);

                                if let Err(e) = send_keys_to_varlink(&key_names).await {
                                    eprintln!("Failed to send to varlink: {:?}", e);
                                }
                            }
                        }
                        0 => {
                            // Key release - just remove from held set, don't send
                            held_keys.remove(&key);
                        }
                        2 => {
                            // Key repeat - ignore
                        }
                        _ => {}
                    }
                }
                _ => continue,
            }
        }
    }
}

/// Convert held keys to human-readable names
fn get_key_names(keys: &HashSet<KeyCode>) -> Vec<String> {
    let mut names: Vec<String> = keys.iter().filter_map(|k| key_to_name(*k)).collect();

    // Sort for consistent ordering
    names.sort();
    names
}

/// Convert a KeyCode to a human-readable name
fn key_to_name(key: KeyCode) -> Option<String> {
    // Get the key code
    let code = key.code();

    // Map common key codes to human-readable names
    // Based on Linux input event codes
    let name = match code {
        // Letters
        16 => "q",
        17 => "w",
        18 => "e",
        19 => "r",
        20 => "t",
        21 => "y",
        22 => "u",
        23 => "i",
        24 => "o",
        25 => "p",
        30 => "a",
        31 => "s",
        32 => "d",
        33 => "f",
        34 => "g",
        35 => "h",
        36 => "j",
        37 => "k",
        38 => "l",
        44 => "z",
        45 => "x",
        46 => "c",
        47 => "v",
        48 => "b",
        49 => "n",
        50 => "m",

        // Numbers
        2 => "1",
        3 => "2",
        4 => "3",
        5 => "4",
        6 => "5",
        7 => "6",
        8 => "7",
        9 => "8",
        10 => "9",
        11 => "0",

        // Function keys
        59 => "f1",
        60 => "f2",
        61 => "f3",
        62 => "f4",
        63 => "f5",
        64 => "f6",
        65 => "f7",
        66 => "f8",
        67 => "f9",
        68 => "f10",
        87 => "f11",
        88 => "f12",

        // Modifiers
        29 => "ctrl",
        97 => "ctrl", // Left/Right Ctrl
        42 => "shift",
        54 => "shift", // Left/Right Shift
        56 => "alt",
        100 => "alt", // Left/Right Alt
        125 => "meta",
        126 => "meta", // Left/Right Meta/Super

        // Special keys
        1 => "escape",
        14 => "backspace",
        15 => "tab",
        28 => "enter",
        57 => "space",
        58 => "capslock",
        111 => "delete",
        110 => "home",
        115 => "end",
        112 => "pageup",
        117 => "pagedown",

        // Arrow keys
        103 => "up",
        108 => "down",
        105 => "left",
        106 => "right",

        // Symbols
        12 => "minus",
        13 => "equal",
        26 => "leftbracket",
        27 => "rightbracket",
        39 => "semicolon",
        40 => "apostrophe",
        41 => "grave",
        43 => "backslash",
        51 => "comma",
        52 => "dot",
        53 => "slash",

        // Numpad
        69 => "numlock",
        71 => "kp7",
        72 => "kp8",
        73 => "kp9",
        75 => "kp4",
        76 => "kp5",
        77 => "kp6",
        79 => "kp1",
        80 => "kp2",
        81 => "kp3",
        82 => "kp0",
        83 => "kpdot",
        78 => "kpplus",
        74 => "kpminus",
        55 => "kpasterisk",
        98 => "kpslash",
        96 => "kpenter",

        // Other
        99 => "sysrq",
        119 => "pause",
        120 => "scrolllock",
        116 => "power",
        142 => "sleep",

        // Unknown - return code number
        _ => {
            return Some(format!("key{}", code));
        }
    };

    Some(name.to_string())
}

/// Send key combination to varlink service using zlink proxy
async fn send_keys_to_varlink(keys: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if keys.is_empty() {
        return Ok(());
    }

    // Connect to varlink socket using zlink::unix::connect
    let mut conn = unix::connect(VARLINK_SOCKET).await?;

    // Convert Vec<String> to Vec<&str> for the proxy
    let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();

    // Use the proxy-generated method directly on the connection
    let result = conn.send_keys(&key_refs).await?;

    match result {
        Ok(_output) => {
            // Success
        }
        Err(KeystrokeError::InvalidKey { message }) => {
            eprintln!("Invalid key error: {}", message);
        }
    }

    Ok(())
}
