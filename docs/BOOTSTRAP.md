# Bootstrap verification note

The initial native shell sources **passed Rust 1.95.0 checks in CI**, but have **not yet been tested in a real GUI**. The assistant runtime has neither rustc nor Cargo, so GitHub Actions provides compilation evidence. Initial Cargo.lock was generated and committed at `8900edaded76ccfb842044e9a4fe2c6f9730fa4d`; CI permissions have now been restored to read-only.

- Language: Rust 2024; intended toolchain Rust 1.95.0 (minimum subject to actual dependency verification).
- UI: eframe 0.37.0 (current upstream API uses App::ui), no Tauri/npm.
- User-facing shell: simple search text input, honest empty-source message, live System/Light/Dark dropdown.
- Preferences: eframe native persistence for theme, later consolidated into explicit XDG config.
- Not implemented: dictionary import, language selection, indexes, live Hyprland system appearance validation, custom typography, keyboard shortcuts and packaging.

See ROADMAP.md for acceptance, including full on-device testing.

## Verified evidence

- Source bootstrap commit: `caab9490799cef11224f12ed0b43b98a09a556d9`.
- Formatting correction: `0850416c45d7b8709a13e3f8b3882aca36ac885b`.
- Lockfile commit: `8900edaded76ccfb842044e9a4fe2c6f9730fa4d`.
- [GitHub Actions run 42743338343](https://github.com/sguzman/lexiva/actions/runs/42743338343): successful `cargo fmt`, `cargo check --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets`.
- Two preferences tests passed. No runtime Wayland/Hyprland GUI validation or dictionary importer yet.

## Foundation checkpoint

The initial shell baseline has been extended with `rusqlite` 0.41.1 (bundled SQLite), Serde and serde_json. [CI run 42746299152](https://github.com/sguzman/lexiva/actions/runs/42746299152) verified compilation, formatting, Clippy and **11 tests passing**. Dependency lockfile updated at `a4ea52d`; no native GUI runtime QA yet. The CI workflow was returned to read-only permissions after lockfile publication.
