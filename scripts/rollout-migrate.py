#!/usr/bin/env python3
"""Bounded, fail-closed CW20 security rollout. No keys are read by this program."""
import argparse
import base64
import datetime as dt
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
HASHES = {
    'token': 'b90a837ed929ef638377ca7e9a72091c7a7c6730f1d291001fe580355e01a33f',
    'controller': 'f78c0175267aed0f70ab4054be5c6fa8303efc37937ed65f92e908494ebe6de6',
}
LEGACY_HASH = 'c77ffcdb0039041fbd071c76efb547232314ce8ccf219c3bf5b7fabd4cf7d4a4'
STAGES = ('store-token', 'store-controller', 'instantiate', 'migrate', 'enroll',
          'assign', 'activate', 'finalize')


def require(ok, message):
    if not ok:
        raise ValueError(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def load(path):
    return json.loads(pathlib.Path(path).read_text())


def write(path, value):
    # Never overwrite a prepared packet or a receipt.
    with pathlib.Path(path).open('x') as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write('\n')


def address(value):
    require(isinstance(value, str) and re.fullmatch(r'terra1[023456789acdefghjklmnpqrstuvwxyz]{38,64}', value),
            'missing/invalid Terra address; agents must resolve actual identities')
    return value


def access(policy, sender):
    """Only known CosmWasm AccessType values; unknown formats fail closed."""
    kind = policy.get('permission')
    if kind in ('Everybody', 'ACCESS_TYPE_EVERYBODY', 3, '3'):
        return
    if kind in ('AnyOfAddresses', 'ACCESS_TYPE_ANY_OF_ADDRESSES', 4, '4'):
        require(sender in policy.get('addresses', []), 'sender is outside chain code permission whitelist')
        return
    if kind in ('OnlyAddress', 'ACCESS_TYPE_ONLY_ADDRESS', 2, '2'):
        require(sender == policy.get('address'), 'sender is outside chain code permission whitelist')
        return
    raise ValueError('chain code permission blocked or unknown; wait for whitelist, no waiver')


class LCD:
    def __init__(self, url):
        require(url.startswith('https://'), 'LCD must use HTTPS')
        self.url = url.rstrip('/')
        self.height = None

    def get(self, path):
        headers = {'Accept': 'application/json'}
        if self.height:
            headers['x-cosmos-block-height'] = str(self.height)
        request = urllib.request.Request(self.url + path, headers=headers)
        with urllib.request.urlopen(request, timeout=30) as response:
            if self.height:
                require(response.headers.get('x-cosmos-block-height') == str(self.height),
                        'LCD did not attest requested height; use an archival/pinned-height LCD')
            return json.load(response)

    def pin(self, chain):
        header = self.get('/cosmos/base/tendermint/v1beta1/blocks/latest')['block']['header']
        require(header['chain_id'] == chain == 'columbus-5', 'wrong chain')
        timestamp = dt.datetime.fromisoformat(header['time'].replace('Z', '+00:00'))
        age = (dt.datetime.now(dt.timezone.utc) - timestamp).total_seconds()
        require(0 <= age <= 90, 'stale/future chain head')
        self.height = int(header['height'])
        self.now = int(timestamp.timestamp())
        return header

    def contract(self, addr):
        return self.get('/cosmwasm/wasm/v1/contract/' + address(addr))['contract_info']

    def code(self, code_id):
        require(isinstance(code_id, int) and code_id > 0, 'resolve actual positive code ID')
        return self.get('/cosmwasm/wasm/v1/code/' + str(code_id))['code_info']

    def smart(self, addr, message):
        query = urllib.parse.quote(base64.b64encode(canonical(message)).decode(), safe='')
        return self.get('/cosmwasm/wasm/v1/contract/' + address(addr) + '/smart/' + query)['data']

    def state(self, addr):
        rows, key, seen = {}, None, set()
        for _ in range(1000):
            params = {'pagination.limit': 100}
            if key:
                params['pagination.key'] = key
            data = self.get('/cosmwasm/wasm/v1/contract/' + address(addr) + '/state?' + urllib.parse.urlencode(params))
            for row in data['models']:
                key_hex = bytes.fromhex(row['key']).hex()
                require(key_hex not in rows, 'duplicate state key')
                rows[key_hex] = row['value']
            key = data.get('pagination', {}).get('next_key')
            if not key:
                return rows
            require(key not in seen, 'repeated pagination cursor')
            seen.add(key)
        raise ValueError('state exceeds bounded 100,000-row snapshot; prepare separately reviewed export')


def account_key(lcd, addr):
    account = lcd.get('/cosmos/auth/v1beta1/accounts/' + address(addr))['account']
    require(account.get('@type') == '/cosmos.auth.v1beta1.BaseAccount',
            'unsupported account/contract authority; independently implement verified forwarding route')
    require(account.get('address') == addr and account.get('pub_key'), 'account identity/public key unavailable')
    return account['pub_key']


def participants(key):
    if key.get('@type') == '/cosmos.crypto.multisig.LegacyAminoPubKey':
        children = key.get('public_keys', [])
        require(2 <= int(key['threshold']) <= len(children), 'privileged multisig needs threshold >=2')
        require(all(k.get('@type') == '/cosmos.crypto.secp256k1.PubKey' for k in children),
                'nested/unknown multisig requires independent route review')
        keys = [k['key'] for k in children]
        require(len(set(keys)) == len(keys), 'duplicate multisig participant')
        return set(keys)
    require(key.get('@type') == '/cosmos.crypto.secp256k1.PubKey', 'unknown signing key type')
    return {key['key']}


def identities(lcd, cfg):
    roles = cfg['roles']
    require(set(roles) == {'governance', 'blacklist', 'recovery', 'dispute', 'responder'}, 'exact role set required')
    addresses = [address(v) for v in roles.values() if v is not None]
    require(len(addresses) == len(set(addresses)), 'roles must be distinct')
    used = set()
    for role, addr in roles.items():
        if addr is None:
            require(role == 'responder', 'required role unresolved')
            continue
        actual = account_key(lcd, addr)
        require(actual == cfg['identity_keys'].get(addr), 'role public key/identity drift')
        if role != 'responder':
            require(actual.get('@type') == '/cosmos.crypto.multisig.LegacyAminoPubKey',
                    'sensitive roles require verified multisig custody')
        keys = participants(actual)
        require(not used.intersection(keys), 'privileged role constituents overlap')
        used.update(keys)


def artifact(cfg, kind):
    path = pathlib.Path(cfg['artifacts'][kind])
    require(hashlib.sha256(path.read_bytes()).hexdigest() == HASHES[kind], 'artifact hash drift')
    return path


def code(lcd, cfg, kind):
    info = lcd.code(cfg['code_ids'][kind])
    require(info['data_hash'].lower() == HASHES[kind], 'stored code checksum drift')
    return info


def controller(lcd, cfg):
    info = lcd.contract(cfg['controller'])
    require(int(info['code_id']) == cfg['code_ids']['controller'], 'controller code ID drift')
    require(info.get('admin') == cfg['controller_admin'], 'controller admin drift')
    code(lcd, cfg, 'controller')
    current = lcd.smart(cfg['controller'], {'config': {}})
    require(current['interface_version'] == 1 and current['roles'] == cfg['roles'], 'controller roles/version drift')
    require(current['paused'] is True, 'controller must remain paused throughout staged rollout')
    require(current['pending_role'] is None and current['pending_delay'] is None, 'pending authority change')
    require(current['rotation_delay'] == cfg['rotation_delay'], 'controller rotation delay drift')
    return current


def preserve(before, after):
    for key, value in before.items():
        if bytes.fromhex(key) != b'contract_info':
            require(after.get(key) == value, 'legacy raw state changed: ' + key)
    for key in after.keys() - before.keys():
        decoded = bytes.fromhex(key)
        # cw-storage-plus Map prefixes begin with a 2-byte namespace length.
        require(decoded.startswith(b'security_v1_') or decoded[2:].startswith(b'security_v1_'),
                'unexpected new storage namespace')
    cw2 = json.loads(base64.b64decode(after[b'contract_info'.hex()]))
    require(cw2 == {'contract': 'crates.io:cw20-base', 'version': '0.2.0'}, 'unexpected migrated cw2 version')


def token(lcd, cfg, migrated):
    item = cfg['token']
    addr = address(item['address'])
    info = lcd.contract(addr)
    require(info.get('admin') == address(item['admin']), 'token admin drift')
    require(int(info['code_id']) == (cfg['code_ids']['token'] if migrated else 10184), 'token code ID drift')
    expected = HASHES['token'] if migrated else LEGACY_HASH
    require(lcd.code(int(info['code_id']))['data_hash'].lower() == expected, 'token code hash drift')
    require(lcd.smart(addr, {'Minter': {}}) == item['minter'], 'primary minter drift')
    # Full minter set is also preserved in raw snapshot; this query checks the configured bounded cohort.
    minters = lcd.smart(addr, {'Minters': {'start_after': None, 'limit': 30}})['minters']
    require(len(minters) < 30 and sorted(minters) == sorted(item['additional_minters']), 'additional minter drift/overflow')
    require(cfg['roles']['governance'] not in minters + [(item['minter'] or {}).get('minter')],
            'resolve governance/minter overlap before migration')
    rows = lcd.state(addr)
    if migrated:
        require(lcd.smart(addr, {'security_config': {}}) == {'controller': cfg['controller']}, 'immutable controller mismatch')
        baseline = load(item['baseline'])
        require(baseline['token'] == addr and baseline['chain_id'] == cfg['chain_id'], 'baseline context mismatch')
        preserve(baseline['state'], rows)
    else:
        cw2 = json.loads(base64.b64decode(rows[b'contract_info'.hex()]))
        require(cw2 == {'contract': 'crates.io:cw20-base', 'version': '0.1.0'}, 'unsupported migration source')
    return rows


def dex(lcd, cfg):
    # Pin actual policy state, not an operator boolean or a waiver marker.
    review = cfg['dex_policy']
    require(re.fullmatch('[0-9a-f]{64}', review['review_sha256']), 'DEX compatibility review digest required')
    require(digest(lcd.smart(review['contract'], review['query'])) == review['response_sha256'],
            'DEX policy query drift; chain upload permission is not DEX code-ID approval')
    require(review['token_code_id'] == cfg['code_ids']['token'], 'DEX review pins different token code ID')
    response = lcd.smart(review['contract'], review['query'])
    require(review['approval_path'], 'DEX approval field path unresolved')
    for field in review['approval_path']:
        response = response[field]
    require(isinstance(response, list) and cfg['code_ids']['token'] in response,
            'actual DEX policy does not list approved token code ID')


def preflight(cfg, stage, lcd):
    require(re.fullmatch(r'[1-9][0-9]*uluna', cfg['fees']), 'explicit finite uluna fee required')
    require(0 < cfg['gas'] <= 20_000_000, 'explicit bounded gas required')
    require(172800 <= cfg['rotation_delay'] <= 31536000, 'invalid controller rotation delay')
    header = lcd.pin(cfg['chain_id'])
    for kind in HASHES:
        artifact(cfg, kind)
    identities(lcd, cfg)
    params = lcd.get('/cosmwasm/wasm/v1/codes/params')['params']
    # Known configuration required even when pre-stored code avoids upload permission.
    require(isinstance(params.get('code_upload_access'), dict), 'unknown chain upload configuration')
    known = ('Everybody', 'ACCESS_TYPE_EVERYBODY', 'AnyOfAddresses', 'ACCESS_TYPE_ANY_OF_ADDRESSES',
             'Nobody', 'ACCESS_TYPE_NOBODY', 'OnlyAddress', 'ACCESS_TYPE_ONLY_ADDRESS', 1, 2, 3, 4, '1', '2', '3', '4')
    require(params['code_upload_access'].get('permission') in known and
            params.get('instantiate_default_permission') in known, 'unknown chain permission configuration')
    result = {'header': header, 'params': params}
    gov = cfg['roles']['governance']
    if stage.startswith('store-'):
        sender = address(cfg['deployer'])
        require(account_key(lcd, sender) == cfg['identity_keys'].get(sender), 'deployer identity drift')
        access(params['code_upload_access'], sender)
        kind = stage.removeprefix('store-')
        require(cfg['code_ids'][kind] is None, 'code ID already configured; verify existing code instead')
        return result, sender, ['store', str(artifact(cfg, kind)), '--instantiate-anyof-addresses', sender], None
    code(lcd, cfg, 'controller')
    if stage == 'instantiate':
        sender = address(cfg['deployer'])
        require(account_key(lcd, sender) == cfg['identity_keys'].get(sender), 'deployer identity drift')
        access(code(lcd, cfg, 'controller')['instantiate_permission'], sender)
        require(cfg['controller'] is None, 'controller already configured')
        require(cfg['controller_admin'] == gov, 'controller chain admin must be verified governance')
        msg = {'roles': cfg['roles'], 'rotation_delay': cfg['rotation_delay']}
        return result, sender, ['instantiate', str(cfg['code_ids']['controller']), canonical(msg).decode(),
                                '--label', 'cw20-security-v1', '--admin', gov], None
    current = controller(lcd, cfg)
    code(lcd, cfg, 'token')
    dex(lcd, cfg)
    rows = token(lcd, cfg, stage != 'migrate')
    addr = cfg['token']['address']
    result['token_state_sha256'] = digest(rows)
    if stage == 'migrate':
        sender = cfg['token']['admin']
        require(account_key(lcd, sender) == cfg['identity_keys'].get(sender), 'admin identity drift or contract forwarding unsupported')
        msg = {'security': {'controller': cfg['controller'], 'governance': gov}}
        return result, sender, ['migrate', addr, str(cfg['code_ids']['token']), canonical(msg).decode()], rows
    guard = lcd.smart(cfg['controller'], {'guard': {'token': addr, 'addresses': []}})
    require(guard['roles'] == cfg['roles'] and guard['paused'] is True, 'guard mismatch')
    if stage == 'enroll':
        require(guard['enrolled'] is False, 'already enrolled')
        target, msg = cfg['controller'], {'set_enrollment': {'token': addr, 'enrolled': True}}
    else:
        require(guard['enrolled'] is True, 'token must be enrolled')
        target = addr
        assignments = cfg['token']['assignments']
        require(assignments, 'integration inventory/assignments unresolved')
        wallets = [(a, lcd.smart(addr, {'wallet_security': {'address': address(a['address'])}})) for a in assignments]
        if stage in ('assign', 'activate'):
            candidates = [(a, w) for a, w in wallets if w['assignment'] != a['assignment']]
            require(candidates, 'all reviewed assignments already active')
            a, wallet = candidates[0]
            desired = a['assignment']
            require(desired['protocol'] is True and desired['exempt'] is True and
                    re.fullmatch('[0-9a-f]{64}', desired['evidence_hash']), 'explicit reviewed protocol exemption required')
            pending = wallet['pending_assignment']
            if stage == 'assign':
                require(pending is None, 'pending exists; activate after real deadline, do not replace')
                msg = {'set_assignment': {'address': a['address'], 'assignment': desired}}
            else:
                require(pending is not None and pending['assignment'] == desired, 'pending payload mismatch')
                require(pending['authority_epoch'] == current['authority_epoch'], 'pending authority epoch drift')
                require(pending['initiator'] == gov, 'pending assignment initiator mismatch')
                require(pending['executable_at'] >= pending['proposed_at'] + max(86400, wallet['state']['policy']['relaxation_delay']),
                        'pending assignment delay below old wallet delay')
                require(lcd.now >= pending['executable_at'], 'wait for actual on-chain assignment deadline')
                msg = {'execute_assignment': {'address': a['address'], 'nonce': pending['nonce']}}
        else:
            require(stage == 'finalize', 'unknown stage')
            cohort = cfg['cohort']
            expected = {item['address'] for item in cohort}
            require(0 < len(expected) == len(cohort) <= 35 and addr in expected, 'exact bounded finalization cohort required')
            prefix = b'\x00\x08enrolled'
            enrolled = {bytes.fromhex(k)[len(prefix):].decode() for k, v in lcd.state(cfg['controller']).items()
                        if bytes.fromhex(k).startswith(prefix) and json.loads(base64.b64decode(v)) is True}
            require(enrolled == expected, 'shared controller enrollment differs from finalization cohort')
            for item in cohort:
                member = dict(cfg, token=item)
                token(lcd, member, True)
                require(item['assignments'], 'cohort integration inventory unresolved')
                for assignment in item['assignments']:
                    wallet = lcd.smart(item['address'], {'wallet_security': {'address': assignment['address']}})
                    require(wallet['assignment'] == assignment['assignment'] and wallet['pending_assignment'] is None,
                            'effective protocol assignment not ready across entire cohort')
                    require(not wallet['state']['locked'], 'protocol wallet is locked')
                    guard = lcd.smart(cfg['controller'], {'guard': {'token': item['address'], 'addresses': [assignment['address']]}})
                    require(guard['enrolled'] is True and guard['blocked'] == [False], 'protocol address is blacklisted/unenrolled')
            target, msg = cfg['controller'], {'unpause': {}}
    return result, gov, ['execute', target, canonical(msg).decode()], None


def run(command):
    result = subprocess.run(command, check=True, capture_output=True, text=True, timeout=60)
    return json.loads(result.stdout)


def generate(cfg, args, sender, height, account):
    require(re.fullmatch(r'[1-9][0-9]*uluna', cfg['fees']), 'explicit finite uluna fee required')
    require(0 < cfg['gas'] <= 20_000_000, 'explicit bounded gas required')
    return run(['terrad', 'tx', 'wasm'] + args + ['--generate-only', '--offline', '--from', sender,
               '--account-number', str(int(account['account_number'])), '--sequence', str(int(account['sequence'])),
               '--fees', cfg['fees'], '--gas', str(cfg['gas']),
               '--timeout-height', str(height + 20), '--output', 'json'])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', nargs='?', default='plan', choices=('plan', 'prepare', 'broadcast', 'receipt'))
    parser.add_argument('--config')
    parser.add_argument('--stage', choices=STAGES)
    parser.add_argument('--packet', type=pathlib.Path)
    parser.add_argument('--signed', type=pathlib.Path)
    parser.add_argument('--txhash')
    parser.add_argument('--execute', action='store_true')
    opts = parser.parse_args(argv)
    if opts.command == 'plan':
        print(json.dumps({'mode': 'offline; no chain queries, signing or transactions', 'stages': STAGES,
                          'blockers': ['Terra Classic deployer/code-upload whitelist', 'verified role identities and quorum custody',
                                       'DEX code-ID policy compatibility', 'admin/issuer separation', 'LP forwarding routes'],
                          'coverage': '0/35; no deployment receipts'}, indent=2))
        return
    require(opts.config and opts.packet, '--config and --packet required')
    cfg = load(opts.config)
    lcd = LCD(cfg['lcd'])
    if opts.command == 'receipt':
        require(opts.txhash and re.fullmatch('[A-Fa-f0-9]{64}', opts.txhash), 'exact transaction hash required')
        packet = load(opts.packet / 'packet.json')
        response = lcd.get('/cosmos/tx/v1beta1/txs/' + opts.txhash)
        tx = response['tx_response']
        require(int(tx['code']) == 0 and int(tx['height']) > 0, 'transaction failed/uncommitted')
        require(response['tx']['body'] == packet['unsigned']['body'], 'receipt transaction body mismatch')
        require(cfg['chain_id'] == packet['chain_id'], 'receipt chain mismatch')
        require(packet['config_sha256'] == digest(cfg), 'receipt configuration drift; use original prepared config')
        lcd.pin(cfg['chain_id'])
        if packet['stage'] == 'migrate':
            controller(lcd, cfg)
            token(lcd, cfg, True)
        if packet['stage'] == 'finalize':
            current = lcd.smart(cfg['controller'], {'config': {}})
            require(current['paused'] is False and current['roles'] == cfg['roles'], 'unpause postcondition failed')
        write(opts.packet / ('receipt-' + opts.txhash.upper() + '.json'), response)
        print('Committed receipt saved; re-run next stage to verify current postconditions. No batch continuation.')
        return
    stage = opts.stage
    if opts.command == 'broadcast':
        require(opts.execute and opts.signed, 'broadcast requires explicit --execute and externally signed JSON')
        require(not (opts.packet / 'submission.json').exists() and not (opts.packet / 'broadcast.json').exists(), 'submission already recorded; reconcile committed receipt before proceeding')
        packet = load(opts.packet / 'packet.json')
        require(packet['config_sha256'] == digest(cfg), 'configuration drift; prepare new packet')
        stage = packet['stage']
    require(stage is not None, '--stage required')
    facts, sender, action, snapshot = preflight(cfg, stage, lcd)
    if opts.command == 'prepare':
        account = lcd.get('/cosmos/auth/v1beta1/accounts/' + sender)['account']
        unsigned = generate(cfg, action, sender, lcd.height, account)
        opts.packet.mkdir(mode=0o700, parents=True, exist_ok=False)
        if snapshot is not None:
            require(pathlib.Path(cfg['token']['baseline']).resolve() == (opts.packet / 'baseline.json').resolve(),
                    'configure token.baseline to this migration packet/baseline.json before preparation')
            write(opts.packet / 'baseline.json', {'chain_id': cfg['chain_id'], 'token': cfg['token']['address'],
                                                 'height': lcd.height, 'state': snapshot})
        write(opts.packet / 'packet.json', {'stage': stage, 'chain_id': cfg['chain_id'], 'config_sha256': digest(cfg),
                                           'sender': sender, 'account_number': account['account_number'], 'sequence': account['sequence'],
                                           'action': action, 'facts': facts, 'unsigned': unsigned})
        write(opts.packet / 'unsigned.json', unsigned)
        print('Prepared one unsigned action. No key access/broadcast. External authorized chain signer/quorum required.')
    else:
        require(packet['sender'] == sender and packet['action'] == action, 'action/authority drift')
        require(packet['facts'].get('token_state_sha256') == facts.get('token_state_sha256'), 'token state drift; prepare new packet')
        signed = load(opts.signed)
        unsigned = packet['unsigned']
        require(signed['body'] == unsigned['body'], 'signed transaction body differs from prepared action')
        require(signed['auth_info']['fee'] == unsigned['auth_info']['fee'], 'signed fee/gas drift')
        fee = signed['auth_info']['fee']
        require(fee['amount'] == [{'denom': 'uluna', 'amount': cfg['fees'][:-5]}] and
                int(fee['gas_limit']) == cfg['gas'] and not fee.get('payer') and not fee.get('granter'),
                'signed fee exceeds configured budget/context')
        require(signed['auth_info'].get('tip') == unsigned['auth_info'].get('tip') is None, 'unexpected tip')
        require(len(signed.get('signatures', [])) == 1 and all(signed['signatures']), 'missing actual chain signatures')
        signers = signed['auth_info']['signer_infos']
        account = lcd.get('/cosmos/auth/v1beta1/accounts/' + sender)['account']
        require(len(signers) == 1 and signers[0]['public_key'] == account['pub_key'] and
                int(signers[0]['sequence']) == int(account['sequence']), 'signed chain key/sequence mismatch')
        latest = LCD(cfg['lcd'])
        latest.pin(cfg['chain_id'])
        require(0 <= latest.height - lcd.height <= 2, 'preflight exceeded two blocks; refresh bounded snapshot before execution')
        require(latest.height <= int(signed['body']['timeout_height']), 'expired packet; prepare again')
        # Check the actual broadcast RPC against the pinned LCD block.
        require(cfg['rpc'].startswith('https://'), 'broadcast RPC must use HTTPS')
        with urllib.request.urlopen(cfg['rpc'].rstrip('/') + '/block?height=' + str(lcd.height), timeout=30) as response:
            rpc_header = json.load(response)['result']['block']['header']
        require(rpc_header == facts['header'], 'broadcast RPC/LCD chain context mismatch')
        # Persist intent before submission: ambiguous timeouts/crashes require receipt reconciliation.
        write(opts.packet / 'submission.json', {'signed_json_sha256': digest(signed), 'height': lcd.height, 'stage': stage})
        # Broadcast exactly the validated bytes via stdin, avoiding a path replacement race.
        result = subprocess.run(['terrad', 'tx', 'broadcast', '-', '--node', cfg['rpc'],
                                 '--chain-id', cfg['chain_id'], '--output', 'json'],
                                input=json.dumps(signed), capture_output=True, text=True, check=True, timeout=60)
        response = json.loads(result.stdout)
        write(opts.packet / 'broadcast.json', response)
        require(int(response['code']) == 0, 'broadcast rejected; inspect receipt; never rollback to legacy')
        print('Submitted one action: ' + response['txhash'] + '. Confirm committed receipt before next stage.')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, TypeError, AttributeError, OSError, subprocess.SubprocessError) as error:
        print('STOP: ' + str(error), file=sys.stderr)
        sys.exit(1)
