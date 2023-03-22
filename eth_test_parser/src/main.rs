use std::fs::File;
use std::io::Write;

use anyhow::Result;
use arg_parsing::ProgArgs;
use clap::Parser;
use common::types::ParsedTestManifest;
use common::utils::init_env_logger;
use fs_scaffolding::prepare_output_dir;
use futures::future::join_all;
use log::warn;

use crate::fs_scaffolding::{get_default_out_dir, get_deserialized_test_bodies};
use crate::{config::ETH_TESTS_REPO_LOCAL_PATH, eth_tests_fetching::clone_or_update_remote_tests};

mod arg_parsing;
mod config;
mod deserialize;
mod eth_tests_fetching;
mod fs_scaffolding;
mod revm_builder;
mod trie_builder;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    init_env_logger();
    let p_args = ProgArgs::parse();

    run(p_args).await
}

async fn run(ProgArgs { no_fetch, out_path }: ProgArgs) -> anyhow::Result<()> {
    let out_path = out_path.map(Ok).unwrap_or_else(get_default_out_dir)?;

    if !no_fetch {
        // Fetch most recent test json.
        clone_or_update_remote_tests();

        // Create output directories mirroring the structure of source tests.
        prepare_output_dir(&out_path)?;
    }

    println!("Converting test json to plonky2 generation inputs");

    let generation_input_handles = get_deserialized_test_bodies()?.filter_map(|res| {
        match res {
            Ok((test_dir_entry, test_body)) => Some(tokio::task::spawn_blocking(move || {
                let parsed_test = test_body.as_plonky2_test_input();
                let revm_variants = match test_body.as_serializable_evm_instances() {
                    Ok(revm_variants) => Some(revm_variants),
                    Err(err) => {
                        warn!(
                            "Unable to generate evm instance for test {} due to error: {}. Skipping!",
                            test_dir_entry.path().display(),
                            err
                        );
                        None
                    }
                };

                let test_manifest = ParsedTestManifest {
                    plonky2_variants: parsed_test,
                    revm_variants,
                };

                (test_dir_entry, serde_cbor::to_vec(&test_manifest).unwrap())
            })),
            Err((err, path_str)) => {
                // Skip any errors in parsing a test. As the upstream repo changes, we may get
                // tests that start to fail (eg. some tests do not have a `merge` field).
                warn!(
                    "Unable to parse test {} due to error: {}. Skipping!",
                    path_str, err
                );
                None
            }
        }
    });

    println!(
        "Writing plonky2 generation input cbor to disk, {:?}",
        out_path.as_os_str()
    );

    for thread in join_all(generation_input_handles).await {
        let (test_dir_entry, generation_inputs) = thread.unwrap();
        let mut path = out_path.join(
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
