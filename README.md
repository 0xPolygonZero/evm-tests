# EVM Test
Parses and runs compatible common Ethereum tests from [ethereum/tests](https://github.com/ethereum/tests) against Polygon Zero's EVM.

> Note: This repo is currently very early in development and is not yet ready to evaluate the EVM completeness!

## Components

### Parser
Since the tests from the Ethereum test repo are meant for a full node, only certain tests are compatible with our EVM. Additionally, for the tests that are compatible, they need to be parsed (or converted) into a format that is usable by our EVM.

The parser has two responsibilities:
- Query the upstream Ethereum tests repo and check if any tests have been added/updated/removed.
- If there is a change, re-parse the tests.

### Runner
The runner feeds the parsed tests into the EVM. Successes are defined as no errors occurring (the tests themselves do not provide an expected final state). If the EVM returns an error or panics, then the test is considered to have failed.

The runner also outputs a results file (likely as a `*.md`) which contains statistics on the last test run.

## Other
[Polygon Hermez](https://github.com/0xPolygonHermez) is doing something similar [here](https://github.com/0xPolygonHermez/zkevm-testvectors).