//! Logic for dealing with converting a
//! [`TestBody`](crate::deserialize::TestBody) into
//! `revm`'s [`InMemoryDB`](revm::InMemoryDB)

use anyhow::Result;
use common::revm::cache_db::SerializableInMemoryDb;
use revm::{
    primitives::{AccountInfo, Bytecode},
    InMemoryDB,
};

use crate::deserialize::TestBody;

impl TestBody {
    pub(crate) fn as_revm_cache_db(&self) -> Result<SerializableInMemoryDb> {
        let mut db = InMemoryDB::default();

        for (address, account) in &self.pre {
            let address = address.to_fixed_bytes().into();
            let account_info = AccountInfo::new(
                account.balance.into(),
                account.nonce,
                Bytecode::new_raw(account.code.0.clone().into()).to_checked(),
            );

            db.insert_account_info(address, account_info);

            for (key, value) in account.storage.iter() {
                db.insert_account_storage(address, (*key).into(), (*value).into())?;
            }
        }

        Ok(db.into())
    }
}

impl TryFrom<&TestBody> for SerializableInMemoryDb {
    type Error = anyhow::Error;

    fn try_from(body: &TestBody) -> Result<Self> {
        body.as_revm_cache_db()
    }
}
