use std::io::Write;
use std::{fs::File, path::PathBuf};

use anyhow::Result;
use arg_parsing::ProgArgs;
use clap::Parser;
use common::types::ParsedTest;
use common::utils::init_env_logger;
use fs_scaffolding::prepare_output_dir;
use futures::future::join_all;
use trie_builder::get_deserialized_test_bodies;

use crate::{
    config::{ETH_TESTS_REPO_LOCAL_PATH, GENERATION_INPUTS_OUTPUT_DIR},
    eth_tests_fetching::clone_or_update_remote_tests,
};

mod arg_parsing;
mod config;
mod deserialize;
mod eth_tests_fetching;
mod fs_scaffolding;
mod trie_builder;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    init_env_logger();
    let p_args = ProgArgs::parse();

    run(p_args).await
}

async fn run(ProgArgs { no_fetch }: ProgArgs) -> Result<()> {
    if !no_fetch {
        // Fetch most recent test json.
        clone_or_update_remote_tests();

        // Create output directories mirroring the structure of source tests.
        prepare_output_dir()?;
    }

    println!("Converting test json to plonky2 generation inputs");
    let generation_inputs_handle =
        get_deserialized_test_bodies()?.map(|(test_dir_entry, test_body)| {
            tokio::task::spawn_blocking(move || {
                (
                    test_dir_entry,
                    serde_cbor::to_vec(&ParsedTest {
                        plonky2_inputs: test_body.into_generation_inputs(),
                        expected_final_account_states: None,
                    })
                    .unwrap(),
                )
            })
        });

    println!("Writing plonky2 generation input cbor to disk");
    for thread in join_all(generation_inputs_handle).await {
        let (test_dir_entry, generation_inputs) = thread.unwrap();
        let mut path = PathBuf::from(GENERATION_INPUTS_OUTPUT_DIR).join(
            test_dir_entry
                .path()
                .strip_prefix(ETH_TESTS_REPO_LOCAL_PATH)
                .unwrap(),
        );
        path.set_extension("cbor");
        let mut file = File::create(path).unwrap();
        file.write_all(&generation_inputs).unwrap();
    }

    Ok(())
}
