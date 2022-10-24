use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum ReportType {
    /// Run tests (flatten, no groups) and render markdown to stdout. Displays
    /// detailed information for each individual test.
    Test,

    /// Run all tests and write a high-level markdown report summary to disk.
    /// The summary does not contain information on individual tests and instead
    /// aggregates all of the tests in a sub-group into row entries.
    Summary,
}

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub(crate) struct ProgArgs {
    /// The path to the parsed tests directory.
    pub(crate) parsed_tests_path: Option<PathBuf>,

    #[arg(short='r', long, value_enum, default_value_t=ReportType::Test)]
    /// The type of report to generate.
    pub(crate) report_type: ReportType,

    #[arg(short = 'f', long)]
    /// An optional filter to only run tests that are a subset of the given
    /// test path.
    pub(crate) test_filter: Option<String>,
}
