use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),
    #[error("unauthorized")]
    Unauthorized,
    #[error("funds are not accepted")]
    FundsNotAccepted,
    #[error("role addresses must be distinct and cannot be the controller")]
    UnsafeRoles,
    #[error("rotation delay must be between 48 hours and 365 days")]
    InvalidDelay,
    #[error("no pending proposal")]
    NoPending,
    #[error("proposal nonce mismatch")]
    InvalidNonce,
    #[error("proposal delay has not elapsed")]
    TooEarly,
    #[error("proposal initiator is no longer governance")]
    StaleProposal,
    #[error("checked counter or timestamp overflow")]
    Overflow,
    #[error("address is not blocked")]
    NotBlocked,
}
