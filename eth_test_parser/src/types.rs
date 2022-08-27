use chrono::FixedOffset;

pub(crate) type DateTime = chrono::DateTime<FixedOffset>;

pub(crate) const ETH_TESTS_REPO_PATH: &str = "eth_tests/";
pub(crate) const SUB_TEST_DIR_LAST_CHANGED_FILE_NAME: &str = "last_parse_commit_date.txt";
