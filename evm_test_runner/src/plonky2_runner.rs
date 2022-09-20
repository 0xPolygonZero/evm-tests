use crate::test_dir_reading::ParsedTestGroup;

#[derive(Debug)]
pub(crate) enum TestResult {
    Passed,
    EvmError(String),
    IncorrectFinalState,
}

#[derive(Debug)]
pub(crate) struct TestGroupRunResults {
    name: String,
    sub_group_res: Vec<TestSubGroupRunResults>,
}

#[derive(Debug)]
pub(crate) struct TestSubGroupRunResults {
    name: String,
    test_res: Vec<TestRunResults>,
}

#[derive(Debug)]
pub(crate) struct TestRunResults {
    name: String,
    res: TestResult,
}

pub(crate) fn run_plonky2_tests(_parsed_tests: &Vec<ParsedTestGroup>) -> TestRunResults {
    todo!()
}
