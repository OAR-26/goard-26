# evalys-rs — User Manual

Static file viewer for OAR simulation, energy series, and event JSON files.

---

## Startup

The app opens directly on the **Gantt** view (empty until a file is imported).

Authentication is not required to view data. Only admin operations need it:
- Create / edit / delete Gantt views

**Admin credentials:** `admin` / `admin` *(proof of concept — hardcoded)*

---

## Menu Bar

### File
- **Log in** / **Log out**
- **📁 Import File** — opens a file dialog to import a JSON file (see [File Import](#file-import))
- **Quit**

### Options
- **Language:** English / Français
- **Font size:** 10–30
- **Save** — writes to `options.json`

### Help (`?`)
Context-sensitive help for the active view.

---

## Toolbar

- **Mode:** `📊 Dashboard` / `📅 Gantt` toggle
- **Filters:** `🔎 Filters` button
- **Light/dark theme:** `☀` / `🌙`

---

## Job Filters

The **Filters** window filters displayed jobs by:
- **Owner**
- **Job state**

Buttons: **Apply** / **Reset**

Filters affect: Dashboard, Gantt, XY panel.

---

## Gantt View

Interactive timeline showing jobs and resources.

### Data source tabs

One tab per imported file or group.
- `+` on a tab — group this file with another (see [File Grouping](#file-grouping))
- `×` on a tab — close and remove this source

### Navigation

| Input | Action |
|-------|--------|
| Left-click drag | Horizontal pan |
| `Ctrl/Cmd + scroll` | Horizontal zoom |
| Right-click drag (vertical) | Horizontal zoom |
| `Alt/Option + scroll` | Vertical zoom |
| Left double-click | Reset view |
| Left-click on a job | Zoom to job |
| Right-click on a job | Open job details |

### Gantt toolbar
- **View** — aggregation view selector
- **🔧 Settings** — job color mode (random / by state)
- **Admin** — administration panel (requires auth)
- **Nav** — `◀ 1w`, `◀ 1d`, `1d ▶`, `1w ▶`
- **⌚ Center on now**

### Summary row
Shows: active view name, filtered job count, summary fields, data state.

---

## Aggregation Views

The **View** dropdown selects the resource hierarchy. Each view defines hierarchy levels, a leaf label template, and an optional filter.

Colored bands to the left of the timeline represent hierarchy levels.

### Managing views (Admin)

Requires login as `admin`.

#### Create
**View** menu → **+ Create view**. Fill in name, levels, leaf label template, summary fields, optional filter. Click **Save view**.

#### Edit
**View** menu → hover a view → click ✏.

#### Delete
**View** menu → hover a view → click 🗑 → confirm.

### Leaf info presets

Define which fields appear when hovering a resource row. Managed from the Create/Edit view panel.

---

## XY Panel (Energy)

Secondary plot below the Gantt showing power consumption over time.

| Files loaded | Series shown |
|-------------|-------------|
| OAR only | Estimated consumption from job allocations |
| Energy Series only | Raw measured power curve |
| OAR + Energy Series group | Both curves overlaid |

**Controls:**
- **Cluster filter** / **Owner filter** — filter the series
- **Reset** — clear filters
- **Fit to figure** — auto-scale Y axis
- Panning/zooming the XY plot syncs the Gantt window
- Draggable divider between Gantt and XY panel

---

## Dashboard

- Total filtered job count
- **Metrics** (colored boxes): total jobs, jobs by state, time range
- Toggle: `Show charts` / `Show metrics`
- **Job table**: column sort, pagination, column visibility
- Click a row → job detail window

---

## File Import

**File → 📁 Import File** opens a file dialog.

After selection, the **Import File** window appears:
- **Auto Detect** — automatic format detection
- Or manual format selection

### Supported formats

| Format | Content | Visualization |
|--------|---------|---------------|
| **OAR Simulation** | Jobs + resources (Grid5000/OAR) | Gantt + estimated energy |
| **Energy Series** | Timestamped power measurements | XY panel (measured curve) |
| **Event** | Point events on named resources | Gantt (circles) |

Click **Import ▶** to load. A tab appears at the top of the Gantt.

---

## File Grouping

Combine an OAR file and an Energy Series file to overlay the measured curve on the estimate.

1. Import the first file (e.g. OAR)
2. On its tab, click **`+`**
3. Select the second file (e.g. Energy Series)
4. Both files form a **group** (tab labeled `groupN`)

From the group tab:
- `v` — list group members
- `+` — add a file to the group
- 🗑 on a member — remove that file from the group
- 🗑 on the group — delete the group and all its files

---

## Per-file Preferences

evalys-rs automatically saves and restores Gantt preferences per imported file.

| Preference | What is saved |
|------------|--------------|
| Zoom | Visible timeline width |
| Position | Horizontal offset (pan) |
| Row height | Vertical height of resource rows |
| Active view | Selected aggregation view index |
| XY panel | Y bounds, "Fit to figure" state, panel height |

**Saved on:** tab switch, tab close (`×`), app exit.

**File identity:** tracked by absolute path AND a content fingerprint (hash of the first 8 KB). Renaming or moving the file → preferences are still found via hash.

Stored in `evalys-rs/tab_states.json` (local to your machine, gitignored).

---

## Feature Summary

- JSON file import (OAR, Energy Series, Event)
- File grouping (OAR + energy overlaid)
- Per-file Gantt preferences (auto-saved and restored)
- Interactive Gantt: zoom, pan, job detail windows
- Dashboard: metrics + chart + sortable/paginated/column-selectable table
- Aggregation views (configurable hierarchies, filters, label templates)
- XY / energy panel synchronized with Gantt
- Multi-criteria job filters
- Light/dark theme, language, font size
