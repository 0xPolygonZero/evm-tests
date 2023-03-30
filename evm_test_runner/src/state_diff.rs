use std::{collections::HashMap, fmt::Display};

use console::{style, Style};
use ethereum_types::{Address, H160, H256, U256};
use keccak_hash::keccak;
use plonky2_evm::generation::outputs::{AccountOutput, AddressOrStateKey};
use revm::primitives::{Account, B160};
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone)]
pub(crate) struct StateDiff {
    revm_state: HashMap<Address, AccountCompare>,
    plonky2_state: HashMap<Address, AccountCompare>,
}

#[derive(Eq, PartialEq, PartialOrd, Ord, Hash, Debug, Clone)]
/// Normalized representation of an account.
struct AccountCompare {
    balance: U256,
    nonce: u64,
    storage: Vec<(U256, U256)>,
}

impl StateDiff {
    /// Construct a new [`StateDiff`](crate::state_diff::StateDiff).
    ///
    /// We normalize both the revm and plonky2 states into `AccountCompare` to
    /// make them comparable.
    pub(crate) fn new(
        revm_state: revm::primitives::HashMap<B160, Account>,
        plonky2_state: HashMap<AddressOrStateKey, AccountOutput>,
    ) -> Self {
        // Store a lookup table from keccak hashes to addresses in the event
        // that plonky2 is missing an address from its output.
        let keccak_address_lookup: HashMap<H256, Address> = revm_state
            .keys()
            .map(|k| (keccak(k.to_fixed_bytes()), H160::from(k.to_fixed_bytes())))
            .collect();

        let revm_state = revm_state
            .into_iter()
            .map(|(k, v)| {
                let mut storage = v
                    .storage
                    .into_iter()
                    .map(|(k, v)| (keccak(k.to_be_bytes::<32>()).0.into(), v.present_value.into()))
                    .collect::<Vec<(U256, U256)>>();
                storage.sort_by(|a, b| b.0.cmp(&a.0));

                (
                    k.to_fixed_bytes().into(),
                    AccountCompare {
                        balance: v.info.balance.into(),
                        nonce: v.info.nonce,
                        storage,
                    },
                )
            })
            .collect();

        let plonky2_state = plonky2_state
            .into_iter()
            .map(|(k, v)| {
                let address = match k {
                    AddressOrStateKey::Address(a) => a,
                    // If the address is missing from the plonky2 output, we can look it up in the
                    // keccak table.
                    AddressOrStateKey::StateKey(k) => keccak_address_lookup[&k],
                };

                let mut storage = v
                    .storage
                    .into_iter()
                    .map(|(k, v)| (k, v))
                    .collect::<Vec<_>>();
                storage.sort_by(|a, b| b.0.cmp(&a.0));

                (
                    address,
                    AccountCompare {
                        balance: v.balance,
                        nonce: v.nonce,
                        storage,
                    },
                )
            })
            .collect();

        Self {
            revm_state,
            plonky2_state,
        }
    }
}

struct Line(Option<usize>);

impl Display for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.0 {
            None => write!(f, "    "),
            Some(idx) => write!(f, "{:<4}", idx + 1),
        }
    }
}

impl Display for StateDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut fst = self.revm_state.iter().collect::<Vec<_>>();
        fst.sort_by_key(|(address, _)| (self.plonky2_state.contains_key(address), *address));
        let mut snd = self.plonky2_state.iter().collect::<Vec<_>>();
        snd.sort_by_key(|(address, _)| (self.revm_state.contains_key(address), *address));

        let text1 = format!("{:#?}", fst);
        let text2 = format!("{:#?}", snd);
        let diff = TextDiff::from_lines(text1.as_str(), text2.as_str());

        for (idx, group) in diff.grouped_ops(10).iter().enumerate() {
            if idx > 0 {
                writeln!(f, "{:-^1$}", "-", 80)?;
            }
            for op in group {
                for change in diff.iter_inline_changes(op) {
                    let (sign, s) = match change.tag() {
                        ChangeTag::Delete => ("-", Style::new().red()),
                        ChangeTag::Insert => ("+", Style::new().green()),
                        ChangeTag::Equal => (" ", Style::new().dim()),
                    };
                    write!(
                        f,
                        "{}{} |{}",
                        style(Line(change.old_index())).dim(),
                        style(Line(change.new_index())).dim(),
                        s.apply_to(sign).bold(),
                    )?;
                    for (emphasized, value) in change.iter_strings_lossy() {
                        if emphasized {
                            write!(f, "{}", s.apply_to(value).underlined().on_black())?;
                        } else {
                            write!(f, "{}", s.apply_to(value))?;
                        }
                    }
                    if change.missing_newline() {
                        writeln!(f)?;
                    }
                }
            }
        }

        Ok(())
    }
}
