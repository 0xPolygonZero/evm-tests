use clap::Parser;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub(crate) struct ProgArgs {
    /// Don't check for updates but force regen all the current existing local
    /// tests from the last Ethereum test repo pull.
    #[clap(short = 'f', long = "regen-local-only", default_value_t = false)]
    pub(crate) force_regen_local: bool,
}
