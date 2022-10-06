//! Report generation.
//!
//! Supports reports in both markdown and output for the terminal.

use std::{fs, path::Path};

use askama::Template;

use crate::plonky2_runner::{
    TestGroupRunResults, TestRunResult, TestStatus, TestSubGroupRunResults,
};

const REPORT_OUTPUT: &str = "reports";

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
    passed_info: TemplateTestPassedInfo,
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
        let passed_info = TemplateTestPassedInfo::new(tot_tests, num_passed);

        Self {
            name: v.name,
            passed_info,
            sub_groups,
        }
    }
}

#[derive(Debug)]
struct TemplateSubGroupResultsData {
    name: String,
    passed_info: TemplateTestPassedInfo,
    tests: Vec<TemplateTestResultData>,
}

impl From<TestSubGroupRunResults> for TemplateSubGroupResultsData {
    fn from(v: TestSubGroupRunResults) -> Self {
        let tests: Vec<TemplateTestResultData> = v.test_res.into_iter().map(|t| t.into()).collect();
        let num_passed = tests
            .iter()
            .filter(|t| matches!(t.status, TestStatus::Passed))
            .count();
        let passed_info = TemplateTestPassedInfo::new(tests.len(), num_passed);

        Self {
            name: v.name,
            passed_info,
            tests,
        }
    }
}

// TODO: Consider removing if there are no different fields from
// `TestRunResult`...
#[derive(Debug)]
struct TemplateTestResultData {
    name: String,
    status: TestStatus,
}

impl From<TestRunResult> for TemplateTestResultData {
    fn from(v: TestRunResult) -> Self {
        Self {
            name: v.name,
            status: v.status,
        }
    }
}

#[derive(Debug)]
struct TemplateTestPassedInfo {
    tot_tests: usize,
    num_passed: usize,
    perc_passed: String,
}

impl TemplateTestPassedInfo {
    fn new(tot_tests: usize, num_passed: usize) -> Self {
        let perc_passed = format!("{:2}%", num_passed as f32 / tot_tests as f32);

        Self {
            tot_tests,
            num_passed,
            perc_passed,
        }
    }
}

pub(crate) fn output_test_report_for_terminal(_res: Vec<TestGroupRunResults>) {
    todo!()
}

pub(crate) fn write_overall_status_report_summary_to_file(
    res: Vec<TestGroupRunResults>,
) -> anyhow::Result<()> {
    let overall_summary_template: TestResultsSummaryTemplate = res.into();
    let report = overall_summary_template
        .render()
        .expect("Error rendering report markdown");
    let summary_path = Path::new(&REPORT_OUTPUT).join("summary.md");

    fs::write(summary_path, report)?;
    Ok(())
}
