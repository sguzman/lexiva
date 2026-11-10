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
    use super::{
        ThemeChoice, configure_motion, interface_scale_from_storage, reader_scale_from_storage,
        reduced_motion_from_storage,
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
    fn invalid_preferences_are_rejected() {
        for value in ["", "DARK", "unknown", "light ", "auto"] {
            assert_eq!(ThemeChoice::from_storage_value(value), None);
        }
        assert_eq!(ThemeChoice::default(), ThemeChoice::System);
    }
}
