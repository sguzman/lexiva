mod app;
mod theme;

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
