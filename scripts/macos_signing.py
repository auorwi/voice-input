#!/usr/bin/env python3
"""Fixed, certificate-pinned signing for Voice Input on its owner's Mac.

No trust-store changes, TCC edits, exported existing keys, or ad-hoc fallback.
Only setup creates a new key. Other commands require the pinned identity.
"""
import argparse
import contextlib
import datetime
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
import uuid

APP_ID = 'com.local.voiceinput.personal'
IDENTITY_NAME = 'Voice Input Local Signing'
REPO = Path(__file__).resolve().parents[1]
DEFAULT_STATE = Path.home() / 'Library/Application Support/com.local.voiceinput.personal-signing'
REGISTER = '/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister'


class SigningError(Exception):
    pass


def run(args, *, data=None, sensitive=False, **kwargs):
    result = subprocess.run(args, input=data, capture_output=True, **kwargs)
    if result.returncode:
        detail = '' if sensitive else ': ' + result.stderr.decode(errors='replace')[-1200:].strip()
        raise SigningError(f'{Path(args[0]).name} failed ({result.returncode}){detail}')
    return result.stdout


def requirement(sha1):
    if not re.fullmatch(r'[0-9a-fA-F]{40}', sha1):
        raise SigningError('Invalid signing certificate fingerprint')
    return f'identifier "{APP_ID}" and certificate leaf = H"{sha1.lower()}"'


def check_requirement(text, sha1):
    # Exact expression: accepting bundle-ID-only or OR expressions lets a different
    # key impersonate the app. A cdhash expression instead breaks every upgrade.
    lines = [line.strip() for line in text.splitlines() if line.startswith('designated => ')]
    if lines != ['designated => ' + requirement(sha1)]:
        raise SigningError('App does not have the fixed certificate-pinned designated requirement')


def read_identity(state_dir, filename='identity.json'):
    state_path = state_dir / filename
    if not state_path.is_file():
        raise SigningError('No fixed signing identity. Run npm run signing:setup once on this Mac.')
    state = json.loads(state_path.read_text())
    if state.get('schema') != 1 or state.get('name') != IDENTITY_NAME:
        raise SigningError('Unsupported signing state; refusing to replace or regenerate it')
    certificate = (state_dir / 'certificate.der').read_bytes()
    if (hashlib.sha1(certificate).hexdigest() != state.get('sha1')
            or hashlib.sha256(certificate).hexdigest() != state.get('sha256')):
        raise SigningError('Certificate fingerprint changed; refusing to build or install')
    requirement(state['sha1'])
    return state


def ensure_identity(state_dir, filename='identity.json'):
    state = read_identity(state_dir, filename)
    identities = run(['/usr/bin/security', 'find-identity', '-p', 'codesigning', state['keychain']]).decode()
    if not re.search(r'\b' + re.escape(state['sha1']) + r'\b', identities, re.I):
        raise SigningError('Pinned signing private key is missing. Restore the original identity; do not generate a replacement.')
    run(['/usr/bin/openssl', 'x509', '-inform', 'DER', '-in', str(state_dir / 'certificate.der'),
         '-checkend', '86400', '-noout'])
    return state


@contextlib.contextmanager
def password_fd(password):
    reader, writer = os.pipe()
    try:
        os.write(writer, password + b'\n')
        os.close(writer)
        writer = None
        yield reader
    finally:
        os.close(reader)
        if writer is not None:
            os.close(writer)


