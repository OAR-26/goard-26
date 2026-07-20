# Goard

Goard (pronounced "guard") is a desktop/web dashboard for monitoring HPC cluster jobs, built with Rust + egui/eframe.

It ships as two separate binaries:

| Binary | Purpose |
|--------|---------|
| **evalys-rs** | Import and visualize static OAR/energy JSON files |
| **liveOAR** | Connect to a live OAR cluster and monitor jobs in real time over SSH |

Both share the same Gantt chart, Dashboard, and XY/energy panel from the `goard_core` library.

## Key Features

- Interactive Gantt chart with configurable hierarchy views
- Interactive Dashboard (metrics + sortable job table)
- XY/energy panel (estimated from jobs or measured from imported series)
- Multi-criteria job filtering + cluster presets
- Light/dark theme, i18n (EN/FR), adjustable font size

**evalys-rs only:**
- File import (OAR Simulation, Energy Series, Event formats)
- File grouping: overlay energy measurements on Gantt job data
- Per-file preference restore (zoom, pan, view, panel state)

**liveOAR only:**
- Real-time job monitoring via SSH (`GOARD_SSH_HOST` env var)
- Auto-refresh with configurable interval
- Web (WASM) build: live data via HTTP backend, mock fallback when offline

## Documentation

- **[UserManuel.md](UserManuel.md)** — end-user guide: all UI features, import, filters, views, admin
- **[DevManuel.md](DevManuel.md)** — developer guide: architecture, state model, adding file types, editing `views.json`, config files

## Getting Started

### Prerequisites

- Rust and Cargo
- SSH access to an HPC cluster *(liveOAR only)*

### Running evalys-rs (file viewer)

```bash
rustup update

# Open the UI with no files pre-loaded
cargo run -p evalys-rs --release

# Pre-load one or more files on launch
cargo run -p evalys-rs --release -- examples/oar.json
cargo run -p evalys-rs --release -- examples/oar.json examples/energy.json   # separate tabs

# Pre-load a group (files joined by + become one overlaid tab)
cargo run -p evalys-rs --release -- examples/oar.json+examples/energy.json

# Mix standalone files and groups
cargo run -p evalys-rs --release -- examples/oar1.json examples/oar2.json+examples/energy2.json
```

### Running liveOAR (live cluster viewer)

Set the SSH host via environment variable, then run:

```bash
GOARD_SSH_HOST=grenoble.g5k cargo run -p liveOAR --release
```

#### Web (WASM) — with live data

The browser cannot do SSH directly, so the web mode splits into two processes:

**Terminal 1 — backend** (SSH polling + HTTP server):
```bash
GOARD_SSH_HOST=grenoble.g5k cargo run -p liveOAR --release -- --serve
# optional: --port 3030 (default)
```

**Terminal 2 — frontend** (WASM build served in the browser):
```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
cd liveOAR
trunk serve
```

Access at `http://localhost:8080` (or replace `localhost` with the machine's IP for other devices on the same network).

> Append `#dev` to the URL to skip PWA caching during development.

The frontend fetches jobs and resources from the backend every 30 seconds, passing its current view window so the SSH query always matches what is displayed. The ⟳ button triggers an immediate SSH fetch. If the backend is not running, the app falls back to mock data automatically.

See [`liveOAR/LIVE_WEB.md`](liveOAR/LIVE_WEB.md) for a full explanation of the architecture and dataflow.

#### Web — mock data only (no backend needed)

```bash
cd liveOAR && trunk serve
```

The app loads with randomly generated mock jobs and resources — useful for frontend development without SSH access.

#### Web Deployment

```bash
cd liveOAR
trunk build --release
```

Deploy `liveOAR/dist/` to any static host and run the backend server on a machine with SSH access to the cluster.

## Contributing

- Report bugs
- Propose features
- Submit PRs

## License

This project is open source and available under the LGPL-2.1 license.
