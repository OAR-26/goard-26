# Developer Manual - Goard

---

## 1) Tech Stack

| Component | Library / Version |
|-----------|------------------|
| UI framework | [egui](https://github.com/emilk/egui) 0.30 + eframe 0.30 |
| Plots | egui_plot 0.30 |
| Serialization | serde 1.0 + serde_json 1.0 |
| Dates | chrono 0.4 (+ chrono-tz) |
| i18n | rust-i18n 3 |
| Targets | native (Linux/macOS/Windows) |

---

## 2) Project Structure

Cargo workspace with three crates:

```
Cargo.toml                  — workspace root
├── goard_core/             — rendering library (lib crate)
├── evalys-rs/              — static file viewer (binary crate)
└── liveOAR/                — live OAR cluster viewer (binary crate)
```

### `goard_core/` — shared rendering library

Pure rendering and data model. No knowledge of where data comes from (no SSH, no file parsing, no energy estimation). Both binaries depend on it.

```
goard_core/
├── config.toml             — Gantt config (colors, timespan)
├── views.json              — saved Gantt views + leaf info presets
└── src/
    ├── lib.rs
    ├── models/
    │   ├── data_structure/
    │   │   ├── application_context.rs  — central app state (see §3)
    │   │   ├── job_data.rs             — jobs, clusters, strata, plot_series
    │   │   ├── gantt_config.rs         — config.toml (colors, timespan, panel heights)
    │   │   ├── application_options.rs  — UI state (zoom, pan, row height)
    │   │   ├── ui_preferences.rs       — font, theme, language
    │   │   ├── filters.rs              — active job filters
    │   │   ├── view_type.rs            — Gantt / Dashboard enum
    │   │   ├── job.rs / resource.rs / strata.rs / marker.rs / job_sorting.rs
    │   │   └── mod.rs
    │   └── utils/
    │       ├── date_converter.rs
    │       ├── utils.rs                — cluster/host/resource helpers
    │       ├── secret.rs
    │       └── mod.rs
    └── views/
        ├── view.rs                     — top-level render dispatch
        ├── menu/
        │   ├── menu.rs                 — menu bar (File, Options, ?)
        │   ├── tools.rs                — toolbar + Gantt summary row
        │   ├── filtering.rs            — Filters panel
        │   ├── options.rs              — Options panel (language, font, theme)
        │   ├── settings_panel.rs       — Gantt config settings panel
        │   ├── field_colors_editor.rs
        │   └── mod.rs
        ├── main_page/
        │   ├── dashboard.rs            — Dashboard view
        │   ├── gantt/
        │   │   ├── mod.rs              — GanttChart: tabs, panels, main render
        │   │   ├── canvas.rs           — resource row + job drawing
        │   │   ├── interaction.rs      — zoom/pan (mouse + keyboard)
        │   │   ├── timeline.rs         — time axis + "now" line
        │   │   ├── labels.rs           — gutter labels
        │   │   ├── jobs.rs             — strata field resolution, resource sorting
        │   │   ├── panels.rs           — Admin, Create/Edit view, XyPanelState
        │   │   ├── xy_plot.rs          — generic XY plot (egui_plot)
        │   │   ├── theme.rs            — colors by light/dark theme
        │   │   └── types.rs            — Options, Info, ResourceFilter, LeafInfoPreset
        │   └── mod.rs
        └── components/
            ├── gantt_job_color.rs
            ├── job_details.rs
            ├── dashboard_components/
            │   ├── job_table.rs / job_table_col_selection.rs / job_table_sorting.rs
            │   ├── metric_box.rs / metric_chart.rs / metric_grid.rs
            │   └── mod.rs
            └── mod.rs
```

### `evalys-rs/` — static file viewer

Imports OAR simulation files and/or energy series files. Handles file detection, parsing, tab state cache, and energy estimation from jobs or raw series.

```
evalys-rs/
├── sim_config.toml         — SSH + display config
├── file_types/             — JSON schemas for supported file types (embedded at compile time)
│   ├── oar.json
│   ├── energy_series.json
│   └── event.json
├── examples/               — sample input files for testing
└── src/
    ├── main.rs             — entry point
    ├── app.rs              — eframe App impl (UI loop)
    ├── sim_state.rs        — app state: imported tabs, active data, sync to ApplicationContext
    ├── sim_config.rs       — load/save sim_config.toml
    ├── file_import.rs      — file open dialog + import pipeline
    ├── tab_state_cache.rs  — per-tab preference cache (tab_states.json)
    ├── energy_estimate.rs  — estimate_from_jobs + series_from_raw (zero-padding)
    └── file_types/
        ├── mod.rs          — FileTypeConfig trait + FileTypeRegistry
        ├── oar.rs          — OAR simulation format
        ├── energy_series.rs — energy series format
        └── event.rs        — event format
```

### `liveOAR/` — live OAR cluster viewer

Polls a live OAR cluster over SSH. No file import. Manages connection auth, polling, cluster presets, and energy estimation from live jobs.

```
liveOAR/
├── live_config.toml        — SSH host + credentials config
├── presets.json            — cluster filter presets
├── data/data.json          — last fetched job data (written by background thread)
└── src/
    ├── main.rs             — entry point
    ├── app.rs              — eframe App impl (UI loop)
    ├── live_engine.rs      — background thread: poll → promote jobs to ApplicationContext
    ├── oar_fetch.rs        — SSH fetch + JSON parsing
    ├── refresh_coordinator.rs — MPSC channels + shared mutexes
    ├── auth_view.rs        — login form UI
    ├── cluster_presets.rs  — cluster preset CRUD + selector widget
    ├── energy_estimate.rs  — estimate_from_jobs (same logic as evalys-rs)
    └── mocker.rs           — fake data for testing without SSH
```

---

## 3) State Architecture (`ApplicationContext`)

`ApplicationContext` is the central container owned by the binary and passed into `goard_core` views. Split into sub-structs:

| Field | Type | Content |
|-------|------|---------|
| `data` | `JobData` | jobs, clusters, strata, markers, `plot_series` |
| `prefs` | `UiPreferences` | font, theme, language, Gantt view state |
| `filters` | `Filters` | active job filter state |
| `options` | `ApplicationOptions` | zoom, pan, row height |

Session flags (`view_type`, `user_connected`, `show_xy_panel`, `show_gantt_panel`) sit flat on `ApplicationContext` — used everywhere.

### `plot_series`

`JobData.plot_series: Vec<(String, Vec<(i64, f64)>)>` is the generic XY data fed to the XY panel. `goard_core` renders whatever is in it. The binary owns the content:

| Binary | What it puts in `plot_series` |
|--------|-------------------------------|
| evalys-rs | Estimated series (from jobs) and/or raw measured series, depending on what files are loaded |
| liveOAR | Estimated series computed from `all_jobs` over the full job time range |

### Swap pattern (liveOAR)

Background thread writes into `swap_all_jobs`. UI thread copies to `all_jobs` only when `check_job_update()` drains the channel. Prevents partial reads of an in-progress snapshot.

---

## 4) Live Data Mode (liveOAR)

### Flow

```
App::new()
  └── LiveEngine::spawn()
        └── thread::spawn ──► loop:
              1. Wait refresh_rate seconds
              2. SSH fetch → parse jobs + resources
              3. Write data/data.json (optional local cache)
              4. jobs_sender.send(jobs)

App::update() each frame:
  └── live_engine.check_job_update()
        ├── jobs_receiver.try_recv()  → swap_all_jobs → promote to app.data
        └── rebuild clusters + strata + estimate plot_series
```

### Shared state (`RefreshCoordinator`)

| Mutex | Role |
|-------|------|
| `refresh_rate: Arc<Mutex<u64>>` | seconds between polls (u64::MAX = never) |
| `is_refreshing: Arc<Mutex<bool>>` | dedup lock |
| `start_date / end_date` | visible time window (updated when user pans the Gantt) |

---

## 5) File Type System (evalys-rs)

### `FileTypeConfig` trait (`evalys-rs/src/file_types/mod.rs`)

```rust
pub trait FileTypeConfig: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn visualization_targets(&self) -> &[VisualizationTarget];
    fn detect(&self, content: &str) -> f32;      // confidence 0.0–1.0
    fn validate(&self, content: &str) -> Vec<ValidationError>;
    fn parse(&self, content: &str) -> Result<ParsedFileData, String>;

    // Optional — override if needed:
    fn supports_hierarchy_controls(&self) -> bool { true }
    fn hierarchy_levels(&self) -> Option<Vec<String>> { None }
}
```

`ParsedFileData` contains: `resources`, `clusters`, `jobs`, `strata_by_resource_id`, `raw_energy_series`, `markers`.

### `FileTypeRegistry`

Registration order sets priority on `detect` score tie. Currently:

```rust
// evalys-rs/src/file_types/mod.rs — impl Default for FileTypeRegistry
registry.register(Box::new(oar::OarFileType::new()));
registry.register(Box::new(energy_series::EnergySeriesFileType::new()));
registry.register(Box::new(event::EventFileType::new()));
```

### Adding a new file type

**Step 1 — Create `evalys-rs/src/file_types/mytype.rs`**

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
        // Return > 0.5 if file matches this format. Higher = higher priority.
        let Ok(val) = serde_json::from_str::<serde_json::Value>(content) else { return 0.0 };
        if val.get("my_required_field").is_some() { 0.9 } else { 0.0 }
    }

    fn validate(&self, content: &str) -> Vec<ValidationError> {
        Vec::new()
    }

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

