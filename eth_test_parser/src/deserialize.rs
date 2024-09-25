use std::{collections::HashMap, marker::PhantomData};

use anyhow::Result;
use bytes::Bytes;
use ethereum_types::{Address, H160, H256, U256};
use evm_arithmetization::generation::mpt::transaction_testing::{
    AddressOption, LegacyTransactionRlp,
};
use hex::FromHex;
use hex_literal::hex;
use rlp::{Decodable, DecoderError, Rlp};
use rlp_derive::RlpDecodable;
use serde::de::MapAccess;
use serde::{
    de::{Error, Visitor},
    Deserialize, Deserializer,
};
use serde_with::serde_as;

use crate::config::UNPROVABLE_VARIANTS;

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
    pub(crate) blob_gas_used: U256,
    pub(crate) excess_blob_gas: U256,
    pub(crate) parent_beacon_block_root: H256,
}

// Some tests store the access list in a way that doesn't respect the specs,
// and hence they require a specific handling.
#[derive(Clone, Debug, RlpDecodable)]
pub struct AccessItemRlp {
    _address: Address,
    _storage_keys: Vec<StorageKey>,
}

#[derive(Clone, Debug)]
pub struct StorageKey;

impl Decodable for StorageKey {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        // We just need to decode the key as a `Vec<u8>`
        // to deal with badly encoded scalars, but we do
        // not care about the result.
        let _key = rlp.as_val::<Vec<u8>>()?;

        Ok(Self)
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
    _chain_id: u64,
    _nonce: U256,
    _gas_price: U256,
    _gas: U256,
    _to: AddressOption,
    _value: U256,
    _data: Bytes,
    _access_list: Vec<AccessItemRlp>,
    _y_parity: U256,
    _r: U256,
    _s: U256,
}

// A custom type-2 txn to handle some edge-cases with the access_list field.
#[derive(RlpDecodable, Debug, Clone)]
pub struct CustomFeeMarketTransactionRlp {
    _chain_id: u64,
    _nonce: U256,
    _max_priority_fee_per_gas: U256,
    _max_fee_per_gas: U256,
    _gas: U256,
    _to: AddressOption,
    _value: U256,
    _data: Bytes,
    _access_list: Vec<AccessItemRlp>,
    _y_parity: U256,
    _r: U256,
    _s: U256,
}

// A custom type-2 txn to handle some edge-cases with the access_list field.
#[derive(RlpDecodable, Debug, Clone)]
pub struct CustomBlobTransactionRlp {
    _chain_id: u64,
    _nonce: U256,
    _max_priority_fee_per_gas: U256,
    _max_fee_per_gas: U256,
    _gas: U256,
    _to: H160,
    _value: U256,
    _data: Bytes,
    _access_list: Vec<AccessItemRlp>,
    _max_fee_per_blob_gas: U256,
    _blob_versioned_hashes: Vec<H256>,
    _y_parity: U256,
    _r: U256,
    _s: U256,
}

#[derive(Clone, Debug)]
pub struct Transaction(pub Vec<u8>);

impl Transaction {
    fn decode_actual_rlp(bytes: &[u8]) -> Result<Self, DecoderError> {
        let first_byte = bytes.first().ok_or(DecoderError::RlpInvalidLength)?;
        match *first_byte {
            1 => CustomAccessListTransactionRlp::decode(&Rlp::new(&bytes[1..]))
                .map(|_| Self(bytes.to_vec())),
            2 => CustomFeeMarketTransactionRlp::decode(&Rlp::new(&bytes[1..]))
                .map(|_| Self(bytes.to_vec())),
            3 => CustomBlobTransactionRlp::decode(&Rlp::new(&bytes[1..]))
                .map(|_| Self(bytes.to_vec())),
            _ => LegacyTransactionRlp::decode(&Rlp::new(bytes)).map(|_| Self(bytes.to_vec())),
        }
    }
}

impl Decodable for Transaction {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let attempt = Transaction::decode_actual_rlp(rlp.as_raw());

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

        let mut pre = value.pre.clone();
        let mut post = value.post_state.clone();

