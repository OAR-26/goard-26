# liveOAR — Developer Reference

Live OAR cluster viewer. Polls a cluster over SSH and streams job data into the shared `goard_core` UI. Supports both a native desktop build and a web (WASM) build backed by an HTTP server.

---

## Module Structure

```
liveOAR/
├── Trunk.toml              — trunk dev-server config + /api/* proxy
├── presets.json            — cluster filter presets
└── src/
    ├── main.rs             — entry point; --serve flag starts HTTP backend
    ├── app.rs              — eframe App impl (UI loop)
    ├── live_engine.rs      — SSH polling + WASM fetch; promotes data to ApplicationContext
    ├── oar_fetch.rs        — SSH command + JSON parsing
    ├── refresh_coordinator.rs — MPSC channels + shared Arc<Mutex<_>> state
    ├── auth_view.rs        — login form UI
    ├── cluster_presets.rs  — cluster preset CRUD + selector widget
    ├── energy_estimate.rs  — estimate_from_jobs (same logic as evalys-rs)
    ├── mocker.rs           — fake data for testing without SSH
    ├── api_types.rs        — ApiSnapshot (jobs + resources + dead_intervals), used by server + WASM
    └── server.rs           — axum HTTP server (native only)
```

---

## Live Data Engine (`live_engine.rs`)

### Native flow

```
App::new()
  └── LiveEngine::new()
        └── update_periodically()
              └── thread::spawn ──► loop:
                    1. Sleep refresh_rate seconds
                    2. SSH fetch → write /tmp/liveOAR_data.json (or liveOAR/data/data.json)
                    3. Parse jobs + resources + dead_intervals
                    4. Send through MPSC channels

App::update() each frame:
  └── live_engine.poll()
        ├── try_recv() jobs → app.data.jobs
        ├── try_recv() resources → app.data.strata
        ├── try_recv() dead_intervals → app.data.dead_intervals
        └── sync start_date/end_date mutexes from app view window
```

### WASM flow

```
App::new()
  └── live_engine.update_periodically()
        └── spawn_local async loop:
              every 30s: GET /api/data?start=TS&end=TS → ApiSnapshot → send channels

First main-view frame (after auth):
  └── live_engine.instant_update()   ← immediate fetch with correct gantt window
        └── spawn_local: GET /api/data → send channels

User navigates (pan/zoom):
  └── live_engine.instant_update()   ← generation counter prevents stale results
```

### Shared state (`RefreshCoordinator`)

| Field | Role |
|-------|------|
| `start_date / end_date: Arc<Mutex<DateTime>>` | Current view window; synced by `poll()` each frame |
| `refresh_rate: Arc<Mutex<u64>>` | Seconds between polls (`u64::MAX` = never) |
| `is_refreshing: Arc<Mutex<bool>>` | Dedup lock (native only; WASM uses `request_gen`) |
| `request_gen: Arc<Mutex<u64>>` | WASM only: incremented on each `instant_update`; stale fetches self-discard |
| `jobs_sender / jobs_receiver` | MPSC channel for job snapshots |
| `resources_sender / resources_receiver` | MPSC channel for resource/strata snapshots |
| `dead_intervals_sender / dead_intervals_receiver` | MPSC channel for dead resource intervals |

### Race condition guard (WASM)

When the user navigates quickly, multiple `instant_update` calls can be in-flight simultaneously. Each call increments `request_gen` and captures the current value. When the fetch completes, if the stored gen no longer matches the captured gen, the result is discarded — a newer request has already been dispatched.

---

## Web / WASM Architecture

The browser cannot do SSH directly, so the web build splits into two processes:

