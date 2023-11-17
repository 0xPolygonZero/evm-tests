//! Handles feeding the parsed tests into `plonky2` and determining the result.
//! Essentially converts parsed tests into test results.

use std::{
    fmt::{Debug, Display},
    time::Duration,
};

use common::types::TestVariantRunInfo;
use ethereum_types::U256;
use futures::executor::block_on;
use indicatif::{ProgressBar, ProgressStyle};
use log::{trace, warn};
use plonky2::{
    field::goldilocks_field::GoldilocksField, plonk::config::KeccakGoldilocksConfig,
    util::timing::TimingTree,
};
use plonky2_evm::{
    all_stark::AllStark, config::StarkConfig, prover::prove, verifier::verify_proof,
};
use tokio::{select, time::timeout};

use crate::{
    persistent_run_state::TestRunEntries,
    test_dir_reading::{ParsedTestGroup, ParsedTestSubGroup, Test},
    ProcessAbortedRecv,
};

pub(crate) type RunnerResult<T> = Result<T, ()>;

trait TestProgressIndicator: Debug {
    fn set_current_test_name(&self, t_name: String);
    fn notify_test_completed(&mut self);
}

/// Simple test progress indicator that uses `println!`s.
#[derive(Debug)]
struct SimpleProgressIndicator {
    num_tests: u64,
    curr_test: usize,
}

impl TestProgressIndicator for SimpleProgressIndicator {
    fn set_current_test_name(&self, t_name: String) {
        println!(
            "({}/{}) Running {}...",
            self.curr_test, self.num_tests, t_name
        );
    }

    // Kinda gross...
    fn notify_test_completed(&mut self) {
        self.curr_test += 1;
    }
}

/// More elegant test progress indicator that uses a progress bar library.
#[derive(Debug)]
struct FancyProgressIndicator {
    prog_bar: ProgressBar,
}

impl TestProgressIndicator for FancyProgressIndicator {
    fn set_current_test_name(&self, t_name: String) {
        self.prog_bar.set_message(t_name);
    }

    fn notify_test_completed(&mut self) {
        self.prog_bar.inc(1);
    }
}

#[derive(Clone, Debug)]
pub(crate) enum TestStatus {
    Passed,
    #[allow(unused)]
    Ignored,
    EvmErr(String),
    TimedOut,
}

impl Display for TestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestStatus::Passed => write!(f, "Passed"),
            TestStatus::Ignored => write!(f, "Ignored"),
            TestStatus::EvmErr(err) => write!(f, "Evm error: {}", err),
            TestStatus::TimedOut => write!(f, "Test timed out"),
        }
    }
}

impl TestStatus {
    pub(crate) fn passed(&self) -> bool {
        matches!(self, TestStatus::Passed)
    }
}

#[derive(Debug)]
pub(crate) struct TestGroupRunResults {
    pub(crate) name: String,
    pub(crate) sub_group_res: Vec<TestSubGroupRunResults>,
}

fn num_tests_in_groups<'a>(groups: impl Iterator<Item = &'a ParsedTestGroup> + 'a) -> u64 {
    groups
        .map(|g| {
            g.sub_groups
                .iter()
                .flat_map(|sub_g| sub_g.tests.iter())
                .count() as u64
        })
        .sum()
}

#[derive(Debug)]
pub(crate) struct TestSubGroupRunResults {
    pub(crate) name: String,
    pub(crate) test_res: Vec<TestRunResult>,
}

#[derive(Debug)]
pub(crate) struct TestRunResult {
    pub(crate) name: String,
    pub(crate) status: TestStatus,
}

#[derive(Debug)]
struct TestRunState<'a> {
    p_indicator: Box<dyn TestProgressIndicator>,
    persistent_test_state: &'a mut TestRunEntries,
    process_aborted_recv: ProcessAbortedRecv,
    test_timeout: Duration,
}

pub(crate) fn run_plonky2_tests(
    parsed_tests: Vec<ParsedTestGroup>,
    simple_progress_indicator: bool,
    persistent_test_state: &mut TestRunEntries,
    process_aborted: ProcessAbortedRecv,
    test_timeout: Option<Duration>,
) -> RunnerResult<Vec<TestGroupRunResults>> {
    let num_tests = num_tests_in_groups(parsed_tests.iter());
    let p_indicator = create_progress_indicator(num_tests, simple_progress_indicator);

    let test_timeout = match test_timeout {
        Some(t) => t,
        None => Duration::MAX,
    };

    let mut t_state = TestRunState {
        p_indicator,
        persistent_test_state,
        process_aborted_recv: process_aborted,
        test_timeout,
    };

    parsed_tests
        .into_iter()
        .map(|g| run_test_group(g, &mut t_state))
        .collect::<RunnerResult<_>>()
}

