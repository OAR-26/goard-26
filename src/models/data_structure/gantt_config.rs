/// RGB color parsed from a hex string like "#88ffff".
#[derive(Debug, Clone, Copy)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl RgbColor {
    fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim_start_matches('#');
        if s.len() == 6 {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(Self(r, g, b))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StateColors {
    pub absent: RgbColor,
    pub suspected: RgbColor,
    pub dead: RgbColor,
    pub standby: RgbColor,
}

impl Default for StateColors {
    fn default() -> Self {
        Self {
            absent:    RgbColor(30,  100, 220),
            suspected: RgbColor(220, 30,  30),
            dead:      RgbColor(120, 120, 120),
            standby:   RgbColor(0x88, 0xff, 0xff),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GanttConfig  {
    pub standby_truncate_to_now: bool,
    pub besteffort_truncate_to_now: bool,
    pub min_state_duration_s: i64,
    pub default_timespan_s: i64,
    pub state_colors: StateColors,
    pub state_colors_light: StateColors,
    pub job_color_min: u8,

    pub gantt_row_height: f32,
    pub gantt_row_height_min: f32,
    pub gantt_row_height_max: f32,

    pub energy_panel_height: f32,
    pub energy_watts_per_resource: f64,
    pub energy_series_colors: Vec<RgbColor>,
    pub now_line_color: RgbColor,

    pub zoom_max_seconds: f32,
    pub zoom_min_seconds: f32,

    pub scroll_zoom_sensitivity: f32,
    pub drag_zoom_sensitivity: f32,
    pub zoom_animation_duration: f64,

    pub gutter_max_width: f32,
    pub job_label_min_width: f32,
    /// Job field displayed inside bar labels. Default: "id".
    /// Valid values: id, owner, name, command, queue, project, state, walltime, job_type.
    pub job_label_field: String,
    /// Job field used to assign a deterministic color when color mode is ByField.
    pub job_color_field: String,
    pub hatch_spacing: f32,
    pub ssh_host: String,
    /// Navigation step buttons: (n, unit) pairs, smallest to largest.
    /// Each entry produces one ◀/▶ button pair.
    pub nav_steps: Vec<(i64, String)>,
    /// Per-field value→color mappings. field name → (value string → fill color).
    /// When ByField coloring is active, looked up by job_color_field then the field's value.
    pub field_colors: std::collections::HashMap<String, std::collections::HashMap<String, RgbColor>>,
}

pub fn unit_seconds(unit: &str) -> i64 {
    match unit { "minute" => 60, "hour" => 3_600, "week" => 7 * 86_400, _ => 86_400 }
}

impl GanttConfig {
    /// Compute step durations in seconds, smallest to largest.
    pub fn nav_steps_s(&self) -> Vec<i64> {
        self.nav_steps.iter().map(|(n, u)| n * unit_seconds(u)).collect()
    }
}

impl Default for GanttConfig {
    fn default() -> Self {
        Self {
            standby_truncate_to_now: true,
            besteffort_truncate_to_now: true,
            min_state_duration_s: 2,
            default_timespan_s: 6 * 3600,
            state_colors: StateColors::default(),
            state_colors_light: StateColors {
                absent:    RgbColor(0x10, 0x40, 0xa0),
                suspected: RgbColor(0xa0, 0x10, 0x10),
                dead:      RgbColor(0x40, 0x40, 0x40),
                standby:   RgbColor(0x00, 0x88, 0x88),
            },
            job_color_min: 140,

            gantt_row_height: 20.0,
            gantt_row_height_min: 8.0,
            gantt_row_height_max: 80.0,

            energy_panel_height: 270.0,
            energy_watts_per_resource: 300.0,
            energy_series_colors: vec![
                RgbColor(31,  119, 180),
                RgbColor(255, 127,  14),
                RgbColor(44,  160,  44),
                RgbColor(214,  39,  40),
                RgbColor(148, 103, 189),
                RgbColor(140,  86,  75),
                RgbColor(227, 119, 194),
                RgbColor(188, 189,  34),
            ],
            now_line_color: RgbColor(220, 0, 0),

            zoom_max_seconds: (2 * 24 * 60 * 60) as f32,
            zoom_min_seconds: 5.0,

            scroll_zoom_sensitivity: 0.0025,
            drag_zoom_sensitivity: 0.01,
            zoom_animation_duration: 0.75,

            gutter_max_width: 650.0,
            job_label_min_width: 30.0,
            job_label_field: "id".to_string(),
            job_color_field: "state".to_string(),
            hatch_spacing: 10.0,
            ssh_host: "grenoble.g5k".to_string(),
            nav_steps: vec![(1, "day".to_string()), (1, "week".to_string())],
            field_colors: std::collections::HashMap::new(),
        }
    }
}

impl GanttConfig {
    pub fn load() -> Self {
        #[cfg(target_arch = "wasm32")]
        let content = include_str!("../../../config.toml").to_string();
        #[cfg(not(target_arch = "wasm32"))]
        let content = match std::fs::read_to_string("config.toml") {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        let val: toml::Value = match toml::from_str(&content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let def = Self::default();

        let f64 = |key: &str| -> Option<f64> {
            val.get(key).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        };
        let i64 = |key: &str| -> Option<i64> {
            val.get(key).and_then(|v| v.as_integer())
        };
        let bool = |key: &str| -> Option<bool> {
            val.get(key).and_then(|v| v.as_bool())
        };
        let str_ = |key: &str| -> Option<&str> {
            val.get(key).and_then(|v| v.as_str())
        };

        let parse_colors = |key: &str, fallback: &StateColors| -> StateColors {
            let tbl = val.get(key).and_then(|v| v.as_table());
            let get = |k: &str| tbl.and_then(|t| t.get(k)).and_then(|v| v.as_str()).and_then(RgbColor::from_hex);
            StateColors {
                absent:    get("Absent").unwrap_or(fallback.absent),
                suspected: get("Suspected").unwrap_or(fallback.suspected),
                dead:      get("Dead").unwrap_or(fallback.dead),
                standby:   get("Standby").unwrap_or(fallback.standby),
            }
        };

        let state_colors       = parse_colors("state_colors",       &def.state_colors);
        let state_colors_light = parse_colors("state_colors_light",  &def.state_colors_light);

        let field_colors: std::collections::HashMap<String, std::collections::HashMap<String, RgbColor>> = val
            .get("field_colors")
            .and_then(|v| v.as_table())
            .map(|outer| outer.iter().filter_map(|(field, values)| {
                let colors = values.as_table()?.iter()
                    .filter_map(|(k, v)| Some((k.clone(), v.as_str().and_then(RgbColor::from_hex)?)))
                    .collect();
                Some((field.clone(), colors))
            }).collect())
            .unwrap_or_default();

        Self {
            standby_truncate_to_now:    bool("standby_truncate_state_to_now").unwrap_or(def.standby_truncate_to_now),
            besteffort_truncate_to_now: bool("besteffort_truncate_job_to_now").unwrap_or(def.besteffort_truncate_to_now),
            min_state_duration_s:       i64("min_state_duration").unwrap_or(def.min_state_duration_s),
            default_timespan_s:         i64("default_timespan").unwrap_or(def.default_timespan_s),
            job_color_min:              i64("job_color_min").map(|v| v.clamp(0, 255) as u8).unwrap_or(def.job_color_min),

            gantt_row_height:           f64("gantt_row_height").map(|v| v as f32).unwrap_or(def.gantt_row_height),
            gantt_row_height_min:       f64("gantt_row_height_min").map(|v| v as f32).unwrap_or(def.gantt_row_height_min),
            gantt_row_height_max:       f64("gantt_row_height_max").map(|v| v as f32).unwrap_or(def.gantt_row_height_max),

            energy_panel_height:        f64("energy_panel_height").map(|v| v as f32).unwrap_or(def.energy_panel_height),
            energy_watts_per_resource:  f64("energy_watts_per_resource").unwrap_or(def.energy_watts_per_resource),
            energy_series_colors: {
                val.get("energy_series_colors")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|c| c.as_str().and_then(RgbColor::from_hex)).collect::<Vec<_>>())
                    .filter(|v| !v.is_empty())
                    .unwrap_or(def.energy_series_colors)
            },
            now_line_color:             str_("now_line_color").and_then(RgbColor::from_hex).unwrap_or(def.now_line_color),

            zoom_max_seconds:           f64("zoom_max_seconds").map(|v| v as f32).unwrap_or(def.zoom_max_seconds),
            zoom_min_seconds:           f64("zoom_min_seconds").map(|v| v as f32).unwrap_or(def.zoom_min_seconds),

            scroll_zoom_sensitivity:    f64("scroll_zoom_sensitivity").map(|v| v as f32).unwrap_or(def.scroll_zoom_sensitivity),
            drag_zoom_sensitivity:      f64("drag_zoom_sensitivity").map(|v| v as f32).unwrap_or(def.drag_zoom_sensitivity),
            zoom_animation_duration:    f64("zoom_animation_duration").unwrap_or(def.zoom_animation_duration),

            gutter_max_width:           f64("gutter_max_width").map(|v| v as f32).unwrap_or(def.gutter_max_width),
            job_label_min_width:        f64("job_label_min_width").map(|v| v as f32).unwrap_or(def.job_label_min_width),
            job_label_field:            str_("job_label_field").map(|s| s.to_string()).unwrap_or(def.job_label_field),
            job_color_field:            str_("job_color_field").map(|s| s.to_string()).unwrap_or(def.job_color_field),
            hatch_spacing:              f64("hatch_spacing").map(|v| v as f32).unwrap_or(def.hatch_spacing),
            ssh_host:                   str_("ssh_host").map(|s| s.to_string()).unwrap_or(def.ssh_host),
            nav_steps: {
                val.get("nav_steps")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|entry| {
                        let n = entry.get("n").and_then(|v| v.as_integer())?;
                        let unit = entry.get("unit").and_then(|v| v.as_str())?.to_string();
                        Some((n, unit))
                    }).collect::<Vec<_>>())
                    .filter(|v| !v.is_empty())
                    .unwrap_or(def.nav_steps)
            },

            state_colors,
            state_colors_light,
            field_colors,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self) {
        fn hex(c: RgbColor) -> String { format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2) }
        let series: String = self.energy_series_colors.iter()
            .map(|&c| format!("    \"{}\",\n", hex(c)))
            .collect();
        let nav_steps_toml: String = self.nav_steps.iter()
            .map(|(n, u)| format!("\n[[nav_steps]]\nn    = {}\nunit = \"{}\"\n", n, u))
            .collect();
        let field_colors_toml: String = {
            let mut fields: Vec<_> = self.field_colors.iter().collect();
            fields.sort_by_key(|(k, _)| k.as_str());
            fields.iter().map(|(field, values)| {
                let mut vals: Vec<_> = values.iter().collect();
                vals.sort_by_key(|(k, _)| k.as_str());
                let mut s = format!("\n[field_colors.{}]\n", field);
                for (val, &c) in &vals { s.push_str(&format!("{:<20}= \"{}\"\n", val, hex(c))); }
                s
            }).collect()
        };
        let content = format!(
"# ── SSH (live data only) ──────────────────────────────────────────────────────

# SSH host used to fetch live OAR data (oarstat command is run on this host)
ssh_host = \"{ssh_host}\"

# ── General behaviour ─────────────────────────────────────────────────────────

# Truncate open-ended Absent intervals to 'now' when a Standby period applies
standby_truncate_state_to_now = {standby}

# Hide the future portion of besteffort jobs so they don't extend past now
besteffort_truncate_job_to_now = {besteffort}

# Minimum interval duration in seconds before it is drawn (0 = always draw)
min_state_duration = {min_state}

# Initial Gantt horizontal window width in seconds (21600 = 6 h)
default_timespan = {default_timespan}

# ── Job colors ────────────────────────────────────────────────────────────────

# Minimum RGB component for random job colors (0–255).
# 0 = full range, 255 = white. 70–140 recommended for dark/light balance.
job_color_min = {job_color_min}

# ── Gantt row sizing ──────────────────────────────────────────────────────────

# Default row height in pixels on first load
gantt_row_height = {gantt_row_height}

# Minimum row height reachable by vertical zoom (Alt+scroll)
gantt_row_height_min = {gantt_row_height_min}

# Maximum row height reachable by vertical zoom (Alt+scroll)
gantt_row_height_max = {gantt_row_height_max}

# ── Energy panel ──────────────────────────────────────────────────────────────

# Default height of the energy diagram panel in pixels
energy_panel_height = {energy_panel_height}

# Estimated watts per assigned resource when no measured energy file is loaded
energy_watts_per_resource = {energy_watts}

# Color palette (hex) for multi-series energy lines. Cycles if more series than entries.
energy_series_colors = [
{series_colors}]

# Color of the vertical 'now' marker line on the energy and Gantt plots
now_line_color = \"{now_line_color}\"

# ── Zoom limits ───────────────────────────────────────────────────────────────

# Maximum number of seconds visible in the Gantt window (zoom-out limit). 172800 = 2 days.
zoom_max_seconds = {zoom_max}

# Minimum number of seconds visible in the Gantt window (zoom-in limit)
zoom_min_seconds = {zoom_min}

# ── Interaction sensitivity ───────────────────────────────────────────────────

# Horizontal zoom speed for Ctrl+scroll. Higher = faster zoom.
scroll_zoom_sensitivity = {scroll_zoom}

# Horizontal zoom speed for right-click drag. Higher = faster zoom.
drag_zoom_sensitivity = {drag_zoom}

# Duration in seconds of the 'Center on now' zoom animation
zoom_animation_duration = {zoom_anim}

# ── Layout ────────────────────────────────────────────────────────────────────

# Maximum width in pixels of the resource-label column on the left
gutter_max_width = {gutter_max}

# Minimum job bar width in pixels before the job label is hidden
job_label_min_width = {job_label_min}

# Field displayed inside job bars. Options: id, owner, name, command, queue, project, state, walltime, job_type
job_label_field = \"{job_label_field}\"

# Field used to assign a deterministic color when color mode is 'By field'
job_color_field = \"{job_color_field}\"

# Spacing in pixels between diagonal lines in dead/absent interval overlays
hatch_spacing = {hatch_spacing}

# ── Navigation ────────────────────────────────────────────────────────────────
# Each [[nav_steps]] entry adds one ◀/▶ button pair.
# Buttons render: ◀ stepN … ◀ step1 | step1 ▶ … stepN ▶
# unit: minute | hour | day | week
{nav_steps_toml}

# ── Resource state colors (dark mode) ────────────────────────────────────────

[state_colors]
Absent    = \"{absent_dark}\"
Suspected = \"{suspected_dark}\"
Dead      = \"{dead_dark}\"
Standby   = \"{standby_dark}\"

# ── Resource state colors (light mode) ───────────────────────────────────────

[state_colors_light]
Absent    = \"{absent_light}\"
Suspected = \"{suspected_light}\"
Dead      = \"{dead_light}\"
Standby   = \"{standby_light}\"

# ── Field value colors (used when color mode is 'By field') ──────────────────
# Add [field_colors.<fieldname>] sections to map field values to colors.
# Values not listed fall back to hash-based random color.
{field_colors_toml}",
            ssh_host             = self.ssh_host,
            standby              = self.standby_truncate_to_now,
            besteffort           = self.besteffort_truncate_to_now,
            min_state            = self.min_state_duration_s,
            default_timespan     = self.default_timespan_s,
            job_color_min        = self.job_color_min,
            gantt_row_height     = self.gantt_row_height,
            gantt_row_height_min = self.gantt_row_height_min,
            gantt_row_height_max = self.gantt_row_height_max,
            energy_panel_height  = self.energy_panel_height,
            energy_watts         = self.energy_watts_per_resource,
            series_colors        = series,
            now_line_color       = hex(self.now_line_color),
            zoom_max             = self.zoom_max_seconds,
            zoom_min             = self.zoom_min_seconds,
            scroll_zoom          = self.scroll_zoom_sensitivity,
            drag_zoom            = self.drag_zoom_sensitivity,
            zoom_anim            = self.zoom_animation_duration,
            gutter_max           = self.gutter_max_width,
            job_label_min        = self.job_label_min_width,
            job_label_field      = self.job_label_field,
            job_color_field      = self.job_color_field,
            hatch_spacing        = self.hatch_spacing,
            nav_steps_toml       = nav_steps_toml,
            field_colors_toml    = field_colors_toml,
            absent_dark          = hex(self.state_colors.absent),
            suspected_dark       = hex(self.state_colors.suspected),
            dead_dark            = hex(self.state_colors.dead),
            standby_dark         = hex(self.state_colors.standby),
            absent_light         = hex(self.state_colors_light.absent),
            suspected_light      = hex(self.state_colors_light.suspected),
            dead_light           = hex(self.state_colors_light.dead),
            standby_light        = hex(self.state_colors_light.standby),
        );
        let _ = std::fs::write("config.toml", content);
    }
}
