# evalys-rs — Developer Reference

Static file viewer for OAR simulation, energy series, and event JSON files.

---

## Module Structure

```
evalys-rs/
├── file_types/             — JSON schemas for supported file types (embedded at compile time)
│   ├── oar.json
│   ├── energy_series.json
│   └── event.json
├── examples/               — sample input files for testing
└── src/
    ├── main.rs             — entry point, CLI args
    ├── app.rs              — eframe App impl (UI loop)
    ├── sim_state.rs        — app state: imported tabs, active data, sync to ApplicationContext
    ├── sim_config.rs       — load/save sim_config.toml
    ├── file_import.rs      — file open dialog + import pipeline
    ├── tab_state_cache.rs  — per-tab preference cache (tab_states.json)
    ├── energy_estimate.rs  — estimate_from_jobs + series_from_raw
    └── file_types/
        ├── mod.rs          — FileTypeConfig trait + FileTypeRegistry
        ├── oar.rs          — OAR simulation format
        ├── energy_series.rs — energy series format
        └── event.rs        — event format
```

---

## File Type System

### `FileTypeConfig` trait (`src/file_types/mod.rs`)

```rust
pub trait FileTypeConfig: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn visualization_targets(&self) -> &[VisualizationTarget];
    fn detect(&self, content: &str) -> f32;      // confidence 0.0–1.0
    fn validate(&self, content: &str) -> Vec<ValidationError>;
    fn parse(&self, content: &str) -> Result<ParsedFileData, String>;

    // Optional overrides:
    fn supports_hierarchy_controls(&self) -> bool { true }
    fn hierarchy_levels(&self) -> Option<Vec<String>> { None }
}
```

`ParsedFileData` contains: `resources`, `clusters`, `jobs`, `strata_by_resource_id`, `raw_energy_series`, `markers`.

### `FileTypeRegistry`

Registration order sets priority on `detect` score tie:

```rust
// src/file_types/mod.rs — impl Default for FileTypeRegistry
registry.register(Box::new(oar::OarFileType::new()));
registry.register(Box::new(energy_series::EnergySeriesFileType::new()));
registry.register(Box::new(event::EventFileType::new()));
```

### Adding a new file type

**Step 1** — create `src/file_types/mytype.rs`:

```rust
use super::{FileTypeConfig, ParsedFileData, ValidationError, VisualizationTarget};

pub struct MyFileType;

impl MyFileType {
    pub fn new() -> Self { Self }
}

impl FileTypeConfig for MyFileType {
    fn name(&self) -> &str { "My Type" }
    fn description(&self) -> &str { "Shown in the import dialog" }
    fn visualization_targets(&self) -> &[VisualizationTarget] {
        &[VisualizationTarget::Gantt]
    }
    fn detect(&self, content: &str) -> f32 {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(content) else { return 0.0 };
        if val.get("my_required_field").is_some() { 0.9 } else { 0.0 }
    }
    fn validate(&self, content: &str) -> Vec<ValidationError> { Vec::new() }
    fn parse(&self, content: &str) -> Result<ParsedFileData, String> {
        Ok(ParsedFileData {
            resources: Vec::new(),
            clusters: Vec::new(),
            jobs: Vec::new(),
            strata_by_resource_id: Default::default(),
            raw_energy_series: None,
            markers: Vec::new(),
        })
    }
}
```

**Step 2** — declare the module in `src/file_types/mod.rs`:

```rust
pub mod mytype;
```

**Step 3** — register in `impl Default for FileTypeRegistry` (same file):

```rust
registry.register(Box::new(mytype::MyFileType::new()));
```

More specific types first.

---

## Per-tab Preference Cache

### Principle

Each imported file has a stable identity from two keys:

| Key | How computed | Role |
|-----|--------------|------|
| Absolute path | `std::fs::canonicalize` at import | O(1) lookup |
| FNV-1a 64-bit hash | first 8 KB of content + total length | Fallback if file moved/renamed |

On tab open, cache is checked by path first, then hash. Match found → preferences restored immediately.

### Persisted fields per tab

| Field | Description |
|-------|-------------|
| `canvas_width_s` | Visible Gantt width in seconds (zoom level) |
| `sideways_pan` | Horizontal offset in points |
| `row_height` | Resource row height |
| `view_index` | Active aggregation view index |
| `energy_y_min/max` | Y bounds of the XY panel |
| `energy_fit` | "Fit to figure" checkbox |
| `energy_panel_height` | XY panel height |

### Save triggers

| Event | Code location |
|-------|---------------|
| Tab switch | `render_compact_toolbar` and `render` in `goard_core` |
| Tab close | `close_ds` handler in `render_data_source_tabs` |
| App exit | `eframe::App::on_exit` → `flush_all_tab_states` |

> For the currently active tab, `persist_tab_state` reads directly from live state (`self.options.*`, `self.xy_panel.*`), not the stale `tab_view_state` HashMap.

### Cache file

`evalys-rs/tab_states.json` — written in the evalys-rs working directory. Gitignored.

```json
{
  "path": "/absolute/path/to/file.json",
  "hash": "494729c9f071c0bc",
  "state": {
    "canvas_width_s": 86400.0,
    "sideways_pan": 0.0,
    "row_height": 20.0,
    "view_index": 0,
    "energy_y_min": null,
    "energy_y_max": null,
    "energy_fit": true,
    "energy_panel_height": 270.0
  }
}
```

Max 200 entries (FIFO). Dedup key is the hash: if a file is moved, the stored path updates automatically on next open.

---

## Energy Estimation

`src/energy_estimate.rs` provides two functions:

- `estimate_from_jobs(jobs, start_s, end_s, step_s, watts_per_resource)` — computes a power series from job allocations. `watts_per_resource` comes from `gantt_config.energy_watts_per_resource`.
- `series_from_raw(raw)` — adds two zero-padding points at each end of a measured series so panning outside the data range shows zero instead of a gap.

---

## Configuration

### `evalys-rs/sim_config.toml`

SSH connection and display preferences. Written by the Settings panel.

---

## Tests

Run with `cargo test -p evalys-rs`.

**`src/energy_estimate.rs`** — 7 tests

| Test | What it checks |
|------|----------------|
| `estimate_no_jobs_gives_zeros` | No jobs → all points at 0 W |
| `estimate_single_job_correct_watts` | 2 resources × 300 W = 600 W at each sample |
| `estimate_job_outside_window_ignored` | Job outside time window → zero contribution |
| `estimate_partial_overlap` | Job starting mid-window: 0 W before, correct after |
| `estimate_returns_empty_for_invalid_range` | `end < start` or `step = 0` → empty vec |
| `series_from_raw_empty` | Empty input → empty output |
| `series_from_raw_pads_zeros` | Raw series gets 2 zero points before and 2 after |

**`src/sim_state.rs`** — 3 tests

| Test | What it checks |
|------|----------------|
| `time_range_empty_jobs` | No jobs → sentinel values `(MAX, MIN)` |
| `time_range_skips_job_0` | Virtual "all_resources" row (id=0) excluded from range |
| `time_range_multiple_jobs` | Global min/max correct across multiple jobs |
