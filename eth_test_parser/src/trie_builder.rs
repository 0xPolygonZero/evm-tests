//! Module responsible for converting deserialized json tests into
//! plonky2 generation inputs.
//!
//! In other words
//! ```ignore
//! crate::deserialize::TestBody -> plonky2_evm::generation::GenerationInputs
//! ```
use std::{
    collections::HashMap,
    fs::{DirEntry, File},
    io::BufReader,
};

use anyhow::{anyhow, Result};
use eth_trie_utils::partial_trie::{Nibbles, PartialTrie};
use ethereum_types::{H160, H256, U256};
use keccak_hash::keccak;
use plonky2_evm::{
    generation::{GenerationInputs, TrieInputs},
    proof::BlockMetadata,
};
use rlp::Encodable;
use rlp_derive::{RlpDecodable, RlpEncodable};

use crate::{
    deserialize::{Env, PreAccount, TestBody},
    fs_scaffolding::get_test_files,
};

#[derive(RlpDecodable, RlpEncodable)]
pub(crate) struct AccountRlp {
    nonce: U256,
    balance: U256,
    storage_hash: H256,
    code_hash: H256,
}

/// Generate an iterator containing the deserialized test bodies (`TestBody`)
/// and their `DirEntry`s.
pub(crate) fn get_deserialized_test_bodies(
) -> Result<impl Iterator<Item = Result<(DirEntry, TestBody), (String, String)>>> {
    Ok(get_test_files()?.map(|entry| {
        let test_body = get_deserialized_test_body(&entry)
            .map_err(|err| (err.to_string(), entry.path().to_string_lossy().to_string()))?;
        Ok((entry, test_body))
    }))
}

fn get_deserialized_test_body(entry: &DirEntry) -> Result<TestBody> {
    let buf = BufReader::new(File::open(entry.path())?);
    let file_json: HashMap<String, TestBody> = serde_json::from_reader(buf)?;

    // Each test JSON always contains a single outer key containing the test name.
    // The test name is irrelevant for deserialization purposes, so we always drop
    // it.
    let test_body = file_json
        .into_values()
        .next()
        .ok_or_else(|| anyhow!("Empty test found: {:?}", entry))?;

    anyhow::Ok(test_body)
}

impl Env {
    fn block_metadata(self) -> BlockMetadata {
        BlockMetadata {
            block_beneficiary: self.current_coinbase,
            block_timestamp: self.current_timestamp,
            block_number: self.current_number,
            block_difficulty: self.current_difficulty,
            block_gaslimit: self.current_gas_limit,
            block_chain_id: 137.into(), // Matic's Chain id.
            block_base_fee: self.current_base_fee,
        }
    }
}

impl TestBody {
    pub fn into_generation_inputs(self) -> GenerationInputs {
        let storage_tries = self.get_storage_tries();
        let state_trie = self.get_state_trie(&storage_tries);

        let tries = TrieInputs {
            state_trie,
            transactions_trie: PartialTrie::Empty, /* TODO: Change to self.get_txn_trie() once
                                                    * zkEVM supports it */
            receipts_trie: PartialTrie::Empty, // TODO: Fill in once we know what we are doing...
            storage_tries,
        };

        let contract_code: HashMap<_, _> = self
            .pre
            .into_iter()
            .filter(|(_, pre)| pre.code.0.is_empty())
            .map(|(_, pre)| (hash(&pre.code.0), pre.code.0.clone()))
            .collect();

        let signed_txns: Vec<Vec<_>> = self.post.merge.into_iter().map(|x| x.txbytes.0).collect();

        GenerationInputs {
            signed_txns,
            tries,
            contract_code,
            block_metadata: self.env.block_metadata(),
        }
    }

    fn get_storage_tries(&self) -> Vec<(H160, PartialTrie)> {
        self.pre
            .iter()
            .filter(|(_, pre_acc)| !pre_acc.code.0.is_empty())
            .map(|(acc_key, pre_acc)| {
                let storage_trie = pre_acc
                    .storage
                    .iter()
                    .map(|(k, v)| {
                        (
                            Nibbles::from_h256_be(hash(&u256_to_be_bytes(*k))),
                            v.rlp_bytes().to_vec(),
                        )
                    })
                    .collect();

                (*acc_key, storage_trie)
            })
            .collect()
    }

    fn get_state_trie(&self, storage_tries: &[(H160, PartialTrie)]) -> PartialTrie {
        self.pre
            .iter()
            .map(|(acc_key, pre_acc)| {
                let addr_hash = hash(acc_key.as_bytes());
                let (code_hash, storage_hash) =
                    get_pre_account_hashes(acc_key, pre_acc, storage_tries);

                let rlp = AccountRlp {
                    nonce: pre_acc.nonce,
                    balance: pre_acc.balance,
                    storage_hash,
                    code_hash,
                }
                .rlp_bytes();

                (Nibbles::from_h256_be(addr_hash), rlp.to_vec())
            })
            .collect()
    }

    #[allow(unused)] // TODO: Will be used later.
    fn get_txn_trie(&self) -> PartialTrie {
        self.post
            .merge
            .iter()
            .enumerate()
            .map(|(txn_idx, post)| {
                (
                    Nibbles::from_bytes_be(&txn_idx.to_be_bytes()).unwrap(),
                    post.txbytes.0.clone(),
                )
            })
            .collect()
    }
}

impl From<TestBody> for GenerationInputs {
    fn from(test_body: TestBody) -> Self {
        test_body.into_generation_inputs()
    }
}

fn get_pre_account_hashes(
    account_address: &H160,
    account: &PreAccount,
    storage_tries: &[(H160, PartialTrie)],
) -> (H256, H256) {
    match account.code.0.is_empty() {
        false => (
            hash(&account.code.0),
            storage_tries
                .iter()
                .find(|(addr, _)| account_address == addr)
                .unwrap()
                .1
                .calc_hash(),
        ),
        true => (H256::zero(), H256::zero()),
    }
}

fn u256_to_be_bytes(x: U256) -> [u8; 32] {
    let mut bytes = [0; 32];
    x.to_big_endian(&mut bytes);
    bytes
}

fn hash(bytes: &[u8]) -> H256 {
    H256::from(keccak(bytes).0)
}
