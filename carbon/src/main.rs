mod runtime;

#[macro_use]
extern crate log;
extern crate carbon_core;

use carbon_core::cyml_import::import_file;
use carbon_core::log::setup;

fn main() {
    setup();
    info!("Start Carbon.");
    let conf = import_file("carbon/example.yml").unwrap();
    runtime::runtime(conf);
}
