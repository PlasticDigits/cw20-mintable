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

## Building this contract

You will need Rust 1.44.1+ with `wasm32-unknown-unknown` target installed.

### Using Docker (Recommended):

```
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.16.1
```

### Without Docker:

For optimization (without docker) you will need `wasm-opt` installed.

You can run unit tests on this via:

`cargo test`

Once you are happy with the content, you can compile it to wasm via:

```
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ./target/wasm32-unknown-unknown/release/cw20_mintable.wasm .
ls -l cw20_mintable.wasm
sha256sum cw20_mintable.wasm
```

For optimization, then use:
`wasm-opt -Oz -o cw20_mintable_optimized.wasm cw20_mintable.wasm`

## Deploying to chain

### Store the contract

```
wasmd tx wasm store artifacts/cw20_base.wasm \
  --from wallet-name \
  --gas auto --gas-adjustment 1.3 \
  --fees 5000stake \
  --broadcast-mode block \
  --chain-id [your-chain-id]
```

### Instantiate contract

First, create an init.json file

```
{
  "name": "My Token",
  "symbol": "MTK",
  "decimals": 18,
  "initial_balances": [
    {
      "address": "your_wallet_address",
      "amount": "1000000000"
    }
  ],
  "mint": {
    "minter": "your_wallet_address"
  },
  "marketing": {
    "project": "https://my-project.com",
    "description": "A detailed description of My Token and its utility",
    "marketing": "your_marketing_wallet_address",
    "logo": {
      "url": "https://my-project.com/logo.png"
    }
  }
}
```

## Importing this contract

You can also import much of the logic of this contract to build another
CW20-contract, such as a bonding curve, overiding or extending what you
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
