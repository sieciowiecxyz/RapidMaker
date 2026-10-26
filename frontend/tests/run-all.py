#!/usr/bin/env python3
"""Repeatable suites; raw reports stay private and distinguish real integrations from contracts."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import signal
import sys

parser = argparse.ArgumentParser()
parser.add_argument('--group', choices=('all', 'core', 'contracts', 'sandbox', 'email', 'registration'), default='all')
parser.add_argument('--no-build', action='store_true')
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
os.umask(0o077)
artifacts = root / 'frontend/tests/artifacts'
artifacts.mkdir(parents=True, exist_ok=True)
artifacts.chmod(0o700)
if not args.no_build:
    for command in (['cargo', 'build', '--locked', '--manifest-path', 'backend/Cargo.toml'], ['npm', '--prefix', 'frontend', 'run', 'build']):
        subprocess.run(command, cwd=root, check=True)

if not os.environ.get('PLAYWRIGHT_CHROMIUM_EXECUTABLE') and not Path('/usr/bin/chromium').is_file():
    subprocess.run(['npx', 'playwright', 'install', 'chromium'], cwd=root / 'frontend', check=True)

suites = {
    'core': ['--local-accounts', 'models.spec.ts', 'pricing.spec.ts', 'boundaries.spec.ts', 'configurator.spec.ts', 'security.spec.ts', 'legal.spec.ts'],
    'sandbox': ['--sandbox', '--client-totp', 'checkout.spec.ts', 'delivery.spec.ts', 'shipping-checkout.spec.ts', 'logistics.spec.ts', 'panels.spec.ts', 'abandonment.spec.ts'],
    'email': ['--sandbox', 'auth.spec.ts', 'journey.spec.ts'],
    'registration-password': ['--sandbox', '--registration', 'registration.spec.ts'],
    'registration-passkey': ['--sandbox', '--registration', 'registration.spec.ts'],
    'contracts': ['admin-csv.spec.ts', 'auth-factors.spec.ts', 'legal.spec.ts', 'live-panels.spec.ts', 'operations.spec.ts', 'operator.spec.ts'],
}
results = []
selected = list(suites) if args.group == 'all' else ['registration-password', 'registration-passkey'] if args.group == 'registration' else [args.group]
for name in selected:
    report = artifacts / f'{name}.json'
    report.unlink(missing_ok=True)
    env = os.environ.copy()
    env.update(E2E_REPORT_NAME=name, PLAYWRIGHT_JSON_OUTPUT_NAME=str(report), PLAYWRIGHT_NO_COPY_PROMPT='1')
    if name.startswith('registration-'): env['E2E_REGISTRATION_METHOD'] = name.removeprefix('registration-')
    if 'PLAYWRIGHT_CHROMIUM_EXECUTABLE' not in env and Path('/usr/bin/chromium').is_file():
        env['PLAYWRIGHT_CHROMIUM_EXECUTABLE'] = '/usr/bin/chromium'
    if name == 'contracts':
        command = ['npx', 'playwright', 'test', '--reporter=list,json', *suites[name]]
        cwd = root / 'frontend'
    else:
        command = [sys.executable, 'frontend/tests/run-integration.py', *suites[name]]
        cwd = root
    print(f'\nSuite: {name}', flush=True)
    child = subprocess.Popen(command, cwd=cwd, env=env)
    def stop(_signal, _frame):
        child.terminate()
        child.wait(timeout=40)
        raise SystemExit(143)
    previous = signal.signal(signal.SIGTERM, stop)
    try:
        returncode = child.wait()
    finally:
        signal.signal(signal.SIGTERM, previous)
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=40)
    if not report.exists():
        print(f'{name}: report missing; cannot confirm any test', file=sys.stderr)
        returncode = returncode or 1
    stats = json.loads(report.read_text()).get('stats', {}) if report.exists() else {}
    def limits(suites):
        found = []
        for suite in suites:
            for spec in suite.get('specs', []):
                for item in spec.get('tests', []):
                    found.extend(a['description'] for a in item.get('annotations', []) if a.get('type') == 'provider-blocked')
            found.extend(limits(suite.get('suites', [])))
        return found
    external = limits(json.loads(report.read_text()).get('suites', [])) if report.exists() else []
    results.append({'group': name, 'exit_code': returncode, 'stats': stats, 'external_limits': external})

(artifacts / 'summary.json').write_text(json.dumps(results, indent=2))
for result in results:
    print(result['group'], 'FAIL' if result['exit_code'] else 'BLOCKED' if result['external_limits'] else 'NOT RUN' if result['stats'].get('skipped', 0) else 'PASS', result['stats'], result['external_limits'])
print('Private reports:', artifacts)
sys.exit(1 if any(result['exit_code'] for result in results) else 2 if any(result['external_limits'] or result['stats'].get('skipped', 0) for result in results) else 0)
