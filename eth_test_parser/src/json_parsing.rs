//! Logic for parsing the extracted raw test JSON into a format that is usable
//! by `Plonky2`.

use std::{any::type_name, collections::HashMap, error::Error, str::FromStr};

use anyhow::Context;
use eth_trie_utils::{
    partial_trie::{Nibbles, PartialTrie},
    trie_builder::InsertEntry,
};
use ethereum_types::{Address, U256};
use plonky2_evm::proof::BlockMetadata;
use serde_json::Value;
use sha3::{digest::core_api::CoreWrapper, Digest, Sha3_256, Sha3_256Core};

use crate::utils::keccak_eth_addr;

type Nonce = u32;
type HashType = U256; // Placeholder

type Sha3256Hasher = CoreWrapper<Sha3_256Core>;

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct JsonAccountsParseOutput {
    pub(crate) account_trie: PartialTrie,
    pub(crate) account_storage_tries: Vec<(Address, PartialTrie)>,
    pub(crate) code_for_contracts: HashMap<U256, Vec<u8>>,
}

#[derive(Debug)]
pub(crate) struct JsonReceiptsParseOutput {}

#[derive(Debug)]
pub(crate) struct JsonTxnParseOutput {
    pub(crate) txn_trie: PartialTrie,
    pub(crate) signed_txns: Vec<Vec<u8>>,
}

pub(crate) fn parse_initial_account_state_from_json(
    accounts_json: &Value,
) -> anyhow::Result<JsonAccountsParseOutput> {
    let mut account_trie = Box::new(PartialTrie::Empty);
    let mut account_storage_tries = Vec::new();
    let mut code_for_contracts = HashMap::new();

    for (addr, v) in json_val_to_addresses_and_sub_json_vals(accounts_json) {
        let (acc_entry, acc_storage_trie, contract_code_opt) = parse_json_account_entry(addr, v)?;

        if let Some(updated_account_trie) =
            PartialTrie::insert_into_trie(&mut account_trie, acc_entry)
        {
            account_trie = updated_account_trie;
        }

        account_storage_tries.push((addr, *acc_storage_trie));

        if let Some(contract_code) = contract_code_opt {
            code_for_contracts.insert(keccak_eth_addr(addr), contract_code);
        }
    }

    Ok(JsonAccountsParseOutput {
        account_trie: *account_trie,
        account_storage_tries,
        code_for_contracts,
    })
}

fn parse_json_account_entry(
    account_addr: Address,
    account_json: &Value,
) -> anyhow::Result<(InsertEntry, Box<PartialTrie>, Option<Vec<u8>>)> {
    let balance: U256 = get_json_field_and_conv(account_json, "balance")?;
    let contract_code = get_json_field_as_bytes(account_json, "code")?.to_vec();
    let nonce: Nonce = get_json_field_and_conv(account_json, "nonce")?;

    // Unwrap for now. Maybe find a nice way to deal with results in iterators
    // without collect?
    let acc_storage_trie = PartialTrie::construct_trie_from_inserts(
        account_json["storage"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, v)| try_create_insert_entry_from_json_entry(k, v).unwrap()),
    );

    let acc_storage_hash = get_hash_of_partial_trie_root(&acc_storage_trie);
    let code_hash = get_hash_of_bytes(&contract_code);

    let mut trie_entry_bytes = Vec::new();

    append_u32_to_byte_buf(nonce, &mut trie_entry_bytes);
    append_u256_to_byte_buf(balance, &mut trie_entry_bytes);
    append_u256_to_byte_buf(acc_storage_hash, &mut trie_entry_bytes);
    append_u256_to_byte_buf(code_hash, &mut trie_entry_bytes);

    let entry = InsertEntry {
        nibbles: keccak_eth_addr(account_addr).into(),
        v: trie_entry_bytes,
    };

    let contract_code_opt = match contract_code.len() {
        0 => None,
        _ => Some(contract_code),
    };

    Ok((entry, acc_storage_trie, contract_code_opt))
}

pub(crate) fn parse_receipt_trie_from_json(_receipt_json: &Value) -> PartialTrie {
    todo!()
}

pub(crate) fn parse_txn_trie_from_json(_blocks_json: &Value) -> JsonTxnParseOutput {
    todo!()
}

