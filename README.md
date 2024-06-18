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
cargo run --release -- -r summary # For a high-level summary report
cargo run --release -- -r test # For detailed information per test (likely want to use a filter with `-f`)
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

## Coverage [zk_evm v0.4.0]

The results below have been obtained against [zk_evm v0.4.0](https://github.com/0xPolygonZero/zk_evm/releases/tag/v0.4.0).

All test vectors have been fetched from [ethereum/legacytests](https://github.com/ethereum/legacytests/tree/master) from
commit [8077d24](https://github.com/ethereum/legacytests/commit/8077d241740de5a448e41156fd33740f562d3b56).

The total number of tests below excludes:

- tests with invalid block RLP encoding (from which all the test data is retrieved)
- tests with a txn gas used greater than $2^{32}-1$, see [Note on ignored tests](#note-on-ignored-tests).
- stress tests with a zkEVM CPU overhead greater than Goldilocks' two-adic subgroup size.


| Total | :white_check_mark: |   :x:  |  Coverage  |
|:-----:|:------------------:|:------:|:----------:|
| 14904	|        14896       | 8 [^1] |   99.95%   |

[^1]: These tests have an impossible initial configuration (i.e. empty accounts with non-empty storage),
and end up not being provable.

### Extended results

|           Test Folder Name           | Total | :white_check_mark: | :x: |   Cov   |
|:------------------------------------:|:-----:|:------------------:|:---:|:------:|
|               Shanghai               |  26   |         26         |  0  | 100.00 |
|         stArgsZeroOneBalance         |  96   |         96         |  0  | 100.00 |
|             stAttackTest             |   2   |         2          |  0  | 100.00 |
|             stBadOpcode              |  340  |        340         |  0  | 100.00 |
|                stBugs                |   9   |         9          |  0  | 100.00 |
|             stCallCodes              |  86   |         86         |  0  | 100.00 |
|       stCallCreateCallCodeTest       |  55   |         55         |  0  | 100.00 |
| stCallDelegateCodesCallCodeHomestead |  58   |         58         |  0  | 100.00 |
|     stCallDelegateCodesHomestead     |  58   |         58         |  0  | 100.00 |
|              stChainId               |   2   |         2          |  0  | 100.00 |
|            stCodeCopyTest            |   2   |         2          |  0  | 100.00 |
|           stCodeSizeLimit            |   7   |         7          |  0  | 100.00 |
|              stCreate2               |  184  |        181         |  3  | 98.37  |
|             stCreateTest             |  204  |        204         |  0  | 100.00 |
|     stDelegatecallTestHomestead      |  31   |         31         |  0  | 100.00 |
|           stEIP150Specific           |  25   |         25         |  0  | 100.00 |
|     stEIP150singleCodeGasPrices      |  340  |        340         |  0  | 100.00 |
|              stEIP1559               |  949  |        949         |  0  | 100.00 |
|           stEIP158Specific           |   8   |         8          |  0  | 100.00 |
|              stEIP2930               |  140  |        140         |  0  | 100.00 |
|              stEIP3607               |   5   |         5          |  0  | 100.00 |
|              stExample               |  38   |         38         |  0  | 100.00 |
|            stExtCodeHash             |  69   |         68         |  1  | 98.56  |
|         stHomesteadSpecific          |   5   |         5          |  0  | 100.00 |
|            stInitCodeTest            |  22   |         22         |  0  | 100.00 |
|              stLogTests              |  46   |         46         |  0  | 100.00 |
|      stMemExpandingEIP150Calls       |  10   |         10         |  0  | 100.00 |
|          stMemoryStressTest          |  82   |         79         |  0  | 100.00 |
|             stMemoryTest             |  567  |        567         |  0  | 100.00 |
|          stNonZeroCallsTest          |  24   |         24         |  0  | 100.00 |
|        stPreCompiledContracts        |  956  |        956         |  0  | 100.00 |
|       stPreCompiledContracts2        |  246  |        246         |  0  | 100.00 |
|      stQuadraticComplexityTest       |  28   |         28         |  0  | 100.00 |
|               stRandom               |  310  |        310         |  0  | 100.00 |
|              stRandom2               |  221  |        221         |  0  | 100.00 |
|          stRecursiveCreate           |   2   |         2          |  0  | 100.00 |
|             stRefundTest             |  26   |         26         |  0  | 100.00 |
|           stReturnDataTest           |  247  |        247         |  0  | 100.00 |
|             stRevertTest             |  270  |        270         |  0  | 100.00 |
|             stSLoadTest              |   1   |         1          |  0  | 100.00 |
|             stSStoreTest             |  475  |        471         |  4  | 99.16  |
|            stSelfBalance             |  42   |         42         |  0  | 100.00 |
|               stShift                |  42   |         42         |  0  | 100.00 |
|            stSolidityTest            |  23   |         23         |  0  | 100.00 |
|            stSpecialTest             |  19   |         19         |  0  | 100.00 |
|             stStackTests             |  375  |        375         |  0  | 100.00 |
|             stStaticCall             |  455  |        455         |  0  | 100.00 |
|         stStaticFlagEnabled          |  34   |         34         |  0  | 100.00 |
|        stSystemOperationsTest        |  83   |         83         |  0  | 100.00 |
|           stTimeConsuming            | 5187  |        5187        |  0  | 100.00 |
|          stTransactionTest           |  162  |        162         |  0  | 100.00 |
|           stTransitionTest           |   6   |         6          |  0  | 100.00 |
|             stWalletTest             |  46   |         46         |  0  | 100.00 |
|          stZeroCallsRevert           |  16   |         16         |  0  | 100.00 |
|           stZeroCallsTest            |  24   |         24         |  0  | 100.00 |
|           stZeroKnowledge2           |  519  |        519         |  0  | 100.00 |
|           stZeroKnowledge            |  944  |        944         |  0  | 100.00 |
|               VMTests                |  649  |        649         |  0  | 100.00 |


## Other

[Polygon Hermez](https://github.com/0xPolygonHermez) is doing something similar [here](https://github.com/0xPolygonHermez/zkevm-testvectors).

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
