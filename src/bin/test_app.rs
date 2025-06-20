//! This is a test app for testing things for the library before lifting them into the library.
//!
//! To run this test app, do `cargo run --bin test_app`.
use std::cell::RefCell;

use apricot::app::{self, run, Scene};
use karta::KartaContext;

fn main() -> Result<(), String> {
    run(
        nalgebra_glm::I32Vec2::new(640, 360),
        "Apricot Test Application",
        &|app| RefCell::new(Box::new(TestApp::new(app))),
    )
}

struct TestApp {}

impl Scene for TestApp {
    fn update(&mut self, _app: &app::App) {}

    fn render(&mut self, _app: &app::App) {}
}

impl TestApp {
    fn new(_app: &app::App) -> Self {
        let mut retval = Self {};
        let _ = retval.open_gui();
        retval
    }

    fn open_gui(&mut self) -> Result<(), String> {
        let mut kctx = KartaContext::new().unwrap();
        kctx.import_file("core", "gui/test.k")?;
        Ok(())
    }
}
