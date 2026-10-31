use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{coins, from_json, Env, OwnedDeps, Response, Timestamp};
use cw20_security_interface::{
    BlacklistResponse, ConfigResponse, ExecuteMsg, GuardResponse, InstantiateMsg, QueryMsg, Role,
    Roles, INTERFACE_VERSION, MAX_DELAY, MIN_ROLE_DELAY,
};

use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use crate::state::STATE;

const GOV: &str = "governance";
const RESPONDER: &str = "responder";
const BLACKLIST: &str = "blacklist";
const RECOVERY: &str = "recovery";
const DISPUTE: &str = "dispute";

fn roles() -> Roles {
    Roles {
        governance: GOV.into(),
        responder: Some(RESPONDER.into()),
        blacklist: BLACKLIST.into(),
        recovery: RECOVERY.into(),
        dispute: DISPUTE.into(),
    }
}

struct Harness {
    deps: OwnedDeps<MockStorage, MockApi, MockQuerier>,
    env: Env,
}

impl Harness {
    fn new() -> Self {
        let mut h = Self {
            deps: mock_dependencies(),
            env: mock_env(),
        };
        h.env.block.time = Timestamp::from_seconds(1_000_000);
        instantiate(
            h.deps.as_mut(),
            h.env.clone(),
            mock_info("deployer", &[]),
            InstantiateMsg {
                roles: roles(),
                rotation_delay: MIN_ROLE_DELAY,
            },
        )
        .unwrap();
        h
    }

    fn exec(&mut self, sender: &str, msg: ExecuteMsg) -> Result<Response, ContractError> {
        execute(
            self.deps.as_mut(),
            self.env.clone(),
            mock_info(sender, &[]),
            msg,
        )
    }

    fn gov(&mut self, msg: ExecuteMsg) -> Response {
        self.exec(GOV, msg).unwrap()
    }

    fn config(&self) -> ConfigResponse {
        from_json(query(self.deps.as_ref(), self.env.clone(), QueryMsg::Config {}).unwrap())
            .unwrap()
    }

