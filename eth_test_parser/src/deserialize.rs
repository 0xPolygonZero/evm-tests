#![allow(dead_code)]
use std::collections::HashMap;
use std::str::FromStr;

use eth_trie_utils::partial_trie::{Nibbles, PartialTrie};
use ethereum_types::{H160, H256, U256, U512};
use keccak_hash::keccak;
use plonky2_evm::{
    generation::{GenerationInputs, TrieInputs},
    proof::BlockMetadata,
};
use rlp::Encodable;
use rlp_derive::{RlpDecodable, RlpEncodable};
use serde::{de::Error, Deserialize, Deserializer};
use serde_with::{serde_as, NoneAsEmptyString};

/// In a couple tests, an entry in the `transaction.value` key will contain
/// the prefix, `0x:bigint`, in addition to containing a value greater than 256
/// bits. This breaks U256 deserialization in two ways:
/// 1. The `0x:bigint` prefix breaks string parsing.
/// 2. The value will be greater than 256 bits.
///
/// This helper takes care of stripping that prefix, if it exists, and
/// additionally pads the value with a U512 to catch overflow. Note that this
/// implementation is specific to a Vec<_>; in the event that this syntax is
/// found to occur more often than this particular instance
/// (`transaction.value`), this logic should be broken out to be modular.
///
/// See [this test](https://github.com/ethereum/tests/blob/develop/GeneralStateTests/stTransactionTest/ValueOverflow.json#L197) for a concrete example.
fn vec_eth_big_int_u512<'de, D>(deserializer: D) -> Result<Vec<U512>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Vec<String> = Deserialize::deserialize(deserializer)?;
    const BIG_INT_PREFIX: &str = "0x:bigint ";

    s.into_iter()
        .map(|s| {
            U512::from_str(s.strip_prefix(BIG_INT_PREFIX).unwrap_or(&s)).map_err(D::Error::custom)
        })
        .collect()
}

#[derive(Deserialize, Debug)]
struct ByteString(#[serde(with = "serde_bytes")] pub Vec<u8>);

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Env {
    current_base_fee: U256,
    current_coinbase: H160,
    current_difficulty: U256,
    current_gas_limit: U256,
    current_number: U256,
    current_random: U256,
    current_timestamp: U256,
    previous_hash: H256,
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

#[derive(Deserialize, Debug)]
struct PostStateIndexes {
    data: u64,
    gas: u64,
    value: u64,
}

#[derive(Deserialize, Debug)]
struct PostState {
    hash: H256,
    indexes: PostStateIndexes,
    logs: H256,
    txbytes: ByteString,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Post {
    merge: Vec<PostState>,
}

#[derive(Deserialize, Debug)]
struct PreAccount {
    balance: U256,
    code: ByteString,
    nonce: U256,
    storage: HashMap<U256, U256>,
}

#[derive(Debug, RlpDecodable, RlpEncodable)]
struct AccountRlp {
    balance: U256,
    nonce: U256,
    code_hash: H256,
    storage_hash: H256,
}

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Transaction {
    data: Vec<ByteString>,
    gas_limit: Vec<U256>,
    gas_price: Option<U256>,
    nonce: U256,
    secret_key: H256,
    sender: H160,
    #[serde_as(as = "NoneAsEmptyString")]
    to: Option<H160>,
    // Protect against overflow.
    #[serde(deserialize_with = "vec_eth_big_int_u512")]
    value: Vec<U512>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct TestBody {
    env: Env,
    post: Post,
    transaction: Transaction,
    pre: HashMap<H160, PreAccount>,
}

impl TestBody {
    fn generation_inputs(self) -> GenerationInputs {
        let storage_tries = self.get_storage_tries();
        let state_trie = self.get_state_trie(&storage_tries);

        let tries = TrieInputs {
            state_trie,
            transactions_trie: self.get_txn_trie(),
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
                let _addr_hash = hash(acc_key.as_bytes());
                let storage_trie = PartialTrie::from_iter(pre_acc.storage.iter().map(|(k, v)| {
                    (
                        Nibbles::from(hash(&u256_to_bytes(*k))),
                        v.rlp_bytes().into(),
                    )
                }));

                (*acc_key, storage_trie)
            })
            .collect()
    }

    fn get_state_trie(&self, storage_tries: &[(H160, PartialTrie)]) -> PartialTrie {
        PartialTrie::from_iter(self.pre.iter().map(|(acc_key, pre_acc)| {
            let addr_hash = hash(acc_key.as_bytes());
            let (code_hash, storage_hash) = get_pre_account_hashes(acc_key, pre_acc, storage_tries);

            let rlp = AccountRlp {
                balance: pre_acc.balance,
                nonce: pre_acc.nonce,
                code_hash,
                storage_hash,
            }
            .rlp_bytes();

            (addr_hash.into(), rlp.into())
        }))
    }

    fn get_txn_trie(&self) -> PartialTrie {
        PartialTrie::from_iter(self.post.merge.iter().enumerate().map(|(txn_idx, post)| {
            (
                Nibbles::from_bytes_be(&txn_idx.to_be_bytes()).unwrap(),
                post.txbytes.0.clone(),
            )
        }))
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

fn u256_to_bytes(x: U256) -> [u8; 32] {
    let mut bytes = [0; 32];
    x.to_big_endian(&mut bytes);
    bytes
}

fn hash(bytes: &[u8]) -> H256 {
    H256::from(keccak(bytes).0)
}
