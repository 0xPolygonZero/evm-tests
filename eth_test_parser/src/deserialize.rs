use std::{collections::HashMap, marker::PhantomData};

use anyhow::Result;
use bytes::Bytes;
use ethereum_types::{Address, H160, H256, U256};
use evm_arithmetization::generation::mpt::transaction_testing::{
    AccessListItemRlp, AccessListTransactionRlp, AddressOption, FeeMarketTransactionRlp,
    LegacyTransactionRlp,
};
use hex::FromHex;
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
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
// and hence they require a specific handling.
#[derive(Clone, Debug, RlpDecodable)]
pub struct AccessItemRlp {
    pub address: Address,
    pub storage_keys: Vec<StorageKey>,
}

#[derive(Clone, Debug)]
pub struct StorageKey(pub U256);

impl Decodable for StorageKey {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        // Decode the key as a `Vec<u8>` to deal with badly encoded scalars,
        // and then convert back to U256.
        let key = rlp.as_val::<Vec<u8>>()?;
        if key.len() == 1 && key[0] == 0x80 {
            return Ok(StorageKey(U256::zero()));
        }

        Ok(StorageKey(U256::from_big_endian(&key)))
    }
}

impl AccessItemRlp {
    fn into_regular(self) -> AccessListItemRlp {
        AccessListItemRlp {
            address: self.address,
            storage_keys: self.storage_keys.iter().map(|k| k.0).collect(),
        }
    }
}

// Some tests represent the `transactions` field of their block in the RLP
// string in a way that doesn't respect the specs, and hence they require a
// specific handling. The different cases are:
// - a regular list of items (i.e. transactions)
// - a single item (i.e. transaction) but not a list
// - a list of strings (i.e. encodings of transactions)
#[derive(Debug)]
pub(crate) struct Transactions(pub(crate) Transaction);

impl Decodable for Transactions {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        if rlp.is_list() {
            let txn = rlp.at(0)?.as_val::<Transaction>()?;
            Ok(Transactions(txn))
        } else {
            let txn = rlp.as_val::<Transaction>()?;
            Ok(Transactions(txn))
        }
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
    pub access_list: Vec<AccessItemRlp>,
    pub y_parity: U256,
    pub r: U256,
    pub s: U256,
}

impl CustomAccessListTransactionRlp {
    fn into_regular(self) -> AccessListTransactionRlp {
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
                .map(|x| x.into_regular())
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
    pub access_list: Vec<AccessItemRlp>,
    pub y_parity: U256,
    pub r: U256,
    pub s: U256,
}

impl CustomFeeMarketTransactionRlp {
    fn into_regular(self) -> FeeMarketTransactionRlp {
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
                .map(|x| x.into_regular())
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

impl Encodable for Transaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self {
            Transaction::Legacy(tx) => s.append(tx),
            Transaction::AccessList(tx) => {
                s.encoder().encode_value(&[0x01]);
                s.append(tx)
            }
            Transaction::FeeMarket(tx) => {
                s.encoder().encode_value(&[0x02]);
                s.append(tx)
            }
        };
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
            let encoded_txn = rlp.as_val::<Vec<u8>>()?;
            return Transaction::decode_actual_rlp(&encoded_txn);
        }

        attempt
    }
}

// Only needed for proper RLP decoding
#[derive(Debug, RlpDecodable)]
pub(crate) struct Withdrawal {
    pub(crate) _index: U256,
    pub(crate) _validator_index: U256,
    pub(crate) address: H160,
    pub(crate) amount: U256,
}

#[derive(Debug, RlpDecodable)]
pub(crate) struct Block {
    pub(crate) block_header: BlockHeader,
    pub(crate) transactions: Transactions,
    pub(crate) _uncle_headers: Vec<BlockHeader>,
    pub(crate) withdrawals: Vec<Withdrawal>,
}

#[derive(Debug, RlpDecodable)]
pub(crate) struct GenesisBlock {
    pub(crate) block_header: BlockHeader,
    pub(crate) _transactions: Vec<Transaction>,
    pub(crate) _uncle_headers: Vec<BlockHeader>,
    pub(crate) _withdrawals: Vec<Withdrawal>,
}

/// Contains the RLP encoding of the block, as well as the `transactionSequence`
/// field (if any) to indicate if this test contains a malformed transaction
/// that *should* be ignored for testing (as all input txns to plonky2 zkEVM are
/// expected to be valid).
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlockRlpWithExceptions {
    pub(crate) rlp: ByteString,
    pub(crate) transaction_sequence: Option<Vec<TransactionSequence>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransactionSequence {
    pub(crate) valid: String,
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
    pub(crate) name: String,
    pub(crate) block: Block,
    // The genesis block has an empty transactions list, which needs a
    // different handling than the logic present in `Block` decoding.
    pub(crate) genesis_block: GenesisBlock,
    pub(crate) pre: HashMap<H160, PreAccount>,
    pub(crate) post: HashMap<H160, PreAccount>,
}

impl TestBody {
    fn from_parsed_json(value: &ValueJson, variant_name: String) -> Self {
        let block: Block = rlp::decode(&value.blocks[0].rlp.0).unwrap();
        let genesis_block: GenesisBlock =
            rlp::decode(&value.genesis_rlp.as_ref().unwrap().0).unwrap();

        Self {
            name: variant_name,
            block,
            genesis_block,
            pre: value.pre.clone(),
            post: value.post_state.clone(),
        }
    }

    pub(crate) fn get_tx(&self) -> Transaction {
        self.block.transactions.0.clone()
    }
}

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ValueJson {
    pub(crate) blocks: Vec<BlockRlpWithExceptions>,
    #[serde(rename = "genesisRLP")]
    pub(crate) genesis_rlp: Option<ByteString>,
    pub(crate) pre: HashMap<H160, PreAccount>,
    pub(crate) post_state: HashMap<H160, PreAccount>,
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
                        if value.blocks[0].transaction_sequence.is_none() {
                            let test_body = TestBody::from_parsed_json(&value, key.clone());

                            // Sanity check: some tests *do not* abide by standard RLP encoding
                            // rules, therefore causing discrepancies between the "expected"
                            // encoding of the transaction that is part of the block RLP and used to
                            // form the transactions trie, and the *regular* encoding computed here
                            // after deserialization and to be fed to plonky2 zkEVM.
                            {
                                let rlp = &value.blocks[0].rlp.0;
                                let encoded_txn = rlp::encode(&test_body.get_tx()).to_vec();
                                // Ensure that the encoding we will provide the zkEVM prover is in
                                // the block RLP.
                                if rlp.windows(encoded_txn.len()).any(|c| c == encoded_txn) {
                                    // Finally, ensure that the gas used fits in 32 bits, otherwise
                                    // the prover will abort.
                                    if TryInto::<u32>::try_into(
                                        test_body.block.block_header.gas_used,
                                    )
                                    .is_ok()
                                    {
                                        map.0.insert(key, test_body);
                                    }
                                }
                            }
                        } else {
                            // Some tests deal with malformed transactions that wouldn't be passed
                            // to plonky2 zkEVM in the first place, so we just ignore them.
                            let exception = value.blocks[0].transaction_sequence.as_ref().unwrap();
                            assert_eq!(exception[0].valid, "false".to_string());
                        }
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
