use egui::{Color32, Rgba};

pub(super) struct ThemeColors {
    pub(super) text: Color32,
    pub(super) text_dim: Color32,
    pub(super) line: Color32,
    pub(super) aggregated_line_level_1: Rgba,
    pub(super) aggregated_line_level_2: Rgba,
    pub(super) background: Color32,
    pub(super) background_timeline: Color32,
    pub(super) hatch: Color32,
}

pub(super) fn get_theme_colors(style: &egui::Style) -> ThemeColors {
    if style.visuals.dark_mode {
        ThemeColors {
            text: Color32::WHITE,
            text_dim: Color32::from_white_alpha(229),
            line: Color32::WHITE,
            aggregated_line_level_1: Rgba::from_white_alpha(0.4),
            aggregated_line_level_2: Rgba::from_white_alpha(0.5),
            background: Color32::from_black_alpha(100),
            background_timeline: Color32::from_black_alpha(150),
            hatch: Color32::from_rgba_premultiplied(0, 150, 150, 150),
        }
    } else {
        ThemeColors {
            text: Color32::BLACK,
            text_dim: Color32::from_black_alpha(229),
            line: Color32::BLACK,
            aggregated_line_level_1: Rgba::from_black_alpha(0.5),
            aggregated_line_level_2: Rgba::from_black_alpha(0.7),
            background: Color32::from_black_alpha(50),
            background_timeline: Color32::from_black_alpha(20),
            hatch: Color32::from_rgba_premultiplied(0, 0, 139, 150),
        }
    }
}
