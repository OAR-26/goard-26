# Goard

Gantt-based dashboard for monitoring HPC cluster jobs, built with Rust + egui/eframe.

| Crate | Purpose |
|-------|---------|
| **evalys-rs** | Import and visualize static OAR/energy JSON files |
| **liveOAR** | Connect to a live OAR cluster over SSH in real time |
| **goard_core** | Shared rendering library (Gantt, Dashboard, XY panel, filters) |

---

## Documentation

| | User manual | Developer reference |
|-|-------------|---------------------|
| **evalys-rs** | [evalys-rs/USER.md](evalys-rs/USER.md) | [evalys-rs/DEV.md](evalys-rs/DEV.md) |
| **liveOAR** | [liveOAR/USER.md](liveOAR/USER.md) | [liveOAR/DEV.md](liveOAR/DEV.md) |
| **goard_core** | — | [goard_core/DEV.md](goard_core/DEV.md) |

---

## Quick Start

### Prerequisites

- Rust + Cargo
- SSH access to an HPC cluster *(liveOAR only)*

### evalys-rs (static file viewer)

```bash
cargo run -p evalys-rs --release
cargo run -p evalys-rs --release -- examples/oar.json
cargo run -p evalys-rs --release -- examples/oar.json+examples/energy.json
```

### liveOAR - native

```bash
GOARD_SSH_HOST=grenoble.g5k cargo run -p liveOAR --release
```

### liveOAR - web (WASM)

```bash
# Terminal 1 - backend (SSH + HTTP server)
GOARD_SSH_HOST=grenoble.g5k cargo run -p liveOAR --release -- --serve

# Terminal 2 - frontend (WASM in browser)
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
cd liveOAR && trunk serve
```

Open `http://localhost:8080`. Replace `localhost` with the machine IP for other devices on the same network.

See [liveOAR/DEV.md](liveOAR/DEV.md) for the full web architecture.

---

## License

LGPL-2.1