    fn blacklist(&self, address: &str) -> BlacklistResponse {
        from_json(
            query(
                self.deps.as_ref(),
                self.env.clone(),
                QueryMsg::Blacklist {
                    address: address.into(),
                },
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn guard(&self, token: &str, addresses: &[&str]) -> GuardResponse {
        from_json(
            query(
                self.deps.as_ref(),
                self.env.clone(),
                QueryMsg::Guard {
                    token: token.into(),
                    addresses: addresses.iter().map(|s| s.to_string()).collect(),
                },
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn at(&mut self, seconds: u64) {
        self.env.block.time = Timestamp::from_seconds(seconds);
    }

    fn advance(&mut self, seconds: u64) {
        self.env.block.time = self.env.block.time.plus_seconds(seconds);
    }

    fn propose(&mut self, role: Role, candidate: &str) -> u64 {
        self.gov(ExecuteMsg::ProposeRole {
            role,
            candidate: candidate.into(),
        });
        self.config().pending_role.unwrap().nonce
    }

    fn block(&mut self, address: &str) {
        self.exec(
            BLACKLIST,
            ExecuteMsg::SetBlacklist {
                address: address.into(),
                blocked: true,
            },
        )
        .unwrap();
    }

    fn clear(&mut self, address: &str) -> u64 {
        self.gov(ExecuteMsg::SetBlacklist {
            address: address.into(),
            blocked: false,
        });
        self.blacklist(address).pending_clear.unwrap().nonce
    }
}

#[test]
fn instantiate_rejects_every_role_overlap_and_unsafe_addresses() {
    // Includes responder vs each other role, and recovery vs dispute.
    for left in 0..5 {
        for right in (left + 1)..5 {
            let mut r = roles();
            let mut addresses = [
                GOV.to_string(),
                RESPONDER.into(),
                BLACKLIST.into(),
                RECOVERY.into(),
                DISPUTE.into(),
            ];
            addresses[right] = addresses[left].clone();
            r.governance = addresses[0].clone();
            r.responder = Some(addresses[1].clone());
            r.blacklist = addresses[2].clone();
            r.recovery = addresses[3].clone();
            r.dispute = addresses[4].clone();
            assert_eq!(
                instantiate(
                    mock_dependencies().as_mut(),
                    mock_env(),
                    mock_info("sender", &[]),
                    InstantiateMsg {
                        roles: r,
                        rotation_delay: MIN_ROLE_DELAY,
                    }
                )
                .unwrap_err(),
                ContractError::UnsafeRoles
            );
        }
    }
    let mut r = roles();
    r.governance = mock_env().contract.address.into();
    assert_eq!(
        instantiate(
            mock_dependencies().as_mut(),
            mock_env(),
            mock_info("sender", &[]),
            InstantiateMsg {
                roles: r,
                rotation_delay: MIN_ROLE_DELAY,
            }
        )
        .unwrap_err(),
        ContractError::UnsafeRoles
    );
    let mut r = roles();
    r.recovery = "x".into();
    assert!(matches!(
        instantiate(
            mock_dependencies().as_mut(),
            mock_env(),
            mock_info("sender", &[]),
            InstantiateMsg {
                roles: r,
                rotation_delay: MIN_ROLE_DELAY,
            }
        ),
        Err(ContractError::Std(_))
    ));
    for delay in [0, MIN_ROLE_DELAY - 1, MAX_DELAY + 1, u64::MAX] {
        assert_eq!(
            instantiate(
                mock_dependencies().as_mut(),
                mock_env(),
                mock_info("sender", &[]),
                InstantiateMsg {
                    roles: roles(),
                    rotation_delay: delay,
                }
            )
            .unwrap_err(),
            ContractError::InvalidDelay
        );
    }
    for delay in [MIN_ROLE_DELAY, MAX_DELAY] {
        let mut r = roles();
        r.responder = None;
        instantiate(
            mock_dependencies().as_mut(),
            mock_env(),
            mock_info("sender", &[]),
            InstantiateMsg {
                roles: r,
                rotation_delay: delay,
            },
        )
        .unwrap();
    }
    let h = Harness::new();
    let config = h.config();
    assert!(config.paused);
    assert_eq!(
        (
            config.epoch,
            config.authority_epoch,
            config.interface_version
        ),
        (1, 1, INTERFACE_VERSION)
    );
    assert_eq!(
        cw2::get_contract_version(&h.deps.storage).unwrap().contract,
        "crates.io:cw20-security-controller"
    );
}

#[test]
fn unauthorized_matrix_including_pause_only_responder() {
    let mut h = Harness::new();
    let governance_only = vec![
        ExecuteMsg::Unpause {},
        ExecuteMsg::SetBlacklist {
            address: "victim".into(),
            blocked: false,
        },
        ExecuteMsg::ExecuteBlacklistClear {
            address: "victim".into(),
            nonce: 1,
        },
        ExecuteMsg::CancelBlacklistClear {
            address: "victim".into(),
        },
        ExecuteMsg::SetEnrollment {
            token: "token".into(),
            enrolled: true,
        },
        ExecuteMsg::SetEnrollment {
            token: "token".into(),
            enrolled: false,
        },
        ExecuteMsg::RevokeResponder {},
        ExecuteMsg::ProposeRole {
            role: Role::Governance,
            candidate: "candidate".into(),
        },
        ExecuteMsg::CancelRole {},
        ExecuteMsg::ProposeRotationDelay { seconds: MAX_DELAY },
        ExecuteMsg::ExecuteRotationDelay { nonce: 1 },
        ExecuteMsg::CancelRotationDelay {},
    ];
    for sender in [RESPONDER, BLACKLIST, RECOVERY, DISPUTE, "outsider"] {
        for msg in &governance_only {
            let before = h.config();
            assert_eq!(
                h.exec(sender, msg.clone()).unwrap_err(),
                ContractError::Unauthorized,
                "{sender}: {msg:?}"
            );
            assert_eq!(h.config(), before);
        }
    }
    for sender in [BLACKLIST, RECOVERY, DISPUTE, "outsider"] {
        assert_eq!(
            h.exec(sender, ExecuteMsg::Pause {}).unwrap_err(),
            ContractError::Unauthorized
        );
    }
    for sender in [RESPONDER, RECOVERY, DISPUTE, "outsider"] {
        assert_eq!(
            h.exec(
                sender,
                ExecuteMsg::SetBlacklist {
                    address: "victim".into(),
                    blocked: true
                }
            )
            .unwrap_err(),
            ContractError::Unauthorized
        );
    }
    h.gov(ExecuteMsg::Unpause {});
    let response = h.exec(RESPONDER, ExecuteMsg::Pause {}).unwrap();
    assert!(response.messages.is_empty());
    assert!(h.config().paused);
    assert_eq!((h.config().epoch, h.config().authority_epoch), (3, 1));
    h.exec(RESPONDER, ExecuteMsg::Pause {}).unwrap();
    assert_eq!(h.config().epoch, 3); // idempotent containment does not mutate state
    h.advance(MAX_DELAY * 2);
    assert!(h.config().paused); // no pause timeout or automation unpause
    h.gov(ExecuteMsg::SetBlacklist {
        address: "victim".into(),
        blocked: true,
    });
    assert!(h.blacklist("victim").blocked);
}

#[test]
fn guard_and_config_are_authoritative_bounded_and_ordered() {
    let mut h = Harness::new();
    assert!(!h.guard("token", &[]).enrolled);
    h.gov(ExecuteMsg::SetEnrollment {
        token: "token".into(),
        enrolled: true,
    });
    h.gov(ExecuteMsg::SetEnrollment {
        token: "token_two".into(),
        enrolled: true,
    });
    h.block("victim");
    h.gov(ExecuteMsg::Unpause {});
    let guard = h.guard("token", &["clean", "victim", "victim"]);
    assert_eq!(guard.blocked, vec![false, true, true]);
    assert_eq!(guard.roles, roles());
    assert_eq!(guard.epoch, h.config().epoch);
    assert_eq!(guard.interface_version, INTERFACE_VERSION);
    assert_eq!(guard.authority_epoch, 1);
    assert!(guard.enrolled);
    assert!(!guard.paused);
    assert!(!h.guard("unknown_token", &[]).enrolled);
    h.exec(RESPONDER, ExecuteMsg::Pause {}).unwrap();
    for token in ["token", "token_two"] {
        let guarded = h.guard(token, &[]);
        assert!(guarded.paused && guarded.enrolled);
        assert_eq!(guarded.epoch, h.config().epoch);
    }
    h.gov(ExecuteMsg::SetEnrollment {
        token: "token".into(),
        enrolled: false,
    });
    assert!(!h.guard("token", &[]).enrolled);
    assert!(h.guard("token_two", &[]).enrolled);
    for (token, addresses) in [
        ("token", vec!["valid".into(); 4]),
        ("x", vec![]),
        ("token", vec!["x".into()]),
    ] {
        assert!(query(
            h.deps.as_ref(),
            h.env.clone(),
            QueryMsg::Guard {
                token: token.into(),
                addresses
            }
        )
        .is_err());
    }
    let blacklisted = h.blacklist("victim");
    assert_eq!(blacklisted.interface_version, INTERFACE_VERSION);
    assert_eq!(blacklisted.epoch, h.config().epoch);
    assert!(blacklisted.blocked);
    assert_eq!(blacklisted.pending_clear, None);
}

#[test]
fn all_five_roles_require_exact_candidate_nonce_and_full_delay() {
    for role in [
        Role::Governance,
        Role::Responder,
        Role::Blacklist,
        Role::Recovery,
        Role::Dispute,
    ] {
        let mut h = Harness::new();
        let nonce = h.propose(role.clone(), "candidate");
        let pending = h.config().pending_role.unwrap();
        assert_eq!(pending.executable_at - pending.proposed_at, MIN_ROLE_DELAY);
        assert_eq!(h.config().authority_epoch, 1);
        assert_eq!(
            h.exec(GOV, ExecuteMsg::AcceptRole { nonce }).unwrap_err(),
            ContractError::Unauthorized
        );
        assert_eq!(
            h.exec("candidate", ExecuteMsg::AcceptRole { nonce: nonce + 1 })
                .unwrap_err(),
            ContractError::InvalidNonce
        );
        h.at(pending.executable_at - 1);
        assert_eq!(
            h.exec("candidate", ExecuteMsg::AcceptRole { nonce })
                .unwrap_err(),
            ContractError::TooEarly
        );
        h.at(pending.executable_at);
        let accepted = h
            .exec("candidate", ExecuteMsg::AcceptRole { nonce })
            .unwrap();
        assert!(accepted.messages.is_empty());
        assert!(accepted
            .attributes
            .iter()
            .any(|a| a.key == "nonce" && a.value == nonce.to_string()));
        let r = h.config().roles;
        let assigned = match role {
            Role::Governance => r.governance,
            Role::Responder => r.responder.unwrap(),
            Role::Blacklist => r.blacklist,
            Role::Recovery => r.recovery,
            Role::Dispute => r.dispute,
        };
        assert_eq!(assigned, "candidate");
        assert_eq!(h.config().authority_epoch, 2);
        assert!(h.config().pending_role.is_none());
        assert_eq!(
            h.exec("candidate", ExecuteMsg::AcceptRole { nonce })
                .unwrap_err(),
            ContractError::NoPending
        );
    }
}

#[test]
fn replacement_restarts_role_delay_and_cancelled_nonce_cannot_replay() {
    let mut h = Harness::new();
    let first = h.propose(Role::Responder, "candidate_one");
    h.advance(MIN_ROLE_DELAY - 1);
    let second = h.propose(Role::Responder, "candidate_two");
    assert!(second > first);
    h.advance(1);
    assert_eq!(
        h.exec("candidate_one", ExecuteMsg::AcceptRole { nonce: first })
            .unwrap_err(),
        ContractError::Unauthorized
    );
    assert_eq!(
        h.exec("candidate_two", ExecuteMsg::AcceptRole { nonce: first })
            .unwrap_err(),
        ContractError::InvalidNonce
    );
    assert_eq!(
        h.exec("candidate_two", ExecuteMsg::AcceptRole { nonce: second })
            .unwrap_err(),
        ContractError::TooEarly
    );
    h.gov(ExecuteMsg::CancelRole {});
    h.advance(MIN_ROLE_DELAY);
    assert_eq!(
        h.exec("candidate_two", ExecuteMsg::AcceptRole { nonce: second })
            .unwrap_err(),
        ContractError::NoPending
    );
    assert_eq!(
        h.exec(GOV, ExecuteMsg::CancelRole {}).unwrap_err(),
        ContractError::NoPending
    );
    let third = h.propose(Role::Responder, "candidate_two");
    assert!(third > second);
    assert_eq!(
        h.exec("candidate_two", ExecuteMsg::AcceptRole { nonce: second })
            .unwrap_err(),
        ContractError::InvalidNonce
    );
    assert_eq!(h.config().authority_epoch, 1);
}

#[test]
fn unsafe_candidates_rejected_at_proposal_and_revalidated_at_acceptance() {
    let mut h = Harness::new();
    for candidate in [GOV, RESPONDER, BLACKLIST, RECOVERY, DISPUTE] {
        assert_eq!(
            h.exec(
                GOV,
                ExecuteMsg::ProposeRole {
                    role: Role::Recovery,
                    candidate: candidate.into()
                }
            )
            .unwrap_err(),
            ContractError::UnsafeRoles
        );
    }
    let controller = h.env.contract.address.to_string();
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ProposeRole {
                role: Role::Governance,
                candidate: controller
            }
        )
        .unwrap_err(),
        ContractError::UnsafeRoles
    );
    let nonce = h.propose(Role::Recovery, "candidate");
    h.advance(MIN_ROLE_DELAY);
    // Simulate incompatible future migration state to verify acceptance independently
    // checks the complete role set rather than relying on the original validation.
    STATE
        .update(
            &mut h.deps.storage,
            |mut state| -> Result<_, ContractError> {
                state.roles.dispute = "candidate".into();
                Ok(state)
            },
        )
        .unwrap();
    assert_eq!(
        h.exec("candidate", ExecuteMsg::AcceptRole { nonce })
            .unwrap_err(),
        ContractError::UnsafeRoles
    );
    assert!(h.config().pending_role.is_some());
    assert_eq!(h.config().authority_epoch, 1);
}

#[test]
fn immediate_responder_revocation_cancels_pending_enablement_and_invalidates_privilege() {
    let mut h = Harness::new();
    let pending = h.propose(Role::Responder, "replacement");
    h.gov(ExecuteMsg::RevokeResponder {});
    assert_eq!(h.config().roles.responder, None);
    assert_eq!(h.config().authority_epoch, 2);
    assert_eq!(h.config().pending_role, None);
    assert_eq!(
        h.exec(RESPONDER, ExecuteMsg::Pause {}).unwrap_err(),
        ContractError::Unauthorized
    );
    h.advance(MIN_ROLE_DELAY);
    assert_eq!(
        h.exec("replacement", ExecuteMsg::AcceptRole { nonce: pending })
            .unwrap_err(),
        ContractError::NoPending
    );
    let before = h.config();
    h.gov(ExecuteMsg::RevokeResponder {});
    assert_eq!(before, h.config());
    let cancelled = h.propose(Role::Responder, "replacement");
    h.gov(ExecuteMsg::RevokeResponder {});
    assert_eq!(h.config().authority_epoch, 2); // pending-only revocation isn't an assignment change
    assert_eq!(
        h.exec("replacement", ExecuteMsg::AcceptRole { nonce: cancelled })
            .unwrap_err(),
        ContractError::NoPending
    );
    let nonce = h.propose(Role::Responder, "replacement");
    assert_eq!(
        h.exec("replacement", ExecuteMsg::AcceptRole { nonce })
            .unwrap_err(),
        ContractError::TooEarly
    );
    h.advance(MIN_ROLE_DELAY);
    h.exec("replacement", ExecuteMsg::AcceptRole { nonce })
        .unwrap();
    h.gov(ExecuteMsg::Unpause {});
    h.exec("replacement", ExecuteMsg::Pause {}).unwrap();
    assert_eq!(h.config().authority_epoch, 3);
}

#[test]
fn delay_shortening_waits_old_delay_and_increase_cancels_stale_shortening() {
    let mut h = Harness::new();
    let old_delay = MIN_ROLE_DELAY * 2;
    h.gov(ExecuteMsg::ProposeRotationDelay { seconds: old_delay });
    assert_eq!(h.config().rotation_delay, old_delay);
    assert_eq!(h.config().pending_delay, None);
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY,
    });
    let first = h.config().pending_delay.unwrap();
    assert_eq!(first.executable_at - first.proposed_at, old_delay);
    assert_eq!(h.config().rotation_delay, old_delay);
    h.advance(MIN_ROLE_DELAY);
    assert_eq!(
        h.exec(GOV, ExecuteMsg::ExecuteRotationDelay { nonce: first.nonce })
            .unwrap_err(),
        ContractError::TooEarly
    );
    let increased = old_delay + MIN_ROLE_DELAY;
    h.gov(ExecuteMsg::ProposeRotationDelay { seconds: increased });
    assert_eq!(h.config().pending_delay, None);
    h.advance(old_delay);
    assert_eq!(
        h.exec(GOV, ExecuteMsg::ExecuteRotationDelay { nonce: first.nonce })
            .unwrap_err(),
        ContractError::NoPending
    );
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY,
    });
    let restarted = h.config().pending_delay.unwrap();
    assert_eq!(restarted.executable_at - restarted.proposed_at, increased);
    assert!(restarted.nonce > first.nonce);
    assert_eq!(
        h.exec(GOV, ExecuteMsg::ExecuteRotationDelay { nonce: first.nonce })
            .unwrap_err(),
        ContractError::InvalidNonce
    );
    h.at(restarted.executable_at - 1);
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteRotationDelay {
                nonce: restarted.nonce
            }
        )
        .unwrap_err(),
        ContractError::TooEarly
    );
    h.at(restarted.executable_at);
    h.gov(ExecuteMsg::ExecuteRotationDelay {
        nonce: restarted.nonce,
    });
    assert_eq!(h.config().rotation_delay, MIN_ROLE_DELAY);
    assert_eq!(h.config().authority_epoch, 1);
    for seconds in [MIN_ROLE_DELAY - 1, MAX_DELAY + 1] {
        assert_eq!(
            h.exec(GOV, ExecuteMsg::ProposeRotationDelay { seconds })
                .unwrap_err(),
            ContractError::InvalidDelay
        );
    }
    h.gov(ExecuteMsg::ProposeRotationDelay { seconds: MAX_DELAY });
    assert_eq!(h.config().rotation_delay, MAX_DELAY);
}

