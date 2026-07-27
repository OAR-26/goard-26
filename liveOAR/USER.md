# liveOAR — User Manual

Live OAR cluster viewer. Connects to an HPC cluster over SSH and monitors jobs in real time.

---

## Startup

The app opens on the **Gantt** view.

Authentication is not required to view data. Only admin operations need it:
- Create / edit / delete Gantt views
- Create / edit / delete cluster presets

**Admin credentials:** `admin` / `admin` *(proof of concept — hardcoded)*

---

## Running

### Native (desktop)

```bash
GOARD_SSH_HOST=grenoble.g5k cargo run -p liveOAR --release
```

### Web (WASM) — live data

**Terminal 1 - backend:**
```bash
GOARD_SSH_HOST=grenoble.g5k cargo run -p liveOAR --release -- --serve
```

**Terminal 2 - frontend:**
```bash
cd liveOAR && trunk serve
```

Access at `http://localhost:8080`.

The frontend fetches data from the backend every 30 seconds, passing its current view window. The ⟳ button triggers an immediate fetch. If the backend is unreachable, the app falls back to mock data.

---

## Menu Bar

### File
- **Log in** / **Log out**
- **Quit**

### Options
- **Language:** English / Français
- **Font size:** 10–30
- **Save** — persists settings (`options.json` on native, browser localStorage on web)

### Help (`?`)
Context-sensitive help for the active view.

---

## Toolbar

- **Mode:** `📊 Dashboard` / `📅 Gantt` toggle
- **Filters:** `🔎 Filters` button
- **Light/dark theme:** `☀` / `🌙`
- **Auto-refresh interval:** `30 s`, `1 min`, `5 min`, `Never`
- **Instant refresh:** `⟳` button (disabled while a refresh is in progress)

A `Refreshing data...` indicator + spinner appears at the bottom during a refresh.

---

## Job Filters

The **Filters** window filters displayed jobs by:
- **Owner**
- **Job state**
- **Cluster preset** — None or a named preset

Buttons: **Apply** / **Reset**

Filters affect: Dashboard, Gantt, XY panel.

---

## Gantt View

Interactive timeline showing live jobs and resources.

### Live Data tab

Single **Live Data** tab (always present when connected).
- `×` — stops live mode and clears data from memory

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
Shows: active view name, filtered job count, summary fields, data state (`refreshing`, `loading`, `ready`).

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

Secondary plot below the Gantt showing estimated power consumption from live jobs.

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

## Cluster Presets

Cluster presets let you filter the Gantt and Dashboard to show only specific clusters.

Requires **Admin** login to manage.

From the **Admin configuration** panel:
- **New Preset** — create a preset (name + checkbox list of clusters)
- **Modify Preset** — edit an existing preset
- **Save** — saves / overwrites the preset
- **Delete** — removes the preset

Active presets appear in **Filters → Cluster preset**.

---

## Feature Summary

- Real-time OAR cluster monitoring over SSH
- Auto-refresh with configurable interval (30 s / 1 min / 5 min / Never)
- Instant refresh ⟳ button
- Web (WASM) build with HTTP backend for browser access
- Cluster filter presets (Admin)
- Interactive Gantt: zoom, pan, job detail windows
- Dashboard: metrics + chart + sortable/paginated/column-selectable table
- Aggregation views (configurable hierarchies, filters, label templates)
- XY / energy panel synchronized with Gantt
- Multi-criteria job filters
- Light/dark theme, language, font size
