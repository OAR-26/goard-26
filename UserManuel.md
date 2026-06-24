# User Manual - Goard

Goard is a Gantt-based job scheduler visualizer. It ships as two separate binaries for different use cases:

| Binary | Use case |
|--------|----------|
| **evalys-rs** | Import and visualize static OAR simulation files |
| **liveOAR** | Connect to a live OAR cluster and visualize jobs in real time |

Both share the same Gantt and Dashboard views. Sections marked **(evalys-rs only)** or **(liveOAR only)** apply to one binary only.

---

## 1) Startup

Both apps open directly on the **Gantt** view.

Authentication is **not required** to view data. Only admin operations need it:
- Create / edit / delete Gantt views
- Create / edit / delete cluster presets (liveOAR)

### Admin credentials
- **User:** `admin`
- **Password:** `admin`

> Note: current authentication is a proof of concept (hardcoded credentials).

---

## 2) Menu Bar (top)

### File
- **Log in** (if not connected) / **Log out** (if connected)
- Connected user displayed
- **Quit** — closes the application

**evalys-rs only:**
- **📁 Import File** — opens a file dialog to import a JSON file (see §10)

**liveOAR only:**
- **📡 Live Data** — toggles live polling mode (grayed out if already active)

### Options
Opens the options window:
- **Language:** English / Français
- **Font size:** 10 to 30
- **Save** — writes to `options.json`

### Help (`?`)
Shows context-sensitive help depending on the active view (Dashboard or Gantt).

---

## 3) Toolbar (below the menu)

Global controls:
- **Mode:** `📊 Dashboard` / `📅 Gantt` toggle
- **Filters:** `🔎 Filters` button
- **Light/dark theme:** `☀` / `🌙`

**liveOAR only:**
- **Auto-refresh interval:** `30 s`, `1 min`, `5 min`, `Never`
- **Instant refresh:** `⟳` button (disabled while a refresh is in progress)
- A `Refreshing data...` indicator + spinner appears at the bottom during refresh

---

## 4) Job Filters

The **Filters** window lets you filter displayed jobs by:
- **Owner**
- **Job state**
- **Cluster preset** — None or a named preset

Buttons:
- **Apply** — applies filters and updates the display
- **Reset** — clears all filters back to defaults

Filters affect:
- Dashboard (metrics + table)
- Gantt
- XY panel data

---

## 5) Gantt View

The Gantt shows jobs and resources on an interactive timeline.

### Data source tabs

A row of tabs at the top of the Gantt lets you switch between data sources:

**evalys-rs:**
- One tab per imported file or group
- `+` on a tab — group this file with another (see §11)
- `×` on a tab — close and remove this source

**liveOAR:**
- Single **Live Data** tab (always present when connected)
- `×` — stops live mode and clears data from memory

### Main interactions

| Input | Action |
|-------|--------|
| Left-click drag | Horizontal pan |
| `Ctrl/Cmd + scroll` | Horizontal zoom |
| Right-click drag (vertical) | Horizontal zoom |
| `Alt/Option + scroll` | Vertical zoom |
| Left double-click | Reset view |
| Left-click on a job | Zoom to job |
| Right-click on a job | Open job details |

### Gantt toolbar controls
- **View** — aggregation view selector (see §6)
- **🔧 Settings** — job color mode (random / by state)
- **Admin** — administration panel (grayed out if not authenticated)
- **Nav** — `◀ 1w`, `◀ 1d`, `1d ▶`, `1w ▶`
- **⌚ Center on now**

### Summary row
Shows: active view name, filtered job count, configured summary fields (e.g. clusters shown / total, hosts shown / total), data state (`refreshing`, `loading`, `ready`).

### Job details
Detail windows stay open individually and can be closed separately.

---

## 6) Aggregation Views

### Using views

The **View** dropdown in the Gantt toolbar selects the resource hierarchy.

Each view defines:
- **Hierarchy levels** — resources grouped left to right (e.g. site → cluster → host)
- **Leaf label** — derived from a configurable template (e.g. `{host|short}`)
- **Optional filter** — restricts resources shown (e.g. only `production = YES`)

Colored bands to the left of the timeline represent hierarchy levels, outermost on the left. Hovering a band shows a summary tooltip of the path (site, cluster, etc.).

---

### Managing views (Admin)

**Admin** authentication is required.

#### Create a view

From the **View** menu → click **+ Create view**.

Fields:
- **Name** — label shown in the View menu
- **Leaf info preset** — fields shown in the hover tooltip on a resource row
- **Hierarchy levels** — grouping levels, coarsest to finest. Click available fields to add, use ◀ / ▶ to reorder, 🗑 to remove.
- **Leaf label template** — template for leaf row labels. Examples: `{host|short}`, `{type}/{vlan}`. The `|short` modifier truncates before the first `.`.
- **Status bar fields** — fields shown in the summary row. Empty = last level.
- **Sort by label** — sort groups by computed label (useful when keys are opaque IDs).
- **Resource filter** — optional filter on a strata field. Choose field, value, and whether the rule is an allowlist (keep) or denylist (exclude).

