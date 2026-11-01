import base64
import contextlib
import copy
import importlib.util
import io
import json
import pathlib
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('rollout', pathlib.Path(__file__).parents[1] / 'rollout-migrate.py')
m = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(m)


def addr(char):
    return 'terra1' + char * 38


def key(value):
    return {'@type': '/cosmos.crypto.secp256k1.PubKey', 'key': value}


def cfg():
    roles = dict(zip(('governance', 'blacklist', 'recovery', 'dispute', 'responder'), map(addr, 'acdef')))
    keys = {a: {'@type': '/cosmos.crypto.multisig.LegacyAminoPubKey', 'threshold': '2',
                'public_keys': [key(a + '1'), key(a + '2')]} for a in roles.values()}
    keys[addr('g')] = key('admin')
    return {'chain_id': 'columbus-5', 'roles': roles, 'identity_keys': keys, 'deployer': addr('g'),
            'code_ids': {'token': 200, 'controller': 201}, 'controller': addr('h'),
            'controller_admin': roles['governance'], 'rotation_delay': 172800,
            'fees': '500000uluna', 'gas': 5000000,
            'token': {'address': addr('j'), 'admin': addr('g'), 'minter': {'minter': addr('k'), 'cap': None},
                      'additional_minters': [], 'assignments': [{'address': addr('k'), 'assignment':
                      {'protocol': True, 'exempt': True, 'allow_lock': False, 'evidence_hash': 'a' * 64}}]}}


class Fake:
    height = 123
    now = 86405

    def __init__(self, conf):
        self.cfg = conf
        self.permission = {'permission': 'Everybody'}
        self.pending = {'nonce': 7, 'proposed_at': 5, 'executable_at': 86405,
                        'authority_epoch': 1, 'initiator': conf['roles']['governance'],
                        'assignment': conf['token']['assignments'][0]['assignment']}
        self.contract_admin = conf['token']['admin']
        self.stored_hash = m.HASHES['token']

    def pin(self, chain):
        return {'height': '123', 'chain_id': chain}

    def get(self, path):
        if path.endswith('/codes/params'):
            return {'params': {'code_upload_access': self.permission, 'instantiate_default_permission': 'Everybody'}}
        a = path.rsplit('/', 1)[-1]
        return {'account': {'@type': '/cosmos.auth.v1beta1.BaseAccount', 'address': a,
                            'pub_key': self.cfg['identity_keys'][a]}}

    def code(self, code_id):
        return {'data_hash': self.stored_hash, 'instantiate_permission': self.permission}

    def contract(self, a):
        return {'admin': self.contract_admin, 'code_id': '10184'}

    def state(self, a):
        return {b'contract_info'.hex(): base64.b64encode(m.canonical(
            {'contract': 'crates.io:cw20-base', 'version': '0.1.0'})).decode()}

    def smart(self, a, msg):
        if 'guard' in msg:
            return {'enrolled': True, 'paused': True, 'roles': self.cfg['roles']}
        if 'wallet_security' in msg:
            return {'assignment': {}, 'pending_assignment': self.pending,
                    'state': {'policy': {'relaxation_delay': 86400}, 'locked': False}}
        if 'Minter' in msg:
            return self.cfg['token']['minter']
        if 'Minters' in msg:
            return {'minters': self.cfg['token']['additional_minters']}
        raise AssertionError(msg)


