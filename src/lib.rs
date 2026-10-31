/*!
This is a basic implementation of a cw20 contract. It implements
the [CW20 spec](https://github.com/CosmWasm/cw-plus/blob/main/packages/cw20/README.md) and is designed to
be deployed as is, or imported into other contracts to easily build
cw20-compatible tokens with custom logic.

Implements:

- [x] CW20 Base
- [x] Mintable extension
- [x] Allowances extension

For more information on this contract, please check out the
[README](https://github.com/CosmWasm/cw-plus/blob/main/contracts/cw20-base/README.md).
*/

pub mod allowances;
pub mod contract;
pub mod enumerable;
mod error;
pub mod msg;
pub mod state;

pub use crate::error::ContractError;

pub mod recovery;
pub mod security;
#[cfg(test)]
pub(crate) mod test_support {
    use cosmwasm_std::{
        testing::{MockApi, MockQuerier},
        to_json_binary, ContractResult, MemoryStorage, OwnedDeps, SystemResult, WasmQuery,
    };
    pub fn mock_dependencies() -> OwnedDeps<MemoryStorage, MockApi, MockQuerier> {
        mock_dependencies_with_balance(&[])
    }
    pub fn mock_dependencies_with_balance(
        balance: &[cosmwasm_std::Coin],
    ) -> OwnedDeps<MemoryStorage, MockApi, MockQuerier> {
        let mut deps = cosmwasm_std::testing::mock_dependencies_with_balance(balance);
        deps.querier.update_wasm(|query| {
            let roles = cw20_security_interface::Roles {
                governance: "governance".into(),
                responder: Some("responder".into()),
                blacklist: "blacklist".into(),
                recovery: "recovery".into(),
                dispute: "dispute".into(),
            };
            let msg = match query {
                WasmQuery::Smart { msg, .. } => {
                    cosmwasm_std::from_json::<cw20_security_interface::QueryMsg>(msg).unwrap()
                }
                _ => panic!("unexpected query"),
            };
            let result = match msg {
                cw20_security_interface::QueryMsg::Config {} => {
                    to_json_binary(&cw20_security_interface::ConfigResponse {
                        interface_version: 1,
                        epoch: 0,
                        authority_epoch: 0,
                        paused: false,
                        roles,
                        rotation_delay: 172800,
                        pending_role: None,
                        pending_delay: None,
                    })
                    .unwrap()
                }
                cw20_security_interface::QueryMsg::Guard { addresses, .. } => {
                    to_json_binary(&cw20_security_interface::GuardResponse {
                        interface_version: 1,
                        epoch: 0,
                        authority_epoch: 0,
                        paused: false,
                        enrolled: true,
                        blocked: vec![false; addresses.len()],
                        roles,
                    })
                    .unwrap()
                }
                _ => panic!("unexpected controller test query"),
            };
            SystemResult::Ok(ContractResult::Ok(result))
        });
        deps
    }
}
