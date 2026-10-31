//! Token-local protection policy. Rolling accounting is never reset by settings changes.
use crate::{msg::ExecuteMsg, state::BALANCES, ContractError};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128};
use cw20_security_interface::{GuardResponse, QueryMsg as ControllerQuery, INTERFACE_VERSION};
use cw_storage_plus::{Item, Map};

pub const DAY: u64 = 86400;
pub const MAX_DELAY: u64 = 31536000;
#[cw_serde]
pub struct SecurityInit {
    pub controller: String,
    pub governance: String,
}
#[cw_serde]
pub struct SecurityConfig {
    pub controller: Addr,
}
#[cw_serde]
pub struct WalletPolicy {
    pub enabled: bool,
    pub limit_bps: u16,
    pub relaxation_delay: u64,
    pub unlock_delay: u64,
}
impl Default for WalletPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            limit_bps: 1000,
            relaxation_delay: DAY,
            unlock_delay: 2 * DAY,
        }
    }
}
#[cw_serde]
pub struct Bucket {
    pub hour: u64,
    pub amount: Uint128,
}
#[cw_serde]
pub struct WalletState {
    pub policy: WalletPolicy,
    pub protected_balance: Uint128,
    pub locked: bool,
    pub buckets: Vec<Bucket>,
}
#[cw_serde]
pub enum Change {
    Policy(WalletPolicy),
    Rebase(Uint128),
    Unlock,
}
#[cw_serde]
pub struct PendingChange {
    pub nonce: u64,
    pub initiator: String,
    pub proposed_at: u64,
    pub executable_at: u64,
    pub change: Change,
}
#[cw_serde]
#[derive(Default)]
pub struct Assignment {
    pub protocol: bool,
    pub exempt: bool,
    pub allow_lock: bool,
    pub evidence_hash: String,
}
#[cw_serde]
pub struct PendingAssignment {
    pub nonce: u64,
    pub initiator: String,
    pub proposed_at: u64,
    pub executable_at: u64,
    pub authority_epoch: u64,
    pub assignment: Assignment,
}
#[cw_serde]
pub struct WalletResponse {
    pub wallet: String,
    pub state: WalletState,
    pub pending: Option<PendingChange>,
    pub assignment: Assignment,
    pub pending_assignment: Option<PendingAssignment>,
    pub reserved: Uint128,
    pub rolling_spent: Uint128,
}
pub const CONFIG: Item<SecurityConfig> = Item::new("security_v1_config");
pub const NONCE: Item<u64> = Item::new("security_v1_nonce");
pub const WALLETS: Map<&Addr, WalletState> = Map::new("security_v1_wallets");
pub const PENDING: Map<&Addr, PendingChange> = Map::new("security_v1_pending");
pub const ASSIGNMENTS: Map<&Addr, Assignment> = Map::new("security_v1_assignments");
pub const PENDING_ASSIGNMENTS: Map<&Addr, PendingAssignment> =
    Map::new("security_v1_pending_assignments");
