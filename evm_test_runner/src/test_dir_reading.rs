use std::path::{Path, PathBuf};

use anyhow::Context;
use common::types::{ParsedTest, PARSED_TESTS_EXT};
use log::{debug, info, trace};
use tokio::{
    fs::{self, read_dir},
    task::JoinSet,
};
use tokio_stream::{wrappers::ReadDirStream, StreamExt};

#[derive(Debug)]
pub(crate) struct ParsedTestGroup {
    name: String,
    sub_groups: Vec<ParsedTestSubGroup>,
}

#[derive(Debug)]
pub(crate) struct ParsedTestSubGroup {
    name: String,
    tests: Vec<Test>,
}

#[derive(Debug)]
pub(crate) struct Test {
    name: String,
    info: ParsedTest,
}

pub(crate) async fn parse_all_tests(
    parsed_tests_path: &Path,
) -> anyhow::Result<Vec<ParsedTestGroup>> {
    let mut groups = Vec::new();
    let mut join_set = JoinSet::new();

    let mut read_dirs = ReadDirStream::new(read_dir(&parsed_tests_path).await?);
    while let Some(entry) = read_dirs.next().await {
        let entry = entry?;

        if !entry.file_type().await?.is_dir() {
            continue;
        }

        let group_path = entry.path();
        join_set.spawn(parse_test_group(group_path));
    }

    while let Some(h) = join_set.join_next().await {
        groups.push(h??);
    }

    Ok(groups)
}

async fn parse_test_group(path: PathBuf) -> anyhow::Result<ParsedTestGroup> {
    info!("Reading in test group {:?}...", path);

    let mut sub_groups = Vec::new();
    let mut join_set = JoinSet::new();

    let mut read_dirs = ReadDirStream::new(read_dir(&path).await?);
    while let Some(entry) = read_dirs.next().await {
        let entry = entry?;

        if !entry.file_type().await?.is_dir() {
            continue;
        }

        let sub_group_path = entry.path();
        join_set.spawn(parse_test_sub_group(path.join(sub_group_path)));
    }

    while let Some(h) = join_set.join_next().await {
        sub_groups.push(h??);
    }

    Ok(ParsedTestGroup {
        name: path.to_string_lossy().to_string(),
        sub_groups,
    })
}

async fn parse_test_sub_group(path: PathBuf) -> anyhow::Result<ParsedTestSubGroup> {
    debug!("Reading in test subgroup {:?}...", path);

    let mut tests = Vec::new();
    let mut join_set = JoinSet::new();

    let mut read_dirs = ReadDirStream::new(read_dir(&path).await?);
    while let Some(entry) = read_dirs.next().await {
        let entry = entry?;

        if path.extension().and_then(|os_str| os_str.to_str()) != Some(PARSED_TESTS_EXT) {
            continue;
        }

        let test_path = entry.path();
        join_set.spawn(read_parsed_test(path.join(test_path)));
    }

    while let Some(h) = join_set.join_next().await {
        tests.push(h??);
    }

    Ok(ParsedTestSubGroup {
        name: get_file_stem(&path)?,
        tests,
    })
}

async fn read_parsed_test(path: PathBuf) -> anyhow::Result<Test> {
    trace!("Reading in {:?}...", path);

    let parsed_test_bytes = fs::read(&path).await?;
    let parsed_test = serde_json::from_slice(&parsed_test_bytes)
        .unwrap_or_else(|_| panic!("Unable to parse the test {:?} (bad format)", path));

    Ok(Test {
        name: get_file_stem(&path)?,
        info: parsed_test,
    })
}

fn get_file_stem(path: &Path) -> anyhow::Result<String> {
    let res = path
        .file_stem()
        .with_context(|| "Unable to get file stem")?
        .to_string_lossy()
        .to_string();
    Ok(res)
}
