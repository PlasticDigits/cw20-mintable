//! Explicit adjudicated recovery; ordinary movement guards never authorize this path.
use crate::{
    security::{self, RESERVED},
    state::BALANCES,
    ContractError,
};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128};
use cw_storage_plus::Map;
pub const MIN_RECOVERY_DELAY: u64 = 2592000;
#[cw_serde]
pub struct Recovery {
    pub nonce: u64,
    pub token: String,
    pub source: Addr,
    pub destination: Addr,
    pub amount: Uint128,
    pub evidence_hash: String,
    pub initiator: String,
    pub proposed_at: u64,
    pub executable_at: u64,
    pub authority_epoch: u64,
    pub accepted: bool,
    pub revoked: bool,
    pub objected: bool,
    pub objection_resolved: bool,
    pub objection_resolution_hash: Option<String>,
    pub vetoed: bool,
    pub cancellation_requested: bool,
    pub completed: bool,
    pub cancelled: bool,
    pub replaced_by: Option<u64>,
}
pub const RECOVERIES: Map<u64, Recovery> = Map::new("security_v1_recoveries");
fn event(action: &str, r: &Recovery) -> Response {
    Response::new()
        .add_attribute("action", action)
        .add_attribute("token", &r.token)
        .add_attribute("source", &r.source)
        .add_attribute("destination", &r.destination)
        .add_attribute("amount", r.amount)
        .add_attribute("evidence_hash", &r.evidence_hash)
        .add_attribute("nonce", r.nonce.to_string())
        .add_attribute("initiator", &r.initiator)
        .add_attribute("proposed_at", r.proposed_at.to_string())
        .add_attribute("executable_at", r.executable_at.to_string())
}
fn active(r: &Recovery) -> Result<(), ContractError> {
    if r.completed || r.cancelled || r.replaced_by.is_some() {
        return Err(security::error("inactive recovery"));
    }
    Ok(())
}
fn release(storage: &mut dyn cosmwasm_std::Storage, r: &Recovery) -> StdResult<()> {
    let reserved = RESERVED.load(storage, &r.source)?;
    RESERVED.save(
        storage,
        &r.source,
        &reserved.checked_sub(r.amount).map_err(StdError::overflow)?,
    )?;
    Ok(())
}
pub fn propose(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    source: String,
    destination: String,
    amount: Uint128,
    evidence_hash: String,
    replace: Option<u64>,
) -> Result<Response, ContractError> {
    security::digest(&evidence_hash)?;
    let source = deps.api.addr_validate(&source)?;
    let destination = deps.api.addr_validate(&destination)?;
    if source == destination || amount.is_zero() {
        return Err(security::error(
            "invalid recovery source/destination/amount",
        ));
    }
    let g = security::controller(deps.as_ref(), &env, vec![destination.to_string()])?;
    if info.sender != g.roles.governance {
        return Err(ContractError::Unauthorized {});
    }
    if g.blocked[0] {
        return Err(security::error("recovery destination blacklisted"));
    }
    let mut old = replace
        .map(|n| RECOVERIES.load(deps.storage, n))
        .transpose()?;
    let reserved = RESERVED
        .may_load(deps.storage, &source)?
        .unwrap_or_default();
    let prior = if let Some(r) = &old {
        active(r)?;
        if r.source != source {
            return Err(security::error("replacement must retain source"));
        }
        r.amount
    } else {
        Uint128::zero()
    };
    let new_reserved = reserved
        .checked_sub(prior)
        .map_err(StdError::overflow)?
        .checked_add(amount)
        .map_err(StdError::overflow)?;
    if new_reserved
        > BALANCES
            .may_load(deps.storage, &source)?
            .unwrap_or_default()
    {
        return Err(security::error("insufficient unreserved balance"));
    }
    let now = env.block.time.seconds();
    let r = Recovery {
        nonce: security::next_nonce(deps.storage)?,
        token: env.contract.address.to_string(),
        source: source.clone(),
        destination,
        amount,
        evidence_hash,
        initiator: info.sender.to_string(),
        proposed_at: now,
        executable_at: now
            .checked_add(MIN_RECOVERY_DELAY)
            .ok_or_else(|| security::error("time overflow"))?,
        authority_epoch: g.authority_epoch,
        accepted: false,
        revoked: false,
        objected: false,
        objection_resolved: false,
        objection_resolution_hash: None,
        vetoed: false,
        cancellation_requested: false,
        completed: false,
        cancelled: false,
        replaced_by: None,
    };
    // Preserve opening protection balance before any later recovery debit/credit.
    let state = security::wallet(deps.as_ref(), &source)?;
    security::WALLETS.save(deps.storage, &source, &state)?;
    RESERVED.save(deps.storage, &source, &new_reserved)?;
    if let Some(o) = &mut old {
        o.replaced_by = Some(r.nonce);
        RECOVERIES.save(deps.storage, o.nonce, o)?;
    }
    RECOVERIES.save(deps.storage, r.nonce, &r)?;
    let mut response = event("propose_recovery", &r);
    if let Some(o) = old {
        response = response.add_event(
            cosmwasm_std::Event::new("recovery_replaced")
                .add_attribute("action", "replace_recovery")
                .add_attribute("token", r.token.clone())
                .add_attribute("nonce", o.nonce.to_string())
                .add_attribute("replacement_nonce", r.nonce.to_string()),
        );
    }
    let _ = &mut deps;
    Ok(response)
}
pub fn accept(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nonce: u64,
    revoke: bool,
) -> Result<Response, ContractError> {
    let mut r = RECOVERIES.load(deps.storage, nonce)?;
    active(&r)?;
    if info.sender != r.destination {
        return Err(ContractError::Unauthorized {});
    }
    if revoke {
        r.accepted = false;
        r.revoked = true;
    } else {
        if r.revoked {
            return Err(security::error("revoked acceptance requires replacement"));
        }
        let g = security::controller(deps.as_ref(), &env, vec![r.destination.to_string()])?;
        if g.authority_epoch != r.authority_epoch || g.blocked[0] {
            return Err(security::error("stale authority or blocked destination"));
        }
        r.accepted = true;
    }
    RECOVERIES.save(deps.storage, nonce, &r)?;
    Ok(event(
        if revoke {
            "revoke_recovery_acceptance"
        } else {
            "accept_recovery"
        },
        &r,
    ))
}
pub fn object(deps: DepsMut, info: MessageInfo, nonce: u64) -> Result<Response, ContractError> {
    let mut r = RECOVERIES.load(deps.storage, nonce)?;
    active(&r)?;
    if info.sender != r.source {
        return Err(ContractError::Unauthorized {});
    }
    r.objected = true;
    RECOVERIES.save(deps.storage, nonce, &r)?;
    Ok(event("object_recovery", &r))
}
pub fn resolve(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nonce: u64,
    evidence_hash: String,
) -> Result<Response, ContractError> {
    security::digest(&evidence_hash)?;
    let g = security::controller(deps.as_ref(), &env, vec![])?;
    if info.sender != g.roles.dispute {
        return Err(ContractError::Unauthorized {});
    }
    let mut r = RECOVERIES.load(deps.storage, nonce)?;
    active(&r)?;
    if !r.objected || r.vetoed || r.objection_resolved {
        return Err(security::error("no unresolved objection"));
    }
    r.objection_resolved = true;
    r.objection_resolution_hash = Some(evidence_hash.clone());
    RECOVERIES.save(deps.storage, nonce, &r)?;
    Ok(event("resolve_recovery_objection", &r).add_attribute("resolution_hash", evidence_hash))
}
pub fn veto(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nonce: u64,
) -> Result<Response, ContractError> {
    let g = security::controller(deps.as_ref(), &env, vec![])?;
    if info.sender != g.roles.dispute {
        return Err(ContractError::Unauthorized {});
    }
    let mut r = RECOVERIES.load(deps.storage, nonce)?;
    active(&r)?;
    r.vetoed = true;
    RECOVERIES.save(deps.storage, nonce, &r)?;
    Ok(event("veto_recovery", &r))
}
/// Cancellation is a two-role adjudication; source-key signatures cannot free a reservation.
pub fn cancel(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nonce: u64,
) -> Result<Response, ContractError> {
    let g = security::controller(deps.as_ref(), &env, vec![])?;
    let mut r = RECOVERIES.load(deps.storage, nonce)?;
    active(&r)?;
    if info.sender == g.roles.governance {
        r.cancellation_requested = true;
        RECOVERIES.save(deps.storage, nonce, &r)?;
        return Ok(event("request_cancel_recovery", &r));
    }
    if info.sender != g.roles.dispute || !r.cancellation_requested {
        return Err(ContractError::Unauthorized {});
    }
    release(deps.storage, &r)?;
    r.cancelled = true;
    RECOVERIES.save(deps.storage, nonce, &r)?;
    Ok(event("cancel_recovery", &r))
}
pub fn execute(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nonce: u64,
) -> Result<Response, ContractError> {
    let mut r = RECOVERIES.load(deps.storage, nonce)?;
    active(&r)?;
    let g = security::controller(deps.as_ref(), &env, vec![r.destination.to_string()])?;
    if info.sender != g.roles.recovery {
        return Err(ContractError::Unauthorized {});
    }
    if g.authority_epoch != r.authority_epoch || g.roles.governance != r.initiator {
        return Err(security::error("stale recovery authority"));
    }
    if env.block.time.seconds() < r.executable_at
        || !r.accepted
        || r.revoked
        || r.vetoed
        || r.cancellation_requested
        || (r.objected && !r.objection_resolved)
        || g.blocked[0]
    {
        return Err(security::error("recovery not executable"));
    }
    security::credit_init(deps.branch(), &r.destination, r.amount)?;
    let source = BALANCES
        .load(deps.storage, &r.source)?
        .checked_sub(r.amount)
        .map_err(StdError::overflow)?;
    let dest = BALANCES
        .may_load(deps.storage, &r.destination)?
        .unwrap_or_default()
        .checked_add(r.amount)
        .map_err(StdError::overflow)?;
    BALANCES.save(deps.storage, &r.source, &source)?;
    BALANCES.save(deps.storage, &r.destination, &dest)?;
    release(deps.storage, &r)?;
    r.completed = true;
    RECOVERIES.save(deps.storage, nonce, &r)?;
    Ok(event("execute_recovery", &r))
}
pub fn query(deps: Deps, nonce: u64) -> StdResult<Recovery> {
    RECOVERIES.load(deps.storage, nonce)
}