#[test]
fn delay_replacement_cancellation_and_existing_role_deadline_are_independent() {
    let mut h = Harness::new();
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY * 3,
    });
    let nonce = h.propose(Role::Blacklist, "candidate");
    let role_deadline = h.config().pending_role.unwrap().executable_at;
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY,
    });
    let first = h.config().pending_delay.unwrap();
    h.advance(100);
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY * 2,
    });
    let replacement = h.config().pending_delay.unwrap();
    assert!(replacement.nonce > first.nonce);
    assert_eq!(replacement.executable_at, first.executable_at + 100);
    h.gov(ExecuteMsg::CancelRotationDelay {});
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteRotationDelay {
                nonce: replacement.nonce
            }
        )
        .unwrap_err(),
        ContractError::NoPending
    );
    assert_eq!(
        h.exec(GOV, ExecuteMsg::CancelRotationDelay {}).unwrap_err(),
        ContractError::NoPending
    );
    h.gov(ExecuteMsg::ProposeRotationDelay { seconds: MAX_DELAY });
    assert_eq!(
        h.config().pending_role.unwrap().executable_at,
        role_deadline
    );
    h.at(role_deadline);
    h.exec("candidate", ExecuteMsg::AcceptRole { nonce })
        .unwrap();
    assert_eq!(h.config().roles.blacklist, "candidate");
}

