"""Opt-in real codesign test. Only copies the supplied, already-signed fixture.

VOICE_INPUT_SIGNED_FIXTURE="/absolute/Voice Input.app" npm run test:signing
The private key is used through codesign; it is never exported or inspected.
"""
import importlib.util
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'macos_signing.py'


@unittest.skipUnless(os.environ.get('VOICE_INPUT_SIGNED_FIXTURE'), 'Requires an existing local signing identity and app fixture')
class RealSigning(unittest.TestCase):
    def test_upgrade_changes_code_hash_but_preserves_identity_and_rejects_tampering(self):
        spec = importlib.util.spec_from_file_location('signing', SCRIPT)
        signing = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(signing)
        state = signing.ensure_identity(signing.DEFAULT_STATE)
        source = Path(os.environ['VOICE_INPUT_SIGNED_FIXTURE'])
        signing.verify(source, state)

        def cdhash(app):
            result = subprocess.run(['/usr/bin/codesign', '-dvvv', str(app)], capture_output=True, check=True)
            return re.search(r'^CDHash=(\w+)', result.stderr.decode(), re.M).group(1)

        with tempfile.TemporaryDirectory(prefix='voice-input-signing-test-') as temporary:
            updated = Path(temporary) / 'Voice Input.app'
            shutil.copytree(source, updated)
            plist = updated / 'Contents/Info.plist'
            with plist.open('rb') as file:
                info = plistlib.load(file)
            info['CFBundleVersion'] = '999.0.1'
            with plist.open('wb') as file:
                plistlib.dump(info, file)
            with self.assertRaises(signing.SigningError):
                signing.verify(updated, state)
            signing.sign(updated, state)
            signing.verify(updated, state)
            self.assertNotEqual(cdhash(source), cdhash(updated))
            self.assertEqual(signing.bundle_info(updated)['CFBundleIdentifier'], 'com.local.voiceinput.personal')


if __name__ == '__main__':
    unittest.main()
