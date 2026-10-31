# CW20 rollout evidence and migration sequence

Read-only snapshot: 2026-10-31T02:15:22.576719+00:00. Source: [current code 10184 registry](https://terra-classic-lcd.publicnode.com/cosmwasm/wasm/v1/code/10184/contracts?pagination.limit=100), [code identity](https://terra-classic-lcd.publicnode.com/cosmwasm/wasm/v1/code/10184), and [dated DEX deployment trace](https://git.cl8y.com/code/cl8y-dex-terraclassic/src/branch/main/deployments/mainnet-soft-launch/deploy-trace.md).

All 35 returned contracts were queried successfully for chain contract info, token info, primary/additional minters and raw cw2 version. Pagination next_key is null for code 10184 at this read. Every cw2 record is crates.io:cw20-base0.1.0, matching the explicit migration allowlist. Code10184 data hash is `C77FFCDB0039041FBD071C76EFB547232314CE8CCF219C3BF5B7FABD4CF7D4A4`. These are latest non-atomic LCD reads; repeat at a pinned height immediately before any transaction. This is a code-specific inventory, not a claim to discover every CW20 or integration.

Raw receipts: [inventory](columbus-5-inventory.json), [95 selected admin/minter/pool/router balance queries](selected-integration-balances.json). Those balances are bounded integration evidence, not a complete holder enumeration or snapshot backup.

Governance identity: `terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7` has an on-chain `/cosmos.crypto.multisig.LegacyAminoPubKey` key, threshold 2 with 3 public keys. [Account evidence](https://terra-classic-lcd.publicnode.com/cosmos/auth/v1beta1/accounts/terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7). This proves registered multisig structure; it does not establish unlocked quorum custody, non-overlapping constituents for recovery/dispute roles, signer availability or authority for third-party contracts.

Available signing checks: terrad 3.5.0-rc.0 is installed; test keyring has no records. The existing encrypted file keyring requires a passphrase: noninteractive listing with EOF fails passphrase attempts and exposes no unlocked signer. Broker variable-name discovery found no Terra/Cosmos/mnemonic/signing credential. No passphrase was guessed or requested; no live transaction was broadcast. The missing capability is an unlocked authorized governance quorum/signing route, plus verified identities for every separate privileged role.

| Symbol | Address | Decimals | Supply base units | Admin | Primary minter |
|---|---|---:|---:|---|---|
| CLUN-USDT-LP | terra10ur635zd6fmt4fxveven8lcx8xkr55t6dxjxh5dctmcc5lrxjqqslq4l3s | 18 | 140331010250648873 | terra17l7eqc5j8vkm09up55etfggpr6y92ka6p03yc765mt3nerqcnhdsl6l7jq | terra17l7eqc5j8vkm09up55etfggpr6y92ka6p03yc765mt3nerqcnhdsl6l7jq |
| UST1-ALPH-LP | terra12ff3nhu239y6a5lh0rs8nwlntc9wulkm3c4x598hp5gjfeqk87xszxkrmp | 18 | 313558847300 | terra1rmdtckz5gd0usja36ydwat6prnmew639ry37yq72xh9ek4s3m83sehn5u6 | terra1rmdtckz5gd0usja36ydwat6prnmew639ry37yq72xh9ek4s3m83sehn5u6 |
| TOPAZ | terra12k67cvfs7y7g8lca3qr4g4py6s6j69fu24gze5pjfamfpckv8mps7cymme | 6 | 10000400000000 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 |
| CLUN-CUST-LP | terra132uuzdnjce0c8g5dalyvdgl47ny697udesk972cg05e5y7gn485qz6tdch | 18 | 46794468281131 | terra15rl8g308yzzt5kxu4skgwlahrvm8adyv0s2cupsmvte0akgs2ttsszau38 | terra15rl8g308yzzt5kxu4skgwlahrvm8adyv0s2cupsmvte0akgs2ttsszau38 |
| CL8Y-CLUN-LP | terra13rfqc8qklg5whh33t55ceah8mwkl88c2tq5wjcvzkpr3jv65y5psz0jcwu | 18 | 478322296752555 | terra1q5karg99p5hc0jvplgn2fd8akg39r4ss00cu5rwkm600yqvmssgqhwqvq0 | terra1q5karg99p5hc0jvplgn2fd8akg39r4ss00cu5rwkm600yqvmssgqhwqvq0 |
| cLUNC | terra1437qslye72t7qmmahn4t5chz50r8a62g45phwkquwpyu2l62u6ksqssgdg | 6 | 706138710382774 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l |
| testa | terra14esahmssytksgamxnqsna2gmc9s6hzxhazsvtxvpm3k0hqatu6kqkvu4wv | 18 | 0 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1saxhfawhepuet3hraeue0gapnts679rkhxpf34q5rl8fncctxeusqgvjxs |
| testa | terra16ahm9hn5teayt2as384zf3uudgqvmmwahqfh0v9e3kaslhu30l8q38ftvh | 18 | 80986418010000000000 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1njpnexcdnqm8cnl7g3fs5y60zm9l6xsflzpqv3 |
| CL8Y-cb | terra16wtml2q66g82fdkx66tap0qjkahqwp4lwq3ngtygacg5q0kzycgqvhpax3 | 18 | 517071511499499741546248 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 |
| ONYX | terra178fgrfzv7njtmdp9vghyf2dx77sah8u8jluzs7ym562chaxnmj2s6mn6m9 | 6 | 10000400000000 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 |
| UST1-USDT-LP | terra17nny5femmkq2nfu7ne9nxw0zjnpgwxl0zvfzqwg88vgskhldztxqytjtus | 18 | 9999999999999000 | terra1qfa8hn0tnu33mnpahl382ekyd4873gmp2vsvcm3apw526tnrh8aqtrknvf | terra1qfa8hn0tnu33mnpahl382ekyd4873gmp2vsvcm3apw526tnrh8aqtrknvf |
| UST1-USTR-LP | terra1ak8w9k34ex237h9pmquxqjevhzvflqaatuaj9pr8ym287n7atj6qw2ty4p | 18 | 256086414142350466 | terra16vxrhpvpcucu05y0nr862vf9hnqeh274uaff4s7hz4n0ea74006qf5hgqy | terra16vxrhpvpcucu05y0nr862vf9hnqeh274uaff4s7hz4n0ea74006qf5hgqy |
| testb | terra1cr32lfrc6xedzr6pzczcwgrucfuu55jgmtwl5lwfupau8pg0ut0qzp6a4a | 18 | 0 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1saxhfawhepuet3hraeue0gapnts679rkhxpf34q5rl8fncctxeusqgvjxs |
| tdec | terra1dfmvdvcpwa2ycvy2zty8eh9q2q9xpkf7a9dqnyaa52fxxzlhqjnsfqkhku | 6 | 0 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1njpnexcdnqm8cnl7g3fs5y60zm9l6xsflzpqv3 |
| EMBER | terra1dmuruhht32x8f47nvm73pwp6q7uf2jtfhdt3nxcql4mmqkyfsraqn2dt94 | 6 | 10004300000000 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 |
| JADE | terra1ejq3mjjgnklpa3pg4jterlfwsny055gpmcjf3fz0ev3ueajnzeysz6xxgr | 6 | 10000300000000 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 |
| UST1 | terra1f0eqgy9w7e5e7up97vjudqwx38tesf8ylx75x2lv3nwm0clry0pqmgfy72 | 6 | 87862406295 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l |
| tdec | terra1f0xcljcamzgqerw5rl2ag2wj42tm6d3u5dvtrjl8gychzd52c7sslhdmlt | 6 | 0 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1saxhfawhepuet3hraeue0gapnts679rkhxpf34q5rl8fncctxeusqgvjxs |
| USTR-USDT-LP | terra1fcmvnf7rseg5jqq8774nwwwtzkre9nvu4dkqv9g0se9pmsdruyds7akmlz | 18 | 0 | terra1864d9fvq6hmuddkyxnr5fcf60prfc2lnujl5kstjnu8lvstxqkyqhg4x9g | terra1864d9fvq6hmuddkyxnr5fcf60prfc2lnujl5kstjnu8lvstxqkyqhg4x9g |
| RUBY | terra1fga508hzx8dd7x8q4uhm6mdhkqv6fxrtsea3r27smdqmv5k2jgxq5zk9fc | 6 | 10000200000000 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 |
| CL8Y-ALPH-LP | terra1hymuueuu43750rzut69hxmefl3m5gg27uxnu9fytcvlah27l5rtsrg87rc | 18 | 456193389812784420 | terra163qm8z5rjgp8av6c6sg673lq2v4kfa0we5uhtzj8alfwhddfhjfs27k40z | terra163qm8z5rjgp8av6c6sg673lq2v4kfa0we5uhtzj8alfwhddfhjfs27k40z |
| UST1-CUST-LP | terra1jv6y058w6qx0xzcspcuv0du4qj95amwhvy4lftlf7mlyjj24v9ms8dzdgy | 18 | 672848767981 | terra1ceprjsxp86ggftf5e38wwt34l83e5gq7penkdnv4wsatkwcs8v6qccw55f | terra1ceprjsxp86ggftf5e38wwt34l83e5gq7penkdnv4wsatkwcs8v6qccw55f |
| CORAL | terra1k6cqupylk0wp4pj273pntwhv9py0q5guyqye8ssvukn9xq7mes7sdlmena | 6 | 10000300000000 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 |
| CLUN-UST1-LP | terra1mk3krh8pk9wslqhasessk8rydqs4y9krprfgtnssfxr3l23pvgqsp2cagk | 18 | 64798135 | terra1su5363453fj326u4t0kqar30f35cm3n0dc9yksg379u6875z350s4mm7h4 | terra1su5363453fj326u4t0kqar30f35cm3n0dc9yksg379u6875z350s4mm7h4 |
| vFDUSD | terra1mnl9azefrqpmu888ar2u6zrcwr80hxlt3avf4300r576cw5ar7esvxsvj3 | 6 | 54621539 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra18m02l2f43c2dagqnz3kfccpgz9pzzz5hk9l5mh5wvr6dcvv47zfqdfs7la |
| cUSTC | terra1nap4dxh9tv35v0ynd9m4k6zt6c0dq6weszc4j5m564kjls56hu7qcr56ch | 6 | 16449608339564 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l |
| tdec | terra1pa7jxtjcu3clmv0v8n2tfrtlfepneyv8pxa7zmhz50kj8unuv0zq37apvv | 6 | 31029986 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1njpnexcdnqm8cnl7g3fs5y60zm9l6xsflzpqv3 |
| UST1-SPAC-LP | terra1s3jk92zeslgxxaux5nj8gtcqxafwsufglw2rhkazv4rrt7sg3wfsqj7twz | 18 | 999999000 | terra1xx5t5em3aza3lst0s5yc7rjgx9psapa3345v2vzfqkrprhw3vv6q3hahxy | terra1xx5t5em3aza3lst0s5yc7rjgx9psapa3345v2vzfqkrprhw3vv6q3hahxy |
| CL8Y-CUST-LP | terra1u277xxcknv2r37d7xa5mnyxu3q26fyu9e9uexmyu2u99g3qfx62q2jen2c | 18 | 27269103075920389 | terra1tz5vwrungh6drd9nt95qym3k892vs3as8nqmu7sg4ypek7wxvv4qm89upc | terra1tz5vwrungh6drd9nt95qym3k892vs3as8nqmu7sg4ypek7wxvv4qm89upc |
| KENA-CLUN-LP | terra1vll5ekvxvxq092gv5fvq0vhk8vd3z5a7d57mjt4ksf9v2qk2k86s8snvwe | 18 | 6158171121114 | terra1vwsq6jd2kxkt4d5w6435fajla3qxgr2vvhjvvufxvlerp3pfjsesm9gpds | terra1vwsq6jd2kxkt4d5w6435fajla3qxgr2vvhjvvufxvlerp3pfjsesm9gpds |
| testb | terra1vqfe2ake427depchntwwl6dvyfgxpu5qdlqzfjuznxvw6pqza0hqalc9g3 | 18 | 46088745000000000000 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1njpnexcdnqm8cnl7g3fs5y60zm9l6xsflzpqv3 |
| USTR | terra1vy3kc0swag2rhn7jz6n72jp0l2ns0p6r6ez5grxq5uhj2rvs97fqfsetxv | 18 | 39845609729561349720540616 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l |
| testb | terra1y8yggsu8gt7lrndtx7rf0ky3ya3vept059kz50rzz4r6s848wl2qw27y35 | 18 | 0 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1njpnexcdnqm8cnl7g3fs5y60zm9l6xsflzpqv3 |
| USDT | terra1z0xe7t5ymmltg4vju8tghkq0pewy4et548ta23nlu9zxtl950uyqkv8mv4 | 18 | 12252366370698519348072 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 | terra1zlmv2xydxcusurtr6rl78wsvytdc6mfex6hep7 |
| testa | terra1z5r33y2fv70vdvdww3uu05rakqxwxc68ua5n0wcadzk5u5kl44qs7clk85 | 18 | 0 | terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l | terra1njpnexcdnqm8cnl7g3fs5y60zm9l6xsflzpqv3 |

Admin distribution: 8 registered governance multisig; 14 terra1xsecn4snv94ezcez0z3vq8an9j4h4kxxcydp8l; 13 distinct LP/pair contract addresses. No admin=None occurs in this code 10184 snapshot. A contract admin address does not establish an executable migration route: verify each pair/contract implementation and actual governance permissions separately. Other code IDs, immutable tokens and third-party admins remain uncovered until separately inventoried.

The six soft-launch tokens (EMBER,CORAL,JADE,ONYX,RUBY,TOPAZ), CL8Y-cb and USDT currently share the historical governance address as primary minter. The new token deliberately rejects governance/minter overlap. Do not migrate these using that same governance input without an independently verified issuer/governance separation transaction and re-query of BOTH primary and additional minters. No invented issuer or arbitrary address is an acceptable workaround. vFDUSD bridge minter, faucet additional minters, LP issuers and wrappers need their own reviewed assignments; contract/minter status never grants automatic exemption.

Current program deployment coverage: controller addresses 0; security-v1 migrated/enrolled tokens 0/35 listed here; no testnet/mainnet receipts. Existing tokens continue using existing code and are not paused by this program. The BSC CL8Y non-upgradeable token, native assets, bridged original nonce 153 and unrelated chains are not covered by a Terra controller. See issue #8 research for actual rescue versus compensation facts.

## Exact staged sequence

1. Bind release to independently reviewed source, generated schemas, final optimized artifact hashes and target-compatible checker receipts. Bounded local cross-contract proof runs actual controller/token entrypoints with deterministic test time; it is not a live-chain migration. Never use clock manipulation to mature a real pending change.
2. Verify all privileged role accounts on chain (type/code hash, threshold/voting/custody and constituent separation), actual admin/issuer permissions and available signing route. Select explicit distinct governance, blacklist, recovery, dispute and pause-only responder identities. Address syntax alone proves none of these. Confirm controller/token chain admin custody or deliberate immutability; admin code replacement can bypass all application timers.
3. Refresh inventory at a consistent height. Export full legacy state off chain with bounded pagination and backups; preserve balance, both allowance indexes, token metadata, primary/additional minters and total supply. Verify each migrate source version/code checksum. New security states must never be discarded by future upgrades. Resolve governance/minter overlap before migration.
4. Complete DEX policy compatibility review before changing code IDs: factory whitelist/new-template audit, pair token-code pins and freeze/unfreeze, router, LP accounting, escrow bridges/wrappers, treasury and faucet flows. Current DEX catalogue treats administrative pause/blacklist/recovery as sensitive policy changes; no automatic whitelist or silent pin update. Each exempt protocol address needs a reviewed evidence digest.
5. Store exact reviewed controller/token bytes using actual authorized custody; record chain/code IDs, data hashes and store txs. Instantiate controller paused=true with verified roles and chain-admin policy. Verify controller config/schema/version/role separation and custody again. No guessed deployment JSON or role placeholders may be broadcast.
6. Migrate one authorized token with exact controller and independent governance input. Verify unchanged supply, sampled balances and allowance/minter state, immutable controller pointer and current governance role. Enroll exact token; unenrollment remains fail-closed. While paused, schedule reviewed exact-address protocol assignments. Initial enabling requires actual old>=24h wallet delay; plan this containment window openly rather than bypassing it with special bootstrap exemptions.
7. At real deadlines, execute assignments with exact nonces/current authority epoch. Confirm effective protocol exemptions, lock compatibility, query failure containment and enrollment/code pins. Then verified governance may unpause. Responder cannot unpause, recover, clear blacklists or assign privileges. Verify guarded transfer/send/From/mint/allowance behavior with finite non-economic amounts and record real receipts.
8. Repeat bounded batches only after independent receipt checks. Publish per-token controller/code/admin/minter/exemption/coverage status. A shared pause is authoritative for all migrated/enrolled tokens immediately; unmigrated or otherwise uncovered assets must stay visibly excluded.
9. Rollback is pause + separately reviewed forward fix, preserving namespaces, reservations, pending payloads/nonces/epochs and old deadlines. This version intentionally accepts only legacy0.1.0 migration; future0.2.x upgrades need a new reviewed migration handler. Never restore unguarded legacy code as an operational workaround. Controller has no migrate handler; replacing its code/address requires separately reviewed state preservation and governance custody.

## Acceptance and ownership

Issues #4–7 stay open for factual deployment, multisig custody, full integration coverage and live receipts not yet available. Source approvals and CI passes do not prove those facts. Contract lead thr_n4vtxxfcb7 owns this rollout; parent thr_4rw76j9ker owns coordination. No human approval or per-issue script is required. Future execution remains with those agents once the real signing capability and role identities exist.

## Reviewed artifact provenance

[Machine-readable build/review/coverage record](artifact-provenance.json) and
[matching CosmWasm 1.5.11 checker output](cosmwasm-check-1.5.11.txt).
Build script: [scripts/rollout-build.sh](../../scripts/rollout-build.sh).
Rebuilding and optimizing twice produced identical hashes:

| Artifact | SHA-256 |
|---|---|
| cw20_mintable.wasm | `b90a837ed929ef638377ca7e9a72091c7a7c6730f1d291001fe580355e01a33f` |
| cw20_security_controller.wasm | `f78c0175267aed0f70ab4054be5c6fa8303efc37937ed65f92e908494ebe6de6` |

Token source/schema pin `2d70d43c651ecd4491c0736c89ae2975ee825686` and
controller source pin `3333807148f096cc5894c2b74442655bc5f4ad93` received separate
independent review. Later CI/docs commits do not change those source trees.
There are 91 passing native tests, strict workspace clippy and deterministic
schema generation. Optimized Wasm passes structural validation and the matching
CosmWasm checker. These checks do not establish target-chain custody or deployment.
