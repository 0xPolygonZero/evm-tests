//! Filesystem helpers. A set of convenience functions for interacting with test
//! input and output directories.
use std::{
    fs::{self, DirEntry, File},
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use common::config::GENERATION_INPUTS_DEFAULT_OUTPUT_DIR;

use crate::{
    config::{ETH_TESTS_REPO_LOCAL_PATH, GENERAL_GROUP, TEST_GROUPS},
    deserialize::{TestBody, TestFile},
};

/// Get the default parsed test output directory.
/// We first check if the flat file, `ETH_TEST_PARSER_DEV`, exists
/// in the current working directory. If so, we assume we're in a development
/// context, and default to the project root. Otherwise, we cannot make any
/// assumptions, fall back to the `GENERATION_INPUTS_DEFAULT_OUTPUT_DIR` value.
pub(crate) fn get_default_out_dir() -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let mut dev_check_path = cwd.clone();
    dev_check_path.push("ETH_TEST_PARSER_DEV");
    if dev_check_path.exists() {
        let mut out_dir = cwd
            .parent()
            .ok_or_else(|| {
                anyhow!(
                    "Unable to read cwd path parent. {:?} has no parent.",
                    cwd.as_os_str()
                )
            })?
            .to_path_buf();
        out_dir.push(GENERATION_INPUTS_DEFAULT_OUTPUT_DIR);
        Ok(out_dir)
    } else {
        Ok(GENERATION_INPUTS_DEFAULT_OUTPUT_DIR.into())
    }
}

/// Generate an iterator over the outer test group folders.
///
/// Expected directory structure
/// ```ignore
/// // {TestGroupN} <--- HERE
/// // ├── {TestNameN}
/// // │   ├── {test_case_1}.json
/// // │   └── {test_case_n}.json
/// ```
pub(crate) fn get_test_group_dirs() -> Result<impl Iterator<Item = DirEntry>> {
    let dirs = fs::read_dir(ETH_TESTS_REPO_LOCAL_PATH.to_owned() + "/" + GENERAL_GROUP)?
        .flatten()
        .filter(|entry| match entry.file_name().to_str() {
            Some(file_name) => TEST_GROUPS.contains(&file_name),
            None => false,
        });

    Ok(dirs)
}

/// Generate an iterator over the inner test group folders.
///
/// Expected directory structure
/// ```ignore
/// // {TestGroupN}
/// // ├── {TestNameN} <--- HERE
/// // │   ├── {test_case_1}.json
/// // │   └── {test_case_n}.json
/// ```
pub(crate) fn get_test_group_sub_dirs() -> Result<impl Iterator<Item = DirEntry>> {
    let dirs = get_test_group_dirs()?
        .flat_map(|entry| fs::read_dir(entry.path()))
        .flatten()
        .flatten();

    Ok(dirs)
}

/// Generate an iterator over the entire set of inner test case files.
///
/// Expected directory structure
/// ```ignore
/// // {TestGroupN}
/// // ├── {TestNameN}
/// // │   ├── {test_case_1}.json  <--- HERE
/// // │   └── {test_case_n}.json
/// ```
pub(crate) fn get_test_files() -> Result<impl Iterator<Item = DirEntry>> {
    let dirs = get_test_group_sub_dirs()?
        .flat_map(|entry| fs::read_dir(entry.path()))
        .flatten()
        .flatten()
        .filter(|entry| match entry.path().extension() {
            None => false,
            Some(ext) => ext == "json",
        });

    Ok(dirs)
}

/// Create output directories mirroring the structure of source test
/// directories.
pub(crate) fn prepare_output_dir(out_path: &Path) -> Result<()> {
    for dir in get_test_group_sub_dirs()? {
        fs::create_dir_all(out_path.join(dir.path().strip_prefix(ETH_TESTS_REPO_LOCAL_PATH)?))?
    }

    Ok(())
}

/// Generate an iterator containing the deserialized test bodies (`TestBody`)
/// and their `DirEntry`s.
pub(crate) fn get_deserialized_test_bodies(
) -> Result<impl Iterator<Item = Result<(DirEntry, Vec<TestBody>), (String, String)>>> {
    Ok(get_test_files()?.map(|entry| {
        let test_body = get_deserialized_test_body(&entry)
            .map_err(|err| (err.to_string(), entry.path().to_string_lossy().to_string()))?;
        Ok((entry, test_body))
    }))
}

fn get_deserialized_test_body(entry: &DirEntry) -> Result<Vec<TestBody>> {
    let buf = BufReader::new(File::open(entry.path())?);
    let test_file: TestFile = serde_json::from_reader(buf)?;

    anyhow::Ok(test_file.0.into_values().collect())
}
