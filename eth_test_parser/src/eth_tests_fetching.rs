use std::{path::Path, process::Command};

use crate::utils::run_cmd;

const ETH_TESTS_REPO_PATH: &str = "eth_tests";
const ETH_TESTS_REPO_URL: &str = "https://github.com/ethereum/tests.git";

pub(crate) fn update_eth_tests_upstream() -> anyhow::Result<()> {
    if !eth_test_repo_exists() {
        clone_eth_test_repo()?;
    }

    // Repo already exists. Try pulling to see if there are any new changes.
    pull_repo()?;

    Ok(())
}

fn eth_test_repo_exists() -> bool {
    Path::new(&format!("{}/.git", ETH_TESTS_REPO_PATH)).exists()
}

fn clone_eth_test_repo() -> anyhow::Result<()> {
    run_cmd(Command::new("git").args(["clone", ETH_TESTS_REPO_URL]))
}

fn pull_repo() -> anyhow::Result<()> {
    run_cmd(Command::new("git").args(["-C", ETH_TESTS_REPO_PATH, "pull"]))
}
