# Ducky Relay

A system for intercepting duckyPad keyboard input, blocking it from reaching the system, and forwarding key combinations to a varlink service.

## Overview

This project consists of two main components:

1. **duckycap** - Capture daemon that intercepts duckyPad input using evdev exclusive grab
2. **duckycap-varlink** - Varlink service that receives keystroke messages

## Architecture

```
duckyPad (USB) → udev rule → /dev/input/duckypad symlink
                            ↓
                    duckycap daemon (EVIOCGRAB)
                            ↓
                    varlink SendKeys call
                            ↓
                    duckycap-varlink service
```

When the duckyPad is plugged in:
1. udev creates `/dev/input/duckypad` symlink
2. udev activates `duckycap.service` via `SYSTEMD_WANTS`
3. duckycap grabs the device exclusively (blocking input from the system)
4. On key press, duckycap sends the current key combination to the varlink service

## Varlink Interface

The service exposes the `io.ducky.Keystroke` interface with two methods:

### SendKey (single key, backward compatible)

**Parameters:**
```json
{
    "key": "a"
}
```

**Returns:**
```json
{
    "success": true,
    "key": "a"
}
```

### SendKeys (key combinations)

**Parameters:**
```json
{
    "keys": ["ctrl", "shift", "a"]
}
```

**Returns:**
```json
{
    "success": true,
    "keys": ["ctrl", "shift", "a"]
}
```

**Errors:**
- `io.ducky.Keystroke.InvalidKey` - The key parameter is invalid or empty

## Key Names

Keys are normalized to human-readable names:

| Category | Examples |
|----------|----------|
| Letters | `a`, `b`, `c`, ... `z` |
| Numbers | `0`, `1`, `2`, ... `9` |
| Modifiers | `ctrl`, `shift`, `alt`, `meta` |
| Special | `enter`, `space`, `tab`, `escape`, `backspace`, `delete` |
| Arrows | `up`, `down`, `left`, `right` |
| Function | `f1`, `f2`, ... `f12` |
| Numpad | `kp0`, `kp1`, `kpenter`, `kpplus`, ... |

Unknown keys are returned as `key{code}` where code is the numeric event code.

## Building

```bash
cargo build --release
```

This produces two binaries:
- `target/release/duckycap` - Capture daemon
- `target/release/duckycap-varlink` - Varlink service

## Installation

### 1. Install the binaries

```bash
sudo cp target/release/duckycap /usr/local/bin/
sudo cp target/release/duckycap-varlink /usr/local/bin/
```

### 2. Install udev rule

```bash
sudo cp systemd/99-duckypad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

### 3. Install systemd units

```bash
sudo cp systemd/duckycap.service /etc/systemd/system/
sudo cp systemd/duckycap-varlink.socket /etc/systemd/system/
sudo cp systemd/duckycap-varlink.service /etc/systemd/system/
sudo systemctl daemon-reload
```

### 4. Create configuration file

```bash
# Create config directory
sudo mkdir -p /etc/duckycap

# Copy example config and edit as needed
sudo cp config.example.toml /etc/duckycap/config.toml
sudo nano /etc/duckycap/config.toml
```

### 5. Enable the varlink socket

```bash
sudo systemctl enable --now duckycap-varlink.socket
```

The `duckycap.service` will be activated automatically by udev when the duckyPad is connected.

## Configuration

The `duckycap-varlink` service uses a TOML configuration file to map key combinations to shell scripts.

### Configuration File Format

```toml
# User to run commands as (required)
user = "your-username"

# Command mappings
[[commands]]
keys = "a"
path = "/home/your-username/scripts/volume-up.sh"

[[commands]]
keys = "b"
path = "/home/your-username/scripts/volume-down.sh"

[[commands]]
keys = "meta+f1"
path = "/home/your-username/scripts/toggle-mute.sh"
```

### Key Combinations

- Single keys: `"a"`, `"f1"`, `"enter"`
- Combinations use `+`: `"meta+f1"`, `"ctrl+shift+k"`
- Keys are normalized (sorted and lowercased), so `"meta+f1"` and `"f1+meta"` are equivalent

### Command Execution

Commands are executed using `runuser` with a login shell:
- Runs as the configured user
- Loads the user's shell profile (`~/.profile`, `~/.bashrc`, etc.)
- Scripts must have executable permissions

### Example Configuration

See [`config.example.toml`](config.example.toml) for a complete example.

## Usage

### Verify installation

1. Plug in the duckyPad
2. Check the symlink exists:
   ```bash
   ls -la /dev/input/duckypad
   ```
3. Check the service started:
   ```bash
   systemctl status duckycap.service
   ```
4. Monitor the varlink service:
   ```bash
   journalctl -u duckycap-varlink.service -f
   ```
5. Press keys on the duckyPad - you should see key combinations logged
6. Verify input is blocked (no keys reach other applications)

### Manual testing

Test the varlink service directly:

```bash
# Send a single key
varlinkctl call /run/duckycap.varlink io.ducky.Keystroke.SendKey '{"key": "a"}'

# Send a key combination
varlinkctl call /run/duckycap.varlink io.ducky.Keystroke.SendKeys '{"keys": ["ctrl", "shift", "a"]}'
```

### Get service info

```bash
varlinkctl info /run/duckycap.varlink
```

### Introspect the interface

```bash
varlinkctl introspect /run/duckycap.varlink io.ducky.Keystroke
```

## Development

### Run directly (without systemd)

```bash
# Terminal 1: Start varlink service with config file
sudo cargo run --bin duckycap-varlink -- --config config.example.toml

# Terminal 2: Start capture daemon (requires root for evdev access)
sudo cargo run --bin duckycap
```

### Debug with socat

```bash
echo '{"method":"io.ducky.Keystroke.SendKeys","parameters":{"keys":["ctrl","a"]}}' | \
  socat - UNIX-CONNECT:/run/duckycap.varlink
```

## Troubleshooting

### Device not found

1. Check the device is connected:
   ```bash
   lsusb | grep -i "0483:d11c"
   ```
2. Check the input device exists:
   ```bash
   ls -la /dev/input/event*
   ```
3. Check udev rule is loaded:
   ```bash
   udevadm info /dev/input/duckypad
   ```

### Permission denied

The capture daemon requires root access to grab input devices. It should run as root via systemd.

### Service not starting

Check systemd logs:
```bash
journalctl -u duckycap.service -n 50
journalctl -u duckycap-varlink.service -n 50
```

### Input not blocked

Make sure the duckycap daemon is running and has successfully grabbed the device. Check the logs for "Device grabbed exclusively" message.

## Files

| File | Purpose |
|------|---------|
| `src/bin/duckycap.rs` | Capture daemon with evdev exclusive grab |
| `src/bin/duckycap-varlink.rs` | Varlink service implementation |
| `config.example.toml` | Example configuration file |
| `systemd/99-duckypad.rules` | udev rule for device identification |
| `systemd/duckycap.service` | systemd unit for capture daemon |
| `systemd/duckycap-varlink.socket` | systemd socket unit for varlink |
| `systemd/duckycap-varlink.service` | systemd unit for varlink service |

## License

MIT
