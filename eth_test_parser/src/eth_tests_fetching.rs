//! Utils to clone and pull the eth test repo.

use std::{path::Path, process::Command};

use crate::{types::ETH_TESTS_REPO_PATH, utils::run_cmd_no_output};

const ETH_TESTS_REPO_URL: &str = "https://github.com/ethereum/tests.git";

pub(crate) fn update_eth_tests_upstream() -> anyhow::Result<()> {
    if !eth_test_repo_exists() {
        return clone_eth_test_repo();
    }

    // Repo already exists. Try pulling to see if there are any new changes.
    pull_repo()?;

    Ok(())
}

fn eth_test_repo_exists() -> bool {
    Path::new(&format!("{}.git", ETH_TESTS_REPO_PATH)).exists()
}

fn clone_eth_test_repo() -> anyhow::Result<()> {
    println!("Cloning Ethereum tests repo... ({})", ETH_TESTS_REPO_URL);
    run_cmd_no_output(Command::new("git").args(["clone", ETH_TESTS_REPO_URL, ETH_TESTS_REPO_PATH]))
}

fn pull_repo() -> anyhow::Result<()> {
    println!("Pulling for the most recent changes for the Ethereum tests repo...");
    run_cmd_no_output(Command::new("git").args(["-C", ETH_TESTS_REPO_PATH, "pull"]))
}
