# Lexiva — Inception contract

**Purpose:** A beautiful and fast offline-first native Rust/egui dictionary reader that controls existing sources rather than owning or inventing lexical data.

**Intended user and platform:** Desktop user of EndeavourOS (Arch) on Hyprland/Wayland; no claimed Windows, macOS, Android or web platform.

**Essential workflow:** Launch; detect or add local dictionaries; choose headword and definition languages separately (Spanish→English, Spanish→Spanish, English→English); select individual sources; search with Unicode and prefix matching; view senses, examples, IPA and grammatical information whenever present; manage sources and persist settings.

**First runnable:** One Cargo binary, real eframe window, honest empty dictionary state, System/Light/Dark control, reproducible CI. This is not an MVP without an actual importer.

**MVP gate:** Working DICT and Wiktextract import/search; source and language filtering; accessible keyboard-first reader, settings and safe import recovery; real target-machine runtime QA and owner's separate acceptance.

**Full release:** Adds StarDict and FreeDict TEI, large dataset behavior, source updates, performance/security hardening, verified installation/packaging and explicit release authorization.

**Non-goals:** No server, online account, cloud sync, telemetry, embedded web frontend, npm, Tauri, Electron, automatic OS package installations, homegrown definition database, flashcards/Anki, unsupported operating-system guarantees or universal proprietary format support.

**Tooling:** Rust edition 2024; Cargo; user's mbx build-coordinator rules on their workstation; install through cargo install --path .; no manual copying to ~/.local/bin, no reboot without authorization.

**Project color:** FNV-1a 32-bit of UTF-8 'lexiva' = 3196054816; hue modulo 360 = 16 degrees. Proposed OKLCH light accent (0.50, 0.12, 16) ≈ #9C424B; dark accent (0.76, 0.10, 16) ≈ #EA979B. Color labels always remain readable without distinguishing only by hue. Contrast to be runtime QA'd.

**Typography:** Proposed Lexend for definitions, Luciole for headings, Atkinson Hyperlegible Next for controls and Monaspace Krypton for technical labels, with graceful fallbacks and no redistribution of third-party fonts without an audit.

**License:** Original Lexiva code remains MIT as already declared; each imported dictionary retains its own data and distribution terms; do not bundle unverified third-party data or fonts.

**Ownership:** Owner has final product/release authority; engineer can develop and commit verified in-scope changes to main, reporting tests, evidence and blockers. 'Continue' means only unfinished scoped work, not optional feature invention.
