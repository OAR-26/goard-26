# Goard

Goard (pronounced "guard") is a desktop/web dashboard for monitoring HPC cluster jobs, built with Rust + egui/eframe. It supports real-time job tracking via SSH and offline analysis through JSON file imports.

## Key Features

- Real-time job monitoring (optional, enabled via `--live` flag)
- File import for offline/historical analysis (OAR, Energy Series, Event formats)
- File grouping: overlay energy measurements on Gantt job data
- Interactive dashboard view (metrics + job table)
- Interactive Gantt chart with configurable hierarchy views
- Energy consumption diagram (estimated or measured)
- Multi-criteria job filtering + cluster presets
- Light/dark theme, i18n (EN/FR), adjustable font size
- WASM build with PWA/offline support

## Documentation

- **[UserManuel.md](UserManuel.md)** — end-user guide: all UI features, import, filters, views, admin
- **[DevManuel.md](DevManuel.md)** — developer guide: architecture, state model, adding file types, editing `views.json`, config files

## Getting Started

### Prerequisites

- Rust and Cargo
- Git
- SSH access to an HPC cluster *(only required for `--live` mode)*

### Quick Start

#### Native

```bash
rustup update
```

**Import-only mode** (default — no live data, import files manually via UI):
```bash
cargo run --release
```

**Pre-load one or more files on launch:**
```bash
cargo run --release -- examples/oar.json
cargo run --release -- examples/oar.json examples/energy.json   # separate tabs
```

**Pre-load a group** (files joined by `+` become one overlaid tab):
```bash
cargo run --release -- examples/oar.json+examples/energy.json
```

**Mix standalone files and groups:**
```bash
cargo run --release -- examples/oar1.json examples/oar2.json+examples/energy2.json
```

**Live data mode** (connects to HPC cluster for real-time monitoring):
```bash
cargo run --release -- --live
cargo run --release -- --live examples/oar.json+examples/energy.json
```

#### Web (WASM)

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
trunk serve
```
Access at `http://127.0.0.1:8080/index.html#dev`

> Append `#dev` to skip PWA caching during development.

#### Web Deployment

```bash
trunk build --release
```

Deploy the generated `dist/` directory to any static hosting platform.

> The app supports offline functionality through service worker caching.

## Contributing

- Report bugs
- Propose features
- Submit PRs

## License

This project is open source and available under the LGPL-2.1 license.
