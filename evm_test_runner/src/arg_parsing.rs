use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Subcommand)]
pub(crate) enum RunCommand {
    /// Run tests (with an optional filter) and only print to stdout.
    Test(TestArgs),

    /// Run all tests and generate a report summary markdown file.
    Report,
}

#[derive(Args, Debug)]
pub(crate) struct TestArgs {
    /// An optional filter to only run tests that are a subset of the given test
    /// path.
    pub(crate) test_filter: Option<String>,
}

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub(crate) struct ProgArgs {
    /// The path to the parsed tests directory.
    pub(crate) parsed_tests_path: PathBuf,

    /// The command to run.
    #[command(subcommand)]
    pub(crate) cmd: RunCommand,
}
