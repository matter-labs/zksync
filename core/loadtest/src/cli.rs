//! Command-line interface for the loadtest.

// Built-in imports
use std::path::PathBuf;
// External uses
use structopt::StructOpt;
// Local uses
use crate::scenarios::ScenarioType;

/// Loadtest parameters.
#[derive(StructOpt, Debug)]
#[structopt(name = "loadtest")]
pub struct CliOptions {
    /// Loadtest scenario to run.
    #[structopt(short, long = "scenario", default_value = "ScenarioType::OutgoingTps")]
    pub scenario_type: ScenarioType,

    /// Path to the test spec JSON file.
    #[structopt(name = "FILE", parse(from_os_str))]
    pub test_spec_path: PathBuf,
}
