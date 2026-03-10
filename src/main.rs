mod app;
mod history;
mod io;
mod model;
mod tools;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 980.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title("LDraw - Infinite Whiteboard"),
        ..Default::default()
    };

    eframe::run_native(
        "LDraw",
        options,
        Box::new(|cc| Box::new(app::LdrawApp::new(cc))),
    )
}
