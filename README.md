# MeshBoard

MeshBoard is a minimal bulletin board system (BBS) designed to manage and share information seamlessly across the Meshtastic network. It enables users to communicate via channels, post messages, and keep in touch even in decentralized mesh networks using Bluetooth Low Energy (BLE).

## Features

- Minimalistic and easy-to-use bulletin board system.
- Supports multiple channels for organizing conversations.
- User identification by public key hashes for security and privacy.
- Persistent storage with an embedded database for channels, messages, and user data.
- BLE connectivity to integrate with Meshtastic devices.
- Commands-based interaction allowing users to join channels, post messages, list channels or messages, and get help.

## Commands Supported

Users interact with MeshBoard via commands as text input:

- `h` : Displays help information about commands.
- `c`: Lists available channels.
- `j <channel>`: Joins the specified channel.
- `p <message>`: Posts a message to the current channel.
- `l`: Lists recent messages from the current channel.

## Getting Started

### Prerequisites

- Rust (edition 2024)
- Bluetooth environment supporting Meshtastic devices
- Linux environment (e.g., Raspberry Pi) recommended for BLE support

### Setup Bluetooth on Raspberry Pi

Read the RASPBERRY_BLE.md for usefull commands

### Build and Run

You can build MeshBoard with:

```bash
cargo build --release --target aarch64-unknown-linux-gnu
```

Then run your application, setting the `BLE_DEVICE` environment variable to your Bluetooth device (or use an .env file):

```bash
export BLE_DEVICE=<your ble device>
cargo run --release -- start
```

### Tool

If you run `cargo run --release -- tool` appears command-line tool interface for interacting with Meshtastic BLE devices. Here are the main features:

- `ble <device_name|auto>`: Connect to a BLE device by name or auto-select if only one is available.
- `listen [all]`: Listen for incoming messages or mesh status updates, optionally showing all radio data.
- `send <node_short_name> <message>`: Send a text message to a specific node by short name.
- `nodes`: List connected nodes by their short names.
- `exit`: Exit the tool.
- `help`: Show available commands.

This project is licensed under the MIT License.
