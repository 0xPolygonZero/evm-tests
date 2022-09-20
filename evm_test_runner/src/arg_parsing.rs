use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub(crate) struct ProgArgs {
    /// Write the output from running the tests to a markdown file.
    #[clap(short = 'm', action)]
    pub(crate) output_result_markdown: bool,

    /// The path to the parsed tests directory.
    pub(crate) parsed_tests_path: PathBuf,
}
