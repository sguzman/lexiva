//! Application-controlled appearance; a real Linux system-mode QA pass is still required.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

#[cfg(target_os = "linux")]
use std::io::{BufRead, BufReader, Write};
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixStream;

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

    fn from_darkman_message(value: &str) -> Option<Self> {
        match value.trim() {
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

/// A Linux Darkman subscription: one socket, initial state and event-driven
/// changes, without a timer, DBus dependency or repeated process launches.
/// Missing Darkman falls through to eframe's regular System preference.
pub struct DarkmanWatcher {
    updates: Receiver<ThemeChoice>,
}

impl DarkmanWatcher {
    pub fn spawn(ctx: &egui::Context) -> Option<Self> {
        #[cfg(target_os = "linux")]
        {
            let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
            let path = std::path::PathBuf::from(runtime).join("darkman/control.sock");
            let stream = UnixStream::connect(path).ok()?;
            Self::from_stream(stream, ctx)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = ctx;
            None
        }
    }

    #[cfg(target_os = "linux")]
    fn from_stream(mut stream: UnixStream, ctx: &egui::Context) -> Option<Self> {
        stream.write_all(b"watch\n").ok()?;
        let (send, updates) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("lexiva-darkman".into())
            .spawn(move || {
                for line in BufReader::new(stream).lines() {
                    let Ok(line) = line else { break };
                    let Some(mode) = ThemeChoice::from_darkman_message(&line) else {
                        continue;
                    };
                    if send.send(mode).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .ok()?;
        Some(Self { updates })
    }

    /// Coalesce quick manual mode toggles; only the newest one is visible.
    pub fn latest(&self) -> Option<ThemeChoice> {
        let mut latest = None;
        while let Ok(mode) = self.updates.try_recv() {
            latest = Some(mode);
        }
        latest
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

/// Restore only bounded interface scales from native app storage.
pub fn interface_scale_from_storage(value: Option<&str>) -> f32 {
    let Some(scale) = value.and_then(|raw| raw.parse::<f32>().ok()) else {
        return 1.0;
    };
    if scale.is_finite() && (0.75..=2.5).contains(&scale) {
        scale
    } else {
        1.0
    }
}

pub fn reduced_motion_from_storage(value: Option<&str>) -> bool {
    value == Some("true")
}

pub fn high_contrast_from_storage(value: Option<&str>) -> bool {
    value == Some("true")
}

/// Applies to light and dark styles, including theme transitions.
/// No animation timer is started while reduction is enabled.
pub fn configure_motion(ctx: &egui::Context, reduced_motion: bool) {
    ctx.all_styles_mut(|style| {
        style.animation_time = if reduced_motion { 0.0 } else { 0.2 };
        style.scroll_animation.duration = if reduced_motion {
            egui::Rangef::new(0.0, 0.0)
        } else {
            egui::Rangef::new(0.1, 0.3)
        };
    });
}

/// Use the locally selected Fontconfig sans-serif, retaining embedded egui
/// fonts for fallback glyphs. Lexiva never bundles third-party font files.
pub fn configure_system_font(ctx: &egui::Context) {
    let Ok(output) = Command::new("fc-match")
        .args(["-f", "%{file}\n", "sans-serif"])
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

/// Consistent light/dark surfaces, so the root background and controls never
/// use different appearance modes. All colors are semantic, not widget-local.
#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: egui::Color32,
    pub surface: egui::Color32,
    pub border: egui::Color32,
    pub primary: egui::Color32,
    pub secondary: egui::Color32,
    pub accent: egui::Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            canvas: egui::Color32::from_rgb(18, 23, 32),
            surface: egui::Color32::from_rgb(29, 35, 45),
            border: egui::Color32::from_rgb(61, 70, 82),
            primary: egui::Color32::from_rgb(239, 242, 246),
            secondary: egui::Color32::from_rgb(175, 187, 202),
            accent: egui::Color32::from_rgb(234, 151, 155),
        }
    } else {
        Palette {
            canvas: egui::Color32::from_rgb(245, 246, 249),
            surface: egui::Color32::WHITE,
            border: egui::Color32::from_rgb(218, 223, 231),
            primary: egui::Color32::from_rgb(34, 41, 53),
            secondary: egui::Color32::from_rgb(94, 103, 118),
            accent: egui::Color32::from_rgb(156, 66, 75),
        }
    }
}

/// Identity colors derived from the Lexiva name (OKLCH hue 16°).
/// Root panels must be painted with the matching `panel_fill`.
pub fn configure_palette(ctx: &egui::Context, high_contrast: bool) {
    let light_palette = palette(false);
    let mut light = egui::Visuals::light();
    light.panel_fill = light_palette.canvas;
    light.window_fill = light_palette.surface;
    light.extreme_bg_color = egui::Color32::from_rgb(237, 239, 244);
    light.weak_text_color = Some(light_palette.secondary);
    light.widgets.noninteractive.fg_stroke.color = light_palette.primary;
    light.widgets.inactive.fg_stroke.color = light_palette.primary;
    light.widgets.inactive.bg_stroke.color = light_palette.border;
    light.widgets.noninteractive.bg_stroke.color = light_palette.border;
    light.hyperlink_color = light_palette.accent;
    light.selection.bg_fill = egui::Color32::from_rgb(156, 66, 75);
    light.selection.stroke.color = egui::Color32::WHITE;
    if high_contrast {
        light.weak_text_color = Some(egui::Color32::from_rgb(55, 55, 55));
        light.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(22, 22, 22);
        light.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(22, 22, 22);
        light.selection.stroke.color = egui::Color32::WHITE;
    }
    ctx.set_visuals_of(egui::Theme::Light, light);

    let dark_palette = palette(true);
    let mut dark = egui::Visuals::dark();
    dark.panel_fill = dark_palette.canvas;
    dark.window_fill = dark_palette.surface;
    dark.extreme_bg_color = egui::Color32::from_rgb(37, 44, 56);
    dark.weak_text_color = Some(dark_palette.secondary);
    dark.widgets.noninteractive.fg_stroke.color = dark_palette.primary;
    dark.widgets.inactive.fg_stroke.color = dark_palette.primary;
    dark.widgets.inactive.bg_stroke.color = dark_palette.border;
    dark.widgets.noninteractive.bg_stroke.color = dark_palette.border;
    dark.hyperlink_color = dark_palette.accent;
    dark.selection.bg_fill = egui::Color32::from_rgb(156, 66, 75);
    dark.selection.stroke.color = egui::Color32::WHITE;
    if high_contrast {
        dark.weak_text_color = Some(egui::Color32::from_rgb(222, 222, 222));
        dark.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(245, 245, 245);
        dark.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(245, 245, 245);
        dark.selection.stroke.color = egui::Color32::WHITE;
    }
    ctx.set_visuals_of(egui::Theme::Dark, dark);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(10.0, 7.0);
        style.spacing.button_padding = egui::vec2(11.0, 6.0);
    });
}

#[cfg(test)]
mod tests {
    use super::{
        ThemeChoice, configure_motion, configure_palette, high_contrast_from_storage, palette,
        interface_scale_from_storage, reader_scale_from_storage, reduced_motion_from_storage,
    };

    #[test]
    fn darkman_notifications_accept_only_explicit_modes() {
        assert_eq!(
            ThemeChoice::from_darkman_message("light\n"),
            Some(ThemeChoice::Light)
        );
        assert_eq!(
            ThemeChoice::from_darkman_message("dark"),
            Some(ThemeChoice::Dark)
        );
        for input in ["", "toggle", "system", "red", "unknown"] {
            assert_eq!(ThemeChoice::from_darkman_message(input), None);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn darkman_socket_watch_delivers_initial_and_changed_modes() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;
        use std::time::{Duration, Instant};

        use eframe::egui;

        use super::DarkmanWatcher;

        let (client, mut server) = UnixStream::pair().unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let watcher = DarkmanWatcher::from_stream(client, &egui::Context::default()).unwrap();
        let mut command = String::new();
        BufReader::new(server.try_clone().unwrap())
            .read_line(&mut command)
            .unwrap();
        assert_eq!(command, "watch\n");

        for (message, expected) in [
            (b"dark\n".as_slice(), ThemeChoice::Dark),
            (b"light\n".as_slice(), ThemeChoice::Light),
        ] {
            server.write_all(message).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if watcher.latest() == Some(expected) {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "Darkman notification not received"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

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
    fn interface_zoom_and_motion_preferences_fail_safely() {
        assert_eq!(interface_scale_from_storage(None), 1.0);
        assert_eq!(interface_scale_from_storage(Some("1.75")), 1.75);
        for invalid in ["", "NaN", "-1", "inf", "0.1", "6.0"] {
            assert_eq!(interface_scale_from_storage(Some(invalid)), 1.0);
        }
        assert!(reduced_motion_from_storage(Some("true")));
        for invalid in [None, Some("false"), Some("yes"), Some("")] {
            assert!(!reduced_motion_from_storage(invalid));
        }
    }

    #[test]
    fn reduced_motion_updates_both_theme_styles() {
        let ctx = eframe::egui::Context::default();
        configure_motion(&ctx, true);
        for mode in [eframe::egui::Theme::Light, eframe::egui::Theme::Dark] {
            let style = ctx.style_of(mode);
            assert_eq!(style.animation_time, 0.0);
            assert_eq!(
                style.scroll_animation.duration,
                eframe::egui::Rangef::new(0.0, 0.0)
            );
        }
        configure_motion(&ctx, false);
        for mode in [eframe::egui::Theme::Light, eframe::egui::Theme::Dark] {
            let style = ctx.style_of(mode);
            assert_eq!(style.animation_time, 0.2);
            assert_eq!(
                style.scroll_animation.duration,
                eframe::egui::Rangef::new(0.1, 0.3)
            );
        }
    }

    #[test]
    fn high_contrast_preserves_both_identity_accents_and_readable_secondary_text() {
        let ctx = eframe::egui::Context::default();
        configure_palette(&ctx, true);
        for mode in [eframe::egui::Theme::Light, eframe::egui::Theme::Dark] {
            let style = ctx.style_of(mode);
            let visuals = &style.visuals;
            assert!(contrast(visuals.weak_text_color(), visuals.panel_fill) >= 4.5);
            assert!(contrast(visuals.selection.stroke.color, visuals.selection.bg_fill) >= 4.5);
        }
        configure_palette(&ctx, false);
        assert_eq!(
            ctx.style_of(eframe::egui::Theme::Light)
                .visuals
                .weak_text_color,
            Some(palette(false).secondary)
        );
        assert!(high_contrast_from_storage(Some("true")));
        assert!(!high_contrast_from_storage(Some("false")));
        assert!(!high_contrast_from_storage(Some("garbled")));
    }

    fn contrast(a: eframe::egui::Color32, b: eframe::egui::Color32) -> f32 {
        fn luminance(c: eframe::egui::Color32) -> f32 {
            fn channel(value: u8) -> f32 {
                let c = f32::from(value) / 255.0;
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            }
            0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
        }
        let l1 = luminance(a);
        let l2 = luminance(b);
        (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
    }

    #[test]
    fn light_and_dark_surfaces_have_legible_text_and_distinct_canvas() {
        let ctx = eframe::egui::Context::default();
        configure_palette(&ctx, false);
        for mode in [eframe::egui::Theme::Light, eframe::egui::Theme::Dark] {
            let dark = mode == eframe::egui::Theme::Dark;
            let expected = palette(dark);
            let visuals = &ctx.style_of(mode).visuals;
            assert_eq!(visuals.panel_fill, expected.canvas);
            assert_eq!(visuals.window_fill, expected.surface);
            assert!(contrast(expected.primary, expected.canvas) >= 7.0);
            assert!(contrast(expected.secondary, expected.canvas) >= 4.5);
            assert!(contrast(visuals.selection.stroke.color, visuals.selection.bg_fill) >= 4.5);
            assert_ne!(expected.canvas, expected.surface);
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
