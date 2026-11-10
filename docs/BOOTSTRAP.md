# Bootstrap verification note

The initial shell sources are written, but **not yet verified by a compiler or GUI**. The assistant runtime has neither rustc nor Cargo, so GitHub Actions is the first available compilation gate. GitHub Actions generates the application's initial Cargo.lock for inspection and commit after successful resolution; the first source commit intentionally does not assert lockfile completeness.

- Language: Rust 2024; intended toolchain Rust 1.95.0 (minimum subject to actual dependency verification).
- UI: eframe 0.37.0 (current upstream API uses App::ui), no Tauri/npm.
- User-facing shell: simple search text input, honest empty-source message, live System/Light/Dark dropdown.
- Preferences: eframe native persistence for theme, later consolidated into explicit XDG config.
- Not implemented: dictionary import, language selection, indexes, live Hyprland system appearance validation, custom typography, keyboard shortcuts and packaging.

See ROADMAP.md for acceptance, including full on-device testing.
