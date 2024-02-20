//! Module responsible for converting deserialized json tests into
//! plonky2 generation inputs.
//!
//! In other words
//! ```ignore
//! crate::deserialize::TestBody -> evm_arithmetization::generation::GenerationInputs
//! ```
use std::collections::HashMap;

use anyhow::Result;
use common::{
    config::ETHEREUM_CHAIN_ID,
    types::{ExpectedFinalRoots, Plonky2ParsedTest, TestMetadata},
};
use ethereum_types::{H256, U256};
use evm_arithmetization::{generation::TrieInputs, proof::BlockMetadata};
use keccak_hash::keccak;
use mpt_trie::{
    nibbles::Nibbles,
    partial_trie::{HashedPartialTrie, PartialTrie},
};
use rlp::Encodable;
use rlp_derive::{RlpDecodable, RlpEncodable};

use crate::deserialize::{Block, TestBody};

#[derive(RlpDecodable, RlpEncodable)]
pub(crate) struct AccountRlp {
    nonce: u64,
    balance: U256,
    storage_hash: H256,
    code_hash: H256,
}

impl Block {
    fn block_metadata(&self) -> BlockMetadata {
        let header = &self.block_header;
        BlockMetadata {
            block_beneficiary: header.coinbase,
            block_timestamp: header.timestamp,
            block_number: header.number,
            block_difficulty: header.difficulty,
            block_gaslimit: header.gas_limit,
            block_chain_id: ETHEREUM_CHAIN_ID.into(),
            block_base_fee: header.base_fee_per_gas,
            block_random: header.mix_hash,
            block_gas_used: header.gas_used,
            block_bloom: header
                .bloom
                .chunks_exact(32)
                .map(U256::from_big_endian)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        }
    }
}

impl TestBody {
    pub fn as_plonky2_test_inputs(&self) -> Plonky2ParsedTest {
        let block = &self.block;

        let storage_tries = self.get_storage_tries();
        let state_trie = self.get_state_trie(&storage_tries);

        let tries = TrieInputs {
            state_trie,
            transactions_trie: HashedPartialTrie::default(),
            receipts_trie: HashedPartialTrie::default(),
            storage_tries,
        };

        let contract_code: HashMap<_, _> = self
            .pre
            .values()
            .map(|pre| (hash(&pre.code.0), pre.code.0.clone()))
            .collect();

        let header = &block.block_header;

        let plonky2_metadata = TestMetadata {
            tries,
            contract_code,
            genesis_state_root: self.genesis_block.block_header.state_root,
            block_metadata: self.block.block_metadata(),
            withdrawals: block
                .withdrawals
                .iter()
                .map(|w| (w.address, w.amount))
                .collect(),
        };

        Plonky2ParsedTest {
            test_name: self.name.clone(),
            txn_bytes: self.get_txn_bytes(),
            final_roots: ExpectedFinalRoots {
                state_root_hash: header.state_root,
                txn_trie_root_hash: header.transactions_trie,
                receipts_trie_root_hash: header.receipt_trie,
            },
            plonky2_metadata,
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

    pub(crate) fn get_txn_bytes(&self) -> Vec<u8> {
        let transaction = &self.get_tx();
        rlp::encode(transaction).to_vec()
    }
}

impl From<TestBody> for Plonky2ParsedTest {
    fn from(test_body: TestBody) -> Self {
        test_body.as_plonky2_test_inputs()
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
