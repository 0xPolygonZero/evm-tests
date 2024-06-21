//! Module responsible for converting deserialized json tests into
//! plonky2 generation inputs.
//!
//! In other words
//! ```ignore
//! crate::deserialize::TestBody -> evm_arithmetization::generation::GenerationInputs
//! ```
use std::collections::HashMap;

use common::{
    config::ETHEREUM_CHAIN_ID,
    types::{ExpectedFinalRoots, Plonky2ParsedTest, TestMetadata},
};
use ethereum_types::{Address, BigEndianHash, H160, H256, U256};
use evm_arithmetization::{generation::TrieInputs, proof::BlockMetadata};
use mpt_trie::partial_trie::HashedPartialTrie;
use rlp_derive::{RlpDecodable, RlpEncodable};
use smt_trie::code::hash_bytecode_u256;
use smt_trie::db::{Db, MemoryDb};
use smt_trie::keys::{key_balance, key_code, key_code_length, key_nonce, key_storage};
use smt_trie::smt::Smt;
use smt_trie::utils::hashout2u;

use crate::deserialize::{Block, PreAccount, TestBody};

#[derive(RlpDecodable, RlpEncodable)]
pub(crate) struct AccountRlp {
    nonce: U256,
    balance: U256,
    code_hash: U256,
    code_length: U256,
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
            block_blob_gas_used: header.blob_gas_used,
            block_excess_blob_gas: header.excess_blob_gas,
            parent_beacon_block_root: header.parent_beacon_block_root,
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

        let state_smt = Self::get_state_smt(self.pre.iter());

        let tries = TrieInputs {
            state_smt: state_smt.serialize(),
            transactions_trie: HashedPartialTrie::default(),
            receipts_trie: HashedPartialTrie::default(),
        };

        let contract_code: HashMap<_, _> = self
            .pre
            .values()
            .map(|pre| (hash_bytecode_u256(pre.code.0.clone()), pre.code.0.clone()))
            .collect();

        let header = &block.block_header;

        let post_state_smt = Self::get_state_smt(self.post.iter());

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
                state_root_hash: H256::from_uint(&hashout2u(post_state_smt.root)),
                txn_trie_root_hash: header.transactions_trie,
                receipts_trie_root_hash: header.receipt_trie,
            },
            plonky2_metadata,
        }
    }

    fn get_state_smt<'a, I>(accounts: I) -> Smt<MemoryDb>
    where
        I: IntoIterator<Item = (&'a H160, &'a PreAccount)>,
    {
        let mut smt = Smt::<MemoryDb>::default();
        for (acc_key, pre_acc) in accounts {
            let code_hash = hash_bytecode_u256(pre_acc.code.0.clone());
            let storage = pre_acc
                .storage
                .iter()
                .filter(|(_, v)| !v.is_zero())
                .map(|(k, v)| (*k, *v))
                .collect();

            let account = AccountRlp {
                nonce: pre_acc.nonce.into(),
                balance: pre_acc.balance,
                code_hash,
                code_length: pre_acc.code.0.len().into(),
            };

            set_account(&mut smt, *acc_key, &account, &storage);
        }

        smt
    }

    pub(crate) fn get_txn_bytes(&self) -> Vec<u8> {
        self.get_tx().0
    }
}

impl From<TestBody> for Plonky2ParsedTest {
    fn from(test_body: TestBody) -> Self {
        test_body.as_plonky2_test_inputs()
    }
}

fn set_account<D: Db>(
    smt: &mut Smt<D>,
    addr: Address,
    account: &AccountRlp,
    storage: &HashMap<U256, U256>,
) {
    smt.set(key_balance(addr), account.balance);
    smt.set(key_nonce(addr), account.nonce);
    smt.set(key_code(addr), account.code_hash);
    smt.set(key_code_length(addr), account.code_length);
    for (&k, &v) in storage {
        smt.set(key_storage(addr, k), v);
    }
}
