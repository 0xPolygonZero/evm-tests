use std::{
    ops::RangeInclusive,
    path::PathBuf,
    str::{FromStr, Split},
};

use anyhow::{anyhow, Context};
use clap::{Parser, ValueEnum};

#[derive(Clone, Debug)]
pub(crate) enum VariantFilterType {
    Single(usize),
    Range(RangeInclusive<usize>),
}

impl FromStr for VariantFilterType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_intern(s)
            .with_context(|| {
                format!(
                    "Expected a single value or a range, but instead got \"{}\".",
                    s
                )
            })
            .map_err(|e| format!("{e:#}"))
    }
}

impl VariantFilterType {
    fn from_str_intern(s: &str) -> anyhow::Result<Self> {
        // Did we get passed a single value?
        if let Ok(v) = s.parse::<usize>() {
            return Ok(Self::Single(v));
        }

        // Check if it's a range.
        let mut range_vals = s.split("..=");

        let start = Self::next_and_try_parse(&mut range_vals)?;
        let end = Self::next_and_try_parse(&mut range_vals)?;

        if range_vals.count() > 0 {
            return Err(anyhow!(
                "Parsed a range but there were unexpected characters afterwards!"
            ));
        }

        Ok(Self::Range(start..=end))
    }

    fn next_and_try_parse(range_vals: &mut Split<&str>) -> anyhow::Result<usize> {
        let unparsed_val = range_vals
            .next()
            .with_context(|| "Parsing a value as a `RangeInclusive`")?;
        let res = unparsed_val
            .parse()
            .with_context(|| format!("Parsing the range val \"{unparsed_val}\" into a usize"))?;

        Ok(res)
    }
}

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

    #[arg(short, long)]
    /// Only run test variants that match this index (either a single value or a
    /// range).
    pub(crate) variant_filter: Option<VariantFilterType>,

    #[arg(short = 'f', long)]
    /// An optional filter to only run tests that are a subset of the given
    /// test path.
    pub(crate) test_filter: Option<String>,
}
