#!/usr/bin/env python3
"""Real browser/API isolation. Does not load mail or provider credentials by default."""
import json
import os
from pathlib import Path
import secrets
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

os.umask(0o077)
root = Path(__file__).resolve().parents[2]
artifacts = root / "frontend/tests/artifacts"
artifacts.mkdir(parents=True, exist_ok=True)
artifacts.chmod(0o700)
queue_restart = "--queue-restart" in sys.argv
queue_recovery = "--queue-recovery" in sys.argv
queue_mode = queue_restart or queue_recovery
queue_flag = "--queue-recovery" if queue_recovery else "--queue-restart"
if queue_mode and sys.argv[1:] != [queue_flag]:
    raise SystemExit(queue_flag + " must run alone, without browser tests or other modes")
queue_report_name = os.environ.get("E2E_REPORT_NAME", queue_flag[2:] + "-" + secrets.token_hex(8))
if queue_mode and not queue_report_name.replace("-", "").isalnum():
    raise SystemExit("Invalid report name")
queue_report = artifacts / (queue_report_name + ".json")
if queue_mode:
    # An explicit reused name must never retain a prior PASS after startup failure.
    queue_report.unlink(missing_ok=True)
registration = "--registration" in sys.argv
sandbox = "--sandbox" in sys.argv
local_accounts = "--local-accounts" in sys.argv
large_models = "--large-models" in sys.argv
client_totp = "--client-totp" in sys.argv or local_accounts
arguments = [arg for arg in sys.argv[1:] if arg not in ("--sandbox", "--client-totp", "--local-accounts", "--registration", "--large-models")]
if client_totp and (not (sandbox or local_accounts) or not arguments or any(Path(arg).name == "auth.spec.ts" for arg in arguments)):
    raise SystemExit("TOTP fixture requires --sandbox or --local-accounts and explicit tests other than auth.spec.ts")
if registration and not sandbox:
    raise SystemExit("--registration requires --sandbox for real test email")
provider_values = {}
container = os.environ.get("TEST_PG_CONTAINER", "rapidmaker-catalog-postgres")
url = None
for line in (root / ".env").read_text().splitlines():
    key, sep, value = line.partition("=")
    if sep and key.strip() == "DATABASE_URL":
        url = shlex.split(value, comments=True)[0]
    if sandbox and sep and key.strip() in {"STRIPE_SECRET_KEY", "STRIPE_WEBHOOK_SECRET", "STRIPE_PUBLISHABLE_KEY", "FURGONETKA_SANDBOX_TOKEN", "FURGONETKA_SENDER_JSON", "FURGONETKA_SENDER_POINTS_JSON", "MAIL_SMTP_HOST", "MAIL_SMTP_PORT", "MAIL_USER", "MAIL_PASSWORD", "MAIL_FROM", "MAIL_REPLY_TO", "MAIL_REPLY_KEY", "MAIL_IMAP_USER", "MAIL_IMAP_PASSWORD", "MAIL_IMAP_HOST", "MAIL_IMAP_PORT", "MAIL_IMAP_MAILBOX"}:
        values = shlex.split(value, comments=True)
        if values: provider_values[key.strip()] = values[0]
if not url:
    raise SystemExit("DATABASE_URL missing; configure the local test PostgreSQL first.")
parsed = urllib.parse.urlsplit(url)
if parsed.hostname not in ("localhost", "127.0.0.1") or parsed.port != 18434:
    raise SystemExit("Tests require the dedicated local PostgreSQL on port 18434.")
if not queue_mode:
    browser_check = subprocess.run([
        "node", "--input-type=module", "-e",
        "import {chromium} from 'playwright'; const b=await chromium.launch({executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}); await b.close();"
    ], cwd=root / "frontend", capture_output=True)
    if browser_check.returncode:
        raise SystemExit("Browser preflight failed; run `cd frontend && npx playwright install chromium` or set a valid PLAYWRIGHT_CHROMIUM_EXECUTABLE. No test database was created.")
name = "rapidmaker_test_browser_" + secrets.token_hex(16)

def sql(query, database="postgres"):
    subprocess.run(["podman", "exec", container, "psql", "-U", "rapidmaker", "-d", database, "-v", "ON_ERROR_STOP=1", "-c", query], check=True, timeout=15, stdout=subprocess.DEVNULL)

binary = root / "backend/target/debug/rapidmaker-api"
if not binary.exists():
    raise SystemExit("Build the backend and frontend first; see docs/testing.md.")
