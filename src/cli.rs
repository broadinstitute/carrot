//! Module for configuring CARROT's command line behavior

use clap::{App, Arg};

/// Configures a clap app for handling command line arguments
pub fn configure() -> App<'static, 'static> {
    App::new("CARROT")
        // Metadata
        .version(env!("CARGO_PKG_VERSION"))
        .about("The Cromwell Automated Runner for Regression and Optimization Testing")
        // Argument for specifying a config file path that is not the default
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .help(
                    "A path to a yaml file containing configuration information for running CARROT",
                )
                .takes_value(true)
                .value_name("FILE")
                .default_value("carrot.yml"),
        )
}