        // TODO: export from plonky2 kernel constants directly
        let exit_root_pre_account = PreAccount {
            balance: U256::zero(),
            nonce: 0,
            code: ByteString(hex!("60806040526004361061004e5760003560e01c80633659cfe6146100655780634f1ef286146100855780635c60da1b146100985780638f283970146100c9578063f851a440146100e95761005d565b3661005d5761005b6100fe565b005b61005b6100fe565b34801561007157600080fd5b5061005b6100803660046106ca565b610118565b61005b6100933660046106e5565b61015f565b3480156100a457600080fd5b506100ad6101d0565b6040516001600160a01b03909116815260200160405180910390f35b3480156100d557600080fd5b5061005b6100e43660046106ca565b61020b565b3480156100f557600080fd5b506100ad610235565b610106610292565b610116610111610331565b61033b565b565b61012061035f565b6001600160a01b0316336001600160a01b031614156101575761015481604051806020016040528060008152506000610392565b50565b6101546100fe565b61016761035f565b6001600160a01b0316336001600160a01b031614156101c8576101c38383838080601f01602080910402602001604051908101604052809392919081815260200183838082843760009201919091525060019250610392915050565b505050565b6101c36100fe565b60006101da61035f565b6001600160a01b0316336001600160a01b03161415610200576101fb610331565b905090565b6102086100fe565b90565b61021361035f565b6001600160a01b0316336001600160a01b0316141561015757610154816103f1565b600061023f61035f565b6001600160a01b0316336001600160a01b03161415610200576101fb61035f565b606061028583836040518060600160405280602781526020016107e460279139610445565b9392505050565b3b151590565b61029a61035f565b6001600160a01b0316336001600160a01b031614156101165760405162461bcd60e51b815260206004820152604260248201527f5472616e73706172656e745570677261646561626c6550726f78793a2061646d60448201527f696e2063616e6e6f742066616c6c6261636b20746f2070726f78792074617267606482015261195d60f21b608482015260a4015b60405180910390fd5b60006101fb610519565b3660008037600080366000845af43d6000803e80801561035a573d6000f35b3d6000fd5b60007fb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d61035b546001600160a01b0316919050565b61039b83610541565b6040516001600160a01b038416907fbc7cd75a20ee27fd9adebab32041f755214dbc6bffa90cc0225b39da2e5c2d3b90600090a26000825111806103dc5750805b156101c3576103eb8383610260565b50505050565b7f7e644d79422f17c01e4894b5f4f588d331ebfa28653d42ae832dc59e38c9798f61041a61035f565b604080516001600160a01b03928316815291841660208301520160405180910390a1610154816105e9565b6060833b6104a45760405162461bcd60e51b815260206004820152602660248201527f416464726573733a2064656c65676174652063616c6c20746f206e6f6e2d636f6044820152651b9d1c9858dd60d21b6064820152608401610328565b600080856001600160a01b0316856040516104bf9190610794565b600060405180830381855af49150503d80600081146104fa576040519150601f19603f3d011682016040523d82523d6000602084013e6104ff565b606091505b509150915061050f828286610675565b9695505050505050565b60007f360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc610383565b803b6105a55760405162461bcd60e51b815260206004820152602d60248201527f455243313936373a206e657720696d706c656d656e746174696f6e206973206e60448201526c1bdd08184818dbdb9d1c9858dd609a1b6064820152608401610328565b807f360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc5b80546001600160a01b0319166001600160a01b039290921691909117905550565b6001600160a01b03811661064e5760405162461bcd60e51b815260206004820152602660248201527f455243313936373a206e65772061646d696e20697320746865207a65726f206160448201526564647265737360d01b6064820152608401610328565b807fb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d61036105c8565b60608315610684575081610285565b8251156106945782518084602001fd5b8160405162461bcd60e51b815260040161032891906107b0565b80356001600160a01b03811681146106c557600080fd5b919050565b6000602082840312156106dc57600080fd5b610285826106ae565b6000806000604084860312156106fa57600080fd5b610703846106ae565b9250602084013567ffffffffffffffff8082111561072057600080fd5b818601915086601f83011261073457600080fd5b81358181111561074357600080fd5b87602082850101111561075557600080fd5b6020830194508093505050509250925092565b60005b8381101561078357818101518382015260200161076b565b838111156103eb5750506000910152565b600082516107a6818460208701610768565b9190910192915050565b60208152600082518060208401526107cf816040850160208701610768565b601f01601f1916919091016040019291505056fe416464726573733a206c6f772d6c6576656c2064656c65676174652063616c6c206661696c6564a26469706673582212204675187caf3a43285d9a2c1844a981e977bd52a85ff073e7fc649f73847d70a464736f6c63430008090033").to_vec()),
            storage: HashMap::new(),
        };
        pre.insert(
            H160(hex!("a40D5f56745a118D0906a34E69aeC8C0Db1cB8fA")),
            exit_root_pre_account.clone(),
        );
        post.insert(
            H160(hex!("a40D5f56745a118D0906a34E69aeC8C0Db1cB8fA")),
            exit_root_pre_account,
        );

        Self {
            name: variant_name,
            block,
            genesis_block,
            pre,
            post,
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
    #[serde(rename = "postState")]
    pub(crate) post_state: HashMap<H160, PreAccount>,
}

// Wrapper around a regular `HashMap` used to conveniently skip
// non-Cancun related tests when deserializing.
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
                // `Cancun` in their key name.
                while let Some((key, value)) = access.next_entry::<String, ValueJson>()? {
                    if key.contains("_Cancun")
                        && !UNPROVABLE_VARIANTS.iter().any(|v| key.contains(v))
                    {
                        if value.blocks[0].transaction_sequence.is_none() {
                            let test_body = TestBody::from_parsed_json(&value, key.clone());

                            // Ensure that the gas used fits in 32 bits, otherwise the prover will
                            // abort.
                            if TryInto::<u32>::try_into(test_body.block.block_header.gas_used)
                                .is_ok()
                            {
                                map.0.insert(key, test_body);
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