#[test]
fn blacklist_clear_is_delayed_replaceable_cancellable_and_reblock_invalidates() {
    let mut h = Harness::new();
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::SetBlacklist {
                address: "victim".into(),
                blocked: false
            }
        )
        .unwrap_err(),
        ContractError::NotBlocked
    );
    h.block("victim");
    let first = h.clear("victim");
    let original = h.blacklist("victim").pending_clear.unwrap();
    assert!(h.blacklist("victim").blocked);
    assert_eq!(
        original.executable_at - original.proposed_at,
        MIN_ROLE_DELAY
    );
    h.advance(MIN_ROLE_DELAY - 1);
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteBlacklistClear {
                address: "victim".into(),
                nonce: first
            }
        )
        .unwrap_err(),
        ContractError::TooEarly
    );
    let second = h.clear("victim");
    assert!(second > first);
    h.advance(1);
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteBlacklistClear {
                address: "victim".into(),
                nonce: first
            }
        )
        .unwrap_err(),
        ContractError::InvalidNonce
    );
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteBlacklistClear {
                address: "victim".into(),
                nonce: second
            }
        )
        .unwrap_err(),
        ContractError::TooEarly
    );
    h.gov(ExecuteMsg::CancelBlacklistClear {
        address: "victim".into(),
    });
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteBlacklistClear {
                address: "victim".into(),
                nonce: second
            }
        )
        .unwrap_err(),
        ContractError::NoPending
    );
    let third = h.clear("victim");
    h.advance(MIN_ROLE_DELAY);
    h.block("victim"); // Reblock also invalidates an already mature clear.
    assert_eq!(h.blacklist("victim").pending_clear, None);
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteBlacklistClear {
                address: "victim".into(),
                nonce: third
            }
        )
        .unwrap_err(),
        ContractError::NoPending
    );
    let fourth = h.clear("victim");
    assert!(fourth > third);
    let until = h.blacklist("victim").pending_clear.unwrap().executable_at;
    h.at(until);
    h.gov(ExecuteMsg::ExecuteBlacklistClear {
        address: "victim".into(),
        nonce: fourth,
    });
    assert!(!h.blacklist("victim").blocked);
    assert_eq!(h.blacklist("victim").pending_clear, None);
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteBlacklistClear {
                address: "victim".into(),
                nonce: fourth
            }
        )
        .unwrap_err(),
        ContractError::NoPending
    );
    assert_eq!(h.config().authority_epoch, 1);
}

