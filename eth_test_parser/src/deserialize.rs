use std::{collections::HashMap, marker::PhantomData};

use anyhow::Result;
use bytes::Bytes;
use ethereum_types::{H160, H256, U256};
use hex::FromHex;
use plonky2_evm::generation::mpt::transaction_testing::{
    AccessListItemRlp, AccessListTransactionRlp, AddressOption, FeeMarketTransactionRlp,
    LegacyTransactionRlp,
};
use rlp::{Decodable, DecoderError, Rlp};
use rlp_derive::RlpDecodable;
use serde::de::MapAccess;
use serde::{
    de::{Error, Visitor},
    Deserialize, Deserializer,
};
use serde_with::serde_as;

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

/// Helper struct to handle decoding fields that *may* not be present
/// in the RLP string.
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

/// An Ethereum block header that can be RLP decoded.
#[derive(Clone, Debug, Default, RlpDecodable)]
pub(crate) struct BlockHeader {
    pub(crate) _parent_hash: H256,
    pub(crate) _uncle_hash: H256,
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
    pub(crate) _extra_data: Bytes,
    pub(crate) mix_hash: H256,
    // Storing nonce as a `U256` leads to RLP decoding failure for some
    // specific cases. As we are not using the nonce anyway, we can just
    // define it as `Vec<u8>` to be fine all the time.
    pub(crate) _nonce: Vec<u8>,
    pub(crate) base_fee_per_gas: U256,
    pub(crate) _withdrawals_root: FieldOption<H256>,
}

// Some tests store the access list in a way that doesn't respect the specs,
// (i.e. a list of a list) and hence they require a specific handling.
#[derive(Clone, Debug)]
pub enum AccessListInner {
    List(Vec<AccessListItemRlp>),
    Item(AccessListItemRlp),
}

impl Decodable for AccessListInner {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        if rlp.is_empty() {
            return Ok(Self::List(vec![]));
        }
        if rlp.is_list() {
            let bytes = rlp.at(0)?;
            let access_list: Vec<AccessListItemRlp> = bytes.as_list()?;
            return Ok(AccessListInner::List(access_list));
        } else {
            let bytes = rlp.at(0)?;
            let access_list = bytes.as_val::<AccessListItemRlp>()?;
            return Ok(AccessListInner::Item(access_list));
        }
    }
}

// Some tests represent the `transactions` field of their block in the RLP
// string in a way that doesn't respect the specs, (i.e. a single txn not in a
// list) and hence they require a specific handling.
// Additionally, some represent them as a list of encoded txn, which require a
// second layer of RLP decoding.
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

// A custom type-1 txn to handle some edge-cases with the access_list field.
#[derive(RlpDecodable, Debug, Clone)]
pub struct CustomAccessListTransactionRlp {
    pub chain_id: u64,
    pub nonce: U256,
    pub gas_price: U256,
    pub gas: U256,
    pub to: AddressOption,
    pub value: U256,
    pub data: Bytes,
    pub access_list: Vec<AccessListInner>,
    pub y_parity: U256,
    pub r: U256,
    pub s: U256,
}

impl CustomAccessListTransactionRlp {
    fn into_regular(&self) -> AccessListTransactionRlp {
        AccessListTransactionRlp {
            chain_id: self.chain_id,
            nonce: self.nonce,
            gas_price: self.gas_price,
            gas: self.gas,
            to: self.to.clone(),
            value: self.value,
            data: self.data.clone(),
            access_list: self
                .access_list
                .clone()
                .into_iter()
                .flat_map(|x| match x {
                    AccessListInner::List(list) => list,
                    AccessListInner::Item(item) => vec![item.clone()],
                })
                .collect(),
            y_parity: self.y_parity,
            r: self.r,
            s: self.s,
        }
    }
}

// A custom type-2 txn to handle some edge-cases with the access_list field.
#[derive(RlpDecodable, Debug, Clone)]
pub struct CustomFeeMarketTransactionRlp {
    pub chain_id: u64,
    pub nonce: U256,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub gas: U256,
    pub to: AddressOption,
    pub value: U256,
    pub data: Bytes,
    pub access_list: Vec<AccessListInner>,
    pub y_parity: U256,
    pub r: U256,
    pub s: U256,
}

impl CustomFeeMarketTransactionRlp {
    fn into_regular(&self) -> FeeMarketTransactionRlp {
        FeeMarketTransactionRlp {
            chain_id: self.chain_id,
            nonce: self.nonce,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas,
            max_fee_per_gas: self.max_fee_per_gas,
            gas: self.gas,
            to: self.to.clone(),
            value: self.value,
            data: self.data.clone(),
            access_list: self
                .access_list
                .clone()
                .into_iter()
                .flat_map(|x| match x {
                    AccessListInner::List(list) => list,
                    AccessListInner::Item(item) => vec![item.clone()],
                })
                .collect(),
            y_parity: self.y_parity,
            r: self.r,
            s: self.s,
        }
    }
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
            1 => CustomAccessListTransactionRlp::decode(&Rlp::new(&bytes[1..]))
                .map(|tx| Transaction::AccessList(tx.into_regular())),
            2 => CustomFeeMarketTransactionRlp::decode(&Rlp::new(&bytes[1..]))
                .map(|tx| Transaction::FeeMarket(tx.into_regular())),
            _ => LegacyTransactionRlp::decode(&Rlp::new(bytes)).map(Transaction::Legacy),
        }
    }
}

impl Decodable for Transaction {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let first_byte = rlp.as_raw().first().ok_or(DecoderError::RlpInvalidLength)?;
        let attempt = match *first_byte {
            1 => CustomAccessListTransactionRlp::decode(&Rlp::new(&rlp.as_raw()[1..]))
                .map(|tx| Transaction::AccessList(tx.into_regular())),
            2 => CustomFeeMarketTransactionRlp::decode(&Rlp::new(&rlp.as_raw()[1..]))
                .map(|tx| Transaction::FeeMarket(tx.into_regular())),
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

// Only needed for proper RLP decoding
#[derive(Debug, RlpDecodable)]
pub(crate) struct Withdrawal {
    pub(crate) _index: U256,
    pub(crate) _validator_index: U256,
    pub(crate) _address: H160,
    pub(crate) _amount: U256,
}

#[derive(Debug, RlpDecodable)]
pub(crate) struct Block {
    pub(crate) block_header: BlockHeader,
    pub(crate) transactions: Transactions,
    pub(crate) _uncle_headers: Vec<BlockHeader>,
    pub(crate) _withdrawals: Vec<Withdrawal>,
}

#[derive(Debug, RlpDecodable)]
pub(crate) struct GenesisBlock {
    pub(crate) block_header: BlockHeader,
    pub(crate) _transactions: Vec<Transaction>,
    pub(crate) _uncle_headers: Vec<BlockHeader>,
    pub(crate) _withdrawals: Vec<Withdrawal>,
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
            type Value = TestFile;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("a `TestFile` map")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut map = TestFile(HashMap::with_capacity(access.size_hint().unwrap_or(0)));

                // While we are parsing many values, we only care about the ones containing
                // `Shanghai` in their key name.
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