folder = Path(tempfile.mkdtemp(prefix="rapidmaker-browser-"))
api = None
browser = None
recovery = None
creation_attempted = False
failed = True
# SIGTERM must also run cleanup; otherwise a stopped test leaves its private DB/server behind.
def interrupted(_signal, _frame):
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    raise SystemExit(143)
signal.signal(signal.SIGTERM, interrupted)
try:
    # The private random name also belongs to this run when CREATE's outcome is unknown.
    creation_attempted = True
    sql("CREATE DATABASE " + name)
    env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "LD_LIBRARY_PATH")}
    env.update(DATABASE_URL=parsed._replace(path="/" + name).geturl(), MODEL_STORAGE_DIR=str(folder / "models"), API_PORT="18436", AUTH_MODE="localhost", AUTH_ORIGIN="http://localhost:18437", AUTH_CODE_KEY=secrets.token_hex(32), MAIL_MODE="disabled", MAIL_INBOUND_MODE="disabled", LOG_LEVEL="warn")
    rates = dict(version="browser-test-only", setup_grosze=1000, handling_per_plate_grosze=500, machine_per_hour_grosze=200, energy_per_kwh_grosze=120, printer_watts=150, loss_bps=500, margin_bps=3000, vat_bps=2300, minimum_net_grosze=2000)
    env["PRICING_RATES_JSON"] = json.dumps(rates)
    if large_models: env["MODEL_PROFILE"] = "large"
    if sandbox:
        if not provider_values.get("STRIPE_SECRET_KEY", "").startswith(("sk_test_", "rk_test_")):
            raise RuntimeError("Stripe test key required; live keys refused")
        env.update(provider_values, MAIL_MODE="test", MAIL_TEST_TO="void@sieciowiec.xyz", MAIL_INBOUND_MODE="test")
    log = folder / "api.jsonl"
    def start_api():
        # Assign immediately: finally owns this process even when readiness fails.
        global api
        if queue_mode:
            # Refuse an occupied port before issuing any HTTP to another server.
            with socket.socket() as probe:
                probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                probe.bind(("127.0.0.1", 18436))
        with log.open("ab") as output:
            os.chmod(log, 0o600)
            api = subprocess.Popen([str(binary)], cwd=root, env=env, stdin=subprocess.DEVNULL, stdout=output, stderr=output, start_new_session=True)
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if api.poll() is not None:
                raise RuntimeError("Isolated API failed to start; inspect private local test report.")
            try:
                req = urllib.request.Request("http://127.0.0.1:18436/api/openapi.json", headers={"Host": "localhost:18437"})
                with urllib.request.urlopen(req, timeout=1):
                    return api.pid
            except (urllib.error.URLError, TimeoutError):
                time.sleep(0.1)
        raise RuntimeError("Isolated API startup timeout (30s)")
    if queue_mode:
        env["SLICER_WORKER_MODE"] = "external"
    start_api()
    # Explicit fixture costs only in this disposable database; every color remains stock=false.
    sql("UPDATE catalog_materials SET cost_net_grosze_per_kg=10000,production_enabled=true WHERE id IN ('GFA00','GFB00','GFU01','GFG02','GFB01','GFN03')", name)
    if queue_mode:
        from queue_restart import run_queue_restart
        if queue_recovery:
            from queue_recovery import QueueRecovery
            recovery = QueueRecovery(root, folder, binary, env)
        def restart_api():
            old_pid = api.pid
            os.killpg(old_pid, signal.SIGTERM)
            exit_code = api.wait(timeout=30)
            if exit_code != 0:
                raise RuntimeError(f"Old API {old_pid} exited {exit_code}, expected graceful exit 0")
            env["SLICER_WORKER_MODE"] = "embedded"
            new_pid = start_api()
            if new_pid == old_pid:
                raise RuntimeError("Restart did not replace process")
            return dict(old_pid=old_pid, old_exit_code=exit_code, new_pid=new_pid)
        report = queue_report
        started = time.monotonic()
        evidence = {}
        error = None
        try:
            run_queue_restart(root, folder, container, name, restart_api, evidence, recovery.run if recovery else None)
            failed = False
        except Exception as exc:
            error = f"{type(exc).__name__}: {exc}"
            raise
        finally:
            report.write_text(json.dumps(dict(stats=dict(expected=0, unexpected=1, skipped=0, flaky=0, duration=round((time.monotonic()-started)*1000)), cleanup_verified=False, evidence=evidence, error=error), indent=2) + "\n")
            report.chmod(0o600)
            print(queue_flag[2:] + " report:", report, flush=True)
        sys.exit(0)
    browser_env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "DISPLAY", "PLAYWRIGHT_CHROMIUM_EXECUTABLE", "E2E_CORPUS_MANIFEST", "E2E_CORPUS_LIMIT")}
    # Chromium otherwise drops Unicode file paths from FileList in this minimal environment.
    browser_env.update(LANG="C.UTF-8", LC_ALL="C.UTF-8")
    if large_models: browser_env["RAPIDMAKER_LARGE_MODELS"] = "1"
    browser_env.update(RAPIDMAKER_TEST_API_PID=str(api.pid), RAPIDMAKER_TEST_DATABASE=name, RAPIDMAKER_TEST_MODEL_ROOT=str(folder / "models"), TEST_PG_CONTAINER=container)
    if sandbox or local_accounts:
        if client_totp: env["TEST_CLIENT_TOTP"] = "1"
        if registration: env["TEST_REGISTER"] = "1"
        fixture_file = folder / "accounts.json"
        with fixture_file.open("wb") as output:
            fixture_file.chmod(0o600)
            subprocess.run(["cargo", "run", "--locked", "--quiet", "--manifest-path", "backend/Cargo.toml", "-p", "rapidmaker-api", "--example", "browser_fixture"], cwd=root, env=env, stdout=output, check=True)
        browser_env.update(RAPIDMAKER_TEST_ACCOUNTS=str(fixture_file))
        if sandbox: browser_env["RAPIDMAKER_SANDBOX"] = "1"
    report_name = os.environ.get("E2E_REPORT_NAME", "integration")
    if not report_name.replace("-", "").isalnum(): raise RuntimeError("Invalid report name")
    browser_env["E2E_REPORT_NAME"] = report_name
    browser_env["PLAYWRIGHT_NO_COPY_PROMPT"] = "1"
    method = os.environ.get("E2E_REGISTRATION_METHOD", "password")
    if method not in ("password", "passkey"): raise RuntimeError("Invalid registration method")
    browser_env["E2E_REGISTRATION_METHOD"] = method
    browser_env["PLAYWRIGHT_JSON_OUTPUT_NAME"] = str(root / "frontend/tests/artifacts" / (report_name + ".json"))
    browser = subprocess.Popen(["node", str(root / "frontend/node_modules/@playwright/test/cli.js"), "test", "--config=playwright.integration.config.ts", *arguments], cwd=root / "frontend", env=browser_env, start_new_session=True)
    result = browser.wait()
    failed = result != 0
    sys.exit(result)
