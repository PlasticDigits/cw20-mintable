# Token security ABI v1 and policy

Authoritative typed messages: src/msg.rs, generated schema/cw20-mintable.json and
schema/raw/*.json. New messages use snake_case. Legacy CW20 PascalCase serialization
and snake_case aliases remain supported. Decimal-free policy uses basis points.

Instantiation requires security {controller, governance}. It rejects missing security,
controller/governance mismatch and governance=minter. Migration accepts only this
fork's recorded crates.io:cw20-base 0.1.0 source; a mismatched source needs a separate
reviewed migration. It never iterates holders or allowances and preserves legacy
storage namespaces. Existing reverse allowance index is retained, not reconstructed.
Controller address is immutable through execute APIs. Current governance follows
controller roles; initial governance is a validation input, not a permanent pin.

Default policy: enabled, 1000bps (10%), 86400s relaxation delay, 172800s unlock delay.
The protected opening balance is lazily fixed at first debit, setting change or
inbound credit. Existing nonzero holders use their pre-credit balance; new zero
holders use their first nonzero receipt. Top-ups never increase an established basis.
`rebase_protection` snapshots current balance and schedules a new basis using the old
relaxation delay. Rebase/policy/exemption/lock changes never clear spending history.
This conservative behavior is deliberate: large later deposits require delayed rebase
or a deliberate delayed opt-out. No oracle or token-decimal assumptions.

Rolling spending uses at most 25 hourly buckets. A debit remains counted until the
hour containing it is more than 24 hours old (24–25h actual retention). This allows
no split or boundary burst above the configured opening-balance cap in any rolling
24h. Exact expiry can be up to one hour conservative. Incoming funds, settings changes,
lock/unlock, exemptions and re-enabling cannot reset buckets. Every ordinary owner
debit (including delegated burn/send) counts, even when exemption/opt-out is active.
Allowance increases are guarded; decreases/revocations stay available in containment,
but decrease cannot extend expiration or switch incomparable expiration clock domains.

`set_wallet_policy` tightens immediately; a mixed change with ANY weakening schedules
the whole exact payload using OLD relaxation_delay (>=24h). Bounds: bps 0..10000,
relaxation >=24h, unlock >=48h, both <=365d. An intervening tightening invalidates all
pending wallet payloads. Replacing restarts, cancellation never changes effective
policy, exact nonce is required to execute; execution removes pending state. Opt-in
`lock_wallet` is immediate and cancels pending unlock/change. `request_unlock` uses
OLD unlock_delay and explicit `execute_wallet_change` is required at expiry.

Governance `set_assignment` requires a lowercase SHA-256 evidence digest. Enabling an
exemption, classifying/declassifying a protocol address, or allowing protocol lock
requires old wallet relaxation delay. Removing privilege is immediate. Assignment
execution requires matching authority epoch and current governance. No automatic
contract, issuer, minter, pool, bridge or treasury exemption. Protocol accounts reject
lock enrollment unless explicitly reviewed allow_lock. Governance must inventory
actual integrations before classification and enrollment. Existing unclassified
protocols cannot be detected from address syntax; deployment inventory is a gate.

Recovery: governance proposes immutable exact source/destination/amount/digest.
Separate recovery role executes at >=2592000s after proposal. Destination signs
`accept_recovery {nonce}` on the actual token/chain; that signature accepts the exact
stored case, not a portal assertion. Revocation permanently invalidates that nonce.
Exact source amount is reserved until execution or two-role cancellation. Multiple
cases cannot over-reserve. Source key may object once; unresolved objection blocks
execution until independent dispute role records resolution digest. Duplicate source
objections after resolution do not restart or veto. Dispute role can veto irreversibly
for a nonce; reservation remains. Governance can atomically replace same-source case
with new exact payload, new acceptance and full 30d delay. Role epoch change invalidates
execution but never frees reservation. Cancellation: governance requests, independent
dispute role confirms; no source-key cancellation. A cancellation does not clear a
controller blacklist. Only destination blacklist blocks this special movement path;
pause, source blacklist and wallet lock do not. Supply/allowances stay unchanged.
No automatic expiry releases funds; governance evidence is a trusted attestation,
not on-chain proof of historic entitlement. Private evidence stays off chain.

Global nonce is shared across every token timer/case/assignment; identity is chain ID,
token contract, kind and nonce. Timer events carry token/wallet or source/destination,
nonce, initiator, proposed_at/executable_at. Pending queries are authoritative.
Events have no PII. Portal users must read exact chain/token/destination/amount/digest
before accepting. Replacement emits old nonce and replacement_nonce.

Controller unavailability or unenrollment prevents ordinary economic actions;
containment cannot silently expire. Safe local cancellation, tightening, objection and
acceptance revocation remain possible. Admin migrations can replace these protections:
verified multisig admin custody and monitoring are deployment requirements.

Compatibility: DEX pools/router/LP, bridge/wrapper escrow and treasury throughput is
usually incompatible with 10% daily debits; enroll only after reviewed exact-address
assignments and delayed exemptions are active. Exemption does not bypass shared pause,
blacklist, opt-in lock or recovery reservation. Source policy tests are not evidence of
live coverage. Unmigrated/immutable/third-party-admin tokens, BSC tokens and native coins
are uncovered. See issue #8 for adjudication procedure and incident distinctions.
