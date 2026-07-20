# goard_core — Developer Reference

Shared rendering library used by both `evalys-rs` and `liveOAR`. Pure UI and data model — no SSH, no file I/O, no energy estimation.

---

## Tech Stack

| Component | Library |
|-----------|---------|
| UI framework | egui 0.30 + eframe 0.30 |
| Plots | egui_plot 0.30 |
| Serialization | serde 1.0 + serde_json 1.0 |
| Dates | chrono 0.4 |
| i18n | rust-i18n 3 |

---

## Workspace Layout

```
Cargo.toml            — workspace root
├── goard_core/       — shared rendering library (this crate)
├── evalys-rs/        — static file viewer binary
└── liveOAR/          — live OAR cluster viewer binary
```

---

## Module Structure

```
goard_core/
├── config.toml                         — Gantt config (colors, timespan)
├── views.json                          — saved Gantt views + leaf info presets
└── src/
    ├── lib.rs
    ├── models/
    │   ├── data_structure/
    │   │   ├── application_context.rs  — central app state
    │   │   ├── job_data.rs             — jobs, clusters, strata, plot_series
    │   │   ├── gantt_config.rs         — config.toml loader
    │   │   ├── application_options.rs  — zoom, pan, row height
    │   │   ├── ui_preferences.rs       — font, theme, language
    │   │   ├── filters.rs              — active job filters
    │   │   ├── job.rs / resource.rs / strata.rs / marker.rs
    │   │   └── mod.rs
    │   └── utils/
    │       ├── date_converter.rs
    │       ├── utils.rs                — cluster/host/resource helpers
    │       ├── secret.rs               — hardcoded auth credentials
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
            └── mod.rs
```

---

## State Architecture (`ApplicationContext`)

`ApplicationContext` is the central container owned by each binary and passed into every `goard_core` render call. It is split into sub-structs:

| Field | Type | Content |
|-------|------|---------|
| `data` | `JobData` | jobs, clusters, strata, markers, `plot_series` |
| `prefs` | `UiPreferences` | font, theme, language, Gantt view state |
| `filters` | `Filters` | active job filter state |
| `options` | `ApplicationOptions` | zoom, pan, row height |

Session flags (`view_type`, `user_connected`, `show_xy_panel`, `show_gantt_panel`) sit flat on `ApplicationContext`.

### `plot_series`

`JobData.plot_series: Vec<(String, Vec<(i64, f64)>)>` is the generic XY data fed to the XY panel. `goard_core` renders whatever the binary puts there:

| Binary | What it puts in `plot_series` |
|--------|-------------------------------|
| evalys-rs | Estimated series from jobs and/or raw measured series |
| liveOAR | Estimated series from live jobs |

---

## Gantt Views (`views.json`)

`views.json` is loaded at startup and rewritten on every Admin UI change. It can also be edited manually while the app is closed.

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
| `leaf_label_template` | string \| null | no | Label template. `{field}` or `{field\|short}` (truncates before first `.`) |
| `sort_by_label` | bool | no | Sort groups by computed label instead of raw key |
| `summary_fields` | string[] | no | Fields in the summary row. Empty = last level |
| `leaf_infos` | string \| null | no | `id` of a `leaf_info_presets` entry |
| `filter` | object \| null | no | Filter on a strata field |

### Filter fields

```json
{ "field": "production", "value": "YES", "exclude": false }
```

- `exclude: false` → keep only `field == value`
- `exclude: true` → exclude when `field == value`

### Available strata fields

`site`, `cluster`, `host`, `type`, `vlan`, `disk`, `disk_id`, `nodeset`, `subnet_address`, `subnet_prefix`, `slash_16`…`slash_22`, `network_address`, `ip`, `comment`, `nodemodel`, `cputype`, `cpufreq`, `core_count`, `thread_count`, `memnode`, `gpu_model`, `chassis`, `resource_id`, `production`, `state`, `besteffort`, `deploy`, `drain`

`site` is derived automatically from the host FQDN.

---

## Configuration (`config.toml`)

Loaded by `GanttConfig::load()` at startup.

```toml
[gantt]
standby_truncate_state_to_now = true
besteffort_truncate_job_to_now = true
min_state_duration = 2
default_timespan = 21600
xy_panel_height = 270.0

[colors]
Absent = "#1e64dc"
Suspected = "#dc1e1e"
Dead = "#787878"
Standby = "#88ffff"

[colors_light]
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
| `default_timespan` | Initial Gantt width in seconds (21600 = 6 h) |
| `xy_panel_height` | Initial XY panel height in pixels |
| `job_color_min` | Minimum RGB component for random job colors (0–255, default 140) |

---

## XY Panel

The XY panel is a generic secondary plot below the Gantt. `goard_core` renders whatever is in `app.data.plot_series` — it has no concept of "energy" or "estimation." The binary is responsible for populating the series.

### Gantt ↔ XY sync

When the user pans inside the XY plot, `XyPanelState::show()` returns `Option<(i64, i64)>` (new visible range). `GanttChart::render()` applies it to `options.canvas_width_s` and `options.sideways_pan_in_points`.

The separator between the Gantt and XY panel is vertically draggable; `XyPanelState.panel_height` updates on drag.

---

## Tests

Run with `cargo test -p goard_core`.

**`src/models/data_structure/job_data.rs`** — 5 tests

| Test | What it checks |
|------|----------------|
| `rebuild_populates_cluster_resource_ids` | 2 resources in same cluster → both IDs present |
| `rebuild_populates_host_resource_ids` | 2 resources on same host → both under same host key |
| `rebuild_cluster_hosts_no_duplicates` | 2 resources on same host → host listed only once |
| `rebuild_multiple_clusters` | Resources from different clusters don't mix |
| `rebuild_clears_previous_state` | Two calls with different strata → old data cleared |
