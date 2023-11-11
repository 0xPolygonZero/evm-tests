//! Module responsible for converting deserialized json tests into
//! plonky2 generation inputs.
//!
//! In other words
//! ```ignore
//! crate::deserialize::TestBody -> plonky2_evm::generation::GenerationInputs
//! ```
use std::borrow::Borrow;
use std::collections::HashMap;

use smt_utils::account::Account;

use anyhow::Result;
use common::{
    config::ETHEREUM_CHAIN_ID,
    types::{ExpectedFinalRoots, Plonky2ParsedTest, TestMetadata},
};
use eth_trie_utils::{
    nibbles::Nibbles,
    partial_trie::{HashedPartialTrie, PartialTrie},
};
use ethereum_types::{Address, H160, H256, U256};
use keccak_hash::keccak;
use plonky2_evm::{generation::TrieInputs, proof::BlockMetadata};
use rlp::Encodable;
use rlp_derive::{RlpDecodable, RlpEncodable};
use smt_utils::bits::Bits;
use smt_utils::smt::{Smt, ValOrHash};

use crate::deserialize::{Block, PreAccount, TestBody};

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

        let storage_smts = Self::get_storage_smts(self.pre.iter());
        let state_smt = Self::get_state_smt(self.pre.iter(), &storage_smts);

        let tries = TrieInputs {
            state_smt: state_smt.serialize(),
            transactions_trie: HashedPartialTrie::default(),
            receipts_trie: HashedPartialTrie::default(),
        };

        let contract_code: HashMap<_, _> = self
            .pre
            .values()
            .map(|pre| (hash(&pre.code.0), pre.code.0.clone()))
            .collect();

        let header = &block.block_header;

        let post_storage_smts = Self::get_storage_smts(self.post.iter());
        let post_state_smt = Self::get_state_smt(self.post.iter(), &post_storage_smts);

        let addresses = self.pre.keys().copied().collect::<Vec<Address>>();

        let plonky2_metadata = TestMetadata {
            tries,
            contract_code,
            genesis_state_root: self.genesis_block.block_header.state_root,
            block_metadata: self.block.block_metadata(),
            addresses,
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
                state_root_hash: post_state_smt.root,
                txn_trie_root_hash: header.transactions_trie,
                receipts_trie_root_hash: header.receipt_trie,
            },
            plonky2_metadata,
        }
    }

    fn get_storage_smts<'a, I>(accounts: I) -> Vec<(H256, Smt)> where I: IntoIterator<Item=(&'a H160, &'a PreAccount)> {
        accounts
            .into_iter()
            .map(|(acc_key, pre_acc)| {
                let storage_smt = pre_acc
                    .storage
                    .iter()
                    .filter(|(_, v)| !v.is_zero())
                    .map(|(k, v)| {
                        (
                            Bits::from(hash(&u256_to_be_bytes(*k))),
                            ValOrHash::from(*v)
                        )
                    });
                let storage_smt = Smt::new(storage_smt).unwrap();

                (hash(acc_key.as_bytes()), storage_smt)
            })
            .collect()
    }

    fn get_state_smt<'a, I>(accounts: I, storage_tries: &[(H256, Smt)]) -> Smt where I: IntoIterator<Item=(&'a H160, &'a PreAccount)>  {
        let accs = accounts
            .into_iter()
            .map(|(acc_key, pre_acc)| {
                let addr_hash = hash(acc_key.as_bytes());
                let code_hash = hash(&pre_acc.code.0);
                let storage_smt = get_storage_hash(&addr_hash, storage_tries);

                let account = Account {
                    nonce: pre_acc.nonce,
                    balance: pre_acc.balance,
                    code_hash,
                    storage_smt,
                };

                (addr_hash.into(), account.into())
            });
        Smt::new(accs).unwrap()
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
    storage_tries: &[(H256, Smt)],
) -> Smt {
    storage_tries
        .iter()
        .find(|(addr, _)| hashed_account_address == addr)
        .unwrap()
        .1.clone()
}

fn u256_to_be_bytes(x: U256) -> [u8; 32] {
    let mut bytes = [0; 32];
    x.to_big_endian(&mut bytes);
    bytes
}

fn hash(bytes: &[u8]) -> H256 {
    H256::from(keccak(bytes).0)
}