**Step 2 — Declare the module** in `evalys-rs/src/file_types/mod.rs`:

```rust
pub mod mytype;
```

**Step 3 — Register** in `impl Default for FileTypeRegistry` (same file):

```rust
registry.register(Box::new(mytype::MyFileType::new()));
```

More specific types first.

---

## 6) Gantt Views — editing `views.json`

`views.json` is loaded at startup and rewritten on every Admin UI change. Can also be edited manually while the app is closed.

### Full format

```json
{
  "views": [
    {
      "name": "Nodes",
      "levels": ["site", "cluster", "host"],
      "leaf_label_template": "{host|short}",
      "sort_by_label": false,
      "summary_fields": ["cluster", "host"],
      "leaf_infos": "host_info",
      "filter": {
        "field": "production",
        "value": "YES",
        "exclude": false
      }
    }
  ],
  "leaf_info_presets": [
    {
      "id": "host_info",
      "name": "Host",
      "fields": ["network_address", "comment", "cputype", "cpuset", "nodeset"]
    }
  ]
}
```

### View fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Label shown in the View menu |
| `levels` | string[] | yes | Hierarchy levels, coarsest to finest |
| `leaf_label_template` | string \| null | no | Label template. Variables: `{field}` or `{field\|short}` (truncates before first `.`) |
| `sort_by_label` | bool | no (default: false) | Sort groups by computed label instead of raw key |
| `summary_fields` | string[] | no | Fields shown in the summary row. Empty = last level |
| `leaf_infos` | string \| null | no | `id` of a `leaf_info_presets` entry |
| `filter` | object \| null | no | Filter on a strata field (see below) |

