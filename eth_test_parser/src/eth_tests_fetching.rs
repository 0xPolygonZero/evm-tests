//! Utils to clone and pull the eth test repo.

use std::{path::Path, process::Command};

use crate::{
    config::{ETH_TESTS_REPO_LOCAL_PATH, ETH_TESTS_REPO_URL, TEST_GROUPS},
    utils::run_cmd,
};

pub(crate) fn clone_or_update_remote_tests() {
    if Path::new(&ETH_TESTS_REPO_LOCAL_PATH).exists() {
        update_remote_tests();
    } else {
        download_remote_tests();
    }
}

fn update_remote_tests() {
    println!("Pulling for the most recent changes for the Ethereum tests repo...");
    run_cmd(Command::new("git").arg("pull")).unwrap();
}

fn download_remote_tests() {
    println!("Cloning Ethereum tests repo... ({})", ETH_TESTS_REPO_URL);

    // Sparse clone the repository with --depth=1. We do this to avoid large
    // download size.
    run_cmd(Command::new("git").args([
        "clone",
        // --depth=1 ignores version history.
        "--depth=1",
        // --sparse employs a sparse-checkout, with only files in the toplevel directory
        // initially being present. The sparse-checkout (below) command is used to
        // grow the working directory as needed.
        "--sparse",
        // --filter=blob:none will filter out all blobs (file contents) until needed by Git
        "--filter=blob:none",
        ETH_TESTS_REPO_URL,
        ETH_TESTS_REPO_LOCAL_PATH,
    ]))
    .unwrap();

    println!(
        "Setting sparse checkout for test groups... ({})",
        TEST_GROUPS.join(", ")
    );
    // sparse-checkout out the relevant test group folders.
    run_cmd(Command::new("git").args([
        "-C",
        ETH_TESTS_REPO_LOCAL_PATH,
        "sparse-checkout",
        "set",
        &TEST_GROUPS.join(" "),
    ]))
    .unwrap();
}
