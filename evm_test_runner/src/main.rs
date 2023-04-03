#![feature(let_chains)]

use std::sync::{atomic::AtomicBool, Arc};

use arg_parsing::{ProgArgs, ReportType};
use clap::Parser;
use common::utils::init_env_logger;
use log::info;
use persistent_run_state::load_existing_pass_state_from_disk_if_exists_or_create;
use plonky2_runner::run_plonky2_tests;
use report_generation::output_test_report_for_terminal;
use test_dir_reading::{get_default_parsed_tests_path, read_in_all_parsed_tests};

use crate::report_generation::write_overall_status_report_summary_to_file;

mod arg_parsing;
mod persistent_run_state;
mod plonky2_runner;
mod report_generation;
mod state_diff;
mod test_dir_reading;

pub(crate) type ProcessAbortedRecv = Arc<AtomicBool>;

#[tokio::main()]
async fn main() -> anyhow::Result<()> {
    init_env_logger();
    let abort_recv = init_ctrl_c_handler();

    let ProgArgs {
        test_filter,
        report_type,
        variant_filter,
        parsed_tests_path,
        simple_progress_indicator,
        update_persistent_state_from_upstream,
    } = ProgArgs::parse();
    let mut persistent_test_state = load_existing_pass_state_from_disk_if_exists_or_create();

    let parsed_tests_path = parsed_tests_path
        .map(Ok)
        .unwrap_or_else(get_default_parsed_tests_path)?;

    let parsed_tests =
        read_in_all_parsed_tests(&parsed_tests_path, test_filter.clone(), variant_filter).await?;

    if update_persistent_state_from_upstream {
        println!("Updating persisted test pass state from locally downloaded tests...");

        let t_names = parsed_tests
            .iter()
            .flat_map(|g| {
                g.sub_groups
                    .iter()
                    .map(|sub_g| sub_g.tests.iter().map(|t| t.name.as_str()))
            })
            .flatten();

        persistent_test_state
            .add_remove_entries_from_upstream_tests(t_names, test_filter.is_some());
    }

    let test_res = match run_plonky2_tests(
        parsed_tests,
        simple_progress_indicator,
        &mut persistent_test_state,
        abort_recv,
    ) {
        Ok(r) => r,
        Err(_) => {
            persistent_test_state.write_to_disk();
            return Ok(());
        }
    };

    match report_type {
        ReportType::Test => {
            info!("Outputting test results to stdout...");
            output_test_report_for_terminal(&test_res, test_filter.clone());
        }
        ReportType::Summary => {
            info!("Generating test results markdown...");
            write_overall_status_report_summary_to_file(test_res)?;
        }
    }

    persistent_test_state.write_to_disk();

    Ok(())
}

fn init_ctrl_c_handler() -> ProcessAbortedRecv {
    let aborted_bool = Arc::new(AtomicBool::new(false));

    let cloned_aborted_bool = aborted_bool.clone();
    ctrlc::set_handler(move || {
        println!("Abort signal received! Stopping currently running test...");
        cloned_aborted_bool.store(true, std::sync::atomic::Ordering::Relaxed);
    })
    .unwrap();

    aborted_bool
}
