#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, Api, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult,
};
use cw20_security_interface::{
    BlacklistResponse, ConfigResponse, ExecuteMsg, GuardResponse, InstantiateMsg, PendingClear,
    PendingDelay, PendingRole, QueryMsg, Role, Roles, INTERFACE_VERSION, MAX_DELAY, MIN_ROLE_DELAY,
};

use crate::error::ContractError;
use crate::state::{State, StoredClear, BLOCKED, ENROLLED, PENDING_CLEAR, STATE};

const CONTRACT_NAME: &str = "crates.io:cw20-security-controller";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_GUARD_ADDRESSES: usize = 3;

fn add(value: u64, increment: u64) -> Result<u64, ContractError> {
    value.checked_add(increment).ok_or(ContractError::Overflow)
}

fn bump(state: &mut State, authority: bool) -> Result<(), ContractError> {
    state.epoch = add(state.epoch, 1)?;
    if authority {
        state.authority_epoch = add(state.authority_epoch, 1)?;
    }
    Ok(())
}

fn nonce(state: &mut State) -> Result<u64, ContractError> {
    state.last_nonce = add(state.last_nonce, 1)?;
    Ok(state.last_nonce)
}

fn validate_delay(seconds: u64) -> Result<(), ContractError> {
    if !(MIN_ROLE_DELAY..=MAX_DELAY).contains(&seconds) {
        return Err(ContractError::InvalidDelay);
    }
    Ok(())
}

fn validate_roles(api: &dyn Api, controller: &Addr, roles: &Roles) -> Result<(), ContractError> {
    let mut addresses = vec![
        api.addr_validate(&roles.governance)?,
        api.addr_validate(&roles.blacklist)?,
        api.addr_validate(&roles.recovery)?,
        api.addr_validate(&roles.dispute)?,
    ];
    if let Some(responder) = &roles.responder {
        addresses.push(api.addr_validate(responder)?);
    }
    for (index, address) in addresses.iter().enumerate() {
        if address == controller || addresses[..index].contains(address) {
            return Err(ContractError::UnsafeRoles);
        }
    }
    Ok(())
}

fn assign(roles: &mut Roles, role: &Role, candidate: String) {
    match role {
        Role::Governance => roles.governance = candidate,
        Role::Responder => roles.responder = Some(candidate),
        Role::Blacklist => roles.blacklist = candidate,
        Role::Recovery => roles.recovery = candidate,
        Role::Dispute => roles.dispute = candidate,
    }
}

fn governance(state: &State, sender: &Addr) -> Result<(), ContractError> {
    if sender.as_str() != state.roles.governance {
        return Err(ContractError::Unauthorized);
    }
    Ok(())
}

// Round the deadline UP if the block timestamp has a fractional second. ABI timestamps
// are integer seconds; truncating the deadline could otherwise permit early execution.
fn deadline(env: &Env, delay: u64) -> Result<u64, ContractError> {
    let rounded = add(
        env.block.time.seconds(),
        u64::from(env.block.time.subsec_nanos() != 0),
    )?;
    add(rounded, delay)
}

fn mature(env: &Env, executable_at: u64) -> Result<(), ContractError> {
    if env.block.time.seconds() < executable_at {
        return Err(ContractError::TooEarly);
    }
    Ok(())
}

