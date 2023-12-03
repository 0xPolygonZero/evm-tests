pub(crate) const ETH_TESTS_REPO_URL: &str = "https://github.com/ethereum/tests.git";
pub(crate) const ETH_TESTS_REPO_LOCAL_PATH: &str = "eth_tests";
pub(crate) const GENERAL_GROUP: &str = "BlockchainTests";
pub(crate) const TEST_GROUPS: [&str; 1] = ["GeneralStateTests"];
// The following subgroups contain subfolders unlike the other test folders.
pub(crate) const SPECIAL_TEST_SUBGROUPS: [&str; 3] = ["Cancun", "Shanghai", "VMTests"];
