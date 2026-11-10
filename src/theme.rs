//! Application-controlled appearance; a real Linux system-mode QA pass is still required.

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
    use super::ThemeChoice;

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
    fn invalid_preferences_are_rejected() {
        for value in ["", "DARK", "unknown", "light ", "auto"] {
            assert_eq!(ThemeChoice::from_storage_value(value), None);
        }
        assert_eq!(ThemeChoice::default(), ThemeChoice::System);
    }
}
