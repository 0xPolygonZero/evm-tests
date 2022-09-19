use std::{
    fs::{self, File},
    ops::BitAnd,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{bail, Context};
use common::types::PARSED_TESTS_PATH;
use ethereum_types::{Address, U256};
use num_traits::PrimInt;

use crate::types::ETH_TESTS_REPO_PATH;

pub(crate) fn run_cmd_no_output(cmd: &mut Command) -> anyhow::Result<()> {
    run_cmd_common(cmd).map(|_| ())
}

pub(crate) fn run_cmd(cmd: &mut Command) -> anyhow::Result<String> {
    let res = run_cmd_common(cmd)?;
    String::from_utf8(res.stdout).with_context(|| "Converting stdout into a UTF8 string")
}

fn run_cmd_common(cmd: &mut Command) -> anyhow::Result<Output> {
    let output = cmd.output().with_context(|| executing_cmd_ctx_str(cmd))?;

    if !output.status.success() {
        let stderr_string = String::from_utf8(output.stderr)?;
        bail!(
            "Got the following error: {} from {}",
            stderr_string,
            executing_cmd_ctx_str(cmd)
        );
    }

    Ok(output)
}

fn executing_cmd_ctx_str(cmd: &Command) -> String {
    format!(
        "Executing the cmd {:?} {:?}",
        cmd.get_program(),
        cmd.get_args()
    )
}

pub(crate) fn check_that_required_tools_are_installed() -> anyhow::Result<()> {
    todo!()
}

pub(crate) fn get_entries_of_dir(dir_path: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(dir_path)
        .unwrap_or_else(|_| panic!("Failed to read files in the directory {:?}", dir_path))
        .map(|entry| {
            entry
                .expect("Error when getting DirEntry from fs::read_dir")
                .path()
        })
}

pub(crate) fn open_file_with_context(path: &Path) -> anyhow::Result<File> {
    File::open(path).with_context(|| format!("Errored on opening an expected file: {:?}", path))
}

/// Get the parsed output path for a given test input.
pub(crate) fn get_parsed_test_path_for_eth_test_path(eth_test_path: &Path) -> PathBuf {
    let mut parsed_path = PathBuf::new();
    parsed_path.push(PARSED_TESTS_PATH);
    parsed_path.push(eth_test_path.strip_prefix(ETH_TESTS_REPO_PATH).unwrap());

    parsed_path
}

pub(crate) fn is_even<T: PrimInt + BitAnd<Output = T>>(num: T) -> bool {
    (num & T::one()) == T::zero()
}

/// Run keccak256 on a Ethereum address to get a U256 hash.
pub(crate) fn keccak_eth_addr(_addr: Address) -> U256 {
    todo!()
}
