//! The [`CacheDb`](revm::db::CacheDB) struct isn't serializable, so we need to
//! have our own representation of it. `From` is implemented on both sides to
//! make it easy to convert between the two.

use revm::{
    db::{AccountState, CacheDB, DbAccount},
    primitives::{AccountInfo, Bytecode, HashMap, Log, B160, B256, U256},
    InMemoryDB,
};
use serde::{Deserialize, Serialize};

/// Serializable version of [`AccountState`](revm::db::AccountState)
#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum SerializableAccountState {
    NotExisting,
    Touched,
    StorageCleared,
    #[default]
    None,
}

impl From<AccountState> for SerializableAccountState {
    fn from(account_state: AccountState) -> Self {
        match account_state {
            AccountState::NotExisting => Self::NotExisting,
            AccountState::Touched => Self::Touched,
            AccountState::StorageCleared => Self::StorageCleared,
            AccountState::None => Self::None,
        }
    }
}

impl From<SerializableAccountState> for AccountState {
    fn from(account_state: SerializableAccountState) -> Self {
        match account_state {
            SerializableAccountState::NotExisting => Self::NotExisting,
            SerializableAccountState::Touched => Self::Touched,
            SerializableAccountState::StorageCleared => Self::StorageCleared,
            SerializableAccountState::None => Self::None,
        }
    }
}

/// Serializable version of [`DbAccount`](revm::db::DbAccount)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SerializableDbAccount {
    pub info: AccountInfo,
    pub account_state: SerializableAccountState,
    pub storage: HashMap<U256, U256>,
}

impl From<DbAccount> for SerializableDbAccount {
    fn from(
        DbAccount {
            info,
            account_state,
            storage,
        }: DbAccount,
    ) -> Self {
        Self {
            info,
            account_state: account_state.into(),
            storage,
        }
    }
}

impl From<SerializableDbAccount> for DbAccount {
    fn from(
        SerializableDbAccount {
            info,
            account_state,
            storage,
        }: SerializableDbAccount,
    ) -> Self {
        Self {
            info,
            account_state: account_state.into(),
            storage,
        }
    }
}

/// Serializable version of [`InMemoryDB`](revm::db::InMemoryDB)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SerializableInMemoryDb {
    pub accounts: HashMap<B160, SerializableDbAccount>,
    pub contracts: HashMap<B256, Bytecode>,
    pub logs: Vec<Log>,
    pub block_hashes: HashMap<U256, B256>,
}

impl From<InMemoryDB> for SerializableInMemoryDb {
    fn from(
        CacheDB {
            accounts,
            contracts,
            logs,
            block_hashes,
            ..
        }: InMemoryDB,
    ) -> Self {
        Self {
            accounts: accounts
                .into_iter()
                .map(|(address, account)| (address, SerializableDbAccount::from(account)))
                .collect(),
            contracts,
            logs,
            block_hashes,
        }
    }
}

impl From<SerializableInMemoryDb> for InMemoryDB {
    fn from(
        SerializableInMemoryDb {
            accounts,
            contracts,
            logs,
            block_hashes,
        }: SerializableInMemoryDb,
    ) -> Self {
        Self {
            accounts: accounts
                .into_iter()
                .map(|(address, account)| (address, DbAccount::from(account)))
                .collect(),
            contracts,
            logs,
            block_hashes,
            ..Default::default()
        }
    }
}