def setup(state_dir):
    state_path = state_dir / 'identity.json'
    pending_path = state_dir / 'identity.pending.json'
    if state_path.exists():
        state = ensure_identity(state_dir)
        print('Reusing the existing signing identity:', state['sha256'])
        return
    if pending_path.exists():
        # Resume an interrupted import only when the pinned public certificate and
        # its private key are both present. Never silently create another identity.
        state = ensure_identity(state_dir, pending_path.name)
        pending_path.replace(state_path)
        print('Recovered the existing signing identity:', state['sha256'])
        return
    keychain = run(['/usr/bin/security', 'default-keychain', '-d', 'user']).decode().strip().strip('"')
    if not keychain or not Path(keychain).is_file():
        raise SigningError('No existing user keychain is available')
    existing = subprocess.run(['/usr/bin/security', 'find-certificate', '-c', IDENTITY_NAME, keychain],
                              capture_output=True)
    if existing.returncode == 0:
        raise SigningError('A signing certificate already exists. Restore its identity.json; refusing to create a second identity.')
    if existing.returncode not in (44,):  # errSecItemNotFound; never interpret access errors as absence.
        raise SigningError('Could not check for an existing certificate; unlock the user keychain first')
    state_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(state_dir, 0o700)
    password = secrets.token_urlsafe(48).encode()
    # Plaintext private-key material never reaches a file. The short-lived PKCS#12
    # archive is encrypted, mode 0600, and deleted immediately after import.
    with tempfile.TemporaryDirectory(prefix='voice-input-signing-') as temporary:
        temp = Path(temporary)
        configuration = temp / 'certificate.cnf'
        configuration.write_text('''[req]
prompt = no
distinguished_name = subject
x509_extensions = signing
[subject]
CN = Voice Input Local Signing
[signing]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
subjectKeyIdentifier = hash
''')
        pem = run(['/usr/bin/openssl', 'req', '-new', '-x509', '-newkey', 'rsa:3072',
                   '-sha256', '-days', '3650', '-config', str(configuration),
                   '-passout', 'stdin', '-keyout', '/dev/stdout', '-out', '/dev/stdout'],
                  data=password + b'\n', sensitive=True)
        certificate = run(['/usr/bin/openssl', 'x509', '-outform', 'DER'], data=pem, sensitive=True)
        with password_fd(password) as input_fd, password_fd(password) as output_fd:
            archive = run(['/usr/bin/openssl', 'pkcs12', '-export', '-name', IDENTITY_NAME,
                           '-passin', f'fd:{input_fd}', '-passout', f'fd:{output_fd}'],
                          data=pem, pass_fds=(input_fd, output_fd), sensitive=True)
        del pem
        archive_path = temp / 'identity.p12'
        descriptor = os.open(archive_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, 'wb') as output:
            output.write(archive)
        del archive
        state = {'schema': 1, 'name': IDENTITY_NAME, 'keychain': keychain,
                 'sha1': hashlib.sha1(certificate).hexdigest(),
                 'sha256': hashlib.sha256(certificate).hexdigest(),
                 'created_at': datetime.datetime.now(datetime.timezone.utc).isoformat()}
        (state_dir / 'certificate.der').write_bytes(certificate)
        pending_path.write_text(json.dumps(state, indent=2) + '\n')
        os.chmod(pending_path, 0o600)
        # This is a newly generated archive password, never the user's login password.
        # Only codesign receives unattended access to this new, non-extractable key.
        run(['/usr/bin/security', 'import', str(archive_path), '-k', keychain, '-f', 'pkcs12',
             '-P', password.decode(), '-x', '-T', '/usr/bin/codesign'], sensitive=True)
    del password
    ensure_identity(state_dir, pending_path.name)
    pending_path.replace(state_path)
    print('Created a fixed local signing identity. Public SHA-256:', state['sha256'])
    print('Private key stays in the user keychain, non-extractable; no trust settings were changed.')


def bundle_info(app):
    if app.is_symlink() or not app.is_dir():
        raise SigningError('Expected a real Voice Input app directory, not a symlink')
    with (app / 'Contents/Info.plist').open('rb') as file:
        info = plistlib.load(file)
    if info.get('CFBundleIdentifier') != APP_ID or info.get('CFBundleExecutable') != 'opentypeless':
        raise SigningError('Refusing to operate on a different application')
    return info


def verify(app, state):
    info = bundle_info(app)
    run(['/usr/bin/codesign', '--verify', '--deep', '--strict', '-R=' + requirement(state['sha1']), str(app)])
    description = subprocess.run(['/usr/bin/codesign', '-d', '-r-', str(app)], capture_output=True)
    if description.returncode:
        raise SigningError('Could not read app signing requirements')
    check_requirement((description.stdout + description.stderr).decode(), state['sha1'])
    # A real certificate signature must also match the pinned public certificate.
    with tempfile.TemporaryDirectory(prefix='voice-input-cert-check-') as temporary:
        prefix = str(Path(temporary) / 'cert')
        run(['/usr/bin/codesign', '-d', '--extract-certificates=' + prefix, str(app)])
        digest = hashlib.sha256(Path(prefix + '0').read_bytes()).hexdigest()
        if digest != state['sha256']:
            raise SigningError('App signer changed; refusing installation')
    return info


