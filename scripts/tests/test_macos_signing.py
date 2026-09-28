import importlib.util
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / 'macos_signing.py'


class SigningGuards(unittest.TestCase):
    def test_setup_recovers_imported_identity_from_pending_public_state(self):
        spec = importlib.util.spec_from_file_location('signing', SCRIPT)
        signing = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(signing)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            certificate = b'existing public signing certificate'
            state = {'schema': 1, 'name': signing.IDENTITY_NAME, 'keychain': '/existing/keychain',
                     'sha1': hashlib.sha1(certificate).hexdigest(),
                     'sha256': hashlib.sha256(certificate).hexdigest()}
            (root / 'certificate.der').write_bytes(certificate)
            (root / 'identity.pending.json').write_text(json.dumps(state))
            with patch.object(signing, 'run', return_value=state['sha1'].encode()) as calls:
                signing.setup(root)
            self.assertEqual(json.loads((root / 'identity.json').read_text()), state)
            self.assertFalse((root / 'identity.pending.json').exists())
            self.assertTrue(all(call.args[0][1] in ('find-identity', 'x509') for call in calls.call_args_list))

    def test_registration_failure_restores_the_previous_app(self):
        spec = importlib.util.spec_from_file_location('signing', SCRIPT)
        signing = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(signing)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, destination = root / 'new.app', root / 'installed.app'
            source.mkdir()
            destination.mkdir()
            (source / 'version').write_text('new')
            (destination / 'version').write_text('old')

            def fake_run(args, **kwargs):
                if args[0] == '/usr/bin/ditto' and len(args) == 3:
                    shutil.copytree(args[1], args[2])
                if args[:2] == [signing.REGISTER, '-f'] and (destination / 'version').read_text() == 'new':
                    raise signing.SigningError('Simulated registration failure')
                return b''

            with patch.object(signing, 'ensure_identity', return_value={}), \
                    patch.object(signing, 'verify'), patch.object(signing, 'bundle_info'), \
                    patch.object(signing.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1)), \
                    patch.object(signing, 'run', side_effect=fake_run):
                with self.assertRaisesRegex(signing.SigningError, 'registration failure'):
                    signing.install(source, destination, root, False)
            self.assertEqual((destination / 'version').read_text(), 'old')
            self.assertFalse(list(root.glob('.voice-input-*')))

    def test_build_stops_before_npm_when_identity_is_missing(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                [sys.executable, str(SCRIPT), '--state-dir', directory, 'build'],
                capture_output=True, text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('No fixed signing identity', result.stderr)

    def test_build_rejects_a_replaced_public_certificate(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            (state / 'certificate.der').write_bytes(b'replaced certificate')
            (state / 'identity.json').write_text(json.dumps({
                'schema': 1, 'sha1': 'a' * 40, 'sha256': 'b' * 64,
                'keychain': '/unused/keychain', 'name': 'Voice Input Local Signing',
            }))
            result = subprocess.run(
                [sys.executable, str(SCRIPT), '--state-dir', directory, 'build'],
                capture_output=True, text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Certificate fingerprint changed', result.stderr)

    def test_identity_requirement_rejects_adhoc_and_bundle_only_matches(self):
        spec = importlib.util.spec_from_file_location('signing', SCRIPT)
        signing = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(signing)
        fingerprint = 'a' * 40
        accepted = 'designated => identifier "com.local.voiceinput.personal" and certificate leaf = H"' + fingerprint + '"'
        signing.check_requirement(accepted, fingerprint)
        for requirement in [
            'designated => cdhash H"' + fingerprint + '"',
            'designated => identifier "com.local.voiceinput.personal"',
            accepted.replace(' and ', ' or '),
            accepted.replace('a' * 40, 'b' * 40),
            accepted.replace('personal"', 'another"'),
            accepted + ' or true',
        ]:
            with self.subTest(requirement=requirement):
                with self.assertRaises(signing.SigningError):
                    signing.check_requirement(requirement, fingerprint)


if __name__ == '__main__':
    unittest.main()
