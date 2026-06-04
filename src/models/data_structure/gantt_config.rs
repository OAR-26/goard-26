use serde_json::Value;

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

#[derive(Debug, Clone)]
pub struct StateColors {
    pub absent: RgbColor,
    pub suspected: RgbColor,
    pub dead: RgbColor,
    pub standby: RgbColor,
}

impl Default for StateColors {
    fn default() -> Self {
        Self {
            absent:   RgbColor(30,  100, 220),
            suspected: RgbColor(220, 30,  30),
            dead:     RgbColor(120, 120, 120),
            standby:  RgbColor(0x88, 0xff, 0xff),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GanttConfig {
    /// Truncate open-ended Absent intervals to now when Standby applies.
    pub standby_truncate_to_now: bool,
    /// Hide the future portion of besteffort jobs.
    pub besteffort_truncate_to_now: bool,
    /// Minimum interval duration in seconds before it is rendered (0 = always render).
    pub min_state_duration_s: i64,
    /// Default gantt width in seconds on first load.
    pub default_timespan_s: i64,
    /// Hatch colors for dark mode.
    pub state_colors: StateColors,
    /// Hatch colors for light mode (darker so they're visible).
    pub state_colors_light: StateColors,
    /// Minimum RGB component value for random job colors (0–255). Higher = lighter colors.
    pub job_color_min: u8,

    // ── Gantt row sizing ──────────────────────────────────────────────────────
    pub gantt_row_height: f32,
    pub gantt_row_height_min: f32,
    pub gantt_row_height_max: f32,

    // ── Energy panel ─────────────────────────────────────────────────────────
    pub energy_panel_height: f32,
    pub energy_watts_per_resource: f64,
    pub energy_series_colors: Vec<RgbColor>,
    pub now_line_color: RgbColor,

    // ── Zoom limits ───────────────────────────────────────────────────────────
    pub zoom_max_seconds: f32,
    pub zoom_min_seconds: f32,

    // ── Interaction sensitivity ───────────────────────────────────────────────
    pub scroll_zoom_sensitivity: f32,
    pub drag_zoom_sensitivity: f32,
    pub zoom_animation_duration: f64,

    // ── Layout ────────────────────────────────────────────────────────────────
    pub gutter_max_width: f32,
    pub job_label_min_width: f32,
    pub hatch_spacing: f32,

    // ── Live data time window ─────────────────────────────────────────────────
    pub live_window_hours_before: i64,
    pub live_window_hours_after: i64,
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
            hatch_spacing: 10.0,

            live_window_hours_before: 1,
            live_window_hours_after: 1,
        }
    }
}

impl GanttConfig {
    pub fn load() -> Self {
        #[cfg(target_arch = "wasm32")]
        let content = include_str!("../../../config.json").to_string();
        #[cfg(not(target_arch = "wasm32"))]
        let content = match std::fs::read_to_string("config.json") {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        let val: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let def = Self::default();

        let parse_colors = |key: &str, fallback: &StateColors| -> StateColors {
            let colors = val.get(key);
            StateColors {
                absent:    colors.and_then(|c| c.get("Absent")).and_then(|v| v.as_str()).and_then(RgbColor::from_hex).unwrap_or(fallback.absent),
                suspected: colors.and_then(|c| c.get("Suspected")).and_then(|v| v.as_str()).and_then(RgbColor::from_hex).unwrap_or(fallback.suspected),
                dead:      colors.and_then(|c| c.get("Dead")).and_then(|v| v.as_str()).and_then(RgbColor::from_hex).unwrap_or(fallback.dead),
                standby:   colors.and_then(|c| c.get("Standby")).and_then(|v| v.as_str()).and_then(RgbColor::from_hex).unwrap_or(fallback.standby),
            }
        };

        let state_colors       = parse_colors("state_colors",       &def.state_colors);
        let state_colors_light = parse_colors("state_colors_light",  &def.state_colors_light);

        Self {
            standby_truncate_to_now:    val.get("standby_truncate_state_to_now").and_then(|v| v.as_bool()).unwrap_or(def.standby_truncate_to_now),
            besteffort_truncate_to_now: val.get("besteffort_truncate_job_to_now").and_then(|v| v.as_bool()).unwrap_or(def.besteffort_truncate_to_now),
            min_state_duration_s:       val.get("min_state_duration").and_then(|v| v.as_i64()).unwrap_or(def.min_state_duration_s),
            default_timespan_s:         val.get("default_timespan").and_then(|v| v.as_i64()).unwrap_or(def.default_timespan_s),
            job_color_min:              val.get("job_color_min").and_then(|v| v.as_u64()).map(|v| v.min(255) as u8).unwrap_or(def.job_color_min),

            gantt_row_height:           val.get("gantt_row_height").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.gantt_row_height),
            gantt_row_height_min:       val.get("gantt_row_height_min").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.gantt_row_height_min),
            gantt_row_height_max:       val.get("gantt_row_height_max").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.gantt_row_height_max),

            energy_panel_height:        val.get("energy_panel_height").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.energy_panel_height),
            energy_watts_per_resource:  val.get("energy_watts_per_resource").and_then(|v| v.as_f64()).unwrap_or(def.energy_watts_per_resource),
            energy_series_colors: {
                val.get("energy_series_colors")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|c| c.as_str().and_then(RgbColor::from_hex)).collect::<Vec<_>>())
                    .filter(|v| !v.is_empty())
                    .unwrap_or(def.energy_series_colors)
            },
            now_line_color:             val.get("now_line_color").and_then(|v| v.as_str()).and_then(RgbColor::from_hex).unwrap_or(def.now_line_color),

            zoom_max_seconds:           val.get("zoom_max_seconds").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.zoom_max_seconds),
            zoom_min_seconds:           val.get("zoom_min_seconds").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.zoom_min_seconds),

            scroll_zoom_sensitivity:    val.get("scroll_zoom_sensitivity").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.scroll_zoom_sensitivity),
            drag_zoom_sensitivity:      val.get("drag_zoom_sensitivity").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.drag_zoom_sensitivity),
            zoom_animation_duration:    val.get("zoom_animation_duration").and_then(|v| v.as_f64()).unwrap_or(def.zoom_animation_duration),

            gutter_max_width:           val.get("gutter_max_width").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.gutter_max_width),
            job_label_min_width:        val.get("job_label_min_width").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.job_label_min_width),
            hatch_spacing:              val.get("hatch_spacing").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def.hatch_spacing),

            live_window_hours_before:   val.get("live_window_hours_before").and_then(|v| v.as_i64()).unwrap_or(def.live_window_hours_before),
            live_window_hours_after:    val.get("live_window_hours_after").and_then(|v| v.as_i64()).unwrap_or(def.live_window_hours_after),

            state_colors,
            state_colors_light,
        }
    }
}
