use std::collections::HashMap;

use ethereum_types::H256;
use plonky2_evm::{
    generation::{GenerationInputs, TrieInputs},
    proof::BlockMetadata,
};
use serde::{Deserialize, Serialize};

/// A parsed Ethereum test that is ready to be fed into `Plonky2`.
///
/// Note that for our runner we break any txn "variants" (see `indexes` under https://ethereum-tests.readthedocs.io/en/latest/test_types/gstate_tests.html#post-section) into separate sub-tests when running. This is because we don't want a single sub-test variant to cause the entire test to fail (we just want the variant to fail).
#[derive(Debug, Deserialize, Serialize)]
pub struct ParsedTest {
    pub test_variants: Vec<TestVariant>,

    /// State that is constant between tests.
    pub const_plonky2_inputs: ConstGenerationInputs,
}

impl ParsedTest {
    /// Construct the actual test variants for the test.
    pub fn get_test_variants(self) -> Vec<TestVariantRunInfo> {
        self.test_variants
            .into_iter()
            .map(|t_var| {
                let gen_inputs = GenerationInputs {
                    signed_txns: vec![t_var.txn_bytes],
                    tries: self.const_plonky2_inputs.tries.clone(),
                    contract_code: self.const_plonky2_inputs.contract_code.clone(),
                    block_metadata: self.const_plonky2_inputs.block_metadata.clone(),
                };

                TestVariantRunInfo {
                    gen_inputs,
                    common: t_var.common,
                }
            })
            .collect()
    }
}

/// A single test that
#[derive(Debug, Deserialize, Serialize)]
pub struct TestVariant {
    /// The txn bytes for each txn in the test.
    pub txn_bytes: Vec<u8>,
    pub common: TestVariantCommon,
}

#[derive(Debug)]
pub struct TestVariantRunInfo {
    pub gen_inputs: GenerationInputs,
    pub common: TestVariantCommon,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TestVariantCommon {
    /// The root hash of the expected final state trie.
    pub expected_final_account_state_root_hash: H256,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConstGenerationInputs {
    pub tries: TrieInputs,
    pub contract_code: HashMap<H256, Vec<u8>>,
    pub block_metadata: BlockMetadata,
}
