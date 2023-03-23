use std::{
    collections::HashMap,
    ops::RangeInclusive,
    str::{FromStr, Split},
};

use anyhow::{anyhow, Context};
use ethereum_types::{Address, H256};
use plonky2_evm::{
    generation::{GenerationInputs, TrieInputs},
    proof::BlockMetadata,
};
use serde::{Deserialize, Serialize};

use crate::revm::SerializableEVMInstance;

#[derive(Debug, Deserialize, Serialize)]
pub struct ParsedTestManifest {
    pub plonky2_variants: Plonky2ParsedTest,
    pub revm_variants: Option<Vec<SerializableEVMInstance>>,
}

impl ParsedTestManifest {
    pub fn into_filtered_variants(
        self,
        v_filter: Option<VariantFilterType>,
    ) -> Vec<TestVariantRunInfo> {
        // If `self.revm_variants` is None, the parser was unable to generate an `revm`
        // instance for any test variant. This occurs when some shared test data was
        // unable to be parsed (e.g. the `transaction` section). In this case, we
        // generate a `None` for each test variant slot so that it can be zipped with
        // plonky2 variants.
        let revm_variants: Vec<Option<SerializableEVMInstance>> = match self.revm_variants {
            // `revm_variants` will be parallel to `plonky2_variants`, given they are both
            // generated from the same vec (`test.post.merge`).
            None => (0..self.plonky2_variants.test_variants.len())
                .map(|_| None)
                .collect(),
            Some(v) => v.into_iter().map(Some).collect(),
        };

        self.plonky2_variants
            .test_variants
            .into_iter()
            .zip(revm_variants.into_iter())
            .enumerate()
            .filter(|(variant_idx, _)| match &v_filter {
                Some(VariantFilterType::Single(v)) => variant_idx == v,
                Some(VariantFilterType::Range(r)) => r.contains(variant_idx),
                None => true,
            })
            .map(|(_, (t_var, revm_variant))| {
                let gen_inputs = GenerationInputs {
                    signed_txns: vec![t_var.txn_bytes],
                    tries: self.plonky2_variants.const_plonky2_inputs.tries.clone(),
                    contract_code: self
                        .plonky2_variants
                        .const_plonky2_inputs
                        .contract_code
                        .clone(),
                    block_metadata: self
                        .plonky2_variants
                        .const_plonky2_inputs
                        .block_metadata
                        .clone(),
                    addresses: self.plonky2_variants.const_plonky2_inputs.addresses.clone(),
                };

                TestVariantRunInfo {
                    gen_inputs,
                    common: t_var.common,
                    revm_variant,
                }
            })
            .collect()
    }
}

/// A parsed Ethereum test that is ready to be fed into `Plonky2`.
///
/// Note that for our runner we break any txn "variants" (see `indexes` under https://ethereum-tests.readthedocs.io/en/latest/test_types/gstate_tests.html#post-section) into separate sub-tests when running. This is because we don't want a single sub-test variant to cause the entire test to fail (we just want the variant to fail).
#[derive(Debug, Deserialize, Serialize)]
pub struct Plonky2ParsedTest {
    pub test_variants: Vec<TestVariant>,

    /// State that is constant between tests.
    pub const_plonky2_inputs: ConstGenerationInputs,
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
    pub revm_variant: Option<SerializableEVMInstance>,
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
    pub addresses: Vec<Address>,
}

#[derive(Clone, Debug)]
pub enum VariantFilterType {
    Single(usize),
    Range(RangeInclusive<usize>),
}

impl FromStr for VariantFilterType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_intern(s)
            .with_context(|| {
                format!(
                    "Expected a single value or a range, but instead got \"{}\".",
                    s
                )
            })
            .map_err(|e| format!("{e:#}"))
    }
}

impl VariantFilterType {
    fn from_str_intern(s: &str) -> anyhow::Result<Self> {
        // Did we get passed a single value?
        if let Ok(v) = s.parse::<usize>() {
            return Ok(Self::Single(v));
        }

        // Check if it's a range.
        let mut range_vals = s.split("..=");

        let start = Self::next_and_try_parse(&mut range_vals)?;
        let end = Self::next_and_try_parse(&mut range_vals)?;

        if range_vals.count() > 0 {
            return Err(anyhow!(
                "Parsed a range but there were unexpected characters afterwards!"
            ));
        }

        Ok(Self::Range(start..=end))
    }

    fn next_and_try_parse(range_vals: &mut Split<&str>) -> anyhow::Result<usize> {
        let unparsed_val = range_vals
            .next()
            .with_context(|| "Parsing a value as a `RangeInclusive`")?;
        let res = unparsed_val
            .parse()
            .with_context(|| format!("Parsing the range val \"{unparsed_val}\" into a usize"))?;

        Ok(res)
    }
}
