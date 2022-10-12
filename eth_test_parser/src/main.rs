use anyhow::Result;
use arg_parsing::ProgArgs;
use clap::Parser;
use common::utils::init_env_logger;
use fs_scaffolding::prepare_output_dir;
use trie_builder::get_deserialized_test_bodies;

use crate::eth_tests_fetching::clone_or_update_remote_tests;

mod arg_parsing;
mod config;
mod deserialize;
mod eth_tests_fetching;
mod fs_scaffolding;
mod trie_builder;
mod utils;

pub(crate) struct ProgState {}

impl ProgState {
    fn new(_: ProgArgs) -> Self {
        Self {}
    }
}

fn main() -> Result<()> {
    init_env_logger();
    let p_args = ProgArgs::parse();
    let state = ProgState::new(p_args);

    run(state)
}

fn run(_: ProgState) -> Result<()> {
    // Fetch most recent test json.
    clone_or_update_remote_tests();

    // Create output directories mirroring the structure of source tests.
    prepare_output_dir()?;

    // TODO: Use deserialized test structs to construct plonky2 generation inputs.
    for (test_dir_entry, test_body) in get_deserialized_test_bodies()? {
        println!("deserialized test {:?}: {:?}", test_dir_entry, test_body);
    }

    Ok(())
}
