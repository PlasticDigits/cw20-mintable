use cosmwasm_std::{Addr, Binary, Empty, Uint128};
use cw20_mintable::{
    contract,
    msg::{ExecuteMsg as T, InstantiateMsg, QueryMsg as Q},
    recovery::{Recovery, MIN_RECOVERY_DELAY},
    security::{Assignment, SecurityInit, WalletPolicy, WalletResponse, DAY},
};
use cw20_security_interface::{
    ConfigResponse, ExecuteMsg as C, InstantiateMsg as CI, QueryMsg as CQ, Role, Roles,
};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
fn token_code() -> Box<dyn Contract<Empty>> {
    Box::new(
        ContractWrapper::new(contract::execute, contract::instantiate, contract::query)
            .with_migrate(contract::migrate),
    )
}
fn controller_code() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(
        cw20_security_controller::contract::execute,
        cw20_security_controller::contract::instantiate,
        cw20_security_controller::contract::query,
    ))
}
struct F {
    app: App,
    controller: Addr,
    token: Addr,
    code: u64,
}
impl F {
    fn new() -> Self {
        let mut app = App::default();
        app.update_block(|b| b.time = cosmwasm_std::Timestamp::from_seconds(3600 * 100));
        let code = app.store_code(token_code());
        let cc = app.store_code(controller_code());
        let controller = app
            .instantiate_contract(
                cc,
                Addr::unchecked("governance"),
                &CI {
                    roles: Roles {
                        governance: "governance".into(),
                        responder: Some("responder".into()),
                        blacklist: "blacklist".into(),
                        recovery: "recovery".into(),
                        dispute: "dispute".into(),
                    },
                    rotation_delay: 2 * DAY,
                },
                &[],
                "controller",
                Some("governance".into()),
            )
            .unwrap();
        let token = app
            .instantiate_contract(
                code,
                Addr::unchecked("issuer"),
                &Self::init(&controller),
                &[],
                "token",
                Some("governance".into()),
            )
            .unwrap();
        let mut f = Self {
            app,
            controller,
            token,
            code,
        };
        f.c(
            "governance",
            C::SetEnrollment {
                token: f.token.to_string(),
                enrolled: true,
            },
        );
        f.c("governance", C::Unpause {});
        f
    }
    fn init(controller: &Addr) -> InstantiateMsg {
        InstantiateMsg {
            name: "Secure Token".into(),
            symbol: "SEC".into(),
            decimals: 6,
            initial_balances: vec![cw20::Cw20Coin {
                address: "alice".into(),
                amount: Uint128::new(10000),
            }],
            mint: Some(cw20::MinterResponse {
                minter: "issuer".into(),
                cap: None,
            }),
            marketing: None,
            security: Some(SecurityInit {
                controller: controller.to_string(),
                governance: "governance".into(),
            }),
        }
    }
    fn c(&mut self, who: &str, msg: C) {
        self.app
            .execute_contract(Addr::unchecked(who), self.controller.clone(), &msg, &[])
            .unwrap();
    }
    fn t(&mut self, who: &str, msg: T) {
        self.app
            .execute_contract(Addr::unchecked(who), self.token.clone(), &msg, &[])
            .unwrap();
    }
    fn fail(&mut self, who: &str, msg: T) {
        assert!(self
            .app
            .execute_contract(Addr::unchecked(who), self.token.clone(), &msg, &[])
            .is_err());
    }
    fn advance(&mut self, s: u64) {
        self.app.update_block(|b| {
            b.time = b.time.plus_seconds(s);
            b.height += 1;
        });
    }
    fn w(&self, address: &str) -> WalletResponse {
        self.app
            .wrap()
            .query_wasm_smart(
                &self.token,
                &Q::WalletSecurity {
                    address: address.into(),
                },
            )
            .unwrap()
    }
    fn balance(&self, address: &str) -> u128 {
        let b: cw20::BalanceResponse = self
            .app
            .wrap()
            .query_wasm_smart(
                &self.token,
                &Q::Balance {
                    address: address.into(),
                },
            )
            .unwrap();
        b.balance.u128()
    }
    fn r(&self, n: u64) -> Recovery {
        self.app
            .wrap()
            .query_wasm_smart(&self.token, &Q::Recovery { nonce: n })
            .unwrap()
    }
    fn config(&self) -> ConfigResponse {
        self.app
            .wrap()
            .query_wasm_smart(&self.controller, &CQ::Config {})
            .unwrap()
    }
    fn proposal(&mut self, amount: u128, replace: Option<u64>) -> u64 {
        let response = self
            .app
            .execute_contract(
                Addr::unchecked("governance"),
                self.token.clone(),
                &T::ProposeRecovery {
                    source: "alice".into(),
                    destination: "rescued".into(),
                    amount: Uint128::new(amount),
                    evidence_hash: "ab".repeat(32),
                    replace,
                },
                &[],
            )
            .unwrap();
        response
            .events
            .iter()
            .flat_map(|e| e.attributes.iter())
            .find(|a| a.key == "nonce")
            .unwrap()
            .value
            .parse()
            .unwrap()
    }
    fn transfer(amount: u128) -> T {
        T::Transfer {
            recipient: "bob".into(),
            amount: Uint128::new(amount),
        }
    }
}
#[test]
fn shared_pause_covers_two_tokens_every_ordinary_path_and_safe_revoke() {
    let mut f = F::new();
    let second = f
        .app
        .instantiate_contract(
            f.code,
            Addr::unchecked("issuer"),
            &F::init(&f.controller),
            &[],
            "second",
            None,
        )
        .unwrap();
    f.c(
        "governance",
        C::SetEnrollment {
            token: second.to_string(),
            enrolled: true,
        },
    );
    f.t(
        "alice",
        T::IncreaseAllowance {
            spender: "spender".into(),
            amount: Uint128::new(1000),
            expires: None,
        },
    );
    f.c("responder", C::Pause {});
    let a = Uint128::new(1);
    for (who, msg) in [
        ("alice", F::transfer(1)),
        (
            "alice",
            T::Send {
                contract: "receiver".into(),
                amount: a,
                msg: Binary::default(),
            },
        ),
        ("alice", T::Burn { amount: a }),
        (
            "spender",
            T::TransferFrom {
                owner: "alice".into(),
                recipient: "bob".into(),
                amount: a,
            },
        ),
        (
            "spender",
            T::SendFrom {
                owner: "alice".into(),
                contract: "receiver".into(),
                amount: a,
                msg: Binary::default(),
            },
        ),
        (
            "spender",
            T::BurnFrom {
                owner: "alice".into(),
                amount: a,
            },
        ),
        (
            "issuer",
            T::Mint {
                recipient: "bob".into(),
                amount: a,
            },
        ),
        (
            "alice",
            T::IncreaseAllowance {
                spender: "spender".into(),
                amount: a,
                expires: None,
            },
        ),
        (
            "issuer",
            T::AddMinter {
                minter: "other".into(),
            },
        ),
    ] {
        f.fail(who, msg);
    }
    assert!(f
        .app
        .execute_contract(Addr::unchecked("alice"), second, &F::transfer(1), &[])
        .is_err());
    f.t(
        "alice",
        T::DecreaseAllowance {
            spender: "spender".into(),
            amount: Uint128::new(1000),
            expires: None,
        },
    );
    assert_eq!(f.balance("alice"), 10000);
    assert_eq!(f.w("alice").rolling_spent.u128(), 0);
    assert!(f
        .app
        .execute_contract(
            Addr::unchecked("responder"),
            f.controller.clone(),
            &C::Unpause {},
            &[]
        )
        .is_err());
    f.advance(100 * DAY);
    assert!(f.config().paused);
}
#[test]
fn delegated_debits_share_owner_budget_and_rollback_failed_send() {
    let mut f = F::new();
    f.t(
        "alice",
        T::IncreaseAllowance {
            spender: "spender".into(),
            amount: Uint128::new(10000),
            expires: None,
        },
    );
    f.t(
        "spender",
        T::TransferFrom {
            owner: "alice".into(),
            recipient: "bob".into(),
            amount: Uint128::new(600),
        },
    );
    f.t(
        "spender",
        T::BurnFrom {
            owner: "alice".into(),
            amount: Uint128::new(400),
        },
    );
    f.fail("alice", F::transfer(1));
    f.fail(
        "spender",
        T::BurnFrom {
            owner: "alice".into(),
            amount: Uint128::new(1),
        },
    );
    assert_eq!(f.w("spender").rolling_spent.u128(), 0);
    assert_eq!(f.w("alice").rolling_spent.u128(), 1000);
    f.advance(25 * 3600);
    let before = f.w("alice");
    f.fail(
        "alice",
        T::Send {
            contract: "missing_receiver".into(),
            amount: Uint128::new(500),
            msg: Binary::default(),
        },
    );
    assert_eq!(f.w("alice"), before);
    assert_eq!(f.balance("alice"), 9000);
}
#[test]
fn rolling_boundary_split_and_topup_never_reset_history() {
    let mut f = F::new();
    f.advance(3599);
    f.t("alice", F::transfer(600));
    f.advance(1);
    for _ in 0..4 {
        f.t("alice", F::transfer(100));
    }
    f.t(
        "issuer",
        T::Mint {
            recipient: "alice".into(),
            amount: Uint128::new(10000),
        },
    );
    assert_eq!(f.w("alice").state.protected_balance.u128(), 10000);
    f.fail("alice", F::transfer(1));
    f.advance(23 * 3600);
    f.fail("alice", F::transfer(1));
    f.advance(3600);
    f.t("alice", F::transfer(600));
    f.fail("alice", F::transfer(1));
    assert!(f.w("alice").state.buckets.len() <= 25);
}
#[test]
fn incoming_dust_cannot_pin_new_wallet_basis() {
    let mut f = F::new();
    f.t(
        "issuer",
        T::Mint {
            recipient: "new_wallet".into(),
            amount: Uint128::new(1),
        },
    );
    f.t(
        "issuer",
        T::Mint {
            recipient: "new_wallet".into(),
            amount: Uint128::new(9999),
        },
    );
    f.t("new_wallet", F::transfer(1000));
    assert_eq!(f.w("new_wallet").state.protected_balance.u128(), 10000);
    f.t(
        "issuer",
        T::Mint {
            recipient: "new_wallet".into(),
            amount: Uint128::new(90000),
        },
    );
    f.fail("new_wallet", F::transfer(1));
    assert_eq!(f.w("new_wallet").state.protected_balance.u128(), 10000);
}
#[test]
fn policy_old_delay_replacement_tightening_cancel_and_replay() {
    let mut f = F::new();
    let mut p = WalletPolicy {
        enabled: false,
        ..WalletPolicy::default()
    };
    f.t("alice", T::SetWalletPolicy { policy: p.clone() });
    let first = f.w("alice").pending.unwrap();
    f.advance(DAY - 1);
    f.fail("alice", T::ExecuteWalletChange { nonce: first.nonce });
    let stronger = WalletPolicy {
        relaxation_delay: 2 * DAY,
        ..WalletPolicy::default()
    };
    f.t(
        "alice",
        T::SetWalletPolicy {
            policy: stronger.clone(),
        },
    );
    f.advance(1);
    f.fail("alice", T::ExecuteWalletChange { nonce: first.nonce });
    p.relaxation_delay = DAY;
    f.t("alice", T::SetWalletPolicy { policy: p.clone() });
    let second = f.w("alice").pending.unwrap();
    assert_eq!(second.executable_at - second.proposed_at, 2 * DAY);
    f.advance(DAY);
    f.t("alice", T::SetWalletPolicy { policy: p });
    let third = f.w("alice").pending.unwrap();
    assert!(third.nonce > second.nonce);
    assert_eq!(third.executable_at - third.proposed_at, 2 * DAY);
    f.advance(DAY);
    f.fail("alice", T::ExecuteWalletChange { nonce: third.nonce });
    f.fail(
        "alice",
        T::ExecuteWalletChange {
            nonce: second.nonce,
        },
    );
    f.t("alice", T::CancelWalletChange {});
    assert!(f.w("alice").state.policy.enabled);
    f.advance(DAY);
    f.fail("alice", T::ExecuteWalletChange { nonce: third.nonce });
}
#[test]
fn lock_unlock_relock_and_reviewed_protocol_compatibility() {
    let mut f = F::new();
    f.t("alice", T::LockWallet {});
    f.t("alice", T::RequestUnlock {});
    let n = f.w("alice").pending.unwrap().nonce;
    f.advance(2 * DAY - 1);
    f.fail("alice", T::ExecuteWalletChange { nonce: n });
    f.fail("alice", F::transfer(1));
    f.t("alice", T::LockWallet {});
    f.advance(1);
    f.fail("alice", T::ExecuteWalletChange { nonce: n });
    f.t("alice", T::RequestUnlock {});
    let n = f.w("alice").pending.unwrap().nonce;
    f.advance(2 * DAY);
    f.t("alice", T::ExecuteWalletChange { nonce: n });
    f.fail("alice", T::ExecuteWalletChange { nonce: n });
    let assignment = Assignment {
        protocol: true,
        exempt: true,
        allow_lock: false,
        evidence_hash: "cd".repeat(32),
    };
    f.t(
        "governance",
        T::SetAssignment {
            address: "alice".into(),
            assignment,
        },
    );
    let n = f.w("alice").pending_assignment.unwrap().nonce;
    f.fail(
        "governance",
        T::ExecuteAssignment {
            address: "alice".into(),
            nonce: n,
        },
    );
    f.advance(DAY);
    f.t(
        "governance",
        T::ExecuteAssignment {
            address: "alice".into(),
            nonce: n,
        },
    );
    f.fail("alice", T::LockWallet {});
    f.t("alice", F::transfer(9000));
    f.c("responder", C::Pause {});
    f.fail("alice", F::transfer(1));
}
#[test]
fn blacklist_owner_actor_recipient_mint_and_allowance_effects() {
    let mut f = F::new();
    f.t(
        "alice",
        T::IncreaseAllowance {
            spender: "spender".into(),
            amount: Uint128::new(1000),
            expires: None,
        },
    );
    for address in ["spender", "alice", "bob"] {
        f.c(
            "blacklist",
            C::SetBlacklist {
                address: address.into(),
                blocked: true,
            },
        );
        f.fail(
            "spender",
            T::TransferFrom {
                owner: "alice".into(),
                recipient: "bob".into(),
                amount: Uint128::new(1),
            },
        );
    }
    f.fail(
        "issuer",
        T::Mint {
            recipient: "bob".into(),
            amount: Uint128::new(1),
        },
    );
    f.fail(
        "alice",
        T::IncreaseAllowance {
            spender: "other".into(),
            amount: Uint128::new(1),
            expires: None,
        },
    );
    f.t(
        "alice",
        T::DecreaseAllowance {
            spender: "spender".into(),
            amount: Uint128::new(1),
            expires: None,
        },
    );
    assert_eq!(f.balance("alice"), 10000);
}
#[test]
fn unenrollment_is_fail_closed_and_governance_is_not_minter() {
    let mut f = F::new();
    f.c(
        "governance",
        C::SetEnrollment {
            token: f.token.to_string(),
            enrolled: false,
        },
    );
    f.fail("alice", F::transfer(1));
    f.fail(
        "issuer",
        T::Mint {
            recipient: "bob".into(),
            amount: Uint128::new(1),
        },
    );
    f.c(
        "governance",
        C::SetEnrollment {
            token: f.token.to_string(),
            enrolled: true,
        },
    );
    f.fail(
        "issuer",
        T::AddMinter {
            minter: "governance".into(),
        },
    );
    let mut msg = F::init(&f.controller);
    msg.security = None;
    assert!(f
        .app
        .instantiate_contract(f.code, Addr::unchecked("issuer"), &msg, &[], "unsafe", None)
        .is_err());
    let mut msg = F::init(&f.controller);
    msg.security.as_mut().unwrap().governance = "issuer".into();
    assert!(f
        .app
        .instantiate_contract(f.code, Addr::unchecked("issuer"), &msg, &[], "unsafe", None)
        .is_err());
}
#[test]
fn recovery_boundary_pause_blacklist_lock_exact_amount_supply_and_replay() {
    let mut f = F::new();
    let n = f.proposal(800, None);
    assert_eq!(f.w("alice").reserved.u128(), 800);
    f.t("rescued", T::AcceptRecovery { nonce: n });
    f.t("alice", T::LockWallet {});
    f.c(
        "blacklist",
        C::SetBlacklist {
            address: "alice".into(),
            blocked: true,
        },
    );
    f.c("responder", C::Pause {});
    f.advance(MIN_RECOVERY_DELAY - 1);
    f.fail("recovery", T::ExecuteRecovery { nonce: n });
    f.fail("governance", T::ExecuteRecovery { nonce: n });
    f.fail("responder", T::ExecuteRecovery { nonce: n });
    f.advance(1);
    f.t("recovery", T::ExecuteRecovery { nonce: n });
    assert_eq!(f.balance("alice"), 9200);
    assert_eq!(f.balance("rescued"), 800);
    assert_eq!(f.w("alice").reserved.u128(), 0);
    f.fail("recovery", T::ExecuteRecovery { nonce: n });
    let supply: cw20::TokenInfoResponse = f
        .app
        .wrap()
        .query_wasm_smart(&f.token, &Q::TokenInfo {})
        .unwrap();
    assert_eq!(supply.total_supply.u128(), 10000);
}
#[test]
fn reservations_source_cancel_reduction_and_overreservation_cannot_drain() {
    let mut f = F::new();
    let n = f.proposal(9500, None);
    f.fail("alice", F::transfer(501));
    f.fail("alice", T::CancelRecovery { nonce: n });
    f.fail(
        "governance",
        T::ProposeRecovery {
            source: "alice".into(),
            destination: "rescued".into(),
            amount: Uint128::new(1),
            evidence_hash: "ab".repeat(32),
            replace: Some(n),
        },
    );
    assert_eq!(f.w("alice").reserved.u128(), 9500);
    f.fail("alice", F::transfer(501));
    f.fail(
        "governance",
        T::ProposeRecovery {
            source: "alice".into(),
            destination: "rescued".into(),
            amount: Uint128::new(501),
            evidence_hash: "ab".repeat(32),
            replace: None,
        },
    );
    f.t("governance", T::CancelRecovery { nonce: n });
    assert_eq!(f.w("alice").reserved.u128(), 9500);
    f.fail("alice", F::transfer(501));
    f.t("dispute", T::CancelRecovery { nonce: n });
    assert_eq!(f.w("alice").reserved.u128(), 0);
    f.fail("dispute", T::CancelRecovery { nonce: n });
    f.fail("recovery", T::ExecuteRecovery { nonce: n });
}
#[test]
fn source_objections_are_triaged_once_independent_veto_keeps_reservation() {
    let mut f = F::new();
    let n = f.proposal(500, None);
    f.t("rescued", T::AcceptRecovery { nonce: n });
    f.t("alice", T::ObjectRecovery { nonce: n });
    let deadline = f.r(n).executable_at;
    f.advance(MIN_RECOVERY_DELAY);
    f.fail("recovery", T::ExecuteRecovery { nonce: n });
    f.fail(
        "governance",
        T::ResolveRecoveryObjection {
            nonce: n,
            evidence_hash: "ef".repeat(32),
        },
    );
    f.t(
        "dispute",
        T::ResolveRecoveryObjection {
            nonce: n,
            evidence_hash: "ef".repeat(32),
        },
    );
    for _ in 0..3 {
        f.t("alice", T::ObjectRecovery { nonce: n });
    }
    assert_eq!(f.r(n).executable_at, deadline);
    assert!(f.r(n).objection_resolved);
    f.t("dispute", T::VetoRecovery { nonce: n });
    f.fail("recovery", T::ExecuteRecovery { nonce: n });
    assert_eq!(f.w("alice").reserved.u128(), 500);
    let fresh = f.proposal(500, Some(n));
    assert_ne!(fresh, n);
    assert_eq!(f.w("alice").reserved.u128(), 500);
    assert_eq!(
        f.r(fresh).executable_at,
        f.app.block_info().time.seconds() + MIN_RECOVERY_DELAY
    );
    f.t("rescued", T::AcceptRecovery { nonce: fresh });
    f.fail("recovery", T::ExecuteRecovery { nonce: fresh });
    f.advance(MIN_RECOVERY_DELAY);
    f.t("recovery", T::ExecuteRecovery { nonce: fresh });
}
#[test]
fn destination_acceptance_revocation_and_blacklist_cannot_be_bypassed() {
    let mut f = F::new();
    let n = f.proposal(500, None);
    f.fail("alice", T::AcceptRecovery { nonce: n });
    f.advance(MIN_RECOVERY_DELAY);
    f.fail("recovery", T::ExecuteRecovery { nonce: n });
    f.t("rescued", T::AcceptRecovery { nonce: n });
    f.t("rescued", T::RevokeRecoveryAcceptance { nonce: n });
    f.fail("rescued", T::AcceptRecovery { nonce: n });
    f.fail("recovery", T::ExecuteRecovery { nonce: n });
    let n = f.proposal(500, Some(n));
    f.t("rescued", T::AcceptRecovery { nonce: n });
    f.c(
        "blacklist",
        C::SetBlacklist {
            address: "rescued".into(),
            blocked: true,
        },
    );
    f.advance(MIN_RECOVERY_DELAY);
    f.fail("recovery", T::ExecuteRecovery { nonce: n });
    assert_eq!(f.w("alice").reserved.u128(), 500);
}
#[test]
fn authority_rotation_invalidates_execution_and_old_cancel_without_unreserving() {
    let mut f = F::new();
    let n = f.proposal(500, None);
    f.t("rescued", T::AcceptRecovery { nonce: n });
    f.t("governance", T::CancelRecovery { nonce: n });
    f.c(
        "governance",
        C::ProposeRole {
            role: Role::Governance,
            candidate: "new_governance".into(),
        },
    );
    let nonce = f.config().pending_role.unwrap().nonce;
    f.advance(2 * DAY);
    f.c("new_governance", C::AcceptRole { nonce });
    f.advance(MIN_RECOVERY_DELAY);
    f.fail("recovery", T::ExecuteRecovery { nonce: n });
    f.fail("dispute", T::CancelRecovery { nonce: n });
    assert_eq!(f.w("alice").reserved.u128(), 500);
    f.fail("governance", T::CancelRecovery { nonce: n });
    f.t("new_governance", T::CancelRecovery { nonce: n });
    f.t("dispute", T::CancelRecovery { nonce: n });
    assert_eq!(f.w("alice").reserved.u128(), 0);
}
#[test]
fn legacy_pascal_and_snake_json_remain_compatible() {
    let upper: T =
        cosmwasm_std::from_json(br#"{"Transfer":{"recipient":"bob","amount":"1"}}"#).unwrap();
    let lower: T =
        cosmwasm_std::from_json(br#"{"transfer":{"recipient":"bob","amount":"1"}}"#).unwrap();
    assert_eq!(upper, lower);
    let upper: Q = cosmwasm_std::from_json(br#"{"Balance":{"address":"alice"}}"#).unwrap();
    let lower: Q = cosmwasm_std::from_json(br#"{"balance":{"address":"alice"}}"#).unwrap();
    assert_eq!(upper, lower);
}
#[test]
fn successful_six_debit_paths_all_charge_same_owner_history() {
    #[cosmwasm_schema::cw_serde]
    enum Receive {
        Receive(cw20::Cw20ReceiveMsg),
    }
    let receiver = ContractWrapper::new(
        |_d: cosmwasm_std::DepsMut,
         _e: cosmwasm_std::Env,
         _i: cosmwasm_std::MessageInfo,
         _m: Receive|
         -> cosmwasm_std::StdResult<cosmwasm_std::Response> {
            Ok(cosmwasm_std::Response::new())
        },
        |_d: cosmwasm_std::DepsMut,
         _e: cosmwasm_std::Env,
         _i: cosmwasm_std::MessageInfo,
         _m: Empty|
         -> cosmwasm_std::StdResult<cosmwasm_std::Response> {
            Ok(cosmwasm_std::Response::new())
        },
        |_d: cosmwasm_std::Deps,
         _e: cosmwasm_std::Env,
         _m: Empty|
         -> cosmwasm_std::StdResult<Binary> { Ok(Binary::default()) },
    );
    let mut f = F::new();
    let code = f.app.store_code(Box::new(receiver));
    let receiver = f
        .app
        .instantiate_contract(
            code,
            Addr::unchecked("anyone"),
            &Empty {},
            &[],
            "receiver",
            None,
        )
        .unwrap();
    let a = Uint128::new(100);
    f.t(
        "alice",
        T::IncreaseAllowance {
            spender: "spender".into(),
            amount: Uint128::new(1000),
            expires: None,
        },
    );
    for (who, msg) in [
        ("alice", F::transfer(100)),
        ("alice", T::Burn { amount: a }),
        (
            "alice",
            T::Send {
                contract: receiver.to_string(),
                amount: a,
                msg: Binary::default(),
            },
        ),
        (
            "spender",
            T::TransferFrom {
                owner: "alice".into(),
                recipient: "bob".into(),
                amount: a,
            },
        ),
        (
            "spender",
            T::BurnFrom {
                owner: "alice".into(),
                amount: a,
            },
        ),
        (
            "spender",
            T::SendFrom {
                owner: "alice".into(),
                contract: receiver.to_string(),
                amount: a,
                msg: Binary::default(),
            },
        ),
    ] {
        f.t(who, msg);
    }
    assert_eq!(f.w("alice").rolling_spent.u128(), 600);
    assert_eq!(f.balance("alice"), 9400);
    assert_eq!(f.balance(receiver.as_str()), 200);
}
#[test]
fn controller_query_errors_versions_and_malformed_guard_fail_closed_with_single_query() {
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, mock_info},
        to_json_binary, ContractResult, SystemResult,
    };
    use cw20_mintable::{
        security::{SecurityConfig, CONFIG, WALLETS},
        state::BALANCES,
    };
    use cw20_security_interface::GuardResponse;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let roles = Roles {
        governance: "governance".into(),
        responder: Some("responder".into()),
        blacklist: "blacklist".into(),
        recovery: "recovery".into(),
        dispute: "dispute".into(),
    };
    for scenario in 0..6 {
        let mut deps = mock_dependencies();
        CONFIG
            .save(
                deps.as_mut().storage,
                &SecurityConfig {
                    controller: Addr::unchecked("controller"),
                },
            )
            .unwrap();
        BALANCES
            .save(
                deps.as_mut().storage,
                &Addr::unchecked("alice"),
                &Uint128::new(10000),
            )
            .unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let roles = roles.clone();
        deps.querier.update_wasm(move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            if scenario == 0 {
                return SystemResult::Ok(ContractResult::Err("unavailable".into()));
            }
            if scenario == 1 {
                return SystemResult::Ok(ContractResult::Ok(Binary::from(b"malformed")));
            }
            SystemResult::Ok(ContractResult::Ok(
                to_json_binary(&GuardResponse {
                    interface_version: if scenario == 2 { 2 } else { 1 },
                    epoch: 1,
                    authority_epoch: 1,
                    paused: scenario == 5,
                    enrolled: scenario != 3,
                    blocked: vec![false; if scenario == 4 { 1 } else { 3 }],
                    roles: roles.clone(),
                })
                .unwrap(),
            ))
        });
        assert!(contract::execute(
            deps.as_mut(),
            mock_env(),
            mock_info("alice", &[]),
            F::transfer(1)
        )
        .is_err());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(
            BALANCES
                .load(deps.as_ref().storage, &Addr::unchecked("alice"))
                .unwrap()
                .u128(),
            10000
        );
        assert!(WALLETS
            .may_load(deps.as_ref().storage, &Addr::unchecked("alice"))
            .unwrap()
            .is_none());
    }
}
#[test]
fn expiry_extension_cannot_hide_inside_containment_safe_decrease() {
    let mut f = F::new();
    let expiration = cw20::Expiration::AtTime(f.app.block_info().time.plus_seconds(100));
    f.t(
        "alice",
        T::IncreaseAllowance {
            spender: "spender".into(),
            amount: Uint128::new(100),
            expires: Some(expiration),
        },
    );
    f.c("responder", C::Pause {});
    f.fail(
        "alice",
        T::DecreaseAllowance {
            spender: "spender".into(),
            amount: Uint128::new(1),
            expires: Some(cw20::Expiration::Never {}),
        },
    );
    f.fail(
        "alice",
        T::DecreaseAllowance {
            spender: "spender".into(),
            amount: Uint128::new(1),
            expires: Some(cw20::Expiration::AtHeight(9999999)),
        },
    );
    f.t(
        "alice",
        T::DecreaseAllowance {
            spender: "spender".into(),
            amount: Uint128::new(1),
            expires: Some(cw20::Expiration::AtTime(
                f.app.block_info().time.plus_seconds(50),
            )),
        },
    );
}
#[test]
fn rebase_and_policy_changes_preserve_spend_and_event_identity() {
    let mut f = F::new();
    f.t("alice", F::transfer(1000));
    f.t(
        "issuer",
        T::Mint {
            recipient: "alice".into(),
            amount: Uint128::new(11000),
        },
    );
    f.t("alice", T::RebaseProtection {});
    let p = f.w("alice").pending.unwrap();
    f.advance(DAY - 1);
    f.fail("alice", T::ExecuteWalletChange { nonce: p.nonce });
    f.advance(1);
    f.t("alice", T::ExecuteWalletChange { nonce: p.nonce });
    assert_eq!(f.w("alice").state.protected_balance.u128(), 20000);
    assert_eq!(f.w("alice").rolling_spent.u128(), 1000);
    f.t("alice", F::transfer(1000));
    f.fail("alice", F::transfer(1));
    f.c(
        "governance",
        C::ProposeRole {
            role: Role::Responder,
            candidate: "next_responder".into(),
        },
    );
    let cr = f.config().pending_role.unwrap();
    assert_eq!(cr.initiator, "governance");
    let events = f
        .app
        .execute_contract(
            Addr::unchecked("governance"),
            f.controller.clone(),
            &C::ProposeRole {
                role: Role::Responder,
                candidate: "other_responder".into(),
            },
            &[],
        )
        .unwrap()
        .events;
    let attributes: Vec<_> = events.iter().flat_map(|e| e.attributes.iter()).collect();
    for key in ["nonce", "initiator", "proposed_at", "executable_at"] {
        assert!(attributes.iter().any(|a| a.key == key));
    }
}
#[test]
fn governance_rotation_to_existing_issuer_fails_closed_in_token() {
    let mut f = F::new();
    f.c(
        "governance",
        C::ProposeRole {
            role: Role::Governance,
            candidate: "issuer".into(),
        },
    );
    let nonce = f.config().pending_role.unwrap().nonce;
    f.advance(2 * DAY);
    f.c("issuer", C::AcceptRole { nonce });
    f.fail(
        "issuer",
        T::Mint {
            recipient: "bob".into(),
            amount: Uint128::new(1),
        },
    );
    f.fail(
        "issuer",
        T::ProposeRecovery {
            source: "alice".into(),
            destination: "rescued".into(),
            amount: Uint128::new(100),
            evidence_hash: "ab".repeat(32),
            replace: None,
        },
    );
    f.fail("alice", F::transfer(1));
    // Controller rotation can repair overlap; pending application recovery never bypasses it.
    f.c(
        "issuer",
        C::ProposeRole {
            role: Role::Governance,
            candidate: "new_governance".into(),
        },
    );
    let nonce = f.config().pending_role.unwrap().nonce;
    f.advance(2 * DAY);
    f.c("new_governance", C::AcceptRole { nonce });
    f.t("alice", F::transfer(1));
}
#[test]
fn fractional_timer_starts_never_mature_before_full_minimum_duration() {
    let mut f = F::new();
    f.app
        .update_block(|b| b.time = b.time.plus_nanos(500_000_000));
    let n = f.proposal(500, None);
    f.t("rescued", T::AcceptRecovery { nonce: n });
    let r = f.r(n);
    assert_eq!(r.executable_at, r.proposed_at + MIN_RECOVERY_DELAY + 1);
    f.t(
        "alice",
        T::SetWalletPolicy {
            policy: WalletPolicy {
                enabled: false,
                ..WalletPolicy::default()
            },
        },
    );
    let p = f.w("alice").pending.unwrap();
    assert_eq!(p.executable_at, p.proposed_at + DAY + 1);
    f.advance(DAY);
    f.fail("alice", T::ExecuteWalletChange { nonce: p.nonce });
    f.advance(1);
    f.t("alice", T::ExecuteWalletChange { nonce: p.nonce });
    f.advance(MIN_RECOVERY_DELAY - DAY - 1);
    f.fail("recovery", T::ExecuteRecovery { nonce: n });
    f.advance(1);
    f.t("recovery", T::ExecuteRecovery { nonce: n });
}