pub const RESERVED: Map<&Addr, Uint128> = Map::new("security_v1_reserved");
pub fn error(msg: &str) -> ContractError {
    StdError::generic_err(msg).into()
}
/// Whole-second deadlines round UP from fractional timestamps, never mature early.
pub fn deadline(env: &Env, delay: u64) -> Result<u64, ContractError> {
    env.block
        .time
        .seconds()
        .checked_add(delay)
        .and_then(|s| s.checked_add(u64::from(env.block.time.subsec_nanos() != 0)))
        .ok_or_else(|| error("time overflow"))
}
pub fn next_nonce(storage: &mut dyn cosmwasm_std::Storage) -> StdResult<u64> {
    let n = NONCE
        .may_load(storage)?
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| StdError::generic_err("nonce exhausted"))?;
    NONCE.save(storage, &n)?;
    Ok(n)
}
pub fn init(deps: DepsMut, env: &Env, security: SecurityInit) -> Result<(), ContractError> {
    if CONFIG.may_load(deps.storage)?.is_some() {
        return Err(error("security already configured"));
    }
    let controller = deps.api.addr_validate(&security.controller)?;
    let governance = deps.api.addr_validate(&security.governance)?;
    let config: cw20_security_interface::ConfigResponse = deps
        .querier
        .query_wasm_smart(&controller, &ControllerQuery::Config {})?;
    if config.interface_version != INTERFACE_VERSION || config.roles.governance != governance {
        return Err(error("controller governance/version mismatch"));
    }
    let _ = env;
    CONFIG.save(deps.storage, &SecurityConfig { controller })?;
    Ok(())
}
pub fn controller(
    deps: Deps,
    env: &Env,
    addresses: Vec<String>,
) -> Result<GuardResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let n = addresses.len();
    let g: GuardResponse = deps.querier.query_wasm_smart(
        config.controller,
        &ControllerQuery::Guard {
            token: env.contract.address.to_string(),
            addresses,
        },
    )?;
    if g.interface_version != INTERFACE_VERSION || !g.enrolled || g.blocked.len() != n {
        return Err(error("invalid controller response or unenrolled token"));
    }
    if let Some(token) = crate::state::TOKEN_INFO.may_load(deps.storage)? {
        let gov = deps.api.addr_validate(&g.roles.governance)?;
        if token.mint.map(|m| m.minter == gov).unwrap_or(false)
            || crate::state::MINTERS.has(deps.storage, &gov)
        {
            return Err(error("controller governance overlaps token minter"));
        }
    }
    Ok(g)
}
pub fn wallet(deps: Deps, owner: &Addr) -> StdResult<WalletState> {
    Ok(WALLETS
        .may_load(deps.storage, owner)?
        .unwrap_or(WalletState {
            policy: WalletPolicy::default(),
            protected_balance: BALANCES.may_load(deps.storage, owner)?.unwrap_or_default(),
            locked: false,
            buckets: vec![],
        }))
}
fn prune(s: &mut WalletState, now: u64) {
    let hour = now / 3600;
    s.buckets.retain(|b| b.hour.saturating_add(24) >= hour);
}
pub fn spent(s: &WalletState, now: u64) -> StdResult<Uint128> {
    s.buckets
        .iter()
        .filter(|b| b.hour.saturating_add(24) >= now / 3600)
        .try_fold(Uint128::zero(), |a, b| {
            a.checked_add(b.amount).map_err(StdError::overflow)
        })
}
pub fn debit(deps: DepsMut, env: &Env, owner: &Addr, amount: Uint128) -> Result<(), ContractError> {
    let balance = BALANCES.may_load(deps.storage, owner)?.unwrap_or_default();
    let remaining = balance.checked_sub(amount).map_err(StdError::overflow)?;
    if remaining < RESERVED.may_load(deps.storage, owner)?.unwrap_or_default() {
        return Err(error("recovery reservation"));
    }
    let mut s = wallet(deps.as_ref(), owner)?;
    if s.locked {
        return Err(error("wallet locked"));
    }
    prune(&mut s, env.block.time.seconds());
    let assignment = ASSIGNMENTS
        .may_load(deps.storage, owner)?
        .unwrap_or_default();
    let total = spent(&s, env.block.time.seconds())?
        .checked_add(amount)
        .map_err(StdError::overflow)?;
    if s.policy.enabled
        && !assignment.exempt
        && total
            > s.protected_balance
                .multiply_ratio(s.policy.limit_bps as u128, 10000u128)
    {
        return Err(error("rolling outgoing limit"));
    }
    let hour = env.block.time.seconds() / 3600;
    if let Some(b) = s.buckets.iter_mut().find(|b| b.hour == hour) {
        b.amount = b.amount.checked_add(amount).map_err(StdError::overflow)?;
    } else {
        s.buckets.push(Bucket { hour, amount });
    }
    WALLETS.save(deps.storage, owner, &s)?;
    Ok(())
}
/// One authoritative query per ordinary economic entrypoint, no cached pause state.
pub fn guard(
    mut deps: DepsMut,
    env: &Env,
    info: &MessageInfo,
    msg: &ExecuteMsg,
) -> Result<(), ContractError> {
    let (owner, recipient, debit_amount, credit_amount) = match msg {
        ExecuteMsg::Transfer { recipient, amount } => (
            info.sender.to_string(),
            Some(recipient.clone()),
            Some(*amount),
            Some(*amount),
        ),
        ExecuteMsg::Send {
            contract, amount, ..
        } => (
            info.sender.to_string(),
            Some(contract.clone()),
            Some(*amount),
            Some(*amount),
        ),
        ExecuteMsg::Burn { amount } => (info.sender.to_string(), None, Some(*amount), None),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => (
            owner.clone(),
            Some(recipient.clone()),
            Some(*amount),
            Some(*amount),
        ),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            ..
        } => (
            owner.clone(),
            Some(contract.clone()),
            Some(*amount),
            Some(*amount),
        ),
        ExecuteMsg::BurnFrom { owner, amount } => (owner.clone(), None, Some(*amount), None),
        ExecuteMsg::Mint { recipient, amount } => (
            info.sender.to_string(),
            Some(recipient.clone()),
            None,
            Some(*amount),
        ),
        ExecuteMsg::IncreaseAllowance { spender, .. } => {
            (info.sender.to_string(), Some(spender.clone()), None, None)
        }
        ExecuteMsg::UpdateMinter { .. }
        | ExecuteMsg::AddMinter { .. }
        | ExecuteMsg::RemoveMinter { .. } => (info.sender.to_string(), None, None, None),
        _ => return Ok(()),
    };
    let owner = deps.api.addr_validate(&owner)?;
    let mut addresses = vec![owner.to_string(), info.sender.to_string()];
    let recipient = recipient.map(|a| deps.api.addr_validate(&a)).transpose()?;
    if let Some(r) = &recipient {
        addresses.push(r.to_string());
    }
    let g = controller(deps.as_ref(), env, addresses)?;
    if g.paused || g.blocked.iter().any(|b| *b) {
        return Err(error("controller containment"));
    }
    match msg {
        ExecuteMsg::Mint { .. } if info.sender == g.roles.governance => {
            return Err(error("governance cannot be minter"))
        }
        ExecuteMsg::AddMinter { minter } if *minter == g.roles.governance => {
            return Err(error("governance cannot be minter"))
        }
        ExecuteMsg::UpdateMinter {
            new_minter: Some(minter),
        } if *minter == g.roles.governance => return Err(error("governance cannot be minter")),
        _ => {}
    }
    if matches!(msg, ExecuteMsg::IncreaseAllowance { .. }) && wallet(deps.as_ref(), &owner)?.locked
    {
        return Err(error("wallet locked"));
    }
    if matches!(
        msg,
        ExecuteMsg::TransferFrom { .. } | ExecuteMsg::BurnFrom { .. } | ExecuteMsg::SendFrom { .. }
    ) {
        if let Some(a) = crate::state::ALLOWANCES.may_load(deps.storage, (&owner, &info.sender))? {
            if a.expires.is_expired(&env.block) {
                return Err(ContractError::Expired {});
            }
        }
    }
    if let Some(a) = debit_amount {
        debit(deps.branch(), env, &owner, a)?;
    }
    let _ = (recipient, credit_amount); // Recipients stay lazy until their first owner action.
    Ok(())
}
fn validate_policy(p: &WalletPolicy) -> Result<(), ContractError> {
    if p.limit_bps > 10000
        || p.relaxation_delay < DAY
        || p.relaxation_delay > MAX_DELAY
        || p.unlock_delay < 2 * DAY
        || p.unlock_delay > MAX_DELAY
    {
        return Err(error("policy bounds"));
    }
    Ok(())
}
fn timer(env: &Env, action: &str, wallet: &Addr, p: &PendingChange) -> Response {
    Response::new()
        .add_attribute("action", action)
        .add_attribute("token", env.contract.address.to_string())
        .add_attribute("wallet", wallet)
        .add_attribute("nonce", p.nonce.to_string())
        .add_attribute("initiator", &p.initiator)
        .add_attribute("proposed_at", p.proposed_at.to_string())
        .add_attribute("executable_at", p.executable_at.to_string())
}
fn schedule(
    deps: DepsMut,
    env: &Env,
    owner: &Addr,
    change: Change,
    delay: u64,
) -> Result<Response, ContractError> {
    let now = env.block.time.seconds();
    let p = PendingChange {
        nonce: next_nonce(deps.storage)?,
        initiator: owner.to_string(),
        proposed_at: now,
        executable_at: deadline(env, delay)?,
        change,
    };
    let previous = PENDING.may_load(deps.storage, owner)?;
    PENDING.save(deps.storage, owner, &p)?;
    let response = timer(env, "schedule_wallet_change", owner, &p);
    Ok(if let Some(old) = previous {
        response.add_attribute("replaced_nonce", old.nonce.to_string())
    } else {
        response
    })
}
pub fn settings(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    p: WalletPolicy,
) -> Result<Response, ContractError> {
    validate_policy(&p)?;
    let mut s = wallet(deps.as_ref(), &info.sender)?;
    let weak = (!p.enabled && s.policy.enabled)
        || p.limit_bps > s.policy.limit_bps
        || p.relaxation_delay < s.policy.relaxation_delay
        || p.unlock_delay < s.policy.unlock_delay;
    if weak {
        let delay = s.policy.relaxation_delay.max(DAY);
        WALLETS.save(deps.storage, &info.sender, &s)?;
        return schedule(deps, &env, &info.sender, Change::Policy(p), delay);
    }
    s.policy = p;
    WALLETS.save(deps.storage, &info.sender, &s)?;
    // Any intervening tightening invalidates prior relaxation and unlock payloads.
    let old = PENDING.may_load(deps.storage, &info.sender)?;
    PENDING.remove(deps.storage, &info.sender);
    let response = Response::new()
        .add_attribute("action", "tighten_wallet_policy")
        .add_attribute("wallet", info.sender);
    Ok(if let Some(old) = old {
        response.add_attribute("cancelled_nonce", old.nonce.to_string())
    } else {
        response
    })
}
pub fn rebase(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let s = wallet(deps.as_ref(), &info.sender)?;
    WALLETS.save(deps.storage, &info.sender, &s)?;
    let balance = BALANCES
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();
    schedule(
        deps,
        &env,
        &info.sender,
        Change::Rebase(balance),
        s.policy.relaxation_delay,
    )
}
pub fn lock(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let a = ASSIGNMENTS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();
    if a.protocol && !a.allow_lock {
        return Err(error("protocol lock requires reviewed assignment"));
    }
    let mut s = wallet(deps.as_ref(), &info.sender)?;
    s.locked = true;
    WALLETS.save(deps.storage, &info.sender, &s)?;
    let old = PENDING.may_load(deps.storage, &info.sender)?;
    PENDING.remove(deps.storage, &info.sender);
    let response = Response::new()
        .add_attribute("action", "lock_wallet")
        .add_attribute("wallet", info.sender);
    Ok(if let Some(old) = old {
        response.add_attribute("cancelled_nonce", old.nonce.to_string())
    } else {
        response
    })
}
pub fn unlock(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let s = wallet(deps.as_ref(), &info.sender)?;
    if !s.locked {
        return Err(error("not locked"));
    }
    schedule(
        deps,
        &env,
        &info.sender,
        Change::Unlock,
        s.policy.unlock_delay,
    )
}
pub fn cancel(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let p = PENDING.load(deps.storage, &info.sender)?;
    PENDING.remove(deps.storage, &info.sender);
    Ok(Response::new()
        .add_attribute("action", "cancel_wallet_change")
        .add_attribute("wallet", info.sender)
        .add_attribute("nonce", p.nonce.to_string()))
}
pub fn apply(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nonce: u64,
) -> Result<Response, ContractError> {
    let p = PENDING.load(deps.storage, &info.sender)?;
    if p.nonce != nonce || env.block.time.seconds() < p.executable_at {
        return Err(error("stale or immature wallet change"));
    }
    let mut s = wallet(deps.as_ref(), &info.sender)?;
    match p.change.clone() {
        Change::Policy(policy) => s.policy = policy,
        Change::Rebase(b) => s.protected_balance = b,
        Change::Unlock => s.locked = false,
    }
    WALLETS.save(deps.storage, &info.sender, &s)?;
    PENDING.remove(deps.storage, &info.sender);
    Ok(timer(&env, "execute_wallet_change", &info.sender, &p))
}
pub fn assignment(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
    a: Assignment,
) -> Result<Response, ContractError> {
    let g = controller(deps.as_ref(), &env, vec![])?;
    if info.sender != g.roles.governance {
        return Err(ContractError::Unauthorized {});
    }
    digest(&a.evidence_hash)?;
    let owner = deps.api.addr_validate(&address)?;
    let old = ASSIGNMENTS
        .may_load(deps.storage, &owner)?
        .unwrap_or_default();
    let s = wallet(deps.as_ref(), &owner)?;
    if a.protocol && !a.allow_lock && s.locked {
        return Err(error("locked protocol incompatible"));
    }
    // Privilege enabling and protocol classification changes are delayed. Removing exemption is immediate.
    if (a.exempt && !old.exempt) || a.protocol != old.protocol || (a.allow_lock && !old.allow_lock)
    {
        let now = env.block.time.seconds();
        let p = PendingAssignment {
            nonce: next_nonce(deps.storage)?,
            initiator: info.sender.to_string(),
            proposed_at: now,
            executable_at: deadline(&env, s.policy.relaxation_delay)?,
            authority_epoch: g.authority_epoch,
            assignment: a,
        };
        PENDING_ASSIGNMENTS.save(deps.storage, &owner, &p)?;
        Ok(Response::new()
            .add_attribute("action", "schedule_assignment")
            .add_attribute("token", env.contract.address)
            .add_attribute("wallet", owner)
            .add_attribute("nonce", p.nonce.to_string())
            .add_attribute("initiator", p.initiator)
            .add_attribute("proposed_at", now.to_string())
            .add_attribute("executable_at", p.executable_at.to_string()))
    } else {
        ASSIGNMENTS.save(deps.storage, &owner, &a)?;
        PENDING_ASSIGNMENTS.remove(deps.storage, &owner);
        Ok(Response::new()
            .add_attribute("action", "tighten_assignment")
            .add_attribute("wallet", owner))
    }
}
pub fn apply_assignment(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
    nonce: u64,
) -> Result<Response, ContractError> {
    let owner = deps.api.addr_validate(&address)?;
    let p = PENDING_ASSIGNMENTS.load(deps.storage, &owner)?;
    let g = controller(deps.as_ref(), &env, vec![])?;
    if info.sender != g.roles.governance {
        return Err(ContractError::Unauthorized {});
    }
    if p.nonce != nonce
        || p.authority_epoch != g.authority_epoch
        || env.block.time.seconds() < p.executable_at
    {
        return Err(error("stale or immature assignment"));
    }
    let s = wallet(deps.as_ref(), &owner)?;
    if p.assignment.protocol && !p.assignment.allow_lock && s.locked {
        return Err(error("locked protocol incompatible"));
    }
    ASSIGNMENTS.save(deps.storage, &owner, &p.assignment)?;
    PENDING_ASSIGNMENTS.remove(deps.storage, &owner);
    Ok(Response::new()
        .add_attribute("action", "execute_assignment")
        .add_attribute("wallet", owner)
        .add_attribute("nonce", nonce.to_string())
        .add_attribute("evidence_hash", p.assignment.evidence_hash))
}
pub fn cancel_assignment(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let g = controller(deps.as_ref(), &env, vec![])?;
    if info.sender != g.roles.governance {
        return Err(ContractError::Unauthorized {});
    }
    let owner = deps.api.addr_validate(&address)?;
    let p = PENDING_ASSIGNMENTS.load(deps.storage, &owner)?;
    PENDING_ASSIGNMENTS.remove(deps.storage, &owner);
    Ok(Response::new()
        .add_attribute("action", "cancel_assignment")
        .add_attribute("wallet", owner)
        .add_attribute("nonce", p.nonce.to_string()))
}
pub fn digest(hash: &str) -> Result<(), ContractError> {
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err(error("expected lowercase SHA-256 hex digest"));
    }
    Ok(())
}
pub fn query_wallet(deps: Deps, env: Env, address: String) -> StdResult<WalletResponse> {
    let owner = deps.api.addr_validate(&address)?;
    let state = wallet(deps, &owner)?;
    let rolling_spent = spent(&state, env.block.time.seconds())?;
    Ok(WalletResponse {
        wallet: address,
        state,
        pending: PENDING.may_load(deps.storage, &owner)?,
        assignment: ASSIGNMENTS
            .may_load(deps.storage, &owner)?
            .unwrap_or_default(),
        pending_assignment: PENDING_ASSIGNMENTS.may_load(deps.storage, &owner)?,
        reserved: RESERVED.may_load(deps.storage, &owner)?.unwrap_or_default(),
        rolling_spent,
    })
}
