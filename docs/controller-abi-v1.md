# Controller ABI v1

Canonical snake_case JSON, Rust source packages/security-interface/src/lib.rs (1.0.0).
Every ordinary economic token execution makes one bounded `guard` query with token
address and up to three addresses (owner, actor, recipient/spender). Require version=1,
enrolled=true, paused=false, exact blocked-vector length and all false; any error
fails closed. No cache, fanout, oracle or pause timeout. Recovery queries the same
state but ignores pause and source blacklist; destination must be unblocked.
`authority_epoch` changes on privilege changes, not pause/blacklist updates.
Token governance is separately configured/validated against controller governance,
never inferred from issuer/minter. Multisig custody must be independently verified.

Timer events: `action` identifies operation; `token` is emitted for token timers,
`wallet` is affected wallet (settings/lock), `nonce`, `initiator`, `proposed_at`,
`executable_at` are decimal strings. Controller contract address identifies controller
rotation timers; recovery also emits `source`, `destination`, `amount`, `evidence_hash`.
Chain ID, height, tx hash, event index and emitting contract are supplied by chain
receipt, never trusted from user input. Identity/dedup = chain_id + contract + kind +
nonce; receipt dedup also includes tx hash/event index. Cancel/execute events carry
same nonce. Events signal timers; queries remain authoritative. No PII in events.

Controller actions: pause, unpause, set_blacklist, set_enrollment, revoke_responder,
propose_role, accept_role, cancel_role, propose_rotation_delay,
execute_rotation_delay, cancel_rotation_delay. Guard and config expose epochs/version.
