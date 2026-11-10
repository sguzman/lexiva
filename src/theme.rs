//! Application-controlled appearance; a real Linux system-mode QA pass is still required.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use eframe::egui;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub const fn storage_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    pub fn apply(self, ctx: &egui::Context) {
        let pref = match self {
            Self::System => egui::ThemePreference::System,
            Self::Light => egui::ThemePreference::Light,
            Self::Dark => egui::ThemePreference::Dark,
        };
        ctx.set_theme(pref);
    }
}

/// Restore a bounded reader scale; refuse invalid and non-finite values.
pub fn reader_scale_from_storage(value: Option<&str>) -> f32 {
    let Some(scale) = value.and_then(|raw| raw.parse::<f32>().ok()) else {
        return 1.0;
    };
    if scale.is_finite() && (0.8..=1.6).contains(&scale) {
        scale
    } else {
        1.0
    }
}

/// Use the locally selected Fontconfig sans-serif, retaining embedded egui
/// fonts for fallback glyphs. Lexiva never bundles third-party font files.
pub fn configure_system_font(ctx: &egui::Context) {
    let Ok(output) = Command::new("fc-match")
        .args(["-f", "%{file}\\n", "sans-serif"])
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let Some(filename) = std::str::from_utf8(&output.stdout)
        .ok()
        .and_then(|output| output.lines().next())
    else {
        return;
    };
    let path = Path::new(filename);
    if !path.is_absolute() {
        return;
    }
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
        return;
    }
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "lexiva_system".to_owned(),
        Arc::new(egui::FontData::from_owned(bytes)),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "lexiva_system".to_owned());
        ctx.set_fonts(fonts);
    }
}

/// Identity colors derived from the Lexiva name (OKLCH hue 16°).
/// Accessible contrast and external Hyprland theme changes require runtime QA.
pub fn configure_palette(ctx: &egui::Context) {
    let mut light = egui::Visuals::light();
    light.hyperlink_color = egui::Color32::from_rgb(156, 66, 75);
    light.selection.bg_fill = egui::Color32::from_rgb(156, 66, 75);
    ctx.set_visuals_of(egui::Theme::Light, light);

    let mut dark = egui::Visuals::dark();
    dark.hyperlink_color = egui::Color32::from_rgb(234, 151, 155);
    dark.selection.bg_fill = egui::Color32::from_rgb(156, 66, 75);
    ctx.set_visuals_of(egui::Theme::Dark, dark);
}

#[cfg(test)]
mod tests {
    use super::{ThemeChoice, reader_scale_from_storage};

    #[test]
    fn persistent_values_round_trip() {
        for mode in ThemeChoice::ALL {
            assert_eq!(
                ThemeChoice::from_storage_value(mode.storage_value()),
                Some(mode)
            );
        }
    }

    #[test]
    fn reader_scale_rejects_invalid_or_unbounded_preferences() {
        assert_eq!(reader_scale_from_storage(None), 1.0);
        assert_eq!(reader_scale_from_storage(Some("1.4")), 1.4);
        assert_eq!(reader_scale_from_storage(Some("0.8")), 0.8);
        for raw in ["", "nan", "inf", "-1", "0.2", "1.7", "broken"] {
            assert_eq!(reader_scale_from_storage(Some(raw)), 1.0);
        }
    }

    #[test]
    fn invalid_preferences_are_rejected() {
        for value in ["", "DARK", "unknown", "light ", "auto"] {
            assert_eq!(ThemeChoice::from_storage_value(value), None);
        }
        assert_eq!(ThemeChoice::default(), ThemeChoice::System);
    }
}