```
┌─────────────────────────────┐     HTTP /api/data?start=TS&end=TS    ┌──────────────────────────────┐
│  Frontend  (trunk serve)    │ ─────────────────────────────────────► │  Backend  (cargo run --serve)│
│  WASM in browser            │                                        │  axum on 0.0.0.0:3030        │
│  egui/eframe rendering      │ ◄───────────────────────────────────── │  SSH → OAR cluster           │
│  gloo-net HTTP fetch        │     ApiSnapshot { jobs, resources,     │  writes /tmp/liveOAR_data.json│
│  gloo-timers periodic loop  │                  dead_intervals }      │                              │
└─────────────────────────────┘                                        └──────────────────────────────┘
         ↑
  Trunk proxy: /api/* → http://localhost:3030/api/*
  (same-origin from browser perspective)
```

### Backend (`--serve` mode)

`cargo run -p liveOAR -- --serve [--port 3030]`

Single endpoint: `GET /api/data?start=<unix_ts>&end=<unix_ts>`

On each request:
1. `spawn_blocking` runs `get_current_jobs_for_period` (synchronous SSH)
2. Parses `jobs`, `resources`, `dead_intervals` from `/tmp/liveOAR_data.json`
3. Returns `ApiSnapshot` as JSON

No background polling, no cache. The frontend drives timing.

### Frontend (WASM)

- `gloo-net` for HTTP fetch
- `gloo-timers` for the 30 s periodic loop
- On startup: `update_periodically()` spawns the periodic loop (first fetch waits 30 s)
- On first main-view frame (after auth): `instant_update()` fires immediately with the correct gantt window
- On user navigation: `instant_update()` fires with the new window; `request_gen` discards stale in-flight results

### Trunk configuration (`Trunk.toml`)

```toml
[serve]
address = "0.0.0.0"        # reachable from other devices on the same network

[watch]
ignore = ["data", "dist"]  # prevent hot-reload loop on SSH data writes

[[proxy]]
rewrite = "/api/"
backend = "http://localhost:3030/api/"
```

The proxy makes `/api/*` requests go to the axum backend transparently (no CORS).

### Mock fallback

If the backend is not reachable (`fetch_snapshot` returns `None`), the WASM app falls back to `mock_jobs()` / `mock_stratas()`. Useful for frontend development without SSH access.

---

## Authentication

Auth lives entirely in `liveOAR` — `goard_core` has no concept of it.

- `src/auth_view.rs` — login form UI; hardcoded check: `username == "admin" && password == "admin"`
- `App.connected_as: Option<String>` — set on login, cleared on logout
- `is_admin` is derived as `connected_as.is_some()` in `app.rs`
- `goard_core`'s Gantt renders create/edit/delete view panels always; `liveOAR/app.rs` gates the Admin button behind `is_admin`

**Do not deploy to production without replacing this mechanism.**

---

## Configuration

### `liveOAR/presets.json`

Cluster filter presets:

```json
[
  { "name": "My preset", "clusters": ["cluster-a", "cluster-b"] }
]
```

### `GOARD_SSH_HOST` environment variable

SSH host for the OAR cluster. Set before running either mode:

```bash
GOARD_SSH_HOST=grenoble.g5k cargo run -p liveOAR --release
GOARD_SSH_HOST=grenoble.g5k cargo run -p liveOAR --release -- --serve
```

---

## Tests

Run with `cargo test -p liveOAR`.

**`src/energy_estimate.rs`** — 5 tests

| Test | What it checks |
|------|----------------|
| `estimate_no_jobs_gives_zeros` | No jobs → all points at 0 W |
| `estimate_single_job_correct_watts` | 2 resources × 300 W = 600 W at each sample |
| `estimate_job_outside_window_ignored` | Job outside time window → zero contribution |
| `estimate_partial_overlap` | Job starting mid-window: 0 W before, correct after |
| `estimate_returns_empty_for_invalid_range` | `end < start` or `step = 0` → empty vec |

**`src/cluster_presets.rs`** — 4 tests

| Test | What it checks |
|------|----------------|
| `cluster_preset_serde_roundtrip` | JSON serialize/deserialize of a preset list |
| `cluster_preset_empty_clusters_allowed` | Preset with no clusters is valid |
| `load_presets_from_nonexistent_file_returns_empty` | Missing file → empty list, no panic |
| `save_and_load_roundtrip` | Write then read back → identical data |
