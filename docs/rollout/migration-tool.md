# Deferred Terra Classic migration tool

Tracking issue: [#16 — deferred whitelist-gated migration](https://git.cl8y.com/code/cw20-mintable/issues/16).
Deployment is currently waiting for the Terra Classic whitelist. No upload,
instantiation, migration, enrollment or chain signing has been performed by this
change. Coverage remains **0/35**. Source review and unsigned packets do not change it.

Run the safe entrypoint now or later:

```sh
python3 scripts/rollout-migrate.py
```

It prints an offline plan without network queries, keys or transactions. Python3
standard library and the installed terrad CLI are the only dependencies. The
program handles one action at a time; it has no retry loop or automatic rollout.
Its broadcaster consumes an externally signed transaction, never a mnemonic,
password, SSH key or private key. Registered on-chain multisig structure does not
prove available quorum custody.

## Inputs agents must resolve after the whitelist is released

Use [migration-config.example.json](migration-config.example.json) as the config
contract. It deliberately contains null unresolved identities and will fail
closed. Agents, rather than the human, must complete and independently review
these inputs before generating a ready execution command:

- Exact deployer, governance, blacklist, recovery, dispute and optional responder
  identities and available authorized signing/quorum routes. `identity_keys`
  maps each role, deployer and token admin address to the exact public `pub_key`
  JSON from `/cosmos/auth/v1beta1/accounts/ADDRESS`. Required sensitive roles use
  registered multisigs with threshold >=2; all roles must have separate
  constituent keys. This bounded tool supports BaseAccount authorities only.
  Contract/multisig-contract admins and nested multisigs require a separate
  independently verified forwarding implementation; they cannot sign as EOAs.
- Reviewed optimized artifact files with the fixed hashes from
  [artifact-provenance.json](artifact-provenance.json), plus actual positive stored
  code IDs when available. Paths are relative to the command's current directory.
  Existing verified code IDs avoid an upload; they do not waive any instantiate,
  DEX policy or authority gate. Upload permissions are queried afresh for every
  store preparation and broadcast. Nobody, blocked sender, stale chain head,
  unrecognized permission or unpinnable LCD query stops the tool.
- An HTTPS archival LCD that attests `x-cosmos-block-height` for every pinned
  query and an HTTPS RPC serving the identical block header. The sample public
  LCD is a candidate endpoint, not a tested availability claim.
- The controller address after instantiation, exact multisig chain admin,
  rotation delay, and token's actual admin, primary `Minter` response (possibly
  null), complete additional-minter list and intended baseline path. Resolve
  governance/minter overlaps in a separately reviewed issuer transaction first.
  The 13 LP contract-admin tokens remain blocked until their real forwarding
  routes are implemented and independently reviewed.
- Actual DEX factory/policy contract query, `response_sha256` of canonical sorted
  compact JSON, nonempty `approval_path` to the response's list of integer
  approved CW20 code IDs, exact `token_code_id`, and a lowercase SHA256
  `review_sha256` identifying the independently reviewed compatibility evidence.
  The tool checks actual queried membership and response equality; a boolean
  approval or upload whitelist is insufficient. If the DEX schema uses another
  shape, agents must add and review that exact adapter rather than fabricate a
  response. Review must include factory/template and pair pins, router, bridge,
  wrapper, LP accounting and faucet behavior.
- Each token's complete reviewed integration `assignments`: exact address and
  `{protocol:true, exempt:true, allow_lock:false, evidence_hash:"64 lowercase hex"}`
  (allow_lock=true only when separately justified). No exemption is inferred
  from being a pool, issuer or contract. Finalization `cohort` contains full token
  objects in the same shape as `token`, including each baseline and assignments.
  It must equal the complete enrolled set of the shared controller, up to35.
  Unmigrated tokens remain visibly uncovered.

The example finite fee is 10 LUNC and gas limit5,000,000 per action, with a
20-block expiry. These are conservative configuration defaults, not a gas quote;
agents should verify current fees and available finite funds before execution.
The tool never simulates with an unlocked key or increases a fee automatically.

## One action, receipt, then the next stage

Commands below describe the later agent workflow; `run.json` is the completed
reviewed config and packet directories must be new. No command here has been
executed against the chain by this change.

```sh
python3 scripts/rollout-migrate.py prepare --config run.json --stage store-controller --packet run/store-controller
```

