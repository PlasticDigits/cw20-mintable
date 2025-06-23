# CW20 Mintable

This is a clean fork of the CW20 Basic contract with enhanced minter extension capabilities. It implements
the [CW20 spec](../../packages/cw20/README.md) with a focus on flexible minting functionality.

## Key Features

- **Clean CW20 Basic Implementation**: Core CW20 token functionality
- **Minters List Management**: Maintain a list of authorized minter addresses
- **Flexible Minting**: Authorized minters can mint new tokens
- **Allowances Support**: Standard CW20 allowance functionality
- **Marketing Extension**: Logo and marketing metadata support

## Minter Management

The contract includes a comprehensive minters list management system:

- **Minters List**: Maintain a list of addresses authorized to mint tokens
- **Add Minters**: Current minter can add new addresses to the minters list
- **Remove Minters**: Current minter can remove addresses from the minters list
- **Query Minters**: Paginated query to retrieve all addresses in the minters list
- **Authorization**: Both primary and additional minters are subject to the same minting cap restrictions

### Available Messages

**Execute Messages:**

- `AddMinter { minter: String }` - Add an address to the minters list
- `RemoveMinter { minter: String }` - Remove an address from the minters list

**Query Messages:**

- `Minters { start_after: Option<String>, limit: Option<u32> }` - Get all minters with pagination

The minting system works as follows:

- **Primary Minter**: The original minter specified during contract instantiation can always mint tokens
- **Additional Minters**: Any address in the minters list can also mint tokens
- **Authorization**: Both primary and additional minters are subject to the same minting cap restrictions

Implements:

- [x] CW20 Base
- [x] Minters list management (add, remove, query)
- [x] Flexible minting using minters list
- [x] Allowances extension
- [x] Marketing extension

## Running this contract

You will need Rust 1.44.1+ with `wasm32-unknown-unknown` target installed.

You can run unit tests on this via:

`cargo test`

Once you are happy with the content, you can compile it to wasm via:

```
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ./target/wasm32-unknown-unknown/release/cw20_mintable.wasm .
ls -l cw20_mintable.wasm
sha256sum cw20_mintable.wasm
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

## Code coverage

Recommended: `cargo install cargo-tarpaulin`
You can then run `cargo tarpaulin -o html`
