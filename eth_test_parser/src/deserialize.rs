#![allow(dead_code)]
use std::collections::HashMap;
use std::str::FromStr;

use ethereum_types::{Address, H160, H256, U256, U512};
use hex::FromHex;
use serde::{
    de::{Error, Visitor},
    Deserialize, Deserializer,
};
use serde_with::{serde_as, DefaultOnNull, NoneAsEmptyString};

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
// "self" just points to this module.
pub(crate) struct ByteString(#[serde(with = "self")] pub(crate) Vec<u8>);

// Gross, but there is no Serde crate that can both parse a hex string with a
// prefix and also deserialize from a `Vec<u8>`.
pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    struct PrefixHexStrVisitor();

    impl<'de> Visitor<'de> for PrefixHexStrVisitor {
        type Value = Vec<u8>;

        fn visit_str<E>(self, data: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            FromHex::from_hex(Self::remove_prefix(data)).map_err(Error::custom)
        }

        fn visit_borrowed_str<E>(self, data: &'de str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            FromHex::from_hex(Self::remove_prefix(data)).map_err(Error::custom)
        }

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "a hex encoded string with a prefix")
        }
    }

    impl PrefixHexStrVisitor {
        fn remove_prefix(data: &str) -> &str {
            &data[2..]
        }
    }

    deserializer.deserialize_string(PrefixHexStrVisitor())
}

fn u64_from_hex<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    u64::from_str_radix(&s[2..], 16).map_err(D::Error::custom)
}

fn vec_u64_from_hex<'de, D>(deserializer: D) -> Result<Vec<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Vec<String> = Deserialize::deserialize(deserializer)?;
    s.into_iter()
        .map(|x| u64::from_str_radix(&x[2..], 16).map_err(D::Error::custom))
        .collect::<Result<Vec<_>, D::Error>>()
}

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
    pub(crate) data: usize,
    pub(crate) gas: usize,
    pub(crate) value: usize,
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
    #[serde(deserialize_with = "u64_from_hex")]
    pub(crate) nonce: u64,
    pub(crate) storage: HashMap<U256, U256>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccessList {
    pub(crate) address: Address,
    pub(crate) storage_keys: Vec<U256>,
}

#[serde_as]
#[derive(Deserialize, Debug)]
/// This is a wrapper around a `Vec<AccessList>` that is used to deserialize a
/// `null` into an empty vec.
pub(crate) struct AccessListsInner(
    #[serde_as(deserialize_as = "DefaultOnNull")] pub(crate) Vec<AccessList>,
);

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Transaction {
    #[serde(default)]
    pub(crate) access_lists: Vec<AccessListsInner>,
    pub(crate) data: Vec<ByteString>,
    #[serde(deserialize_with = "vec_u64_from_hex")]
    pub(crate) gas_limit: Vec<u64>,
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

#[cfg(test)]
mod tests {
    use super::ByteString;

    const TEST_HEX_STR: &str = "\"0xf863800a83061a8094095e7baea6a6c7c4c2dfeb977efac326af552d87830186a0801ba0ffb600e63115a7362e7811894a91d8ba4330e526f22121c994c4692035dfdfd5a06198379fcac8de3dbfac48b165df4bf88e2088f294b61efb9a65fe2281c76e16\"";

    #[test]
    fn deserialize_hex_str_works() {
        let byte_str: ByteString = serde_json::from_str(TEST_HEX_STR).unwrap();

        assert_eq!(byte_str.0[0], 0xf8);
        assert_eq!(byte_str.0[1], 0x63);

        assert_eq!(byte_str.0[byte_str.0.len() - 1], 0x16);
        assert_eq!(byte_str.0[byte_str.0.len() - 2], 0x6e);
    }
}