fn create_progress_indicator(
    num_tests: u64,
    simple_progress_indicator: bool,
) -> Box<dyn TestProgressIndicator> {
    match simple_progress_indicator {
        false => Box::new({
            FancyProgressIndicator {
                prog_bar: ProgressBar::new(num_tests).with_style(
                    ProgressStyle::with_template(
                        "{bar:60.magenta} {pos}/{len} ETA: [{eta_precise}] | Test: {msg}",
                    )
                    .unwrap(),
                ),
            }
        }),
        true => Box::new(SimpleProgressIndicator {
            curr_test: 0,
            num_tests,
        }),
    }
}

fn run_test_group(
    group: ParsedTestGroup,
    t_state: &mut TestRunState,
) -> RunnerResult<TestGroupRunResults> {
    Ok(TestGroupRunResults {
        name: group.name,
        sub_group_res: group
            .sub_groups
            .into_iter()
            .map(|sub_g| run_test_sub_group(sub_g, t_state))
            .collect::<RunnerResult<_>>()?,
    })
}

fn run_test_sub_group(
    sub_group: ParsedTestSubGroup,
    t_state: &mut TestRunState,
) -> RunnerResult<TestSubGroupRunResults> {
    Ok(TestSubGroupRunResults {
        name: sub_group.name,
        test_res: sub_group
            .tests
            .into_iter()
            .map(|sub_g| run_test(sub_g, t_state))
            .collect::<RunnerResult<_>>()?,
    })
}

fn run_test(test: Test, t_state: &mut TestRunState) -> RunnerResult<TestRunResult> {
    trace!("Running test {}...", test.name);

    t_state
        .p_indicator
        .set_current_test_name(test.name.to_string());
    let res = run_test_or_fail_on_timeout(test.info, t_state)?;

    t_state
        .persistent_test_state
        .update_test_state(&test.name, res.clone().into());
    t_state.p_indicator.notify_test_completed();

    Ok(TestRunResult {
        name: test.name,
        status: res,
    })
}

fn run_test_or_fail_on_timeout(
    test: TestVariantRunInfo,
    t_state: &mut TestRunState,
) -> RunnerResult<TestStatus> {
    block_on(async {
        let proof_gen_fut = async { run_test_and_get_test_result(test) };
        let proof_gen_with_timeout_fut = timeout(t_state.test_timeout, proof_gen_fut);
        let process_aborted_fut = t_state.process_aborted_recv.recv();

        select! {
            res = proof_gen_with_timeout_fut => {
                match res {
                    Ok(t_res) => Ok(t_res),
                    Err(_) => Ok(TestStatus::TimedOut),
                }
            },
            // Process was aborted.
            _ = process_aborted_fut => Err(()),
        }
    })
}

/// Run a test against `plonky2` and output a result based on what happens.
fn run_test_and_get_test_result(test: TestVariantRunInfo) -> TestStatus {
    let timing = TimingTree::new("prove", log::Level::Debug);

    // plonky2 does not support a block gaslimit that does not fit in a u32
    // If a test has such issue, we "try" proving it with an altered gaslimit,
    // and will ignore it if proving the altered inputs failed so as to not
    // have false positives.
    let mut inputs = test.gen_inputs;
    let is_gaslimit_changed =
        TryInto::<u32>::try_into(inputs.block_metadata.block_gaslimit).is_err();

    if is_gaslimit_changed {
        inputs.block_metadata.block_gaslimit = U256::from(u32::MAX);
    }

    let proof_run_res = prove::<GoldilocksField, KeccakGoldilocksConfig, 2>(
        &AllStark::default(),
        &StarkConfig::standard_fast_config(),
        inputs,
        &mut TimingTree::default(),
    );

    timing.filter(Duration::from_millis(100)).print();

    let proof_run_output = match proof_run_res {
        Ok(v) => v,
        Err(evm_err) => {
            if is_gaslimit_changed {
                // We altered the inputs, so we just skip this test in case of failure.
                return TestStatus::Ignored;
            }

            // The prover failed with unmodified inputs, so this is an actual error.
            warn!("Proving failed with error: {:?}", evm_err);
            return TestStatus::EvmErr(evm_err.to_string());
        }
    };

    let verif_output = verify_proof(
        &AllStark::default(),
        proof_run_output,
        &StarkConfig::standard_fast_config(),
    );
    if verif_output.is_err() {
        warn!("Verification failed with error: {:?}", verif_output);
        return TestStatus::EvmErr("Proof verification failed.".to_string());
    }

    TestStatus::Passed
}
