//! This is a test app for testing things for the library before lifting them into the library.
//!
//! To run this test app, do `cargo run --bin test_app`.
use std::cell::RefCell;

use apricot::app::{self, run, Scene};
use utils::blob::KartaFile;

mod utils;

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
        let k_file = KartaFile::from_file("src/bin/gui/test.k").unwrap();

        let hmm_query = k_file.query().get_atom(".hmm");

        let res: i64 = hmm_query.as_int().unwrap();
        println!("{:?}", res);
        println!("truthy?: {}", hmm_query.truthy());

        let nested_query = k_file.query().get_atom(".nested").get_atom(".maps");
        let res: f64 = nested_query.as_float().unwrap();
        println!("{:?}", res);
        println!("falsey?: {}", nested_query.falsey());

        let string_query = k_file.query().get_atom(".some-string");
        let res = string_query.as_string().unwrap();
        println!("{:?}", res);

        Self {}
    }
}