Supported stages, in order: `store-controller`, `store-token`, `instantiate`,
`migrate`, `enroll`, `assign`, `activate`, `finalize`. Upload stages are unnecessary
when exact stored code IDs are already verified. For store, leave the respective
code ID null; after the committed receipt, agents extract the code ID, verify its
hash, and update the next stage's config. Store restricts instantiation to the
configured deployer. The controller constructor always starts paused; the
prepared constructor uses the actual typed roles/rotation_delay ABI and has no
fictional `paused` field.

For a `migrate` packet named `run/token-a`, set `token.baseline` to
`run/token-a/baseline.json` **before** preparing it. The snapshot exports every
raw legacy key/value with bounded pagination (maximum100,000 rows,100/page),
including both allowance indexes, balances, metadata, supply and minters. Packet
files contain public chain state and should be retained as evidence. Raw Model
keys are normalized hex; values are base64, matching the upstream
[wasmd protobuf schema](https://raw.githubusercontent.com/CosmWasm/wasmd/v0.46.0/proto/cosmwasm/wasm/v1/types.proto).
For this supported source, only cw2 version and new security namespaces may
change. Every later stage compares all original legacy values to the snapshot.
A live legacy transfer racing migration can therefore stop receipt verification;
pause and investigate with a forward fix, never restore unguarded legacy code.

`prepare` writes `unsigned.json` and a packet containing the exact action,
config digest, sender, account number/sequence, pinned-height facts and raw-state
digest. It uses `terrad --generate-only --offline` with an address rather than a
key name. In installed terrad3.5.0-rc.0, offline generation requires account number
and sequence and forbids `--chain-id`; the packet binds columbus-5 and the external
signer must use that chain ID. This CLI behavior was checked locally without a
key or network transaction.

An available authorized chain signer or registered quorum signs `unsigned.json`
using the recorded sender/account number/sequence and **columbus-5**. Keep exact
body, fee/gas and timeout; do not add funds or change sender. The agent must use
its established secure signer workflow. If custody is unavailable, retain the
unsigned packet and status in the issue; no SSH key can substitute for a chain
key and no extra human signing ceremony is implied.

```sh
python3 scripts/rollout-migrate.py broadcast --config run.json --packet run/store-controller --signed run/signed.json --execute
python3 scripts/rollout-migrate.py receipt --config run.json --packet run/store-controller --txhash COMMITTED_TRANSACTION_HASH
```

Only `broadcast` with both `--signed` and `--execute` sends a transaction. It
repeats fresh preflight, compares body/action/identity/sequence/state/config,
checks fixed fees and gas, requires preflight to finish within two blocks, verifies expiry and the RPC/LCD block context, then
submits exactly one signed transaction via stdin. It does not read the keyring.
Submission intent is saved before network broadcast; an attempted submission cannot be broadcast again through the same packet, including after a timeout/crash. After
an ambiguous network response, query/reconcile the actual chain receipt before
preparing another packet; there is no automatic retry. Sync submission success
is not commitment. `receipt` requires a committed successful transaction with
exact matching body and saves the chain response; migrate receipts additionally
verify controller binding and every original legacy state value.

Retain each stage's original config alongside its packet: receipt verification
requires that exact config, so collect receipts before editing config for a
later stage. Never edit a prepared packet to renew expiry. Prepare a fresh
packet if its20-block signing window expires or any checked state changes.

Enrollment precedes assignments. Each `assign` schedules the first outstanding
reviewed address; repeated preparation cannot replace an existing timer.
`activate` reads the actual pending nonce, authority epoch and exact payload,
checks at least the old wallet delay (>=86400s), and waits for the real chain
block time. There is no local clock override, bootstrap exemption or skip.
Repeat these short steps for each address/token while the controller is paused.
`finalize` verifies all enrolled cohort tokens and effective assignments before
preparing **governance's** unpause action. Responder unpause is never generated.

Receipt confirmation and current state checks are prerequisites to the next
agent-owned batch. Finalization is not end-to-end live economic acceptance:
finite guarded transfer/send/From/mint/allowance checks and integration receipts
from the rollout plan remain required. On failure use pause and a separately
reviewed state-preserving forward fix. No legacy rollback exists in this tool.

## Local verification

```sh
python3 -m unittest discover -s scripts/tests -v
```

Tests use fixtures and never sign or broadcast. They cover offline defaults,
whitelist rejection, artifact/admin/code/key drift, constituent separation,
contract-admin denial, complete legacy state preservation, real assignment
nonces/deadlines/epochs and submission resume protection. Existing contract tests
and the optimizer are intentionally not rerun for these script/docs changes.
