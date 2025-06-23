# CW20 Mintable

This is a clean fork of the CW20 Basic contract with enhanced minter extension capabilities. It implements
the [CW20 spec](../../packages/cw20/README.md) with a focus on flexible minting functionality.

## Key Features

- **Clean CW20 Basic Implementation**: Core CW20 token functionality
- **Enhanced Minter Extension**: Owner can add and remove minters
- **Flexible Minting**: Authorized minters can mint new tokens
- **Allowances Support**: Standard CW20 allowance functionality

## Minter Management

- Contract owner has full control over minter permissions
- Owner can add new minters to the authorized list
- Owner can remove existing minters from the authorized list
- Only authorized minters can mint new tokens

Implements:

- [x] CW20 Base
- [x] Enhanced Mintable extension with minter management
- [x] Allowances extension

## Running this contract

You will need Rust 1.44.1+ with `wasm32-unknown-unknown` target installed.

You can run unit tests on this via:

`cargo test`

Once you are happy with the content, you can compile it to wasm via:

```
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ../../target/wasm32-unknown-unknown/release/cw20_base.wasm .
ls -l cw20_base.wasm
sha256sum cw20_base.wasm
```

Or for a production-ready (optimized) build, run a build command in the
the repository root: https://github.com/CosmWasm/cw-plus#compiling.

## Importing this contract

You can also import much of the logic of this contract to build another
ERC20-contract, such as a bonding curve, overiding or extending what you
need.

Basically, you just need to write your handle function and import
`cw20_base::contract::handle_transfer`, etc and dispatch to them.
This allows you to use custom `ExecuteMsg` and `QueryMsg` with your additional
calls, but then use the underlying implementation for the standard cw20
messages you want to support. The same with `QueryMsg`. You _could_ reuse `instantiate`
as it, but it is likely you will want to change it. And it is rather simple.

Look at [`cw20-staking`](https://github.com/CosmWasm/cw-tokens/tree/main/contracts/cw20-staking) for an example of how to "inherit"
all this token functionality and combine it with custom logic.
