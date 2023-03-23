//! Serializable wrapper types for an [`EVM`](revm::EVM) instance.
//!
//! Getting a fully constructed [`revm`](revm) environment requires the
//! following steps:
//!
//! 1. Construct an [`EVM`](revm::EVM) instance.
//! 2. Configure the instance's [`Env`](revm::primitives::Env). Note this
//!    includes setting up the transaction we're testing at this step.
//! 3. Construct a [`Db`](revm::db::Database). In our case, an
//!    [`InMemoryDB`](revm::InMemoryDB).
//! 4. Load the database with the accounts and their storage.

use revm::{primitives::Env, InMemoryDB, EVM};
use serde::{Deserialize, Serialize};

use self::cache_db::SerializableInMemoryDb;

pub mod cache_db;

/// Serialized version of a hydrated evm instance.
#[derive(Deserialize, Serialize, Debug)]
pub struct SerializableEVMInstance {
    pub env: Env,
    pub db: SerializableInMemoryDb,
}

impl SerializableEVMInstance {
    pub fn into_hydrated(self) -> EVM<InMemoryDB> {
        EVM {
            db: Some(self.db.into()),
            env: self.env,
        }
    }
}

impl From<SerializableEVMInstance> for EVM<InMemoryDB> {
    fn from(value: SerializableEVMInstance) -> Self {
        value.into_hydrated()
    }
}
