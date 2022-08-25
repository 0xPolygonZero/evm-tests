use anyhow::bail;
use git2::{ErrorCode, Repository};

const ETH_TESTS_REPO_PATH: &str = "eth_tests";

pub(crate) fn update_eth_tests_upstream() -> anyhow::Result<Repository> {
    let repo = match Repository::open(ETH_TESTS_REPO_PATH) {
        Ok(repo) => repo,
        Err(err) => match err.code() {
            ErrorCode::NotFound => clone_eth_test_repo(),
            _ => bail!(
                "Error when attempting to open an existing eth test repo: {}",
                err
            ),
        },
    };

    // Repo already exists. Try pulling to see if there are any new changes.
    pull_repo();

    Ok(repo)
}

fn clone_eth_test_repo() -> Repository {
    todo!()
}

fn pull_repo() {
    todo!()
}
