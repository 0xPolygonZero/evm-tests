use common::config::MAIN_TEST_DIR;

// The PR <https://github.com/ethereum/tests/pull/1380> moved all test versions prior Cancun HF
// to the `LegacyTests` folder instead.
pub(crate) const ETH_TESTS_REPO_URL: &str = "https://github.com/ethereum/legacytests.git";
pub(crate) const ETH_TESTS_REPO_LOCAL_PATH: &str = "eth_tests";
pub(crate) const GENERAL_GROUP: &str = MAIN_TEST_DIR;
pub(crate) const TEST_GROUPS: [&str; 1] = ["GeneralStateTests"];
// The following subgroups contain subfolders unlike the other test folders.
pub(crate) const SPECIAL_TEST_SUBGROUPS: [&str; 2] = ["Shanghai", "VMTests"];

/// These test variants are used for stress testing. As such, they have
/// unrealistic scenarios that go beyond the provable bounds of the zkEVM.
/// Witness generation for these variants is still possible, but takes too
/// much time to be useful and usable in testing occuring regularly.
pub(crate) const UNPROVABLE_VARIANTS: [&str; 17] = [
    "CALLBlake2f_d9g0v0_Shanghai",
    "CALLCODEBlake2f_d9g0v0_Shanghai",
    "Call50000_d0g1v0_Shanghai",
    "Callcode50000_d0g1v0_Shanghai",
    "static_Call50000_d1g0v0_Shanghai",
    "static_Call50000_ecrec_d0g0v0_Shanghai",
    "static_Call50000_ecrec_d1g0v0_Shanghai",
    "static_Call50000_identity2_d0g0v0_Shanghai",
    "static_Call50000_identity2_d1g0v0_Shanghai",
    "static_Call50000_identity_d0g0v0_Shanghai",
    "static_Call50000_identity_d1g0v0_Shanghai",
    "static_Call50000_rip160_d0g0v0_Shanghai",
    "static_Call50000_sha256_d0g0v0_Shanghai",
    "static_Call50000_sha256_d1g0v0_Shanghai",
    "static_Return50000_2_d0g0v0_Shanghai",
    "Return50000_d0g1v0_Shanghai",
    "Return50000_2_d0g1v0_Shanghai",
];
