#![feature(let_chains)]

use arg_parsing::{ProgArgs, ReportType};
use clap::Parser;
use common::utils::init_env_logger;
use log::info;
use plonky2_runner::run_plonky2_tests;
use report_generation::output_test_report_for_terminal;
use test_dir_reading::read_in_all_parsed_tests;

use crate::report_generation::write_overall_status_report_summary_to_file;

mod arg_parsing;
mod plonky2_runner;
mod report_generation;
mod test_dir_reading;

#[tokio::main()]
async fn main() -> anyhow::Result<()> {
    init_env_logger();

    let ProgArgs {
        test_filter,
        report_type,
        parsed_tests_path,
    } = ProgArgs::parse();

    let parsed_tests = read_in_all_parsed_tests(&parsed_tests_path, test_filter.clone()).await?;
    let test_res = run_plonky2_tests(parsed_tests);

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

    Ok(())
}
