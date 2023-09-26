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
    config::ETHEREUM_CHAIN_ID,
    types::{ExpectedFinalRoots, Plonky2ParsedTest, TestMetadata},
};
use eth_trie_utils::{
    nibbles::Nibbles,
    partial_trie::{HashedPartialTrie, PartialTrie},
};
use ethereum_types::{Address, H256, U256};
use keccak_hash::keccak;
use plonky2_evm::{
    generation::{
        mpt::{
            AccessListItemRlp, AccessListTransactionRlp, AddressOption, FeeMarketTransactionRlp,
            LegacyTransactionRlp,
        },
        TrieInputs,
    },
    proof::BlockMetadata,
};
use rlp::Encodable;
use rlp_derive::{RlpDecodable, RlpEncodable};

use crate::deserialize::{AccessListsInner, Block, TestBody};

#[derive(RlpDecodable, RlpEncodable)]
pub(crate) struct AccountRlp {
    nonce: u64,
    balance: U256,
    storage_hash: H256,
    code_hash: H256,
}

impl Block {
    fn block_metadata(&self) -> BlockMetadata {
        let header = self.block_header.clone().unwrap_or_default();
        BlockMetadata {
            block_beneficiary: header.coinbase,
            block_timestamp: header.timestamp,
            block_number: header.number,
            block_difficulty: header.difficulty,
            block_gaslimit: header.gas_limit,
            block_chain_id: ETHEREUM_CHAIN_ID.into(),
            block_base_fee: header.base_fee_per_gas.unwrap_or_default(),
            block_random: header.mix_hash,
            block_gas_used: header.gas_used,
            block_bloom: header.bloom,
        }
    }
}

impl TestBody {
    pub fn as_plonky2_test_inputs(&self) -> Plonky2ParsedTest {
        let block = &self.blocks[0];

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

        let header = block.block_header.clone().unwrap_or_default();

        let addresses = self.pre.keys().copied().collect::<Vec<Address>>();

        let plonky2_metadata = TestMetadata {
            tries,
            contract_code,
            genesis_state_root: self.genesis_block_header.state_root,
            block_metadata: self.blocks[0].block_metadata(),
            addresses,
        };

        Plonky2ParsedTest {
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

    // #[allow(unused)] // TODO: Will be used later.
    // fn get_txn_trie(&self) -> HashedPartialTrie {
    //     self.post
    //         .0
    //         .iter()
    //         .enumerate()
    //         .map(|(txn_idx, post)| {
    //             (
    //                 Nibbles::from_bytes_be(&txn_idx.to_be_bytes()).unwrap(),
    //                 post.txbytes.0.clone(),
    //             )
    //         })
    //         .collect()
    // }

    fn get_txn_bytes(&self) -> Vec<u8> {
        let transaction = &self.blocks[0].transactions.as_ref().unwrap()[0];
        match transaction.max_priority_fee_per_gas {
            None => {
                if transaction.access_lists.is_empty() {
                    // Try legacy transaction, and check
                    let txn = LegacyTransactionRlp {
                        nonce: transaction.nonce,
                        gas_price: transaction.gas_price.unwrap_or_default(),
                        gas: transaction.gas_limit.into(),
                        to: AddressOption(transaction.to),
                        value: transaction.value.try_into().unwrap(),
                        data: transaction.data.0.clone().into(),
                        v: transaction.v,
                        r: transaction.r,
                        s: transaction.s,
                    };

                    rlp::encode(&txn).to_vec()
                } else {
                    let txn = AccessListTransactionRlp {
                        access_list: transaction
                            .access_lists
                            .get(0)
                            .unwrap_or(&AccessListsInner::default())
                            .0
                            .iter()
                            .map(|l| AccessListItemRlp {
                                address: l.address,
                                storage_keys: l.storage_keys.clone(),
                            })
                            .collect(),
                        chain_id: ETHEREUM_CHAIN_ID,
                        nonce: transaction.nonce,
                        gas_price: transaction.gas_price.unwrap_or_default(),
                        gas: transaction.gas_limit.into(),
                        to: AddressOption(transaction.to),
                        value: transaction.value.try_into().unwrap(),
                        data: transaction.data.0.clone().into(),
                        y_parity: transaction.v,
                        r: transaction.r,
                        s: transaction.s,
                    };

                    let rlp = rlp::encode(&txn).to_vec();
                    let mut output = Vec::with_capacity(rlp.len() + 1);
                    output.push(0x01);
                    output.extend(&rlp);

                    output
                }
            }
            Some(max_priority_fee_per_gas) => {
                // Type 2 (FeeMarket) transaction
                let txn = FeeMarketTransactionRlp {
                    access_list: transaction
                        .access_lists
                        .get(0)
                        .unwrap_or(&AccessListsInner::default())
                        .0
                        .iter()
                        .map(|l| AccessListItemRlp {
                            address: l.address,
                            storage_keys: l.storage_keys.clone(),
                        })
                        .collect(),
                    chain_id: ETHEREUM_CHAIN_ID,
                    nonce: transaction.nonce,
                    max_priority_fee_per_gas,
                    max_fee_per_gas: transaction.max_fee_per_gas.unwrap(),
                    gas: transaction.gas_limit.into(),
                    to: AddressOption(transaction.to),
                    value: transaction.value.try_into().unwrap(),
                    data: transaction.data.0.clone().into(),
                    y_parity: transaction.v,
                    r: transaction.r,
                    s: transaction.s,
                };

                let rlp = rlp::encode(&txn).to_vec();
                let mut output = Vec::with_capacity(rlp.len() + 1);
                output.push(0x02);
                output.extend(&rlp);

                output
            }
        }
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
