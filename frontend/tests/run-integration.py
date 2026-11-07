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
if queue_restart and sys.argv[1:] != ["--queue-restart"]:
    raise SystemExit("--queue-restart must run alone, without browser tests or other modes")
queue_report_name = os.environ.get("E2E_REPORT_NAME", "queue-restart-" + secrets.token_hex(8))
if queue_restart and not queue_report_name.replace("-", "").isalnum():
    raise SystemExit("Invalid report name")
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
if not queue_restart:
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
        if queue_restart:
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
    if queue_restart:
        env["SLICER_WORKER_MODE"] = "external"
    start_api()
    # Explicit fixture costs only in this disposable database; every color remains stock=false.
    sql("UPDATE catalog_materials SET cost_net_grosze_per_kg=10000,production_enabled=true WHERE id IN ('GFA00','GFB00','GFU01','GFG02','GFB01','GFN03')", name)
    if queue_restart:
        from queue_restart import run_queue_restart
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
        report = artifacts / (queue_report_name + ".json")
        started = time.monotonic()
        evidence = {}
        error = None
        try:
            run_queue_restart(root, folder, container, name, restart_api, evidence)
            failed = False
        except Exception as exc:
            error = f"{type(exc).__name__}: {exc}"
            raise
        finally:
            report.write_text(json.dumps(dict(stats=dict(expected=int(not failed), unexpected=int(failed), skipped=0, flaky=0, duration=round((time.monotonic()-started)*1000)), evidence=evidence, error=error), indent=2) + "\n")
            report.chmod(0o600)
            print("Queue restart report:", report, flush=True)
        sys.exit(0)
    browser_env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "DISPLAY", "PLAYWRIGHT_CHROMIUM_EXECUTABLE", "E2E_CORPUS_MANIFEST", "E2E_CORPUS_LIMIT")}
    # Chromium otherwise drops Unicode file paths from FileList in this minimal environment.
    browser_env.update(LANG="C.UTF-8", LC_ALL="C.UTF-8")
    if large_models: browser_env["RAPIDMAKER_LARGE_MODELS"] = "1"
    browser_env.update(RAPIDMAKER_TEST_DATABASE=name, RAPIDMAKER_TEST_MODEL_ROOT=str(folder / "models"), TEST_PG_CONTAINER=container)
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
    for process, stop_signal in ((browser, signal.SIGINT), (api, signal.SIGTERM)):
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
                process.wait()
    try:
        if creation_attempted:
            sql("DROP DATABASE IF EXISTS " + name + " WITH (FORCE)")
    finally:
        try:
            if failed and (folder / "api.jsonl").exists():
                report = artifacts / ((queue_report_name if queue_restart else "integration") + "-api.jsonl")
                report.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(folder / "api.jsonl", report)
                report.chmod(0o600)
                print("Private API failure report:", report)
        finally:
            # This directory belongs exclusively to this run. Never touches developer uploads.
            shutil.rmtree(folder)
