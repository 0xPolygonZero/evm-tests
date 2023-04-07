#![feature(let_chains)]

use std::{rc::Rc, sync::Arc};

use arg_parsing::{ProgArgs, ReportType};
use clap::Parser;
use common::utils::init_env_logger;
use futures::executor::block_on;
use log::info;
use persistent_run_state::load_existing_pass_state_from_disk_if_exists_or_create;
use plonky2_runner::run_plonky2_tests;
use report_generation::output_test_report_for_terminal;
use test_dir_reading::{get_default_parsed_tests_path, read_in_all_parsed_tests};
use tokio::{
    runtime::{self},
    sync::mpsc,
};

use crate::report_generation::write_overall_status_report_summary_to_file;

mod arg_parsing;
mod persistent_run_state;
mod plonky2_runner;
mod report_generation;
mod state_diff;
mod test_dir_reading;

// Oneshot is ideal here, but I can't get it to the abort handler.
pub(crate) type ProcessAbortedRecv = mpsc::Receiver<()>;

fn main() -> anyhow::Result<()> {
    init_env_logger();

    let rt = runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Creating Tokio runtime");
    let res = rt.block_on(run());

    match res {
        // True if we exited without an error but need to stop any Plonky2 threads.
        Ok(true) | Err(_) => {
            // Don't wait for any plonky2 threads to finish.
            rt.shutdown_background();
        }
        _ => (),
    };

    res.map(|_| ())
}

async fn run() -> anyhow::Result<bool> {
    let abort_recv = init_ctrl_c_handler();

    let ProgArgs {
        test_filter,
        report_type,
        variant_filter,
        skip_passed,
        test_timeout,
        parsed_tests_path,
        simple_progress_indicator,
        update_persistent_state_from_upstream,
    } = ProgArgs::parse();
    let mut persistent_test_state = load_existing_pass_state_from_disk_if_exists_or_create();
    let filters_used = test_filter.is_some() || variant_filter.is_some();
    let passed_t_names = skip_passed.then(|| {
        Arc::new(
            persistent_test_state
                .get_tests_that_have_passed()
                .map(|t| t.to_string())
                .collect(),
        )
    });

    let parsed_tests_path = parsed_tests_path
        .map(Ok)
        .unwrap_or_else(get_default_parsed_tests_path)?;

    let parsed_tests = Rc::new(
        read_in_all_parsed_tests(
            &parsed_tests_path,
            test_filter.clone(),
            variant_filter,
            passed_t_names,
        )
        .await?,
    );

    if update_persistent_state_from_upstream {
        println!("Updating persisted test pass state from locally downloaded tests...");

        let parsed_tests = match filters_used {
            false => parsed_tests.clone(),

            // I too like lifetime issues...
            // If filters are used, then we need to reparse the tests.
            // `add_remove_entries_from_upstream_tests` requires all the tests in the test directory
            // in order to function correctly.
            true => Rc::new(read_in_all_parsed_tests(&parsed_tests_path, None, None, None).await?),
        };

        let t_names = parsed_tests
            .iter()
            .flat_map(|g| {
                g.sub_groups
                    .iter()
                    .map(|sub_g| sub_g.tests.iter().map(|t| t.name.as_str()))
            })
            .flatten();

        persistent_test_state.add_remove_entries_from_upstream_tests(t_names);
    }

    // Remove the Rc since we no longer need it.
    let parsed_tests = Rc::try_unwrap(parsed_tests).unwrap();

    let test_res = match run_plonky2_tests(
        parsed_tests,
        simple_progress_indicator,
        &mut persistent_test_state,
        abort_recv,
        test_timeout.map(|t| t.into()),
    ) {
        Ok(r) => r,
        Err(_) => {
            println!("Got an error. Writing to disk...");
            persistent_test_state.write_to_disk();

            return Ok(true);
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

    Ok(false)
}

fn init_ctrl_c_handler() -> ProcessAbortedRecv {
    let (send, recv) = mpsc::channel(2);

    ctrlc::set_handler(move || {
        println!("Abort signal received! Stopping currently running test...");
        block_on(send.send(())).unwrap();
    })
    .unwrap();

    recv
}