#[test]
fn proposal_nonces_are_global_and_address_scoped_clear_cannot_cross_replay() {
    let mut h = Harness::new();
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY * 2,
    });
    let role_nonce = h.propose(Role::Recovery, "candidate");
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY,
    });
    let delay_nonce = h.config().pending_delay.unwrap().nonce;
    h.block("victim");
    let clear_nonce = h.clear("victim");
    h.block("other_victim");
    let other_nonce = h.clear("other_victim");
    assert!(role_nonce < delay_nonce && delay_nonce < clear_nonce && clear_nonce < other_nonce);
    h.advance(MIN_ROLE_DELAY * 2);
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteBlacklistClear {
                address: "other_victim".into(),
                nonce: clear_nonce
            }
        )
        .unwrap_err(),
        ContractError::InvalidNonce
    );
    h.gov(ExecuteMsg::ExecuteRotationDelay { nonce: delay_nonce });
    h.gov(ExecuteMsg::ExecuteBlacklistClear {
        address: "victim".into(),
        nonce: clear_nonce,
    });
    assert!(!h.blacklist("victim").blocked);
    assert!(h.blacklist("other_victim").blocked);
    h.gov(ExecuteMsg::CancelRole {});
    let next = h.propose(Role::Recovery, "candidate");
    assert!(next > other_nonce);
}

