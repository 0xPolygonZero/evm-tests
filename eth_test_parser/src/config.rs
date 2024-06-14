use common::config::MAIN_TEST_DIR;

// The PR <https://github.com/ethereum/tests/pull/1380> moved all test versions prior Cancun HF
// to the `LegacyTests` folder instead.
pub(crate) const ETH_TESTS_REPO_URL: &str = "https://github.com/ethereum/legacytests.git";
pub(crate) const ETH_TESTS_REPO_LOCAL_PATH: &str = "eth_tests";
pub(crate) const GENERAL_GROUP: &str = MAIN_TEST_DIR;
pub(crate) const TEST_GROUPS: [&str; 1] = ["GeneralStateTests"];
// The following subgroups contain subfolders unlike the other test folders.
pub(crate) const SPECIAL_TEST_SUBGROUPS: [&str; 2] = ["Shanghai", "VMTests"];