Click **Save view**. View is immediately available in the menu.

#### Edit a view

In the **View** menu, hover over an existing view → click ✏ to the right of its name.

The **Edit view** panel opens with the same fields. Modify as needed then click **Apply**.

#### Delete a view

In the **View** menu, hover over a view → click 🗑. A confirmation window appears. Click **Delete** to confirm.

> Deletion is immediate and irreversible. Saved to `views.json`.

---

### Leaf info presets

A **leaf info preset** defines which fields appear in the tooltip when hovering a resource row.

#### Create a preset

In **Create view** or **Edit view**, click **+** next to the preset selector.

Fill in:
- **Preset name**
- **Fields** — check the fields to display (e.g. `cluster`, `network_address`, `cputype`, `memnode`)

A search field filters the available fields list. Click **Save preset**.

#### Edit a preset

In the preset dropdown of any view, click ✏ on the preset to modify. Change name and/or fields, then **Apply**.

#### Delete a preset

In the preset dropdown, click 🗑 → confirm in the dialog.

> Deleting a preset removes tooltip fields for all views that used it.

---

## 7) XY Panel (Energy)

Below the Gantt, a secondary plot shows power consumption over time.

**evalys-rs** — series shown depends on what files are loaded:

| Situation | Series displayed |
|-----------|-----------------|
| OAR file only | Estimated consumption from job allocations |
| Energy Series file only | Raw measured power curve |
| OAR + Energy Series group | Both curves overlaid |

**liveOAR** — always shows estimated consumption from live jobs.

### Controls
- **Cluster filter** — filter the series by cluster
- **Owner filter** — filter the series by job owner
- **Reset** — clear filters
- **Fit to figure** (checkbox) — auto-scale Y axis
- Hovering the plot shows timestamp + value
- Panning / zooming the plot syncs the Gantt's visible time window

The **divider** between the Gantt and the XY panel is vertically draggable to resize both areas.

---

## 8) Dashboard

The **Dashboard** view shows:
- Total filtered job count
- **Metrics** (colored boxes): total jobs, jobs by state, time range
- Or a **chart by job state** (toggle `Show charts` / `Show metrics`)
- **Job table** with column sort, pagination, and column visibility selection

Clicking a table row opens the job detail window.

---

## 9) Cluster Presets (liveOAR only)

The **Admin** button is only clickable for the `admin` user.

From the **Admin configuration** panel:
- **New Preset** — create a preset
- **Modify Preset** — edit an existing preset
- Checkbox list of clusters to include
- **Save** — saves / overwrites the preset
- **Delete** — removes the preset

Saved presets appear in the **Filters** window under **Cluster preset**.

---

## 10) File Import (evalys-rs only)

Via **File → 📁 Import File**, a dialog opens to select a JSON file.

After selection, an **Import File** window appears:
- **Auto Detect** — automatic format detection
- Or manual selection from the available types

### Supported file types

| Type | Content | Visualization |
|------|---------|---------------|
| **OAR Simulation** | Jobs + resources (Grid5000/OAR format) | Gantt + estimated energy |
| **Energy Series** | Timestamped power measurements | XY panel (measured curve) |
| **Event** | Point events on named resources | Gantt (circles) |

Click **Import ▶** to load. A tab appears at the top of the Gantt.

---

## 11) File Grouping (evalys-rs only)

You can **combine** an OAR file and an Energy Series file to overlay the measured curve on the estimate.

Steps:
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

## 12) Per-file Preferences (evalys-rs only)

evalys-rs automatically remembers Gantt preferences **per imported file**:

| Preference | What is saved |
|------------|--------------|
| Zoom | Visible timeline width |
| Position | Horizontal offset (pan) |
| Row height | Vertical height of resource rows |
| Active view | Selected aggregation view index |
| XY panel | Y bounds, "Fit to figure" state, panel height |

### When preferences are saved

- **Tab switch** — when leaving a tab for another
- **Tab close** (`×`)
- **App exit** (File → Quit or window close button)

> Forcing the process to quit (`kill`, power loss) does not guarantee the last state is saved.

### File identity

Preferences are linked to a file by its **absolute path** AND a **content fingerprint** (hash of the first 8 KB). This means:
- Renaming or moving the file → preferences are still found (via hash)
- Modifying file content → the path alone is enough to find the previous session

Stored in `evalys-rs/tab_states.json`. Local to your machine, not shared.

---

## 13) Feature Summary

### Common (both binaries)
- Gantt with interactive zoom, pan, job detail windows
- Dashboard (metrics + chart + sortable/paginated/column-selectable table)
- Aggregation views (configurable hierarchies, filters, label templates)
- Leaf info presets for resource tooltips
- Multi-criteria job filters
- XY / energy panel synchronized with the Gantt timeline
- Light/dark theme, language, font size

### evalys-rs
- JSON file import (OAR, Energy Series, Event)
- File grouping (OAR + energy overlaid)
- Per-file Gantt preferences (auto-saved and restored)

### liveOAR
- Live OAR cluster polling over SSH
- Auto-refresh with configurable interval
- Cluster filter presets (Admin)
