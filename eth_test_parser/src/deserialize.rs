#![allow(dead_code)]
use std::str::FromStr;
use std::{collections::HashMap, marker::PhantomData};

use anyhow::Result;
use bytes::Bytes;
use ethereum_types::{H160, H256, U256, U512};
use hex::FromHex;
use plonky2_evm::generation::mpt::{
    AccessListTransactionRlp, FeeMarketTransactionRlp, LegacyTransactionRlp,
};
use rlp::{Decodable, DecoderError, Rlp};
use rlp_derive::RlpDecodable;
use serde::de::MapAccess;
use serde::{
    de::{Error, Visitor},
    Deserialize, Deserializer,
};
use serde_with::serde_as;

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

#[derive(Deserialize, Debug, Clone)]
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

#[derive(Clone, Debug, Default)]
pub(crate) struct FieldOption<T: Decodable>(pub(crate) Option<T>);

impl<T: Decodable> Decodable for FieldOption<T> {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        if rlp.is_empty() {
            Ok(FieldOption(None))
        } else {
            Ok(FieldOption(Some(T::decode(rlp)?)))
        }
    }
}

#[derive(Clone, Debug, Default, RlpDecodable)]
pub(crate) struct BlockHeader {
    pub(crate) parent_hash: H256,
    pub(crate) uncle_hash: H256,
    pub(crate) coinbase: H160,
    pub(crate) state_root: H256,
    pub(crate) transactions_trie: H256,
    pub(crate) receipt_trie: H256,
    pub(crate) bloom: Bytes,
    pub(crate) difficulty: U256,
    pub(crate) number: U256,
    pub(crate) gas_limit: U256,
    pub(crate) gas_used: U256,
    pub(crate) timestamp: U256,
    pub(crate) extra_data: Bytes,
    pub(crate) mix_hash: H256,
    // Storing nonce as a `U256` leads to RLP decoding failure for some
    // specific cases. As we are not using the nonce anyway, we can just
    // define it as `Vec<u8>` to be fine all the time.
    pub(crate) nonce: Vec<u8>,
    pub(crate) base_fee_per_gas: U256,
    pub(crate) withdrawals_root: FieldOption<H256>,
}

// Somes tests like `addressOpcodes_d0g0v0_Shanghai` didn't
// encode the transaction list as a list but as an actual,
// single transaction, which requires this hacky enum.
#[derive(Debug)]
pub(crate) enum Transactions {
    List(Vec<Transaction>),
    Item(Transaction),
}

impl Transactions {
    pub(crate) fn get_tx(&self) -> Transaction {
        match self {
            Transactions::List(list) => list[0].clone(),
            Transactions::Item(tx) => tx.clone(),
        }
    }
}

impl Decodable for Transactions {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let bytes = rlp.at(0)?;

        let attempt_txn = bytes.as_val::<Transaction>();
        if let Ok(txn) = attempt_txn {
            return Ok(Transactions::Item(txn));
        }

        let attempt_list: Result<Vec<Transaction>, DecoderError> = bytes.as_list();

        if let Ok(list) = attempt_list {
            if !list.is_empty() {
                return Ok(Transactions::List(list));
            } else {
                let encoded_txn = bytes.as_val::<Vec<u8>>()?;

                return Ok(Transactions::Item(Transaction::decode(&Rlp::new(
                    &encoded_txn,
                ))?));
            }
        }

        Err(DecoderError::Custom("Invalid txn encoding?"))
    }
}

#[derive(Debug, RlpDecodable)]
pub(crate) struct Block {
    pub(crate) block_header: BlockHeader,
    pub(crate) transactions: Transactions,
    pub(crate) uncle_headers: Vec<BlockHeader>,
    pub(crate) withdrawals: Vec<Withdrawal>,
}

#[derive(Debug, RlpDecodable)]
pub(crate) struct GenesisBlock {
    pub(crate) block_header: BlockHeader,
    pub(crate) transactions: Vec<Transaction>,
    pub(crate) uncle_headers: Vec<BlockHeader>,
    pub(crate) withdrawals: Vec<Withdrawal>,
}

#[derive(Debug, RlpDecodable)]
pub(crate) struct Withdrawal {
    pub(crate) index: U256,
    pub(crate) validator_index: U256,
    pub(crate) address: H160,
    pub(crate) amount: U256,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockRlp {
    pub(crate) rlp: ByteString,
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct PreAccount {
    pub(crate) balance: U256,
    pub(crate) code: ByteString,
    #[serde(deserialize_with = "u64_from_hex")]
    pub(crate) nonce: u64,
    pub(crate) storage: HashMap<U256, U256>,
}

#[derive(Clone, Debug)]
pub enum Transaction {
    Legacy(LegacyTransactionRlp),
    AccessList(AccessListTransactionRlp),
    FeeMarket(FeeMarketTransactionRlp),
}

impl Transaction {
    fn decode_actual_rlp(bytes: &[u8]) -> Result<Self, DecoderError> {
        let first_byte = bytes.first().ok_or(DecoderError::RlpInvalidLength)?;
        match *first_byte {
            1 => AccessListTransactionRlp::decode(&Rlp::new(&bytes[1..]))
                .map(Transaction::AccessList),
            2 => {
                FeeMarketTransactionRlp::decode(&Rlp::new(&bytes[1..])).map(Transaction::FeeMarket)
            }
            _ => LegacyTransactionRlp::decode(&Rlp::new(bytes)).map(Transaction::Legacy),
        }
    }
}

impl Decodable for Transaction {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let first_byte = rlp.as_raw().first().ok_or(DecoderError::RlpInvalidLength)?;
        let attempt = match *first_byte {
            1 => AccessListTransactionRlp::decode(&Rlp::new(&rlp.as_raw()[1..]))
                .map(Transaction::AccessList),
            2 => FeeMarketTransactionRlp::decode(&Rlp::new(&rlp.as_raw()[1..]))
                .map(Transaction::FeeMarket),
            _ => LegacyTransactionRlp::decode(rlp).map(Transaction::Legacy),
        };

        // Somes tests have a different format and store the RLP encoding of the
        // transaction, which needs an additional layer of decoding.
        if attempt.is_err() && rlp.as_raw().len() >= 2 {
            return Transaction::decode_actual_rlp(&rlp.as_raw()[2..]);
        }

        attempt
    }
}

#[derive(Debug)]
pub(crate) struct TestBody {
    pub(crate) block: Block,
    // The genesis block has an empty transactions list, which needs a
    // different handling than the logic present in `Block` decoding.
    pub(crate) genesis_block: GenesisBlock,
    pub(crate) pre: HashMap<H160, PreAccount>,
}

impl TestBody {
    fn from_parsed_json(value: &ValueJson) -> Self {
        let block: Block = rlp::decode(&value.blocks[0].rlp.0).unwrap();
        let genesis_block: GenesisBlock =
            rlp::decode(&value.genesis_rlp.as_ref().unwrap().0).unwrap();

        Self {
            block,
            genesis_block,
            pre: value.pre.clone(),
        }
    }

    pub(crate) fn get_tx(&self) -> Transaction {
        self.block.transactions.get_tx()
    }
}

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ValueJson {
    pub(crate) blocks: Vec<BlockRlp>,
    #[serde(rename = "genesisRLP")]
    pub(crate) genesis_rlp: Option<ByteString>,
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
                while let Some((key, value)) = access.next_entry::<String, ValueJson>()? {
                    if key.contains("Shanghai") {
                        let value = TestBody::from_parsed_json(&value);
                        map.0.insert(key, value);
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
