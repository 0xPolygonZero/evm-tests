use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use common::types::VariantFilterType;

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

    /// The type of report to generate.
    #[arg(short='r', long, value_enum, default_value_t=ReportType::Test)]
    pub(crate) report_type: ReportType,

    /// Only run test variants that match this index (either a single value or a
    /// range).
    ///
    /// Eg: `0`, `0..=5`
    #[arg(short, long)]
    pub(crate) variant_filter: Option<VariantFilterType>,

    /// An optional filter to only run tests that are a subset of the given
    /// test path.
    #[arg(short = 'f', long)]
    pub(crate) test_filter: Option<String>,

    /// Do not run tests that have already passed in the past or that are
    /// ignored.
    #[arg(short = 'p', long)]
    pub(crate) skip_passed: bool,

    /// Mark a test as timed out if it takes longer than this amount of time.
    #[arg(short = 't', long)]
    pub(crate) test_timeout: Option<humantime::Duration>,

    /// Use a simple progress indicator that relies on `println!`s instead of an
    /// actual progress bar to display the current test status. In some
    /// situations, the more elegant progress bar may interfere with
    /// stdout/stderr.
    #[arg(short, long, default_value_t = false)]
    pub(crate) simple_progress_indicator: bool,

    /// Add/remove the persistent test pass state from the upstream parsed
    /// tests. If a new test exists upstream, we add an entry to the persistent
    /// state. If it's removed, we purge it from our persistent state.
    #[arg(short = 'u', long, default_value_t = false)]
    pub(crate) update_persistent_state_from_upstream: bool,
}
