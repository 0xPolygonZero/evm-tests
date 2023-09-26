#![allow(dead_code)]
use std::str::FromStr;
use std::{collections::HashMap, marker::PhantomData};

use anyhow::Result;
use ethereum_types::{Address, H160, H256, U256, U512};
use hex::FromHex;
use serde::de::MapAccess;
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
/// additionally pads the value with a U512 to catch overflow.
fn eth_big_int_u512<'de, D>(deserializer: D) -> Result<U512, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    const BIG_INT_PREFIX: &str = "0x:bigint ";

    U512::from_str(s.strip_prefix(BIG_INT_PREFIX).unwrap_or(&s)).map_err(D::Error::custom)
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

fn bloom_array_from_hex<'de, D>(deserializer: D) -> Result<[U256; 8], D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    let mut bloom = [U256::zero(); 8];

    for (b, c) in bloom
        .iter_mut()
        .zip(s[2..].chars().collect::<Vec<char>>().chunks(64))
    {
        *b = U256::from_str_radix(&c.iter().collect::<String>(), 16).map_err(D::Error::custom)?;
    }

    Ok(bloom)
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockHeader {
    #[serde(default)]
    pub(crate) base_fee_per_gas: Option<U256>,
    #[serde(deserialize_with = "bloom_array_from_hex")]
    pub(crate) bloom: [U256; 8],
    pub(crate) coinbase: H160,
    pub(crate) difficulty: U256,
    pub(crate) gas_limit: U256,
    pub(crate) gas_used: U256,
    pub(crate) number: U256,
    pub(crate) mix_hash: H256,
    pub(crate) receipt_trie: H256,
    pub(crate) state_root: H256,
    pub(crate) transactions_trie: H256,
    pub(crate) timestamp: U256,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockHeaderOnlyRoot {
    pub(crate) state_root: H256,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Block {
    pub(crate) block_header: Option<BlockHeader>,
    pub(crate) rlp: Rlp,
    pub(crate) transactions: Option<Vec<Transaction>>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Rlp(pub(crate) ByteString);

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
    #[serde(default)]
    pub(crate) storage_keys: Vec<U256>,
}

#[serde_as]
#[derive(Deserialize, Debug, Default)]
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
    pub(crate) data: ByteString,
    pub(crate) max_fee_per_gas: Option<U256>,
    pub(crate) max_priority_fee_per_gas: Option<U256>,
    #[serde(deserialize_with = "u64_from_hex")]
    pub(crate) gas_limit: u64,
    pub(crate) gas_price: Option<U256>,
    pub(crate) nonce: U256,
    pub(crate) sender: H160,
    #[serde_as(as = "NoneAsEmptyString")]
    pub(crate) to: Option<H160>,
    pub(crate) r: U256,
    pub(crate) s: U256,
    pub(crate) v: U256,
    // Protect against overflow.
    #[serde(deserialize_with = "eth_big_int_u512")]
    pub(crate) value: U512,
}

#[derive(Debug)]
pub(crate) struct TestBodyCompact(pub(crate) Vec<TestBody>);

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestBody {
    pub(crate) blocks: Vec<Block>,
    pub(crate) genesis_block_header: BlockHeaderOnlyRoot,
    pub(crate) pre: HashMap<H160, PreAccount>,
}

// Wrapper around a regular `HashMap` used to conveniently skip
// non-Shanghai related tests when deserializing.
#[derive(Default, Debug)]
pub(crate) struct TestFile(pub(crate) HashMap<String, TestBody>);

impl<'de> Deserialize<'de> for TestFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TestFileVisitor {
            marker: PhantomData<fn() -> TestFile>,
        }

        impl TestFileVisitor {
            fn new() -> Self {
                TestFileVisitor {
                    marker: PhantomData,
                }
            }
        }

        impl<'de> Visitor<'de> for TestFileVisitor {
            // The type that our Visitor is going to produce.
            type Value = TestFile;

            // Format a message stating what data this Visitor expects to receive.
            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("a very special map")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut map = TestFile(HashMap::with_capacity(access.size_hint().unwrap_or(0)));

                // While there are entries remaining in the input, add them
                // into our map if they contain `Shanghai` in their key name.
                while let Some(key) = access.next_key::<String>()? {
                    if key.contains("Shanghai") {
                        // Remove the needless suffix.
                        let value = access.next_value::<TestBody>()?;
                        // Some tests have no transactions in the clear, only the txn RLP.
                        // TODO: handle those tests through txn RLP decoding + sender recovery from
                        // signature fields.
                        if value.blocks[0].transactions.is_some() {
                            map.0.insert(key, value);
                        }
                    } else {
                        let _ = access.next_value::<TestBody>()?;
                    }
                }

                Ok(map)
            }
        }

        deserializer.deserialize_map(TestFileVisitor::new())
    }
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