def sign(app, state):
    bundle_info(app)
    # This personal app currently has a single executable and no nested code.
    # Fail closed if that packaging contract changes; do not blindly --deep sign.
    executable = app / 'Contents/MacOS/opentypeless'
    for entry in (app / 'Contents').rglob('*'):
        if entry.is_file() and entry != executable:
            with entry.open('rb') as file:
                magic = file.read(4)
            if magic in (b'\xcf\xfa\xed\xfe', b'\xce\xfa\xed\xfe', b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca'):
                raise SigningError('Nested code was found; add explicit inside-out signing before releasing')
    run(['/usr/bin/codesign', '--force', '--sign', state['sha1'], '--keychain', state['keychain'],
         '--options', 'runtime', '--timestamp=none', '--entitlements',
         str(REPO / 'src-tauri/Entitlements.plist'), '--generate-entitlement-der',
         '-r=' + 'designated => ' + requirement(state['sha1']), str(app)])
    verify(app, state)
    print('Signed and verified:', app)


def build(state_dir, offline):
    state = ensure_identity(state_dir)  # Must run before npm/cargo; no fallback identity.
    target = Path(os.environ.get('CARGO_TARGET_DIR', str(REPO / 'src-tauri/target')))
    if not target.is_absolute():
        raise SigningError('CARGO_TARGET_DIR must be absolute for the guarded release build')
    command = ['npm', 'run', 'tauri', '--', 'build', '--bundles', 'app', '--ci', '--no-sign', '--', '--locked']
    if offline:
        command.append('--offline')
    subprocess.run(command, cwd=REPO, check=True)
    app = target / 'release/bundle/macos/Voice Input.app'
    sign(app, state)
    print('Build ready. Install with signing:install; do not replace the running app manually.')


def install(source, destination, state_dir, migrate):
    state = ensure_identity(state_dir)
    verify(source, state)
    if destination.suffix != '.app' or source.resolve() == destination.resolve():
        raise SigningError('Source and destination must be distinct app paths')
    if destination.is_symlink():
        raise SigningError('Destination must not be a symlink')
    if destination.exists():
        bundle_info(destination)
        current = subprocess.run(['/usr/bin/pgrep', '-f', '^' + re.escape(str(destination / 'Contents/MacOS/opentypeless')) + '( |$)'], capture_output=True)
        if current.returncode == 0:
            raise SigningError('Quit Voice Input before installing; the running app has not been changed')
        if current.returncode != 1:
            raise SigningError('Could not determine whether Voice Input is running')
        try:
            verify(destination, state)
        except SigningError:
            if not migrate:
                raise SigningError('Installed identity differs. Use --migrate-ad-hoc only for the one-time migration.')
            description = subprocess.run(['/usr/bin/codesign', '-dvv', str(destination)], capture_output=True)
            if description.returncode or 'Signature=adhoc' not in description.stderr.decode():
                raise SigningError('Migration accepts only the existing ad-hoc personal app; refusing a different signer')
            run(['/usr/bin/codesign', '--verify', '--deep', '--strict', str(destination)])
        backups = state_dir / 'backups'
        backups.mkdir(exist_ok=True, mode=0o700)
        backup = backups / (datetime.datetime.now().strftime('%Y%m%d-%H%M%S') + '-' + uuid.uuid4().hex[:8] + '.zip')
        run(['/usr/bin/ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', str(destination), str(backup)])
    destination.parent.mkdir(parents=True, exist_ok=True)
    stage = destination.parent / ('.voice-input-update-' + uuid.uuid4().hex)
    previous = destination.parent / ('.voice-input-previous-' + uuid.uuid4().hex)
    try:
        run(['/usr/bin/ditto', str(source), str(stage)])
        verify(stage, state)
        if destination.exists():
            run([REGISTER, '-u', str(destination)])
            destination.rename(previous)
        try:
            stage.rename(destination)
            verify(destination, state)
            run([REGISTER, '-f', str(destination)])
        except Exception:
            if destination.exists():
                shutil.rmtree(destination)
            if previous.exists():
                previous.rename(destination)
                run([REGISTER, '-f', str(destination)])
            raise
        if previous.exists():
            shutil.rmtree(previous)
    finally:
        if stage.exists():
            shutil.rmtree(stage)
    # Registration cleanup is noncritical and only touches the verified source.
    subprocess.run([REGISTER, '-u', str(source)], capture_output=True)
    print('Installed the verified fixed-identity app:', destination)
    print('No TCC permissions or Keychain item access lists were modified by this installer.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state-dir', type=Path, default=DEFAULT_STATE)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('setup')
    commands.add_parser('status')
    build_parser = commands.add_parser('build')
    build_parser.add_argument('--offline', action='store_true')
    for command in ('sign', 'verify'):
        commands.add_parser(command).add_argument('app', type=Path)
    install_parser = commands.add_parser('install')
    install_parser.add_argument('source', type=Path)
    install_parser.add_argument('destination', type=Path)
    install_parser.add_argument('--migrate-ad-hoc', action='store_true')
    args = parser.parse_args()
    try:
        if args.command == 'setup':
            setup(args.state_dir)
        elif args.command == 'build':
            build(args.state_dir, args.offline)
        elif args.command == 'install':
            install(args.source.absolute(), args.destination.absolute(), args.state_dir, args.migrate_ad_hoc)
        else:
            state = ensure_identity(args.state_dir)
            if args.command == 'sign':
                sign(args.app.absolute(), state)
            elif args.command == 'verify':
                verify(args.app.absolute(), state)
                print('Fixed signing identity verified:', state['sha256'])
            else:
                print(json.dumps({k: state[k] for k in ('name', 'sha1', 'sha256')}, indent=2))
    except (SigningError, OSError, ValueError, subprocess.CalledProcessError) as error:
        # No secret material is ever included in SigningError messages.
        print(str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
