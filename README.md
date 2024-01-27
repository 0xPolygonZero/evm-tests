# EVM Test

Parses and runs compatible common Ethereum tests from [ethereum/tests](https://github.com/ethereum/tests) against Polygon Zero's zkEVM.

## Components

### Parser

Since the tests from the Ethereum test repo are meant for a full node, only certain tests are compatible with our zkEVM.
Additionally, for the tests that are compatible, they need to be parsed (or converted) into an Intermediary Representation (IR) format that is usable by our zkEVM.

The parser has two responsibilities:

- Query the upstream Ethereum tests repo and check if any tests have been added/updated/removed.
- If there is a change, re-parse the tests.

### Runner

The runner feeds the parsed tests into the zkEVM. Successes are defined as no errors occurring (the expected final state being enforced internally by the zkEVM when
generating proofs).
If the zkEVM returns an error or panics, then the test is considered to have failed.

The runner also outputs a results file (likely as a `*.md`) which contains statistics on the last test run.

## Quick Start

Run the parser to parse the Ethereum tests into a format usable by `plonky2`:

```sh
cd eth_test_parser
cargo run
```

If the tests have already been fetched but need another preprocessing, for instance following breaking changes on the zkEVM format,
one can run the parser again as `cargo run -- --no_fetch` to directly deserialize local files without fetching the remote location. 

Then launch the runner pointing it at the parsed tests directory:

```sh
cd ../evm_test_runner
cargo run --release -- -r summary ../generation_inputs/BlockchainTests # For a high-level summary report
cargo run --release -- -r test ../generation_inputs/BlockchainTests # For detailed information per test (likely want to use a filter with `-f`)
```

The test runner supports secondary arguments to customize the testing flow. While they are all displayed by calling `cargo run -- --help`,
below are listed the most useful ones:

* `--blacklist-path` (short `b`): An optional relative path to a blacklist file containing test variants to prevent from running.
This can be used to skip particularly heavy or badly configured tests.
* `--variant-filter` (short `v`): Only run specified test variants (either a single value or a range), e.g. `0` or `0..=5`
for instance. Note that the variant `n` for test `foo` isn't represented as `foo_n`, as variants keep the same naming
format as their remote, namely `foo_dx_gy_vz` with `x`, `y`, `z` varying integers.
* `--test-filter` (short `f`): An optional filter to only run tests that are a subset of the given test path. By default,
the runner will process all tests included in the initial path provided.
* `--witness-only` (short `w`): Only generate the witness and not the entire proof for a test.
This is significantly faster than proving, but may give false negatives if constraints were to not be satisfiable, and
hence should not be taken as a guarantee of completeness.
* `--skip-passed` (short `p`): Skip tests that have already passed in the past or are ignored (see below the section for ignored
tests). If this argument is passed along with `--witness-only`, any previously passed test will be ignored. If the `--witness-only`
is not present, then this will skip only tests for which we did generate proofs, and will re-run tests for which only a witness had
been generated.


### Note on ignored tests

The zkEVM design makes some assumptions on the transaction IR format. For instance, the `gas_used` field in transactions as well as
the `gas_limit` field in block headers must fit in 32 bits. This design choice is perfectly acceptable in practice, with a usual
block gas limit for Ethereum at 30M, but isn't sufficient for some tests of the Ethereum test suite.

To this extent, the test parser will *ignore* all tests for which the transaction `gas_used` would overflow a `u32`, as these transactions
would not be provable anyway. For tests that have an acceptable transaction `gas_used`, but a block `gas_limit` overflowing, we manually
alter the latter to be `0xFFFFFFFF` (i.e. the maximum value fitting in a `u32`). If the runner manages to generate a valid witness / proof
for this altered test, we log it as valid. If it fails, then we flag the test as ignored.

## Other

[Polygon Hermez](https://github.com/0xPolygonHermez) is doing something similar [here](https://github.com/0xPolygonHermez/zkevm-testvectors).

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
