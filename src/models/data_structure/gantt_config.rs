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
    /// Hatch colors for each resource state.
    pub state_colors: StateColors,
}

impl Default for GanttConfig {
    fn default() -> Self {
        Self {
            standby_truncate_to_now: true,
            besteffort_truncate_to_now: true,
            min_state_duration_s: 2,
            default_timespan_s: 6 * 3600,
            state_colors: StateColors::default(),
        }
    }
}

impl GanttConfig {
    pub fn load() -> Self {
        #[cfg(target_arch = "wasm32")]
        let content = include_str!("../../../../config.json").to_string();
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

        let sc = &def.state_colors;
        let colors = val.get("state_colors");
        let state_colors = StateColors {
            absent:   colors.and_then(|c| c.get("Absent")).and_then(|v| v.as_str()).and_then(RgbColor::from_hex).unwrap_or(sc.absent),
            suspected: colors.and_then(|c| c.get("Suspected")).and_then(|v| v.as_str()).and_then(RgbColor::from_hex).unwrap_or(sc.suspected),
            dead:     colors.and_then(|c| c.get("Dead")).and_then(|v| v.as_str()).and_then(RgbColor::from_hex).unwrap_or(sc.dead),
            standby:  colors.and_then(|c| c.get("Standby")).and_then(|v| v.as_str()).and_then(RgbColor::from_hex).unwrap_or(sc.standby),
        };

        Self {
            standby_truncate_to_now:    val.get("standby_truncate_state_to_now").and_then(|v| v.as_bool()).unwrap_or(def.standby_truncate_to_now),
            besteffort_truncate_to_now: val.get("besteffort_truncate_job_to_now").and_then(|v| v.as_bool()).unwrap_or(def.besteffort_truncate_to_now),
            min_state_duration_s:       val.get("min_state_duration").and_then(|v| v.as_i64()).unwrap_or(def.min_state_duration_s),
            default_timespan_s:         val.get("default_timespan").and_then(|v| v.as_i64()).unwrap_or(def.default_timespan_s),
            state_colors,
        }
    }
}
