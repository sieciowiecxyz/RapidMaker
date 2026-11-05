#!/usr/bin/env python3
"""Ignored storage tests, each on its own disposable local PostgreSQL database."""
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import signal
import subprocess
import sys
from urllib.parse import urlsplit

os.umask(0o077)
root = Path(__file__).resolve().parents[2]
artifacts = root / 'frontend/tests/artifacts'
artifacts.mkdir(parents=True, exist_ok=True)
artifacts.chmod(0o700)
logs = artifacts / ('storage-' + secrets.token_hex(16))
logs.mkdir(mode=0o700)
report_name = os.environ.get('E2E_REPORT_NAME', 'storage')
if not re.fullmatch(r'[A-Za-z0-9-]{1,80}', report_name):
    raise SystemExit('Invalid report name')
report = artifacts / (report_name + '.json')
report.unlink(missing_ok=True)
env = {k: v for k, v in os.environ.items() if k in (
    'PATH', 'HOME', 'USER', 'CARGO_HOME', 'RUSTUP_HOME', 'RUSTUP_TOOLCHAIN',
    'LD_LIBRARY_PATH', 'XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS',
)}
container = os.environ.get('TEST_PG_CONTAINER', 'rapidmaker-catalog-postgres')
cargo = ['cargo', 'test', '--locked', '--manifest-path', 'backend/Cargo.toml',
         '-p', 'rapidmaker-storage', '--lib', '--']
cases, results, failure = [], [], None


def stop(_signal, _frame):
    for sig in (signal.SIGTERM, signal.SIGINT):
        signal.signal(sig, signal.SIG_IGN)
    raise SystemExit(1)


def run(command, child_env, log, timeout):
    with log.open('ab') as output:
        process = subprocess.Popen(command, cwd=root, env=child_env, stdout=output,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        try:
            return process.wait(timeout=timeout)
        finally:
            # Also terminate descendants when the group leader has already exited.
            previous = {sig: signal.signal(sig, signal.SIG_IGN)
                        for sig in (signal.SIGTERM, signal.SIGINT)}
            try:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    pass
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            finally:
                try:
                    process.wait(timeout=5)
                finally:
                    for sig, handler in previous.items():
                        signal.signal(sig, handler)


def sql(query, log):
    command = ['podman', 'exec', container, 'psql', '-U', 'rapidmaker', '-d',
               'postgres', '-v', 'ON_ERROR_STOP=1', '-c', query]
    if run(command, env, log, 15):
        raise RuntimeError('database operation failed')


for sig in (signal.SIGTERM, signal.SIGINT):
    signal.signal(sig, stop)
try:
    url = None
    for line in (root / '.env').read_text().splitlines():
        key, sep, value = line.partition('=')
        if sep and key.strip() == 'DATABASE_URL':
            values = shlex.split(value, comments=True)
            url = values[0] if len(values) == 1 else None
    parsed = urlsplit(url or '')
    if parsed.scheme not in ('postgres', 'postgresql') or parsed.hostname not in (
        'localhost', '127.0.0.1'
    ) or parsed.port != 18434 or parsed.query or parsed.fragment:
        raise ValueError('dedicated local PostgreSQL on port 18434 required')
    discovery = logs / 'discovery.log'
    if run([*cargo, '--list', '--ignored'], env, discovery, 300):
        raise RuntimeError('test discovery failed')
    cases = re.findall(r'^([\w:]+): test$', discovery.read_text(), re.MULTILINE)
    if not cases:
        raise RuntimeError('no ignored storage tests discovered')
    for index, case in enumerate(cases):
        name = 'rapidmaker_test_storage_' + secrets.token_hex(16)
        log = logs / f'{index + 1:02d}.log'
        created_attempted, passed = False, False
        case_result = {'name': case}
        try:
            created_attempted = True
            sql('CREATE DATABASE ' + name, log)
            child_env = dict(env, TEST_DATABASE_URL=parsed._replace(path='/' + name).geturl(),
                             AUTH_TEST_DATABASE_URL=parsed._replace(path='/' + name).geturl())
            code = run([*cargo, case, '--exact', '--ignored', '--test-threads=1'],
                       child_env, log, 180)
            passed = code == 0 and bool(re.search(
                r'test result: ok\. 1 passed; 0 failed; 0 ignored;', log.read_text()))
        except Exception as error:
            failure = type(error).__name__
        finally:
            previous = {sig: signal.signal(sig, signal.SIG_IGN)
                        for sig in (signal.SIGTERM, signal.SIGINT)}
            try:
                if created_attempted:
                    sql('DROP DATABASE IF EXISTS ' + name + ' WITH (FORCE)', log)
            except Exception as error:
                passed, failure = False, type(error).__name__
                case_result['cleanup_database'] = name
            finally:
                for sig, handler in previous.items():
                    signal.signal(sig, handler)
                case_result['status'] = 'passed' if passed else 'failed'
                results.append(case_result)
                print(case, 'PASS' if passed else 'FAIL', flush=True)
except BaseException as error:
    failure = type(error).__name__
finally:
    stats = {'expected': sum(item['status'] == 'passed' for item in results),
             'unexpected': sum(item['status'] == 'failed' for item in results),
             'skipped': len(cases) - len(results), 'flaky': 0}
    if failure and not stats['unexpected']:
        stats['unexpected'] = 1
    report.write_text(json.dumps({'stats': stats, 'cases': results, 'error': failure}, indent=2))
    report.chmod(0o600)
    print('Storage:', stats, 'Private logs:', logs)
sys.exit(1 if failure or stats['unexpected'] or stats['skipped'] else 0)
