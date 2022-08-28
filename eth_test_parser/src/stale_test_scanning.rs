use std::{
    fs::{self, DirEntry},
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use anyhow::Context;
use common::types::PARSED_TESTS_PATH;
use log::debug;

use crate::{
    types::{DateTime, ETH_TESTS_REPO_PATH, SUB_TEST_DIR_LAST_CHANGED_FILE_NAME},
    utils::run_cmd,
};

const TEST_GROUPS: [&str; 1] = ["GeneralStateTests"];

pub(crate) fn determine_which_test_dirs_need_reparsing() -> anyhow::Result<Vec<PathBuf>> {
    let mut test_subgroup_dirs_needing_reparse = Vec::new();

    for entry in fs::read_dir(ETH_TESTS_REPO_PATH)
        .with_context(|| "Reading test directories from the Ethereum test repo")?
    {
        let entry = entry?;
        let f_name = get_file_name_from_fs_entry(&entry);

        if !entry.file_type()?.is_dir() || !TEST_GROUPS.contains(&f_name.as_str()) {
            continue;
        }

        let repo_path_buf = PathBuf::from_str(ETH_TESTS_REPO_PATH)
            .unwrap()
            .join(&f_name);

        // This is a test directory that we need to test.
        get_group_sub_test_dirs_that_have_changed_upstream(
            repo_path_buf,
            &mut test_subgroup_dirs_needing_reparse,
        )
        .with_context(|| {
            format!(
                "Checking for upstream changes for the test group {}",
                f_name
            )
        })?;
    }

    Ok(test_subgroup_dirs_needing_reparse)
}

fn get_group_sub_test_dirs_that_have_changed_upstream(
    test_group: PathBuf,
    test_subgroup_dirs_needing_reparse: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(test_group.clone()).with_context(|| {
        format!(
            "Getting entries for the test group directory {:?}",
            test_group
        )
    })? {
        let entry = entry?;
        let sub_group_path = entry.path();

        let dir_last_parse_commit_date_time =
            get_last_commit_datetime_used_by_last_parse_for_sub_test_dir(&entry.path());

        let dir_last_commit_date_time = get_latest_commit_date_of_dir_from_git(&entry.path())?;
        if dir_last_parse_commit_date_time == Some(dir_last_commit_date_time) {
            debug!(
                "Skipping parsing of test sub directory {:?} because it's already up to date...",
                &sub_group_path
            );
            continue;
        }

        debug!(
            "Reparsing the test sub-directory {:?} as it has been changed upstream...",
            &sub_group_path
        );
        test_subgroup_dirs_needing_reparse.push(sub_group_path);
    }

    Ok(())
}

fn get_last_commit_datetime_used_by_last_parse_for_sub_test_dir(
    sub_group_path: &Path,
) -> Option<DateTime> {
    let last_commit_parse_datetime_path = PathBuf::from_str(PARSED_TESTS_PATH)
        .unwrap()
        .join(sub_group_path)
        .join(Path::new(SUB_TEST_DIR_LAST_CHANGED_FILE_NAME));

    if !last_commit_parse_datetime_path.exists() {
        return None;
    }

    let last_commit_parse_datetime_string = fs::read_to_string(last_commit_parse_datetime_path)
        .expect("Reading the last commit parse datetime from file");

    Some(parse_datetime_from_string(
        &last_commit_parse_datetime_string,
    ))
}

pub(crate) fn get_latest_commit_date_of_dir_from_git(dir: &Path) -> anyhow::Result<DateTime> {
    // Since we are not using `cd`, we have to not include the repo root in the
    // path.
    let dir_without_repo = dir
        .strip_prefix(ETH_TESTS_REPO_PATH)
        .expect("Stripping the repo from the test directory path");

    let stdout = run_cmd(Command::new("git").args([
        "-C",
        ETH_TESTS_REPO_PATH,
        "log",
        "--decorate=short",
        "-n",
        "1",
        "--pretty=format:%cd",
        dir_without_repo.to_str().unwrap(),
    ]))
    .unwrap_or_else(|err| {
        panic!(
            "Getting the last commit datetime for the directory {:?} ({})",
            dir, err
        )
    });

    Ok(parse_datetime_from_string(&stdout))
}

fn get_file_name_from_fs_entry(entry: &DirEntry) -> String {
    entry
        .path()
        .file_name()
        .and_then(|o_str| o_str.to_str())
        .unwrap_or_else(|| panic!("File name somehow missing for directory {:?}!", entry))
        .to_string()
}

fn parse_datetime_from_string(datetime_str: &str) -> DateTime {
    DateTime::parse_from_str(datetime_str, "%a %h %e %T %Y %z").unwrap_or_else(|_| {
        panic!(
            "Parsing the last commit datetime string from {}",
            datetime_str
        )
    })
}
