use std::{
    collections::HashSet,
    fs::create_dir_all,
    io::BufReader,
    path::{Path, PathBuf},
};

use common::types::PARSED_TESTS_PATH;
use log::{debug, info};
use serde_json::Value;

use crate::{
    types::ETH_TESTS_REPO_PATH,
    utils::{get_entries_of_dir, open_file_expected},
};

type JsonFieldWhiteList = HashSet<&'static str>;

pub(crate) async fn parse_test_directories(
    test_dirs_needing_reparse: Vec<PathBuf>,
) -> anyhow::Result<()> {
    for dir in test_dirs_needing_reparse {
        create_dir_all(&dir)?;
        parse_test_dir(dir)?;
    }

    Ok(())
}

pub(crate) async fn parse_test_directories_forced() -> anyhow::Result<()> {
    todo!()
}

fn parse_test_dir(eth_test_repo_test_sub_dir: PathBuf) -> anyhow::Result<()> {
    info!("Parsing test directory {:?}...", eth_test_repo_test_sub_dir);

    let mut parsed_test_sub_dir = PathBuf::new();
    parsed_test_sub_dir.push(PARSED_TESTS_PATH);
    parsed_test_sub_dir.push(
        eth_test_repo_test_sub_dir
            .strip_prefix(ETH_TESTS_REPO_PATH)
            .unwrap(),
    );

    let whitelist = init_json_field_whitelist();

    for f_path in get_entries_of_dir(&eth_test_repo_test_sub_dir) {
        debug!("Parsing test {:?}", f_path);

        let test_json = serde_json::from_reader(BufReader::new(open_file_expected(&f_path)))?;
        parse_eth_test(test_json, &parsed_test_sub_dir, &whitelist);
    }

    Ok(())
}

fn parse_eth_test(
    eth_test_contents: Value,
    _parsed_out_path: &Path,
    whitelist: &JsonFieldWhiteList,
) {
    let mut relevant_fields = Vec::new();
    extract_relevant_fields("root", eth_test_contents, &mut relevant_fields, whitelist);
}

fn extract_relevant_fields(
    k: &str,
    v: Value,
    relevant_fields: &mut Vec<Value>,
    whitelist: &JsonFieldWhiteList,
) {
    if whitelist.contains(k) {
        relevant_fields.push(v);
        return;
    }

    if let Value::Object(map) = v {
        for (k, v) in map {
            extract_relevant_fields(&k, v, relevant_fields, whitelist);
        }
    }
}

fn init_json_field_whitelist() -> HashSet<&'static str> {
    let mut whitelist = HashSet::new();

    whitelist.insert("Berlin");
    whitelist.insert("pre");
    whitelist.insert("transaction");

    whitelist
}
