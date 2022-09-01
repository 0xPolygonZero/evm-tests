use std::{
    collections::{HashMap, HashSet},
    fs::{self, create_dir_all},
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::Context;
use common::types::PARSED_TESTS_PATH;
use log::{debug, info};
use plonky2_evm::generation::GenerationInputs;
use serde_json::Value;

use crate::{
    json_parsing::accounts_from_json,
    stale_test_scanning::get_latest_commit_date_of_dir_from_git,
    types::{ETH_TESTS_REPO_PATH, SUB_TEST_DIR_LAST_CHANGED_FILE_NAME},
    utils::{get_entries_of_dir, get_parsed_test_path_for_eth_test_path, open_file_with_context},
};

type JsonFieldWhiteList = HashSet<&'static str>;
type ExtractedWhitelistedJson = HashMap<String, Value>;

const BERLIN_JSON_FIELD: &str = "berlin";
const ACCOUNTS_JSON_FIELD: &str = "pre";
const TXNS_JSON_FIELD: &str = "transactions";

/// All inputs needed for feeding tests into the VM.
struct ParsedTest {
    evm_gen_inputs: GenerationInputs,
}

pub(crate) async fn parse_test_directories(
    test_dirs_needing_reparse: Vec<PathBuf>,
) -> anyhow::Result<()> {
    for dir in test_dirs_needing_reparse {
        parse_test_directory(&dir)
            .with_context(|| format!("Parsing the sub test directory {:?}", dir))?;
    }

    Ok(())
}

fn parse_test_directory(dir: &Path) -> anyhow::Result<()> {
    let parsed_test_dir = get_parsed_test_path_for_eth_test_path(dir);

    create_dir_all(&parsed_test_dir).with_context(|| {
        format!(
            "Creating any missing sub-directories for parsed sub-tests at {:?}",
            &parsed_test_dir
        )
    })?;
    parse_test_dir(dir).with_context(|| "Parsing the test directory")?;
    write_commit_datetime_of_last_parse_file(dir)
        .with_context(|| "Writing the last commit date parsed to file")?;

    Ok(())
}

pub(crate) async fn parse_test_directories_forced() -> anyhow::Result<()> {
    todo!()
}

fn parse_test_dir(eth_test_repo_test_sub_dir: &Path) -> anyhow::Result<()> {
    println!("Parsing test directory {:?}...", eth_test_repo_test_sub_dir);

    let mut parsed_test_sub_dir = PathBuf::new();
    parsed_test_sub_dir.push(PARSED_TESTS_PATH);
    parsed_test_sub_dir.push(
        eth_test_repo_test_sub_dir
            .strip_prefix(ETH_TESTS_REPO_PATH)
            .unwrap(),
    );

    let whitelist = init_json_field_whitelist();

    for f_path in get_entries_of_dir(eth_test_repo_test_sub_dir)
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
    _parsed_out_path: &Path,
    whitelist: &JsonFieldWhiteList,
) -> anyhow::Result<()> {
    let mut relevant_fields = HashMap::new();
    extract_relevant_fields("root", eth_test_contents, &mut relevant_fields, whitelist);

    let _parsed_test = process_extracted_fields(relevant_fields)?;

    Ok(())
}

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
    let _parsed_accounts = accounts_from_json(&fields[ACCOUNTS_JSON_FIELD])?;
    todo!();
}

fn init_json_field_whitelist() -> HashSet<&'static str> {
    let mut whitelist = HashSet::new();

    whitelist.insert(BERLIN_JSON_FIELD);
    whitelist.insert(ACCOUNTS_JSON_FIELD);
    whitelist.insert(TXNS_JSON_FIELD);

    whitelist
}

fn write_commit_datetime_of_last_parse_file(sub_test_dir: &Path) -> anyhow::Result<()> {
    let last_commit_time = get_latest_commit_date_of_dir_from_git(sub_test_dir)?;
    let last_commit_time_path = get_parsed_test_path_for_eth_test_path(
        &sub_test_dir.join(SUB_TEST_DIR_LAST_CHANGED_FILE_NAME),
    );

    info!(
        "Updating commit datetime for last parse test subdirectory {:?} to {}.",
        last_commit_time_path, last_commit_time
    );
    fs::write(&last_commit_time_path, last_commit_time.to_string())?;

    Ok(())
}
