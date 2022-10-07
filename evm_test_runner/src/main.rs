#![feature(let_chains)]

use arg_parsing::{parse_prog_args, RunCommand};
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
    let p_args = parse_prog_args();

    let filter = match &p_args.cmd {
        RunCommand::Test(filter) => &filter.test_filter,
        RunCommand::Report => &None,
    };

    let parsed_tests = read_in_all_parsed_tests(&p_args.parsed_tests_path, filter.as_ref()).await?;
    let test_res = run_plonky2_tests(parsed_tests);

    match p_args.cmd {
        RunCommand::Test(_) => {
            info!("Outputting test results to stdout...");
            output_test_report_for_terminal(test_res);
        }
        RunCommand::Report => {
            info!("Generating test results markdown...");
            write_overall_status_report_summary_to_file(test_res)?;
        }
    }

    Ok(())
}
