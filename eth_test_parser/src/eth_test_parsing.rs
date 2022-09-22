//! High level logic for parsing an full node test into a `PartialTrie` format
//! usable by `Plonky2`.
//!
//! The general flow of parsing is as follows:
//! - Read in the raw full node test and extract the JSON fields we care about.
//! - Pass each extracted piece of JSON into the corresponding `parse` function
//!   in `json_parsing`.
//! - Move the parsed JSON into `Plonky2`'s `GenerationInputs` and serialize
//!   this to file in the parsed test directory.

use std::{
    collections::{HashMap, HashSet},
    fs::{self, create_dir_all},
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::Context;
use common::types::ParsedTest;
use log::debug;
use plonky2_evm::generation::GenerationInputs;
use serde_json::Value;

use crate::{
    config::{ETH_TESTS_REPO_LOCAL_PATH, TEST_GROUPS},
    json_parsing::{
        parse_block_metadata_from_json, parse_initial_account_state_from_json,
        parse_receipt_trie_from_json, parse_txn_trie_from_json,
    },
    utils::{
        get_entries_of_dir, get_parsed_test_path_for_eth_test_path, get_paths_of_dir,
        open_file_with_context,
    },
};

type JsonFieldWhiteList = HashSet<&'static str>;
type ExtractedWhitelistedJson = HashMap<String, Value>;

const BERLIN_JSON_FIELD: &str = "berlin";
const ACCOUNTS_JSON_FIELD: &str = "pre";
const RECEIPTS_JSON_FIELD: &str = "receiptTrie"; // Likely incorrect...
const BLOCKS_JSON_FIELD: &str = "blocks";
const GENESIS_BLOCK_JSON_FIELD: &str = "genesisBlockHeader";

pub(crate) fn get_test_group_sub_dirs() -> Vec<PathBuf> {
    // Expected directory structure
    // {TestGroupN}
    // ├── {TestNameN}
    // │   ├── {test_case_1}.json
    // │   └── {test_case_n}.json
    get_entries_of_dir(ETH_TESTS_REPO_LOCAL_PATH)
        .filter(|entry| {
            TEST_GROUPS.contains(
                &entry
                    .file_name()
                    .to_str()
                    .expect("Couldn't convert filename to &str"),
            )
        })
        .flat_map(|entry| get_paths_of_dir(entry.path().to_str().unwrap()).collect::<Vec<_>>())
        .collect()
}

pub(crate) fn parse_test_directories(
    test_dirs_needing_reparse: Vec<PathBuf>,
) -> anyhow::Result<()> {
    for dir in test_dirs_needing_reparse {
        prep_and_parse_test_directory(&dir)
            .with_context(|| format!("Parsing the sub test directory {:?}", dir))?;
    }

    Ok(())
}

fn prep_and_parse_test_directory(dir: &Path) -> anyhow::Result<()> {
    let parsed_test_dir = get_parsed_test_path_for_eth_test_path(dir);

    create_dir_all(&parsed_test_dir).with_context(|| {
        format!(
            "Creating any missing sub-directories for parsed sub-tests at {:?}",
            &parsed_test_dir
        )
    })?;

    parse_test_directory(dir).with_context(|| "Parsing the test directory")?;

    Ok(())
}

/// Parses all json tests in the given sub-test directory.
fn parse_test_directory(eth_test_repo_test_sub_dir: &Path) -> anyhow::Result<()> {
    println!("Parsing test directory {:?}...", eth_test_repo_test_sub_dir);

    let parsed_test_sub_dir = get_parsed_test_path_for_eth_test_path(eth_test_repo_test_sub_dir);
    let whitelist = init_json_field_whitelist();

    for f_path in get_paths_of_dir(eth_test_repo_test_sub_dir)
        .filter(|p| p.extension().and_then(|os_str| os_str.to_str()) == Some("json"))
    {
        debug!("Parsing test {:?}", f_path);

        let test_json = serde_json::from_reader(BufReader::new(open_file_with_context(&f_path)?))
            .with_context(|| format!("Parsing the eth test {:?}", f_path))?;

        parse_eth_test(test_json, &parsed_test_sub_dir, &whitelist)?;
    }

    Ok(())
}

fn parse_eth_test(
    eth_test_contents: Value,
    parsed_out_path: &Path,
    whitelist: &JsonFieldWhiteList,
) -> anyhow::Result<()> {
    let mut relevant_fields = HashMap::new();
    extract_relevant_fields("root", eth_test_contents, &mut relevant_fields, whitelist);

    let generated_inputs = process_extracted_fields(relevant_fields)?;
    fs::write(
        parsed_out_path,
        &serde_json::to_string(&generated_inputs).unwrap(),
    )
    .unwrap();

    Ok(())
}

/// Extract any JSON fields that are in the whitelist.
/// If a field matches, the entire value is extracted (which could include
/// multiple JSON values).
fn extract_relevant_fields(
    k: &str,
    v: Value,
    relevant_fields: &mut ExtractedWhitelistedJson,
    whitelist: &JsonFieldWhiteList,
) {
    if whitelist.contains(k) {
        relevant_fields.insert(k.to_string(), v);
        return;
    }

    if let Value::Object(map) = v {
        for (k, v) in map {
            extract_relevant_fields(&k, v, relevant_fields, whitelist);
        }
    }
}

fn process_extracted_fields(fields: ExtractedWhitelistedJson) -> anyhow::Result<ParsedTest> {
    let account_info = parse_initial_account_state_from_json(&fields[ACCOUNTS_JSON_FIELD])?;
    let receipts_trie = parse_receipt_trie_from_json(&fields[RECEIPTS_JSON_FIELD]);
    let txn_info = parse_txn_trie_from_json(&fields[BLOCKS_JSON_FIELD]);
    let block_metadata = parse_block_metadata_from_json(
        &fields[BLOCKS_JSON_FIELD],
        &fields[GENESIS_BLOCK_JSON_FIELD],
    );

    let plonky2_inputs = GenerationInputs {
        signed_txns: txn_info.signed_txns,
        state_trie: account_info.account_trie,
        transactions_trie: txn_info.txn_trie,
        receipts_trie,
        storage_tries: account_info.account_storage_tries,
        block_metadata,
    };

    // TODO: Parse from the `Post` JSON field if present...
    let expected_final_account_states = None;

    Ok(ParsedTest {
        plonky2_inputs,
        expected_final_account_states,
    })
}

fn init_json_field_whitelist() -> HashSet<&'static str> {
    let mut whitelist = HashSet::new();

    whitelist.insert(BERLIN_JSON_FIELD);
    whitelist.insert(ACCOUNTS_JSON_FIELD);
    whitelist.insert(RECEIPTS_JSON_FIELD);
    whitelist.insert(BLOCKS_JSON_FIELD);
    whitelist.insert(GENESIS_BLOCK_JSON_FIELD);

    whitelist
}
