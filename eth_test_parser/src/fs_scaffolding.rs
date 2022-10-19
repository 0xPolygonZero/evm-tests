//! Filesystem helpers. A set of convenience functions for interacting with test
//! input and output directories.
use std::{
    fs::{self, DirEntry},
    path::PathBuf,
};

use anyhow::Result;

use crate::config::{ETH_TESTS_REPO_LOCAL_PATH, GENERATION_INPUTS_OUTPUT_DIR, TEST_GROUPS};

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
    let dirs = fs::read_dir(ETH_TESTS_REPO_LOCAL_PATH)?
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
pub(crate) fn prepare_output_dir() -> Result<()> {
    for dir in get_test_group_sub_dirs()? {
        fs::create_dir_all(
            PathBuf::from(GENERATION_INPUTS_OUTPUT_DIR)
                .join(dir.path().strip_prefix(ETH_TESTS_REPO_LOCAL_PATH)?),
        )?
    }

    Ok(())
}
