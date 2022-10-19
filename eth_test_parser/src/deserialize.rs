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
pub(crate) struct ByteString(#[serde(with = "serde_bytes")] pub(crate) Vec<u8>);

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Env {
    pub(crate) current_base_fee: U256,
    pub(crate) current_coinbase: H160,
    pub(crate) current_difficulty: U256,
    pub(crate) current_gas_limit: U256,
    pub(crate) current_number: U256,
    pub(crate) current_random: U256,
    pub(crate) current_timestamp: U256,
    pub(crate) previous_hash: H256,
}

#[derive(Deserialize, Debug)]
pub(crate) struct PostStateIndexes {
    pub(crate) data: u64,
    pub(crate) gas: u64,
    pub(crate) value: u64,
}

#[derive(Deserialize, Debug)]
pub(crate) struct PostState {
    pub(crate) hash: H256,
    pub(crate) indexes: PostStateIndexes,
    pub(crate) logs: H256,
    pub(crate) txbytes: ByteString,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Post {
    pub(crate) merge: Vec<PostState>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct PreAccount {
    pub(crate) balance: U256,
    pub(crate) code: ByteString,
    pub(crate) nonce: U256,
    pub(crate) storage: HashMap<U256, U256>,
}

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Transaction {
    pub(crate) data: Vec<ByteString>,
    pub(crate) gas_limit: Vec<U256>,
    pub(crate) gas_price: Option<U256>,
    pub(crate) nonce: U256,
    pub(crate) secret_key: H256,
    pub(crate) sender: H160,
    #[serde_as(as = "NoneAsEmptyString")]
    pub(crate) to: Option<H160>,
    // Protect against overflow.
    #[serde(deserialize_with = "vec_eth_big_int_u512")]
    pub(crate) value: Vec<U512>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct TestBody {
    pub(crate) env: Env,
    pub(crate) post: Post,
    pub(crate) transaction: Transaction,
    pub(crate) pre: HashMap<H160, PreAccount>,
}
