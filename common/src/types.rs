use eth_trie_utils::partial_trie::PartialTrie;
use plonky2_evm::generation::GenerationInputs;
use serde::{Deserialize, Serialize};

pub const PARSED_TESTS_PATH: &str = "parsed_tests";

/// A parsed JSON Ethereum test that is ready to be fed into `Plonky2`.
#[derive(Debug, Deserialize, Serialize)]
pub struct ParsedTest {
    pub plonky2_inputs: GenerationInputs,

    /// If the test specifies a final account trie state, this will be filled.
    pub expected_final_account_states: Option<PartialTrie>,
}
