use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw20_security_interface::{PendingClear, PendingDelay, PendingRole, Roles};
use cw_storage_plus::{Item, Map};

#[cw_serde]
pub struct State {
    pub roles: Roles,
    pub paused: bool,
    pub rotation_delay: u64,
    pub epoch: u64,
    pub authority_epoch: u64,
    pub governance_epoch: u64,
    pub last_nonce: u64,
    pub pending_role: Option<PendingRole>,
    pub pending_delay: Option<PendingDelay>,
}

pub const STATE: Item<State> = Item::new("security_state");
pub const BLOCKED: Map<&Addr, bool> = Map::new("blocked");
pub const ENROLLED: Map<&Addr, bool> = Map::new("enrolled");
#[cw_serde]
pub struct StoredClear {
    pub pending: PendingClear,
    pub governance_epoch: u64,
}

pub const PENDING_CLEAR: Map<&Addr, StoredClear> = Map::new("pending_clear");