### Filter fields

```json
{
  "field": "production",
  "value": "YES",
  "exclude": false
}
```

- `exclude: false` → allowlist (keep only `field == value`)
- `exclude: true` → denylist (exclude when `field == value`)

### leaf_info_preset fields

| Field | Description |
|-------|-------------|
| `id` | Unique identifier, referenced by `leaf_infos` in views |
| `name` | Label shown at the top of the tooltip |
| `fields` | Strata fields to display in the tooltip |

### Available levels (strata fields)

`site`, `cluster`, `host`, `type`, `vlan`, `disk`, `disk_id`, `nodeset`, `subnet_address`, `subnet_prefix`, `slash_16` … `slash_22`, `network_address`, `ip`, `comment`, `nodemodel`, `cputype`, `cpufreq`, `core_count`, `thread_count`, `memnode`, `gpu_model`, `chassis`, `resource_id`, `production`, `state`, `besteffort`, `deploy`, `drain`

`site` is derived automatically from the host FQDN (first component before the first short segment).

---

## 7) Configuration Files

### `goard_core/config.toml`

Loaded by `GanttConfig::load()` at startup.

```toml
[gantt]
standby_truncate_state_to_now = true
besteffort_truncate_job_to_now = true
min_state_duration = 2
default_timespan = 21600
xy_panel_height = 270.0

[colors]
# dark theme
Absent = "#1e64dc"
Suspected = "#dc1e1e"
Dead = "#787878"
Standby = "#88ffff"

[colors_light]
# light theme
Absent = "#1040a0"
Suspected = "#a01010"
Dead = "#404040"
Standby = "#008888"
```

