use ethereum_types::H256;
use plonky2_evm::generation::GenerationInputs;
use serde::{Deserialize, Serialize};

/// A parsed Ethereum test that is ready to be fed into `Plonky2`.
#[derive(Debug, Deserialize, Serialize)]
pub struct ParsedTest {
    pub plonky2_inputs: GenerationInputs,

    /// If the test specifies a final account trie state, this will be filled.
    pub expected_final_account_states: Option<H256>,
}