class Rollout(unittest.TestCase):
    def test_default_is_offline_and_no_process(self):
        with patch.object(m.urllib.request, 'urlopen', side_effect=AssertionError('network')), \
             patch.object(m.subprocess, 'run', side_effect=AssertionError('process')), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            m.main([])
        self.assertIn('0/35', output.getvalue())

    def test_unknown_or_blocked_whitelist_fails(self):
        for permission in ('Nobody', 'Unspecified', 'new-format', None):
            with self.subTest(permission=permission), self.assertRaises(ValueError):
                m.access({'permission': permission}, addr('a'))
        with self.assertRaises(ValueError):
            m.access({'permission': 'AnyOfAddresses', 'addresses': [addr('c')]}, addr('a'))
        m.access({'permission': 'AnyOfAddresses', 'addresses': [addr('a')]}, addr('a'))

    def test_real_artifact_hash_drift(self):
        with tempfile.TemporaryDirectory() as d:
            file = pathlib.Path(d) / 'token.wasm'
            file.write_bytes(b'wrong')
            with self.assertRaisesRegex(ValueError, 'artifact hash drift'):
                m.artifact({'artifacts': {'token': str(file)}}, 'token')

    def test_identity_drift_and_constituent_overlap(self):
        c = cfg()
        f = Fake(c)
        m.identities(f, c)
        with patch.object(m, 'account_key', return_value=key('unexpected')):
            with self.assertRaisesRegex(ValueError, 'identity drift'):
                m.identities(f, c)
        c['identity_keys'][c['roles']['dispute']]['public_keys'][0] = copy.deepcopy(
            c['identity_keys'][c['roles']['governance']]['public_keys'][0])
        with self.assertRaisesRegex(ValueError, 'constituents overlap'):
            m.identities(f, c)

    def test_contract_admin_is_not_direct_signer(self):
        c = cfg()
        f = Fake(c)
        with patch.object(f, 'get', return_value={'account': {'@type': '/cosmwasm.wasm.v1.ContractInfo'}}):
            with self.assertRaisesRegex(ValueError, 'forwarding route'):
                m.account_key(f, addr('a'))

    def test_admin_drift(self):
        c = cfg()
        f = Fake(c)
        f.contract_admin = addr('p')
        with self.assertRaisesRegex(ValueError, 'admin drift'):
            m.token(f, c, False)

    def test_no_primary_minter_is_supported(self):
        c = cfg()
        c['token']['minter'] = None
        f = Fake(c)
        f.stored_hash = m.LEGACY_HASH
        self.assertTrue(m.token(f, c, False))

    def test_stored_code_drift(self):
        c = cfg()
        f = Fake(c)
        f.stored_hash = '0' * 64
        with self.assertRaisesRegex(ValueError, 'checksum drift'):
            m.code(f, c, 'token')

    def test_governance_minter_overlap(self):
        c = cfg()
        c['token']['minter']['minter'] = c['roles']['governance']
        f = Fake(c)
        f.stored_hash = m.LEGACY_HASH
        with self.assertRaisesRegex(ValueError, 'governance/minter overlap'):
            m.token(f, c, False)

    def test_full_raw_preservation_includes_reverse_allowances(self):
        def b(v):
            return base64.b64encode(v).decode()
        cw2 = b'contract_info'.hex()
        before = {cw2: b(m.canonical({'contract': 'crates.io:cw20-base', 'version': '0.1.0'})),
                  b'\x00\x12allowances_spenderxyz'.hex(): b(b'123'), b'balances-holder'.hex(): b(b'456')}
        after = dict(before)
        after[cw2] = b(m.canonical({'contract': 'crates.io:cw20-base', 'version': '0.2.0'}))
        after[b'security_v1_config'.hex()] = b(b'controller')
        m.preserve(before, after)
        after[b'\x00\x12allowances_spenderxyz'.hex()] = b(b'124')
        with self.assertRaisesRegex(ValueError, 'legacy raw state changed'):
            m.preserve(before, after)

    def test_unknown_added_state_fails(self):
        b = lambda v: base64.b64encode(v).decode()
        state = {b'contract_info'.hex(): b(m.canonical({'contract': 'crates.io:cw20-base', 'version': '0.2.0'})),
                 b'other_namespace'.hex(): b(b'x')}
        with self.assertRaisesRegex(ValueError, 'unexpected new storage'):
            m.preserve({}, state)

    def activate(self, f, c):
        with contextlib.ExitStack() as stack:
            for name in ('artifact', 'identities', 'code', 'dex', 'token'):
                stack.enter_context(patch.object(m, name, return_value={}))
            stack.enter_context(patch.object(m, 'controller', return_value={'authority_epoch': 1}))
            return m.preflight(c, 'activate', f)

    def test_assignment_real_deadline_and_exact_nonce(self):
        c = cfg()
        f = Fake(c)
        f.now = 86404
        with self.assertRaisesRegex(ValueError, 'actual on-chain assignment deadline'):
            self.activate(f, c)
        f.now = 86405
        _, sender, action, _ = self.activate(f, c)
        self.assertEqual(sender, c['roles']['governance'])
        self.assertEqual(json.loads(action[-1])['execute_assignment']['nonce'], 7)

    def test_assignment_epoch_payload_and_no_timer_skip(self):
        for field, value in (('authority_epoch', 2), ('executable_at', 86404), ('initiator', addr('p'))):
            c = cfg()
            f = Fake(c)
            f.pending[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.activate(f, c)

    def test_unsigned_generation_has_no_key_access_or_broadcast(self):
        with patch.object(m, 'run', return_value={}) as run:
            m.generate(cfg(), ['execute', addr('a'), '{}'], addr('g'), 123, {'account_number': '9', 'sequence': '4'})
        args = run.call_args.args[0]
        self.assertIn('--generate-only', args)
        self.assertIn('--offline', args)
        self.assertNotIn('--yes', args)
        self.assertEqual(args[args.index('--from') + 1], addr('g'))
        self.assertEqual(args[args.index('--timeout-height') + 1], '143')

    def test_resume_will_not_rebroadcast_recorded_submission(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d)
            c = cfg()
            c['lcd'] = 'https://example.invalid'
            (p / 'config.json').write_text(json.dumps(c))
            (p / 'broadcast.json').write_text('{}')
            with patch.object(m.subprocess, 'run', side_effect=AssertionError('must not broadcast')):
                with self.assertRaisesRegex(ValueError, 'submission already recorded'):
                    m.main(['broadcast', '--config', str(p / 'config.json'), '--packet', d,
                            '--signed', 'unused', '--execute'])

    def test_broadcast_requires_explicit_optin(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / 'config.json'
            p.write_text('{"lcd":"https://example.invalid"}')
            with patch.object(m.subprocess, 'run', side_effect=AssertionError('must not broadcast')):
                with self.assertRaisesRegex(ValueError, 'explicit --execute'):
                    m.main(['broadcast', '--config', str(p), '--packet', d, '--signed', 'unused'])


    def test_lcd_state_normalizes_actual_uppercase_hex_keys(self):
        lcd = m.LCD('https://example.invalid')
        response = {'models': [{'key': b'contract_info'.hex().upper(), 'value': 'e30='}],
                    'pagination': {'next_key': None}}
        with patch.object(lcd, 'get', return_value=response):
            self.assertEqual(lcd.state(addr('a')), {b'contract_info'.hex(): 'e30='})
        response['models'].append({'key': b'contract_info'.hex(), 'value': 'e30='})
        with patch.object(lcd, 'get', return_value=response), self.assertRaisesRegex(ValueError, 'duplicate'):
            lcd.state(addr('a'))

    def test_signed_body_fee_and_action_drift_prevents_broadcast(self):
        for drift in ('body', 'fee', 'action'):
            with self.subTest(drift=drift), tempfile.TemporaryDirectory() as d:
                p = pathlib.Path(d)
                c = cfg()
                c['lcd'] = 'https://example.invalid'
                signed = {'body': {'timeout_height': '143', 'messages': []},
                          'auth_info': {'fee': {'amount': [{'denom': 'uluna', 'amount': '500000'}],
                                               'gas_limit': '5000000', 'payer': '', 'granter': ''}, 'tip': None},
                          'signatures': ['signature']}
                packet = {'stage': 'enroll', 'config_sha256': m.digest(c), 'sender': addr('g'),
                          'action': ['execute', addr('a'), '{}'], 'facts': {}, 'unsigned': copy.deepcopy(signed)}
                if drift == 'body':
                    signed['body']['messages'] = ['unexpected']
                elif drift == 'fee':
                    signed['auth_info']['fee']['gas_limit'] = '9000000'
                else:
                    packet['action'] = ['execute', addr('c'), '{}']
                for name, value in (('config.json', c), ('signed.json', signed), ('packet.json', packet)):
                    (p / name).write_text(json.dumps(value))
                with patch.object(m, 'preflight', return_value=({}, addr('g'), ['execute', addr('a'), '{}'], None)), \
                     patch.object(m.subprocess, 'run', side_effect=AssertionError('must not broadcast')):
                    with self.assertRaisesRegex(ValueError, 'drift|differs'):
                        m.main(['broadcast', '--config', str(p / 'config.json'), '--packet', d,
                                '--signed', str(p / 'signed.json'), '--execute'])

    def finalize(self, c, f):
        with contextlib.ExitStack() as stack:
            for name in ('artifact', 'identities', 'code', 'dex', 'token'):
                stack.enter_context(patch.object(m, name, return_value={}))
            stack.enter_context(patch.object(m, 'controller', return_value={'authority_epoch': 1}))
            return m.preflight(c, 'finalize', f)

    def test_finalize_enforces_entire_enrolled_cohort_and_pending_state(self):
        c = cfg()
        first = copy.deepcopy(c['token'])
        second = copy.deepcopy(first)
        second['address'] = addr('p')
        c['cohort'] = [first, second]
        f = Fake(c)
        prefix = b'\x00\x08enrolled'
        rows = {(prefix + item['address'].encode()).hex(): base64.b64encode(b'true').decode()
                for item in c['cohort']}
        def smart(a, msg):
            if 'guard' in msg:
                return {'enrolled': True, 'paused': True, 'roles': c['roles'],
                        'blocked': [False] if msg['guard']['addresses'] else []}
            if 'wallet_security' in msg:
                return {'assignment': first['assignments'][0]['assignment'],
                        'pending_assignment': None, 'state': {'locked': False}}
            raise AssertionError(msg)
        with patch.object(f, 'state', return_value=rows), patch.object(f, 'smart', side_effect=smart):
            _, signer, action, _ = self.finalize(c, f)
            self.assertEqual(signer, c['roles']['governance'])
            self.assertEqual(json.loads(action[-1]), {'unpause': {}})
        with patch.object(f, 'state', return_value={}), patch.object(f, 'smart', side_effect=smart):
            with self.assertRaisesRegex(ValueError, 'enrollment differs'):
                self.finalize(c, f)
        def pending(a, msg):
            response = smart(a, msg)
            if a == second['address'] and 'wallet_security' in msg:
                response['pending_assignment'] = {'nonce': 8}
            return response
        with patch.object(f, 'state', return_value=rows), patch.object(f, 'smart', side_effect=pending):
            with self.assertRaisesRegex(ValueError, 'entire cohort'):
                self.finalize(c, f)


if __name__ == '__main__':
    unittest.main()
