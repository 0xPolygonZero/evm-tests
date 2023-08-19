//! Utils to clone and pull the eth test repo.

use std::{fs, path::Path, process::Command};

use crate::{
    config::{ETH_TESTS_REPO_LOCAL_PATH, ETH_TESTS_REPO_URL, SPECIAL_TEST_SUBGROUPS, TEST_GROUPS},
    fs_scaffolding::get_test_group_dirs,
    utils::run_cmd,
};

pub(crate) fn clone_or_update_remote_tests() {
    if Path::new(&ETH_TESTS_REPO_LOCAL_PATH).exists() {
        update_remote_tests();
    } else {
        download_remote_tests();
    }

    // Flatten special folders before parsing test files
    flatten_special_folders();
}

#[allow(clippy::permissions_set_readonly_false)]
fn flatten_special_folders() {
    let dirs = get_test_group_dirs()
        .unwrap()
        .flat_map(|entry| fs::read_dir(entry.path()).unwrap())
        .flatten()
        .filter(|entry| match entry.file_name().to_str() {
            Some(file_name) => SPECIAL_TEST_SUBGROUPS.contains(&file_name),
            None => false,
        });

    dirs.for_each(|d| {
        let subdirs = fs::read_dir(d.path())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_type().unwrap().is_dir());

        for sd in subdirs {
            let new_folder_path = d.path();

            fs::read_dir(sd.path())
                .unwrap()
                .flatten()
                .filter(|entry| match entry.path().extension() {
                    None => false,
                    Some(ext) => ext == "json",
                })
                .for_each(|f| {
                    let mut new_path = new_folder_path.clone();

                    // Give write access
                    let mut permissions = new_path.metadata().unwrap().permissions();
                    permissions.set_readonly(false);
                    fs::set_permissions(new_path.clone(), permissions).unwrap();

                    new_path.push(f.file_name().into_string().unwrap().as_str());

                    fs::copy(f.path(), new_path).unwrap();
                })
        }
    });
}

fn update_remote_tests() {
    println!("Pulling for the most recent changes for the Ethereum tests repo...");
    run_cmd(
        Command::new("git")
            .arg("pull")
            .current_dir(ETH_TESTS_REPO_LOCAL_PATH),
    )
    .unwrap();
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
