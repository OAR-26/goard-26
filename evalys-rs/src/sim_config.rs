fn hex_to_rgb(s: &str) -> Option<[u8; 3]> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 { return None; }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some([r, g, b])
}

fn rgb_to_hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn default_colors() -> Vec<[u8; 3]> {
    vec![
        [0x1f, 0x77, 0xb4],
        [0xff, 0x7f, 0x0e],
        [0x2c, 0xa0, 0x2c],
        [0xd6, 0x27, 0x28],
        [0x94, 0x67, 0xbd],
        [0x8c, 0x56, 0x4b],
        [0xe3, 0x77, 0xc2],
        [0xbc, 0xbd, 0x22],
    ]
}

/// Color palette for multi-series energy lines — `goard_core` has no notion
/// of this; it falls back to a deterministic per-index color when none is
/// set. This setting lives entirely in evalys-rs's own config file.
pub fn load_energy_series_colors() -> Vec<[u8; 3]> {
    match std::fs::read_to_string("evalys-rs/sim_config.toml") {
        Ok(content) => toml::from_str::<toml::Value>(&content)
            .ok()
            .and_then(|v| {
                v.get("energy_series_colors")
                    .and_then(|a| a.as_array())
                    .map(|arr| arr.iter().filter_map(|c| c.as_str().and_then(hex_to_rgb)).collect::<Vec<_>>())
            })
            .filter(|v: &Vec<[u8; 3]>| !v.is_empty())
            .unwrap_or_else(default_colors),
        Err(_) => default_colors(),
    }
}

pub fn save_energy_series_colors(colors: &[[u8; 3]]) {
    let series: String = colors.iter().map(|&c| format!("    \"{}\",\n", rgb_to_hex(c))).collect();
    let content = format!(
        "# evalys-rs-only settings. goard_core's config.toml has no notion of\n\
         # this — it falls back to a deterministic per-index color when none is set.\n\n\
         # Color palette (hex) for multi-series energy lines. Cycles if more series than entries.\n\
         energy_series_colors = [\n{}]\n",
        series
    );
    let _ = std::fs::write("evalys-rs/sim_config.toml", content);
}