finally:
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    cleanup_errors = []
    def cleanup_step(label, action):
        try:
            action()
        except Exception as exc:
            cleanup_errors.append(f"{label}: {type(exc).__name__}")

    if recovery:
        cleanup_step("workers", recovery.cleanup)
    def stop_process(process, stop_signal):
        if process and process.poll() is None:
            try:
                os.killpg(process.pid, stop_signal)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=10)
    for label, process, stop_signal in (("browser", browser, signal.SIGINT), ("api", api, signal.SIGTERM)):
        cleanup_step(label, lambda: stop_process(process, stop_signal))
    if creation_attempted:
        cleanup_step("database", lambda: sql("DROP DATABASE IF EXISTS " + name + " WITH (FORCE)"))

    def copy_failure_logs():
        if (folder / "api.jsonl").exists():
            private_log = artifacts / ((queue_report_name if queue_mode else "integration") + "-api.jsonl")
            shutil.copyfile(folder / "api.jsonl", private_log)
            private_log.chmod(0o600)
            print("Private API failure report:", private_log)
        if recovery:
            recovery.copy_failure_logs(artifacts, queue_report_name)
    if failed or cleanup_errors:
        cleanup_step("failure_logs", copy_failure_logs)
    # This directory belongs exclusively to this run. Never touches developer uploads.
    cleanup_step("private_root", lambda: shutil.rmtree(folder))
    if cleanup_errors and folder.exists():
        cleanup_step("remaining_failure_logs", copy_failure_logs)
    if queue_mode and queue_report.exists():
        saved = json.loads(queue_report.read_text())
        saved['cleanup_verified'] = not cleanup_errors
        saved['cleanup_errors'] = cleanup_errors
        saved['stats'].update(expected=int(not failed and not cleanup_errors), unexpected=int(failed or bool(cleanup_errors)))
        if cleanup_errors:
            saved['error'] = saved.get('error') or 'Isolated teardown failed'
        queue_report.write_text(json.dumps(saved, indent=2) + "\n")
    if cleanup_errors:
        raise RuntimeError("Isolated teardown failed (" + "; ".join(cleanup_errors) + ")")
