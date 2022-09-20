use arg_parsing::ProgArgs;
use clap::Parser;
use test_dir_reading::parse_all_tests;

mod arg_parsing;
mod test_dir_reading;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let p_args = ProgArgs::parse();
    let _parsed_tests = parse_all_tests(&p_args.parsed_tests_path).await?;

    Ok(())
}
