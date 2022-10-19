//! Performs two types of report generation:
//! - Generates a summary markdown report which contains an entry for each
//!   `sub-group` in each `group` showing the number of tests passed/failed (no
//!   specific info per individual test).
//! - Generates markdown for all tests that match a string filter output to
//!   `stdout`. Tests are not displayed in groups and instead are shown in a
//!   single table with information of failures if any.

use std::{fs, path::Path};

use anyhow::Context;
use askama::Template;

use crate::plonky2_runner::{
    TestGroupRunResults, TestRunResult, TestStatus, TestSubGroupRunResults,
};

const REPORT_OUTPUT: &str = "reports";

/// Template for writing a summary markdown report to file.
#[derive(Debug, Template)]
#[template(path = "filtered_test_results.md")]
struct FilteredTestResultsTemplate {
    filter_str_template: String,
    passed_info: PassedInfo,
    tests: Vec<TestRunResult>,
}

impl TestGroupRunResults {
    /// Flattens all test groups/subgroups into individual tests using their
    /// full paths as the test name.
    fn flatten_tests(&self) -> impl Iterator<Item = TestRunResult> + '_ {
        self.sub_group_res.iter().flat_map(move |sub_g| {
            sub_g.test_res.iter().map(move |test| {
                let full_path = Path::new(&self.name).join(&sub_g.name).join(&test.name);

                TestRunResult {
                    name: full_path.to_str().unwrap().to_string(),
                    status: test.status.clone(),
                }
            })
        })
    }
}

impl FilteredTestResultsTemplate {
    // Note: Tests are already filtered from a previous step.
    fn new(res: &[TestGroupRunResults], filter_str_template: &Option<String>) -> Self {
        let tests: Vec<_> = res.iter().flat_map(|g| g.flatten_tests()).collect();
        let num_passed = tests.iter().filter(|t| t.status.passed()).count();

        let filter_str_template = match filter_str_template {
            Some(filter_str) => format!("({})", filter_str),
            None => "".to_string(),
        };

        Self {
            filter_str_template,
            passed_info: PassedInfo::new(tests.len(), num_passed),
            tests,
        }
    }
}

/// Template for displaying filtered tests to `stdout`.
#[derive(Debug, Template)]
#[template(path = "test_results_summary.md")]
struct TestResultsSummaryTemplate {
    groups: Vec<TemplateGroupResultsData>,
}

impl From<Vec<TestGroupRunResults>> for TestResultsSummaryTemplate {
    fn from(v: Vec<TestGroupRunResults>) -> Self {
        Self {
            groups: v.into_iter().map(|g| g.into()).collect(),
        }
    }
}

#[derive(Debug)]
struct TemplateGroupResultsData {
    name: String,
    passed_info: PassedInfo,
    sub_groups: Vec<TemplateSubGroupResultsData>,
}

impl From<TestGroupRunResults> for TemplateGroupResultsData {
    fn from(v: TestGroupRunResults) -> Self {
        let sub_groups: Vec<TemplateSubGroupResultsData> =
            v.sub_group_res.into_iter().map(|g| g.into()).collect();

        let (tot_tests, num_passed) =
            sub_groups
                .iter()
                .fold((0, 0), |(tot_tests, num_passed), sub_g| {
                    (
                        tot_tests + sub_g.passed_info.tot_tests,
                        num_passed + sub_g.passed_info.num_passed,
                    )
                });

        Self {
            name: v.name,
            passed_info: PassedInfo::new(tot_tests, num_passed),
            sub_groups,
        }
    }
}

#[derive(Debug)]
struct TemplateSubGroupResultsData {
    name: String,
    passed_info: PassedInfo,
}

impl From<TestSubGroupRunResults> for TemplateSubGroupResultsData {
    fn from(v: TestSubGroupRunResults) -> Self {
        let tests: Vec<TestRunResult> = v.test_res.into_iter().collect();
        let num_passed = tests
            .iter()
            .filter(|t| matches!(t.status, TestStatus::Passed))
            .count();

        Self {
            name: v.name,
            passed_info: PassedInfo::new(tests.len(), num_passed),
        }
    }
}

/// Aggregate stats on tests that have passed/failed.
#[derive(Debug)]
struct PassedInfo {
    tot_tests: usize,
    num_passed: usize,
    perc_passed: String,
}

impl PassedInfo {
    fn new(tot_tests: usize, num_passed: usize) -> Self {
        let perc_passed = format!("{:2}%", num_passed as f32 / tot_tests as f32);

        Self {
            tot_tests,
            num_passed,
            perc_passed,
        }
    }
}

///
pub(crate) fn output_test_report_for_terminal(
    res: &[TestGroupRunResults],
    test_filter_str: Option<String>,
) {
    let filtered_tests_output_template = FilteredTestResultsTemplate::new(res, &test_filter_str);
    let report = filtered_tests_output_template
        .render()
        .expect("Error rendering filtered test output markdown");

    termimad::print_text(&report);
}

/// Write a generalized markdown report to file showing the number of passing
/// tests per each group's sub-groups. Does not include any information on
/// specific test failures.
pub(crate) fn write_overall_status_report_summary_to_file(
    res: Vec<TestGroupRunResults>,
) -> anyhow::Result<()> {
    let overall_summary_template: TestResultsSummaryTemplate = res.into();
    let report = overall_summary_template
        .render()
        .expect("Error rendering summary report markdown");

    let summary_path = Path::new(&REPORT_OUTPUT).join("summary.md");
    fs::create_dir_all(summary_path.parent().unwrap())
        .with_context(|| format!("Creating report subdirectory {}", REPORT_OUTPUT))?;

    fs::write(&summary_path, report)
        .with_context(|| format!("Writing report to {:?}", summary_path))?;
    Ok(())
}
