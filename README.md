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
- Real-time job monitoring via SSH
- Auto-refresh with configurable interval
- WASM build with PWA/offline support

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

Configure your SSH host in `liveOAR/live_config.toml`, then:

```bash
cargo run -p liveOAR --release
```

#### Web (WASM)

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
cd liveOAR
trunk serve
```

Access at `http://127.0.0.1:8080/index.html#dev`

> Append `#dev` to skip PWA caching during development.

#### Web Deployment

```bash
cd liveOAR
trunk build --release
```

Deploy the generated `liveOAR/dist/` directory to any static hosting platform.

## Contributing

- Report bugs
- Propose features
- Submit PRs

## License

This project is open source and available under the LGPL-2.1 license.