#[test]
fn governance_rotation_invalidates_pending_weakening_even_after_address_returns() {
    let mut h = Harness::new();
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY * 2,
    });
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY,
    });
    let delay = h.config().pending_delay.unwrap().nonce;
    h.block("victim");
    let clear = h.clear("victim");
    let rotation = h.propose(Role::Governance, "new_governance");
    h.advance(MIN_ROLE_DELAY * 2);
    h.exec("new_governance", ExecuteMsg::AcceptRole { nonce: rotation })
        .unwrap();
    assert_eq!(h.config().pending_delay, None);
    assert_eq!(
        h.exec(GOV, ExecuteMsg::Unpause {}).unwrap_err(),
        ContractError::Unauthorized
    );
    assert_eq!(
        h.exec(
            "new_governance",
            ExecuteMsg::ExecuteRotationDelay { nonce: delay }
        )
        .unwrap_err(),
        ContractError::NoPending
    );
    assert_eq!(
        h.exec(
            "new_governance",
            ExecuteMsg::ExecuteBlacklistClear {
                address: "victim".into(),
                nonce: clear
            }
        )
        .unwrap_err(),
        ContractError::StaleProposal
    );
    h.exec(
        "new_governance",
        ExecuteMsg::ProposeRole {
            role: Role::Governance,
            candidate: GOV.into(),
        },
    )
    .unwrap();
    let back = h.config().pending_role.unwrap().nonce;
    h.advance(MIN_ROLE_DELAY * 2);
    h.exec(GOV, ExecuteMsg::AcceptRole { nonce: back }).unwrap();
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteBlacklistClear {
                address: "victim".into(),
                nonce: clear
            }
        )
        .unwrap_err(),
        ContractError::StaleProposal
    );
    let fresh = h.clear("victim");
    assert!(fresh > back);
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ExecuteBlacklistClear {
                address: "victim".into(),
                nonce: fresh
            }
        )
        .unwrap_err(),
        ContractError::TooEarly
    );
    assert_eq!(h.config().authority_epoch, 3);
}