| Key | Effect |
|-----|--------|
| `standby_truncate_state_to_now` | Truncates in-progress Absent intervals to now when Standby applies |
| `besteffort_truncate_job_to_now` | Hides the future portion of besteffort jobs |
| `min_state_duration` | Minimum duration (seconds) to render a state interval |
| `default_timespan` | Initial Gantt width in seconds (21600 = 6h) |
| `xy_panel_height` | Initial height of the XY panel in pixels |
| `job_color_min` | Minimum RGB component for random job colors (0–255). Default 140: light colors, readable black labels. |

### `goard_core/views.json`

Gantt views + leaf info presets. Written by the Admin UI, editable manually (see §6).

### `liveOAR/presets.json`

Cluster filter presets. Format:

```json
[
  { "name": "My preset", "clusters": ["cluster-a", "cluster-b"] }
]
```

### `evalys-rs/sim_config.toml` / `liveOAR/live_config.toml`

SSH connection config and display preferences. Written by the Settings panel.

---

## 8) Authentication

Auth is a **proof of concept**: hardcoded credentials (`admin` / `admin`) in `goard_core/src/models/utils/secret.rs`.

`is_admin()` on `ApplicationContext` checks `user_connected == Some("admin")`.

Protected operations: create/edit/delete Gantt views, create/edit/delete cluster presets.

**Do not deploy to production without replacing this mechanism.**

---

## 9) Per-tab Preference Cache (evalys-rs)

### Principle

Each imported file has a stable identity from two keys:

| Key | How computed | Role |
|-----|--------------|------|
| **Absolute path** | canonicalized at import (`std::fs::canonicalize`) | O(1) lookup |
| **FNV-1a 64-bit hash** | first 8 KB of content + total length | Fallback if file moved/renamed |

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

| Event | Code |
|-------|------|
| Tab switch | save blocks in `render_compact_toolbar` and `render` |
| Tab close | `close_ds` handler in `render_data_source_tabs` (via `persist_tab_state`) |
| App exit | `eframe::App::on_exit` → `flush_all_tab_states` |

> **Important:** for the currently active tab, `persist_tab_state` reads directly from `self.options.*` and `self.xy_panel.*` (live state), not the `tab_view_state` HashMap (stale snapshot). Background tabs read the HashMap updated on last departure.

### Relevant files

| File | Role |
|------|------|
| `evalys-rs/src/tab_state_cache.rs` | `TabStateCache` (load/save/lookup/store) + `compute_file_hash()` |
| `evalys-rs/src/sim_state.rs` | `ImportedDataSource.file_hash: Option<String>` |
| `goard_core/src/views/main_page/gantt/mod.rs` | `persist_tab_state`, `flush_all_tab_states`, `restore_from_cache` |

### Cache file

`evalys-rs/tab_states.json` — written in the evalys-rs working directory. Gitignored: never commit machine-specific paths.

Entry format:

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

Max 200 entries (FIFO). Dedup key is the hash (not path): if a file is moved, the stored path updates automatically on next open.

---

## 10) XY Panel

The XY panel is a generic secondary plot below the Gantt. `goard_core` renders whatever series are in `app.data.plot_series` — it has no concept of "energy" or "estimation."

### Data sources by binary

**evalys-rs:**

| Situation | Series in `plot_series` |
|-----------|------------------------|
| OAR file only | Estimated series from jobs (full job time range) |
| Energy series file only | Raw measured series (zero-padded at edges) |
| OAR + Energy Series group | Both: estimated + measured |

