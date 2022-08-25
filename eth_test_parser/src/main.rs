use anyhow::Context;
use arg_parsing::ProgArgs;
use clap::Parser;
use eth_test_parsing::{parse_test_directories, parse_test_directories_forced};
use eth_tests_fetching::{update_eth_tests_upstream, EthTestRepoUpdateInfo};

mod arg_parsing;
mod eth_test_parsing;
mod eth_tests_fetching;

pub(crate) struct ProgState {
    forced_regen: bool,
}

impl ProgState {
    fn new(p_args: ProgArgs) -> Self {
        Self {
            forced_regen: p_args.force_regen_local,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let p_args = ProgArgs::try_parse().with_context(|| "Parsing program arguments")?;
    let state = ProgState::new(p_args);

    run(state);

    Ok(())
}

fn run(state: ProgState) {
    match state.forced_regen {
        false => {
            match update_eth_tests_upstream() {
                EthTestRepoUpdateInfo::AlreadyUpToDate => {
                    println!("Tests already up to date! Nothing to do...");
                }
                EthTestRepoUpdateInfo::ChangesMade => {
                    println!("Upstream test repo has changed. Checking if individual tests have changed...");
                    parse_test_directories();
                }
            }
        }
        true => {
            parse_test_directories_forced();
        }
    }
}