#[test]
fn fractional_timestamps_cannot_shorten_any_proposal_delay() {
    let mut h = Harness::new();
    h.env.block.time = Timestamp::from_nanos(1_000_000_999_999_999);
    let nonce = h.propose(Role::Recovery, "candidate");
    let pending = h.config().pending_role.unwrap();
    assert_eq!(
        pending.executable_at,
        pending.proposed_at + MIN_ROLE_DELAY + 1
    );
    h.env.block.time = h.env.block.time.plus_seconds(MIN_ROLE_DELAY);
    assert_eq!(
        h.exec("candidate", ExecuteMsg::AcceptRole { nonce })
            .unwrap_err(),
        ContractError::TooEarly
    );
    h.at(pending.executable_at);
    h.exec("candidate", ExecuteMsg::AcceptRole { nonce })
        .unwrap();

    h.env.block.time = h.env.block.time.plus_nanos(1);
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY * 2,
    });
    h.gov(ExecuteMsg::ProposeRotationDelay {
        seconds: MIN_ROLE_DELAY,
    });
    let pending = h.config().pending_delay.unwrap();
    assert_eq!(
        pending.executable_at,
        pending.proposed_at + MIN_ROLE_DELAY * 2 + 1
    );
    h.block("victim");
    h.clear("victim");
    let pending = h.blacklist("victim").pending_clear.unwrap();
    assert_eq!(
        pending.executable_at,
        pending.proposed_at + MIN_ROLE_DELAY * 2 + 1
    );
}