fn no_funds(info: &MessageInfo) -> Result<(), ContractError> {
    if !info.funds.is_empty() {
        return Err(ContractError::FundsNotAccepted);
    }
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    no_funds(&info)?;
    validate_delay(msg.rotation_delay)?;
    validate_roles(deps.api, &env.contract.address, &msg.roles)?;
    let state = State {
        roles: msg.roles,
        paused: true,
        rotation_delay: msg.rotation_delay,
        epoch: 1,
        authority_epoch: 1,
        governance_epoch: 1,
        last_nonce: 0,
        pending_role: None,
        pending_delay: None,
    };
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("interface_version", INTERFACE_VERSION.to_string())
        .add_attribute("paused", "true")
        .add_attribute("governance", &state.roles.governance)
        .add_attribute(
            "responder",
            state.roles.responder.as_deref().unwrap_or("none"),
        )
        .add_attribute("blacklist", &state.roles.blacklist)
        .add_attribute("recovery", &state.roles.recovery)
        .add_attribute("dispute", &state.roles.dispute)
        .add_attribute("rotation_delay", state.rotation_delay.to_string())
        .add_attribute("epoch", "1")
        .add_attribute("authority_epoch", "1"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    no_funds(&info)?;
    let mut state = STATE.load(deps.storage)?;
    let mut response = Response::new();
    match msg {
        ExecuteMsg::Pause {} => {
            if info.sender.as_str() != state.roles.governance
                && state.roles.responder.as_deref() != Some(info.sender.as_str())
            {
                return Err(ContractError::Unauthorized);
            }
            if !state.paused {
                bump(&mut state, false)?;
                state.paused = true;
            }
            response = response.add_attribute("action", "pause");
        }
        ExecuteMsg::Unpause {} => {
            governance(&state, &info.sender)?;
            if state.paused {
                bump(&mut state, false)?;
                state.paused = false;
            }
            response = response.add_attribute("action", "unpause");
        }
        ExecuteMsg::SetBlacklist { address, blocked } => {
            let address = deps.api.addr_validate(&address)?;
            if blocked {
                if info.sender.as_str() != state.roles.governance
                    && info.sender.as_str() != state.roles.blacklist
                {
                    return Err(ContractError::Unauthorized);
                }
                let pending = PENDING_CLEAR.may_load(deps.storage, &address)?;
                if !BLOCKED.may_load(deps.storage, &address)?.unwrap_or(false) || pending.is_some()
                {
                    bump(&mut state, false)?;
                    BLOCKED.save(deps.storage, &address, &true)?;
                    PENDING_CLEAR.remove(deps.storage, &address);
                }
                response = response.add_attribute("action", "block");
                if let Some(pending) = pending {
                    response = response
                        .add_attribute("cancelled_nonce", pending.pending.nonce.to_string());
                }
            } else {
                governance(&state, &info.sender)?;
                if !BLOCKED.may_load(deps.storage, &address)?.unwrap_or(false) {
                    return Err(ContractError::NotBlocked);
                }
                let pending = PendingClear {
                    nonce: nonce(&mut state)?,
                    initiator: info.sender.to_string(),
                    proposed_at: env.block.time.seconds(),
                    executable_at: deadline(&env, state.rotation_delay)?,
                };
                bump(&mut state, false)?;
                if let Some(previous) = PENDING_CLEAR.may_load(deps.storage, &address)? {
                    response = response
                        .add_attribute("replaced_nonce", previous.pending.nonce.to_string());
                }
                PENDING_CLEAR.save(
                    deps.storage,
                    &address,
                    &StoredClear {
                        pending: pending.clone(),
                        governance_epoch: state.governance_epoch,
                    },
                )?;
                response = response
                    .add_attribute("action", "propose_blacklist_clear")
                    .add_attribute("nonce", pending.nonce.to_string())
                    .add_attribute("executable_at", pending.executable_at.to_string())
                    .add_attribute("initiator", &pending.initiator)
                    .add_attribute("proposed_at", pending.proposed_at.to_string());
            }
            response = response.add_attribute("address", address);
        }
        ExecuteMsg::ExecuteBlacklistClear { address, nonce } => {
            governance(&state, &info.sender)?;
            let address = deps.api.addr_validate(&address)?;
            let pending = PENDING_CLEAR
                .may_load(deps.storage, &address)?
                .ok_or(ContractError::NoPending)?;
            if pending.pending.nonce != nonce {
                return Err(ContractError::InvalidNonce);
            }
            if pending.governance_epoch != state.governance_epoch
                || pending.pending.initiator != state.roles.governance
            {
                return Err(ContractError::StaleProposal);
            }
            mature(&env, pending.pending.executable_at)?;
            bump(&mut state, false)?;
            BLOCKED.remove(deps.storage, &address);
            PENDING_CLEAR.remove(deps.storage, &address);
            response = response
                .add_attribute("action", "execute_blacklist_clear")
                .add_attribute("address", address)
                .add_attribute("nonce", nonce.to_string());
        }
        ExecuteMsg::CancelBlacklistClear { address } => {
            governance(&state, &info.sender)?;
            let address = deps.api.addr_validate(&address)?;
            let pending = PENDING_CLEAR
                .may_load(deps.storage, &address)?
                .ok_or(ContractError::NoPending)?;
            bump(&mut state, false)?;
            PENDING_CLEAR.remove(deps.storage, &address);
            response = response
                .add_attribute("action", "cancel_blacklist_clear")
                .add_attribute("address", address)
                .add_attribute("nonce", pending.pending.nonce.to_string());
        }
        ExecuteMsg::SetEnrollment { token, enrolled } => {
            governance(&state, &info.sender)?;
            let token = deps.api.addr_validate(&token)?;
            if ENROLLED.may_load(deps.storage, &token)?.unwrap_or(false) != enrolled {
                bump(&mut state, false)?;
                if enrolled {
                    ENROLLED.save(deps.storage, &token, &true)?;
                } else {
                    ENROLLED.remove(deps.storage, &token);
                }
            }
            response = response
                .add_attribute("action", "set_enrollment")
                .add_attribute("token", token)
                .add_attribute("enrolled", enrolled.to_string());
        }
        ExecuteMsg::RevokeResponder {} => {
            governance(&state, &info.sender)?;
            let pending_responder = matches!(
                state.pending_role.as_ref().map(|p| &p.role),
                Some(Role::Responder)
            );
            if state.roles.responder.is_some() || pending_responder {
                let authority_changed = state.roles.responder.is_some();
                bump(&mut state, authority_changed)?;
                if let Some(responder) = state.roles.responder.take() {
                    response = response.add_attribute("revoked_responder", responder);
                }
                if pending_responder {
                    if let Some(pending) = state.pending_role.take() {
                        response =
                            response.add_attribute("cancelled_nonce", pending.nonce.to_string());
                    }
                }
            }
            response = response.add_attribute("action", "revoke_responder");
        }
        ExecuteMsg::ProposeRole { role, candidate } => {
            governance(&state, &info.sender)?;
            let candidate = deps.api.addr_validate(&candidate)?.to_string();
            let mut roles = state.roles.clone();
            assign(&mut roles, &role, candidate.clone());
            validate_roles(deps.api, &env.contract.address, &roles)?;
            if roles == state.roles {
                return Err(ContractError::UnsafeRoles);
            }
            let pending = PendingRole {
                role,
                candidate,
                nonce: nonce(&mut state)?,
                initiator: info.sender.to_string(),
                proposed_at: env.block.time.seconds(),
                executable_at: deadline(&env, state.rotation_delay)?,
            };
            bump(&mut state, false)?;
            response = response
                .add_attribute("action", "propose_role")
                .add_attribute("role", role_name(&pending.role))
                .add_attribute("candidate", &pending.candidate)
                .add_attribute("nonce", pending.nonce.to_string())
                .add_attribute("executable_at", pending.executable_at.to_string())
                .add_attribute("initiator", &pending.initiator)
                .add_attribute("proposed_at", pending.proposed_at.to_string());
            if let Some(previous) = state.pending_role.replace(pending) {
                response = response.add_attribute("replaced_nonce", previous.nonce.to_string());
            }
        }
        ExecuteMsg::AcceptRole { nonce } => {
            let pending = state.pending_role.clone().ok_or(ContractError::NoPending)?;
            if info.sender.as_str() != pending.candidate {
                return Err(ContractError::Unauthorized);
            }
            if nonce != pending.nonce {
                return Err(ContractError::InvalidNonce);
            }
            if pending.initiator != state.roles.governance {
                return Err(ContractError::StaleProposal);
            }
            mature(&env, pending.executable_at)?;
            let mut roles = state.roles.clone();
            assign(&mut roles, &pending.role, pending.candidate.clone());
            validate_roles(deps.api, &env.contract.address, &roles)?;
            bump(&mut state, true)?;
            state.roles = roles;
            state.pending_role = None;
            if pending.role == Role::Governance {
                state.governance_epoch = state.authority_epoch;
                if let Some(delay) = state.pending_delay.take() {
                    response =
                        response.add_attribute("cancelled_delay_nonce", delay.nonce.to_string());
                }
            }
            response = response
                .add_attribute("action", "accept_role")
                .add_attribute("role", role_name(&pending.role))
                .add_attribute("candidate", pending.candidate)
                .add_attribute("nonce", nonce.to_string());
        }
        ExecuteMsg::CancelRole {} => {
            governance(&state, &info.sender)?;
            let pending = state.pending_role.take().ok_or(ContractError::NoPending)?;
            bump(&mut state, false)?;
            response = response
                .add_attribute("action", "cancel_role")
                .add_attribute("nonce", pending.nonce.to_string());
        }
        ExecuteMsg::ProposeRotationDelay { seconds } => {
            governance(&state, &info.sender)?;
            validate_delay(seconds)?;
            if seconds >= state.rotation_delay {
                // Tightening is immediate and invalidates any older weakening proposal.
                if seconds != state.rotation_delay || state.pending_delay.is_some() {
                    bump(&mut state, false)?;
                    state.rotation_delay = seconds;
                    if let Some(pending) = state.pending_delay.take() {
                        response =
                            response.add_attribute("cancelled_nonce", pending.nonce.to_string());
                    }
                }
                response = response.add_attribute("action", "increase_rotation_delay");
            } else {
                let pending = PendingDelay {
                    seconds,
                    nonce: nonce(&mut state)?,
                    initiator: info.sender.to_string(),
                    proposed_at: env.block.time.seconds(),
                    executable_at: deadline(&env, state.rotation_delay)?,
                };
                bump(&mut state, false)?;
                response = response
                    .add_attribute("action", "propose_rotation_delay")
                    .add_attribute("nonce", pending.nonce.to_string())
                    .add_attribute("executable_at", pending.executable_at.to_string())
                    .add_attribute("initiator", &pending.initiator)
                    .add_attribute("proposed_at", pending.proposed_at.to_string());
                if let Some(previous) = state.pending_delay.replace(pending) {
                    response = response.add_attribute("replaced_nonce", previous.nonce.to_string());
                }
            }
            response = response.add_attribute("seconds", seconds.to_string());
        }
        ExecuteMsg::ExecuteRotationDelay { nonce } => {
            governance(&state, &info.sender)?;
            let pending = state
                .pending_delay
                .clone()
                .ok_or(ContractError::NoPending)?;
            if nonce != pending.nonce {
                return Err(ContractError::InvalidNonce);
            }
            if pending.initiator != state.roles.governance {
                return Err(ContractError::StaleProposal);
            }
            mature(&env, pending.executable_at)?;
            bump(&mut state, false)?;
            state.rotation_delay = pending.seconds;
            state.pending_delay = None;
            response = response
                .add_attribute("action", "execute_rotation_delay")
                .add_attribute("seconds", pending.seconds.to_string())
                .add_attribute("nonce", nonce.to_string());
        }
        ExecuteMsg::CancelRotationDelay {} => {
            governance(&state, &info.sender)?;
            let pending = state.pending_delay.take().ok_or(ContractError::NoPending)?;
            bump(&mut state, false)?;
            response = response
                .add_attribute("action", "cancel_rotation_delay")
                .add_attribute("nonce", pending.nonce.to_string());
        }
    }
    STATE.save(deps.storage, &state)?;
    Ok(response
        .add_attribute("sender", info.sender)
        .add_attribute("epoch", state.epoch.to_string())
        .add_attribute("authority_epoch", state.authority_epoch.to_string()))
}

fn role_name(role: &Role) -> &'static str {
    match role {
        Role::Governance => "governance",
        Role::Responder => "responder",
        Role::Blacklist => "blacklist",
        Role::Recovery => "recovery",
        Role::Dispute => "dispute",
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let state = STATE.load(deps.storage)?;
    match msg {
        QueryMsg::Config {} => to_json_binary(&ConfigResponse {
            interface_version: INTERFACE_VERSION,
            epoch: state.epoch,
            authority_epoch: state.authority_epoch,
            paused: state.paused,
            roles: state.roles,
            rotation_delay: state.rotation_delay,
            pending_role: state.pending_role,
            pending_delay: state.pending_delay,
        }),
        QueryMsg::Guard { token, addresses } => {
            if addresses.len() > MAX_GUARD_ADDRESSES {
                return Err(StdError::generic_err("guard accepts at most 3 addresses"));
            }
            let token = deps.api.addr_validate(&token)?;
            let blocked = addresses
                .iter()
                .map(|address| {
                    let address = deps.api.addr_validate(address)?;
                    Ok(BLOCKED.may_load(deps.storage, &address)?.unwrap_or(false))
                })
                .collect::<StdResult<Vec<bool>>>()?;
            to_json_binary(&GuardResponse {
                interface_version: INTERFACE_VERSION,
                epoch: state.epoch,
                authority_epoch: state.authority_epoch,
                paused: state.paused,
                enrolled: ENROLLED.may_load(deps.storage, &token)?.unwrap_or(false),
                blocked,
                roles: state.roles,
            })
        }
        QueryMsg::Blacklist { address } => {
            let address = deps.api.addr_validate(&address)?;
            to_json_binary(&BlacklistResponse {
                interface_version: INTERFACE_VERSION,
                epoch: state.epoch,
                authority_epoch: state.authority_epoch,
                blocked: BLOCKED.may_load(deps.storage, &address)?.unwrap_or(false),
                pending_clear: PENDING_CLEAR
                    .may_load(deps.storage, &address)?
                    .map(|p| p.pending),
            })
        }
    }
}
