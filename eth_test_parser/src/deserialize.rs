#![allow(dead_code)]
use std::collections::HashMap;
use std::str::FromStr;

use ethereum_types::{H160, H256, U256, U512};
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
    pre: HashMap<String, PreAccount>,
}