#[test]
fn funds_are_rejected_and_counters_fail_closed_on_overflow() {
    let mut h = Harness::new();
    let before = h.config();
    assert_eq!(
        execute(
            h.deps.as_mut(),
            h.env.clone(),
            mock_info(GOV, &coins(1, "ucosm")),
            ExecuteMsg::Unpause {}
        )
        .unwrap_err(),
        ContractError::FundsNotAccepted
    );
    assert_eq!(h.config(), before);
    assert_eq!(
        instantiate(
            mock_dependencies().as_mut(),
            mock_env(),
            mock_info("sender", &coins(1, "ucosm")),
            InstantiateMsg {
                roles: roles(),
                rotation_delay: MIN_ROLE_DELAY
            }
        )
        .unwrap_err(),
        ContractError::FundsNotAccepted
    );
    STATE
        .update(
            &mut h.deps.storage,
            |mut state| -> Result<_, ContractError> {
                state.last_nonce = u64::MAX;
                Ok(state)
            },
        )
        .unwrap();
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::ProposeRole {
                role: Role::Recovery,
                candidate: "candidate".into()
            }
        )
        .unwrap_err(),
        ContractError::Overflow
    );
    assert_eq!(h.config().pending_role, None);
    STATE
        .update(
            &mut h.deps.storage,
            |mut state| -> Result<_, ContractError> {
                state.epoch = u64::MAX;
                Ok(state)
            },
        )
        .unwrap();
    assert_eq!(
        h.exec(
            GOV,
            ExecuteMsg::SetEnrollment {
                token: "token".into(),
                enrolled: true
            }
        )
        .unwrap_err(),
        ContractError::Overflow
    );
    assert!(!h.guard("token", &[]).enrolled);
}
