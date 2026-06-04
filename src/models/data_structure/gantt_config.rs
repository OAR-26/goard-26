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
            absent:    RgbColor(30,  100, 220),
            suspected: RgbColor(220, 30,  30),
            dead:      RgbColor(120, 120, 120),
            standby:   RgbColor(0x88, 0xff, 0xff),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GanttConfig {
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
    pub hatch_spacing: f32,

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
            hatch_spacing:              f64("hatch_spacing").map(|v| v as f32).unwrap_or(def.hatch_spacing),

            state_colors,
            state_colors_light,
        }
    }
}
