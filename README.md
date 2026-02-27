# Ducky Relay

A varlink service that listens for keystroke messages.

## Message Format

```json
{ "key": "a" }
```

## Interface

The service exposes the `io.ducky.Keystroke` interface with the following method:

### SendKey

Sends a single keystroke to the service.

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

**Errors:**
- `io.ducky.Keystroke.InvalidKey` - The key parameter is invalid or empty

## Building

```bash
cargo build --release
```

## Installation

### 1. Install the binary

```bash
sudo cp target/release/duckycap-varlink /usr/local/bin/
```

### 2. Install systemd unit files

```bash
sudo cp systemd/duckycap-varlink.socket /etc/systemd/system/
sudo cp systemd/duckycap-varlink.service /etc/systemd/system/
sudo systemctl daemon-reload
```

### 3. Enable and start the socket

```bash
sudo systemctl enable --now duckycap-varlink.socket
```

## Usage

### Send a keystroke

```bash
varlinkctl call /run/duckycap.varlink io.ducky.Keystroke.SendKey '{"key": "a"}'
```

Expected response:
```json
{
    "success": true,
    "key": "a"
}
```

### Get service info

```bash
varlinkctl info /run/duckycap.varlink
```

### Introspect the interface

```bash
varlinkctl introspect /run/duckycap.varlink io.ducky.Keystroke
```

### Debug with socat

```bash
echo '{"method":"io.ducky.Keystroke.SendKey","parameters":{"key":"a"}}' | \
  socat - UNIX-CONNECT:/run/duckycap.varlink
```

## Development

### Run directly (without systemd)

```bash
# Create the /run directory if needed (requires root for /run)
sudo mkdir -p /run

# Run the service
sudo cargo run --bin duckycap-varlink
```

Or for testing without root, modify the `SOCKET_PATH` constant in the source to use `/tmp/duckycap.varlink`.

### Testing

```bash
# In one terminal
sudo cargo run --bin duckycap-varlink

# In another terminal
varlinkctl call /run/duckycap.varlink io.ducky.Keystroke.SendKey '{"key": "a"}'
```

## Systemd Socket Activation

The service supports systemd socket activation. When started via systemd socket activation:

1. systemd listens on `/run/duckycap.varlink`
2. When a client connects, systemd starts `duckycap-varlink.service`
3. The service inherits the socket and handles the connection
4. The service continues running until idle (configurable)

This provides:
- Zero resource usage when idle
- Automatic service startup on demand
- Centralized service management

## Files

| File | Purpose |
|------|---------|
| `src/bin/duckycap-varlink.rs` | Main varlink service implementation |
| `systemd/duckycap-varlink.socket` | Systemd socket unit for activation |
| `systemd/duckycap-varlink.service` | Systemd service unit |

## License

MIT
