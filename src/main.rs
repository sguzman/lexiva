mod app;
mod reader;
mod search_worker;
mod source_manager;
// Early backend adapter, not yet linked to the GUI.
#[allow(dead_code)]
mod dict;
// Backend integration into the GUI is tracked separately in M05/M06.
#[allow(dead_code)]
mod model;
#[allow(dead_code)]
mod storage;
mod theme;
// Streaming Wiktextract adapter; GUI integration belongs to M05/M06.
#[allow(dead_code)]
mod wiktextract;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 720.0])
            .with_min_inner_size([760.0, 500.0])
            .with_app_id("lexiva"),
        ..Default::default()
    };

    eframe::run_native(
        "Lexiva",
        options,
        Box::new(|cc| Ok(Box::new(app::LexivaApp::new(cc)))),
    )
}
