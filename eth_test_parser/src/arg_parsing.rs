use clap::Parser;

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub(crate) struct ProgArgs {
    #[arg(short, long, default_value_t = false)]
    /// Allow deserializing without fetching git remote
    pub no_fetch: bool,
}
