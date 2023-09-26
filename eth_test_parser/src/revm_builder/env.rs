//! Convert a `TestBody` into a `Vec` of [`Env`](revm::primitives::Env).
use anyhow::{anyhow, Result};
use common::config::MATIC_CHAIN_ID;
use revm::primitives::{BlockEnv, CfgEnv, Env, TransactTo, TxEnv, B160};
use ruint::Uint;

use crate::deserialize::{AccessListsInner, TestBody, Transaction};

impl TestBody {
    fn to_txn_env(transaction: &Transaction) -> Result<TxEnv> {
        let to = match transaction.to {
            Some(to) => TransactTo::Call(to.to_fixed_bytes().into()),
            None => TransactTo::Call(Default::default()),
        };
        let v = transaction.value;
        let value: ethereum_types::U256 = v
            .try_into()
            .expect("Unable to convert transaction.value to U256");

        let access_list = transaction
            .access_lists
            .get(0)
            .unwrap_or(&AccessListsInner::default())
            .0
            .iter()
            .map(|x| {
                (
                    B160::from(x.address.to_fixed_bytes()),
                    x.storage_keys
                        .iter()
                        .map(|x| (*x).into())
                        .collect::<Vec<Uint<256, 4>>>(),
                )
            })
            .collect();

        Ok(TxEnv {
            caller: transaction.sender.to_fixed_bytes().into(),
            gas_limit: transaction.gas_limit,
            gas_price: transaction.gas_price.map(|p| p.into()).unwrap_or_default(),
            gas_priority_fee: None,
            transact_to: to,
            value: value.into(),
            data: transaction.data.0.clone().into(),
            chain_id: Some(MATIC_CHAIN_ID),
            nonce: transaction.nonce.try_into().ok(),
            // `access_list` is defined parallel to `transaction.data` in the test
            // filler definitions.
            // https://ethereum-tests.readthedocs.io/en/latest/test_filler/test_transaction_state.html?highlight=access#fields
            access_list,
        })
    }

    pub(crate) fn as_revm_env(&self) -> Result<Env> {
        let cfg = CfgEnv {
            chain_id: MATIC_CHAIN_ID.try_into()?,
            ..Default::default()
        };

        if self.blocks.is_empty() {
            return Err(anyhow!("No block"));
        }

        let block = &self.blocks[0];

        let transaction = &block.transactions.as_ref().unwrap()[0];

        let block_header = &block.block_header.clone().unwrap_or_default();

        let block = BlockEnv {
            number: block_header.number.into(),
            coinbase: block_header.coinbase.to_fixed_bytes().into(),
            timestamp: block_header.timestamp.into(),
            difficulty: block_header.difficulty.into(),
            prevrandao: Some(block_header.mix_hash.into()),
            basefee: block_header.base_fee_per_gas.unwrap_or_default().into(),
            gas_limit: block_header.gas_limit.into(),
        };

        Ok(Env {
            cfg,
            block,
            tx: TestBody::to_txn_env(transaction).unwrap(),
        })
    }
}

impl TryFrom<&TestBody> for Env {
    type Error = anyhow::Error;

    fn try_from(body: &TestBody) -> Result<Self> {
        body.as_revm_env()
    }
}
