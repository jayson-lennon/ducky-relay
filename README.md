# Ducky Relay

A system for intercepting duckyPad keyboard input, blocking it from reaching the system, and forwarding key combinations to a varlink service.

## Overview

This project consists of two main components:

1. **duckycap** - Capture daemon that intercepts duckyPad input using evdev exclusive grab
2. **duckycap-varlink** - Varlink service that receives keystroke messages

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
    "keys": ["ctrl", "shift", "a"],
    "pressed": true
}
```

**Returns:**
```json
{
    "success": true,
    "keys": ["ctrl", "shift", "a"],
    "pressed": true
}
```

The `pressed` parameter indicates:
- `true` - key down event
- `false` - key up event

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

### Arch Linux (recommended)

Build and install using makepkg:

```bash
makepkg -si
```

This will:
- Compile the binaries
- Install binaries to `/usr/bin/`
- Install udev rules to `/usr/lib/udev/rules.d/`
- Install systemd units to `/usr/lib/systemd/system/`
- Install example config to `/etc/duckycap/config.example.toml`

### Post-installation setup

1. Create your configuration file:
   ```bash
   sudo cp /etc/duckycap/config.example.toml /etc/duckycap/config.toml
   sudo nano /etc/duckycap/config.toml
   ```

2. Enable the varlink socket:
   ```bash
   sudo systemctl enable --now duckycap-varlink.socket
   ```

The `duckycap.service` will be activated automatically by udev when the duckyPad is connected.

## Configuration

The `duckycap-varlink` service uses a TOML configuration file to map key combinations to commands.

### Configuration File Format

```toml
# User to run commands as (required)
user = "your-username"

# Command mappings
# Each mapping has:
#   - keys: Key combination string using + to combine keys (e.g., "meta+f1", "a", "ctrl+shift+b")
#   - cmd: Command to execute
#     - If cmd starts with '/', it's treated as an absolute path to a script
#     - Otherwise, it's run as a shell command

# Shell command example
[[commands]]
keys = "a"
cmd = "obs-cmd recording start"

# Script path example (absolute path)
[[commands]]
keys = "b"
cmd = "/home/your-username/scripts/volume-down.sh"

# More examples
[[commands]]
keys = "meta+f1"
cmd = "/home/your-username/scripts/toggle-mute.sh"

[[commands]]
keys = "ctrl+shift+k"
cmd = "loginctl lock-session"
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

# Send a key combination (key down)
varlinkctl call /run/duckycap.varlink io.ducky.Keystroke.SendKeys '{"keys": ["ctrl", "shift", "a"], "pressed": true}'
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

## License

AGPL-3.0
