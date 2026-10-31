use cosmwasm_schema::{cw_serde, QueryResponses};

pub const INTERFACE_VERSION: u32 = 1;
pub const MIN_ROLE_DELAY: u64 = 172800;
pub const MAX_DELAY: u64 = 31536000;

#[cw_serde]
pub enum Role { Governance, Responder, Blacklist, Recovery, Dispute }
#[cw_serde]
pub struct Roles {
    pub governance: String,
    pub responder: Option<String>,
    pub blacklist: String,
    pub recovery: String,
    pub dispute: String,
}
#[cw_serde]
pub struct InstantiateMsg { pub roles: Roles, pub rotation_delay: u64 }
#[cw_serde]
pub enum ExecuteMsg {
    Pause {},
    Unpause {},
    SetBlacklist { address: String, blocked: bool },
    SetEnrollment { token: String, enrolled: bool },
    RevokeResponder {},
    ProposeRole { role: Role, candidate: String },
    AcceptRole { nonce: u64 },
    CancelRole {},
    ProposeRotationDelay { seconds: u64 },
    ExecuteRotationDelay { nonce: u64 },
    CancelRotationDelay {},
}
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(GuardResponse)]
    Guard { token: String, addresses: Vec<String> },
    #[returns(ConfigResponse)]
    Config {},
}
#[cw_serde]
pub struct GuardResponse {
    pub interface_version: u32,
    pub epoch: u64,
    pub authority_epoch: u64,
    pub paused: bool,
    pub enrolled: bool,
    /// In the same order as the bounded query's addresses (maximum 3).
    pub blocked: Vec<bool>,
    pub roles: Roles,
}
#[cw_serde]
pub struct PendingRole {
    pub role: Role, pub candidate: String, pub nonce: u64,
    pub initiator: String, pub proposed_at: u64, pub executable_at: u64,
}
#[cw_serde]
pub struct PendingDelay {
    pub seconds: u64, pub nonce: u64, pub initiator: String,
    pub proposed_at: u64, pub executable_at: u64,
}
#[cw_serde]
pub struct ConfigResponse {
    pub interface_version: u32, pub epoch: u64, pub authority_epoch: u64,
    pub paused: bool, pub roles: Roles, pub rotation_delay: u64,
    pub pending_role: Option<PendingRole>, pub pending_delay: Option<PendingDelay>,
}
