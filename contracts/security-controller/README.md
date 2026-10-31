# Shared CW20 security controller

Deployable CosmWasm contract implementing `cw20-security-interface` ABI v1.
All enrolled token guards read the same authoritative pause flag. Pause writes
one item; it never enumerates tokens or sends messages to them. Guard reads
one token's enrollment and at most three blacklist entries, preserving input
order (including duplicates). Enrollment and blacklist entries use direct map
lookups. There are no fleet lists, pagination, callbacks, or fanout operations.
Storage can grow with enrolled tokens, blocked addresses, and one pending clear
per address; each operation touches a constant number of entries. Roles have
one pending proposal globally; the rotation delay has one pending proposal.

The controller starts **paused**, with no enrolled tokens. Pauses never expire.
Verified governance must enroll the intended token addresses and unpause after
deployment checks. Tokens must reject an unenrolled guard result and fail
closed on controller query errors, malformed responses, or incompatible ABI
versions. A controller outage therefore stops guarded operations for the whole
enrolled fleet; it is a shared availability dependency.

## Authorization

| Action | Governance | Responder | Blacklist | Recovery / dispute |
| --- | --- | --- | --- | --- |
| Pause | yes | yes | no | no |
| Unpause / enrollment | yes | no | no | no |
| Block immediately | yes | no | yes | no |
| Propose, execute, cancel blacklist clear | yes | no | no | no |
| Propose / cancel role rotation | yes | no | no | no |
| Accept role rotation | exact nominated candidate only | same | same | same |
| Revoke responder immediately | yes | no | no | no |
| Set / propose, execute, cancel rotation delay | yes | no | no | no |

Recovery and dispute roles are exposed to token policy; this controller gives
them no executable asset or administration capabilities. The responder has
only persistent pause containment. It cannot unpause, change enrollment or
blacklists, rotate roles, or recover assets. There are no recovery, migration,
message forwarding, or arbitrary execution entry points here. Every entry
point rejects attached funds.

All configured role addresses must be valid and pairwise distinct, including
recovery versus dispute. The controller itself cannot hold a role. Proposal
and acceptance both validate the complete resulting role configuration. These
checks cannot detect overlapping constituent keys behind different accounts,
multisigs, or contract wallets; deployment verification must check those too.

## Delays, epochs, and audit trail

Initial configured delay must be between 172800 seconds (48 hours) and
31536000 seconds (365 days), inclusive. Governance proposes a candidate;
only that candidate can accept the exact proposal nonce once its deadline is
reached. Replacement always creates a fresh nonce and restarts the entire
currently configured delay. Cancellation removes the proposal, and nonces
are never reused. Governance rotation immediately removes the previous
governance's controller privileges and cancels its pending delay change.
Responder revocation also cancels a pending responder replacement or enablement.
Reenabling a revoked responder always requires another delayed proposal.

Increasing the configured delay through `propose_rotation_delay` takes effect
immediately and cancels pending delay shortening. An equal value also cancels
a pending shortening. Shortening requires a separate exact-nonce execution
after the **old** configured delay. Replacing a shortening restarts that old
delay. Existing role and blacklist-clear proposals retain their recorded
deadlines when the configured delay changes; changes are prospective.

`set_blacklist {address, blocked:false}` is governance-only and schedules a
clear; the address stays blocked until governance executes that address and
nonce after the old/current rotation delay. Replacing a clear restarts the
delay. Any immediate block by governance or blacklist authority cancels that
address's pending clear, even if it was already blocked. Clears are bound
internally to a governance tenure: rotating A to B and later back to A cannot
revive an old A proposal. Stale proposals remain individually queryable and
may be canceled or replaced; invalidation needs no map iteration.

All timing uses consensus block time. Integer-second ABI deadlines round up
fractional proposal timestamps, so truncation cannot shorten a delay. Execution
at the recorded second is allowed; the preceding second is rejected. There
are no block-height shortcuts, local-clock assumptions, timeout unpause, or
test-mode production branches.

`epoch` starts at 1 and increments on each actual security state mutation,
including pending proposal changes. Idempotent operations leave it unchanged.
`authority_epoch` starts at 1 and increments only on actual role assignment
changes, including revoking an existing responder. Proposals, cancellations,
pause, enrollment, blacklist, and delay changes do not increment it. Nonces
are globally unique across role, delay, and blacklist-clear proposals; all
counters use checked arithmetic. Mutation responses emit action, sender,
epoch, authority epoch, and relevant candidate/address, nonce, deadline,
replacement, or cancellation attributes. Queries expose interface version
and both epochs; `blacklist` exposes blocked state and the pending clear.

## Deployment and upgrade verification

No deployment has been performed for this implementation. An arbitrary
governance address is **not proof of a multisig**. Before enrolling or
unpausing, the deploying agents must record and independently verify:

* Chain ID, controller address, code ID/checksum, compiled artifact checksum,
  source commit, and initial config query.
* Governance account type or contract code/checksum, threshold, voting rules,
  constituent signer identities/custody separation, and upgrade/admin control.
* Distinct role addresses, constituent-key overlap, and responder service scope.
* Every token address, expected immutable controller pointer, token code and
  governance binding, guard ABI, enrollment, and fail-closed behavior.
* Chain-level controller admin: immutable (`None`) or explicitly verified
  governance custody. A responder/blacklist/untrusted chain admin can bypass
  application-level permissions by replacing code and is unacceptable.

The contract deliberately exports no `migrate` handler. Code upgrades, if a
chain admin is retained, require a separately reviewed destination contract
that explicitly validates source version and preserves roles, pause,
enrollment, blacklist, nonces, pending deadlines, and both epochs. The chain
admin's external policy must enforce the required upgrade delay; this
controller cannot enforce the chain's admin permissions. Pause first, verify
the destination code and state transformation against the pinned artifact,
execute through verified governance, and verify guard state before unpausing.
An immutable controller deployment requires deploying a new controller and
explicitly migrating/redeploying the dependent token contracts according to
their independently reviewed migration policy. Never treat a controller
address change as an automatic fallback or disable the guard on query failure.

## Reproduce

From the workspace root:

```sh
cargo test -p cw20-security-controller --locked
cargo clippy -p cw20-security-controller --all-targets --locked -- -D warnings
cargo +1.81.0 build -p cw20-security-controller --lib --release --target wasm32-unknown-unknown --locked
```

Generate the checked-in controller schemas from this directory:

```sh
cargo run --bin security-controller-schema --locked
```

The Wasm build is verified with Rust 1.81.0 and its installed Wasm target.
The workspace's default Rust 1.99.0 toolchain fails to link CosmWasm 1.5 host
imports; use the explicit toolchain above for this artifact.
The resulting Wasm is `target/wasm32-unknown-unknown/release/cw20_security_controller.wasm`
under the workspace root. A production release must pin an optimizer/toolchain,
validate the Wasm with the target chain's checker, and record the artifact's
identity separately; a local build and unit tests establish no live deployment.