pub(crate) fn parse_block_metadata_from_json(
    _blocks_json: &Value,
    _genesis_block_header_json: &Value,
) -> BlockMetadata {
    todo!()
}

fn get_json_field_as_str<'a>(json: &'a Value, k: &'static str) -> anyhow::Result<&'a str> {
    parse_json_val_as_str(&json[k]).with_context(|| format!("Parsing the value of key {}", k))
}

fn get_json_field_as_bytes<'a>(json: &'a Value, k: &'static str) -> anyhow::Result<&'a [u8]> {
    let str = get_json_field_as_str(json, k)?;
    Ok(str.as_bytes())
}

fn get_json_field_and_conv<T>(json: &Value, k: &'static str) -> anyhow::Result<T>
where
    T: FromStr,
    <T as FromStr>::Err: Sync + Send + Error + 'static,
{
    let str = get_json_field_as_str(json, k)?;
    T::from_str(str)
        .with_context(|| format!("Failed to convert string {} to a {}", str, type_name::<T>()))
}

// Since `PartialTrie`s do not have access to the hashes like a merkle trie
// does, we're going to go the hacky route for now and just hash the entire trie
// to calculate the root hash.
fn get_hash_of_partial_trie_root(trie: &PartialTrie) -> HashType {
    let mut h = Sha3_256::new();
    trie_hash_rec(trie, &mut h);

    U256::from_big_endian(h.finalize().as_ref())
}

fn trie_hash_rec(trie: &PartialTrie, h: &mut Sha3256Hasher) {
    match trie {
        PartialTrie::Empty => h.update([0]),
        PartialTrie::Hash(_hash) => unreachable!(
            "Found a hash node when hashing a trie! These should not exist in the Eth tests!"
        ),
        PartialTrie::Branch { children, value } => {
            for c in children {
                trie_hash_rec(c, h);
            }

            let mut byte_buf = [0; 32];
            value.unwrap_or(U256::zero()).to_big_endian(&mut byte_buf);

            h.update(byte_buf);
        }
        PartialTrie::Extension { nibbles, child } => {
            trie_hash_rec(child, h);
            hash_nibbles(nibbles, h);
        }
        PartialTrie::Leaf { nibbles, value } => {
            hash_nibbles(nibbles, h);
            h.update(value)
        }
    };
}

fn hash_nibbles(n: &Nibbles, h: &mut Sha3256Hasher) {
    let mut byte_buf = [0; 32];
    n.packed.to_big_endian(&mut byte_buf);

    h.update(n.count.to_be_bytes());
    h.update(byte_buf);
}

fn get_hash_of_bytes(bytes: &Vec<u8>) -> HashType {
    let mut h = Sha3_256::new();
    h.update(bytes);

    U256::from_big_endian(h.finalize().as_ref())
}

fn json_val_to_addresses_and_sub_json_vals(
    json_val: &Value,
) -> impl Iterator<Item = (Address, &Value)> {
    json_val.as_object().into_iter().flatten().map(|(k, v)| {
        let addr = k
            .parse()
            .with_context(|| format!("Parsing {} to an eth address (H160)", k))
            .unwrap();
        (addr, v)
    })
}

fn try_create_insert_entry_from_json_entry(
    k_str: &str,
    json_val: &Value,
) -> anyhow::Result<InsertEntry> {
    let k = k_str
        .parse()
        .with_context(|| format!("Parsing trie key {} into a U256", k_str))?;

    let v_bytes = hex::decode(parse_json_val_as_str(json_val)?)
        .with_context(|| format!("Parsing {} as a vec of bytes", json_val))?;

    Ok(InsertEntry::from_eth_addr_and_bytes(k, v_bytes))
}

fn parse_json_val_as_str(v: &Value) -> anyhow::Result<&str> {
    v.as_str()
        .with_context(|| format!("Could not convert json value to str (json: {})", v))
}

fn append_u256_to_byte_buf(v: U256, buf: &mut Vec<u8>) {
    let mut byte_buff: [u8; 32] = [0; 32];
    v.to_big_endian(&mut byte_buff);
    buf.extend(byte_buff);
}

fn append_u32_to_byte_buf(v: u32, buf: &mut Vec<u8>) {
    buf.extend(v.to_be_bytes())
}
