//! Module responsible for converting deserialized json tests into
//! plonky2 generation inputs.
//!
//! In other words
//! ```ignore
//! crate::deserialize::TestBody -> plonky2_evm::generation::GenerationInputs
//! ```
use std::collections::HashMap;

use anyhow::Result;
use common::{
    config,
    types::{ConstGenerationInputs, Plonky2ParsedTest, TestVariant, TestVariantCommon},
};
use eth_trie_utils::{
    nibbles::Nibbles,
    partial_trie::{HashedPartialTrie, PartialTrie},
};
use ethereum_types::{Address, H256, U256};
use keccak_hash::keccak;
use plonky2_evm::{generation::TrieInputs, proof::BlockMetadata};
use rlp::Encodable;
use rlp_derive::{RlpDecodable, RlpEncodable};

use crate::deserialize::{Env, TestBody};

#[derive(RlpDecodable, RlpEncodable)]
pub(crate) struct AccountRlp {
    nonce: u64,
    balance: U256,
    storage_hash: H256,
    code_hash: H256,
}

impl Env {
    fn block_metadata(&self) -> BlockMetadata {
        BlockMetadata {
            block_beneficiary: self.current_coinbase,
            block_timestamp: self.current_timestamp,
            block_number: self.current_number,
            block_difficulty: self.current_difficulty,
            block_gaslimit: self.current_gas_limit,
            block_chain_id: config::MATIC_CHAIN_ID.into(),
            block_base_fee: self.current_base_fee,
        }
    }
}

impl TestBody {
    pub fn as_plonky2_test_input(&self) -> Plonky2ParsedTest {
        let storage_tries = self.get_storage_tries();
        let state_trie = self.get_state_trie(&storage_tries);

        let tries = TrieInputs {
            state_trie,
            transactions_trie: HashedPartialTrie::default(), /* TODO: Change to
                                                              * self.get_txn_trie()
                                                              * once
                                                              * zkEVM supports it */
            receipts_trie: HashedPartialTrie::default(), /* TODO: Fill in once we know what we
                                                          * are
                                                          * doing... */
            storage_tries,
        };

        let contract_code: HashMap<_, _> = self
            .pre
            .values()
            .map(|pre| (hash(&pre.code.0), pre.code.0.clone()))
            .collect();

        let test_variants = self
            .post
            .shanghai
            .iter()
            .map(|x| TestVariant {
                txn_bytes: x.txbytes.0.clone(),
                common: TestVariantCommon {
                    expected_final_account_state_root_hash: x.hash,
                },
            })
            .collect();

        let addresses = self.pre.keys().copied().collect::<Vec<Address>>();

        let const_plonky2_inputs = ConstGenerationInputs {
            tries,
            contract_code,
            block_metadata: self.env.block_metadata(),
            addresses,
        };

        Plonky2ParsedTest {
            test_variants,
            const_plonky2_inputs,
        }
    }

    fn get_storage_tries(&self) -> Vec<(H256, HashedPartialTrie)> {
        self.pre
            .iter()
            .map(|(acc_key, pre_acc)| {
                let storage_trie = pre_acc
                    .storage
                    .iter()
                    .filter(|(_, v)| !v.is_zero())
                    .map(|(k, v)| {
                        (
                            Nibbles::from_h256_be(hash(&u256_to_be_bytes(*k))),
                            v.rlp_bytes().to_vec(),
                        )
                    })
                    .collect();

                (hash(acc_key.as_bytes()), storage_trie)
            })
            .collect()
    }

    fn get_state_trie(&self, storage_tries: &[(H256, HashedPartialTrie)]) -> HashedPartialTrie {
        self.pre
            .iter()
            .map(|(acc_key, pre_acc)| {
                let addr_hash = hash(acc_key.as_bytes());
                let code_hash = hash(&pre_acc.code.0);
                let storage_hash = get_storage_hash(&addr_hash, storage_tries);

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
    fn get_txn_trie(&self) -> HashedPartialTrie {
        self.post
            .shanghai
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

impl From<TestBody> for Plonky2ParsedTest {
    fn from(test_body: TestBody) -> Self {
        test_body.as_plonky2_test_input()
    }
}

fn get_storage_hash(
    hashed_account_address: &H256,
    storage_tries: &[(H256, HashedPartialTrie)],
) -> H256 {
    storage_tries
        .iter()
        .find(|(addr, _)| hashed_account_address == addr)
        .unwrap()
        .1
        .hash()
}

fn u256_to_be_bytes(x: U256) -> [u8; 32] {
    let mut bytes = [0; 32];
    x.to_big_endian(&mut bytes);
    bytes
}

fn hash(bytes: &[u8]) -> H256 {
    H256::from(keccak(bytes).0)
}
