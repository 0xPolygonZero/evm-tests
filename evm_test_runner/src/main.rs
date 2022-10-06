#![feature(let_chains)]

use arg_parsing::ProgArgs;
use clap::Parser;
use log::info;
use plonky2_runner::run_plonky2_tests;
use test_dir_reading::read_in_all_parsed_tests;

use crate::report_generation::write_overall_status_report_summary_to_file;

mod arg_parsing;
mod plonky2_runner;
mod report_generation;
mod test_dir_reading;

#[tokio::main()]
async fn main() -> anyhow::Result<()> {
    let p_args = ProgArgs::parse();

    let parsed_tests = read_in_all_parsed_tests(&p_args.parsed_tests_path).await?;
    let test_res = run_plonky2_tests(parsed_tests);

    if p_args.output_result_markdown {
        info!("Generating test results markdown...");
        write_overall_status_report_summary_to_file(test_res);
    }

    Ok(())
}