**liveOAR:**

Always one series: estimated power from live jobs, computed over the full job time range on every job update.

### Estimation logic

Both binaries have `energy_estimate.rs` with `estimate_from_jobs(jobs, start_s, end_s, step_s, watts_per_resource)`. `watts_per_resource` comes from `gantt_config.energy_watts_per_resource`.

evalys-rs also has `series_from_raw(raw)` which adds two zero-padding points at each end so panning outside the data range shows zero instead of a gap.

### Gantt ↔ XY sync

When the user pans inside the XY plot, `XyPanelState::show()` returns an `Option<(i64, i64)>` (new visible range). `GanttChart::render()` applies it by updating `options.canvas_width_s` and `options.sideways_pan_in_points`.

The separator between the two areas is vertically draggable; `XyPanelState.panel_height` is updated by the drag delta.

---

## 11) Tests

Tests use Rust's built-in test system. Run all tests:

```bash
cargo test
```

### Where tests live

Unit tests are in the same file as the code they test, inside a `#[cfg(test)]` block. This block is excluded from release builds (no binary impact). Private functions are accessible directly.

Integration tests (if added) go in a `tests/` directory at the crate root.

### Existing tests

#### `goard_core`

**`src/models/data_structure/job_data.rs`** — 5 tests

| Test | What it checks |
|------|----------------|
| `rebuild_populates_cluster_resource_ids` | 2 resources in same cluster → both IDs present |
| `rebuild_populates_host_resource_ids` | 2 resources on same host → both under same host key |
| `rebuild_cluster_hosts_no_duplicates` | 2 resources on same host → host listed only once |
| `rebuild_multiple_clusters` | Resources from different clusters don't mix |
| `rebuild_clears_previous_state` | Two calls with different strata → old data cleared |

---

#### `evalys-rs`

**`src/energy_estimate.rs`** — 7 tests

| Test | What it checks |
|------|----------------|
| `estimate_no_jobs_gives_zeros` | No jobs → all points at 0 W |
| `estimate_single_job_correct_watts` | 2 resources × 300 W = 600 W at each sample |
| `estimate_job_outside_window_ignored` | Job outside time window → zero contribution |
| `estimate_partial_overlap` | Job starting mid-window: 0 W before, correct value after |
| `estimate_returns_empty_for_invalid_range` | `end < start` or `step = 0` → empty vec |
| `series_from_raw_empty` | Empty input → empty output |
| `series_from_raw_pads_zeros` | Raw series gets 2 zero points before and 2 after; data unchanged |

**`src/sim_state.rs`** — 3 tests

| Test | What it checks |
|------|----------------|
| `time_range_empty_jobs` | No jobs → sentinel values `(MAX, MIN)` |
| `time_range_skips_job_0` | Virtual "all_resources" row (id=0) excluded from range |
| `time_range_multiple_jobs` | Global min/max correct across multiple jobs |

---

#### `liveOAR`

**`src/energy_estimate.rs`** — 5 tests (same logic as evalys-rs, no `series_from_raw`)

| Test | What it checks |
|------|----------------|
| `estimate_no_jobs_gives_zeros` | No jobs → all points at 0 W |
| `estimate_single_job_correct_watts` | 2 resources × 300 W = 600 W at each sample |
| `estimate_job_outside_window_ignored` | Job outside time window → zero contribution |
| `estimate_partial_overlap` | Job starting mid-window: 0 W before, correct value after |
| `estimate_returns_empty_for_invalid_range` | `end < start` or `step = 0` → empty vec |

**`src/cluster_presets.rs`** — 4 tests

| Test | What it checks |
|------|----------------|
| `cluster_preset_serde_roundtrip` | JSON serialize/deserialize of a preset list |
| `cluster_preset_empty_clusters_allowed` | Preset with no clusters is valid and survives JSON |
| `load_presets_from_nonexistent_file_returns_empty` | Missing file → empty list, no panic |
| `save_and_load_roundtrip` | Write then read back from disk → identical data |
