use std::path::PathBuf;

use git2::Repository;

const TEST_GROUPS: [&str; 1] = ["GeneralStateTests"];

pub(crate) async fn determine_which_tests_need_reparsing(
    _repo: &Repository,
) -> anyhow::Result<Vec<PathBuf>> {
    todo!();
}
