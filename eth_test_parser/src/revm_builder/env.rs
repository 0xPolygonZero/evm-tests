//! Convert a `TestBody` into a `Vec` of [`Env`](revm::primitives::Env).
use anyhow::Result;
use common::config::MATIC_CHAIN_ID;
use revm::primitives::{BlockEnv, Bytes, CfgEnv, Env, TransactTo, TxEnv};

use crate::deserialize::TestBody;

struct TxSharedData {
    data: Vec<Bytes>,
    access_list: Vec<Vec<(revm::primitives::B160, Vec<revm::primitives::U256>)>>,
    gas_limit: Vec<u64>,
    value: Vec<ruint::aliases::U256>,
}

impl TestBody {
    fn try_as_tx_shared_data(&self) -> Result<TxSharedData> {
        let data = self
            .transaction
            .data
            .iter()
            .map(|byte_string| byte_string.0.clone().into())
            .collect();

        let access_list = self
            .transaction
            .access_lists
            .iter()
            .map(|access_list| {
                access_list
                    .0
                    .iter()
                    .map(|x| {
                        (
                            x.address.to_fixed_bytes().into(),
                            x.storage_keys.iter().map(|x| (*x).into()).collect(),
                        )
                    })
                    .collect()
            })
            .collect();

        let gas_limit = self.transaction.gas_limit.to_vec();

        let value = self
            .transaction
            .value
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let u256: ethereum_types::U256 = v.try_into().map_err(|_| {
                    anyhow::Error::msg("Overflow").context(format!(
                        "Unable to convert transaction.value[{i}] to U256. Got {v}"
                    ))
                })?;
                Ok(u256.into())
            })
            .collect::<Result<Vec<revm::primitives::U256>>>()?;

        Ok(TxSharedData {
            data,
            access_list,
            gas_limit,
            value,
        })
    }

    pub(crate) fn as_revm_env(&self) -> Result<Vec<Env>> {
        let cfg = CfgEnv {
            chain_id: MATIC_CHAIN_ID.try_into()?,
            ..Default::default()
        };

        let block = BlockEnv {
            number: self.env.current_number.into(),
            coinbase: self.env.current_coinbase.to_fixed_bytes().into(),
            timestamp: self.env.current_timestamp.into(),
            difficulty: self.env.current_difficulty.into(),
            prevrandao: Some(
                <ethereum_types::U256 as std::convert::Into<revm::primitives::U256>>::into(
                    self.env.current_difficulty,
                )
                .to_be_bytes()
                .into(),
            ),
            basefee: self.env.current_base_fee.into(),
            gas_limit: self.env.current_gas_limit.into(),
        };

        let gas_price = self
            .transaction
            .gas_price
            .map(|p| p.into())
            .unwrap_or_default();

        let transact_to = match self.transaction.to {
            Some(to) => TransactTo::Call(to.to_fixed_bytes().into()),
            None => TransactTo::Call(Default::default()),
        };

        let tx_shared_data: TxSharedData = self.try_into()?;

        Ok(self
            .post
            .merge
            .iter()
            .map(|m| Env {
                cfg: cfg.clone(),
                block: block.clone(),
                tx: TxEnv {
                    caller: self.transaction.sender.to_fixed_bytes().into(),
                    gas_limit: tx_shared_data.gas_limit[m.indexes.gas],
                    gas_price,
                    gas_priority_fee: None,
                    transact_to: transact_to.clone(),
                    value: tx_shared_data.value[m.indexes.value],
                    data: tx_shared_data.data[m.indexes.data].clone(),
                    chain_id: Some(MATIC_CHAIN_ID),
                    nonce: self.transaction.nonce.try_into().ok(),
                    // `access_list` is defined parallel to `transaction.data` in the test filler
                    // definitions.
                    // https://ethereum-tests.readthedocs.io/en/latest/test_filler/test_transaction_state.html?highlight=access#fields
                    access_list: tx_shared_data
                        .access_list
                        .get(m.indexes.data)
                        .cloned()
                        .unwrap_or_default(),
                },
            })
            .collect())
    }
}

impl TryFrom<&TestBody> for TxSharedData {
    type Error = anyhow::Error;

    fn try_from(body: &TestBody) -> Result<Self> {
        body.try_as_tx_shared_data()
    }
}

impl TryFrom<&TestBody> for Vec<Env> {
    type Error = anyhow::Error;

    fn try_from(body: &TestBody) -> Result<Self> {
        body.as_revm_env()
    }
}
