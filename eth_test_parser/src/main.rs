use arg_parsing::ProgArgs;
use clap::Parser;
use common::utils::init_env_logger;
use eth_test_parsing::{get_test_group_sub_dirs, parse_test_directories};

use crate::eth_tests_fetching::clone_or_update_remote_tests;

mod arg_parsing;
mod config;
mod eth_test_parsing;
mod eth_tests_fetching;
mod json_parsing;
mod utils;

pub(crate) struct ProgState {}

impl ProgState {
    fn new(_: ProgArgs) -> Self {
        Self {}
    }
}

fn main() {
    init_env_logger();
    let p_args = ProgArgs::parse();
    let state = ProgState::new(p_args);

    run(state)
}

fn run(_: ProgState) {
    clone_or_update_remote_tests();
    parse_test_directories(get_test_group_sub_dirs()).unwrap()
}
