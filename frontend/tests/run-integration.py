#!/usr/bin/env python3
"""Real browser/API isolation. Does not load mail or provider credentials by default."""
import json
import os
from pathlib import Path
import secrets
import shlex
import shutil
import signal
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
registration = "--registration" in sys.argv
sandbox = "--sandbox" in sys.argv
local_accounts = "--local-accounts" in sys.argv
client_totp = "--client-totp" in sys.argv or local_accounts
arguments = [arg for arg in sys.argv[1:] if arg not in ("--sandbox", "--client-totp", "--local-accounts", "--registration")]
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
browser_check = subprocess.run([
    "node", "--input-type=module", "-e",
    "import {chromium} from 'playwright'; const b=await chromium.launch({executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE}); await b.close();"
], cwd=root / "frontend", capture_output=True)
if browser_check.returncode:
    raise SystemExit("Browser preflight failed; run `cd frontend && npx playwright install chromium` or set a valid PLAYWRIGHT_CHROMIUM_EXECUTABLE. No test database was created.")
name = "rapidmaker_test_browser_" + secrets.token_hex(6)

def sql(query, database="postgres"):
    subprocess.run(["podman", "exec", container, "psql", "-U", "rapidmaker", "-d", database, "-v", "ON_ERROR_STOP=1", "-c", query], check=True, stdout=subprocess.DEVNULL)

binary = root / "backend/target/debug/rapidmaker-api"
if not binary.exists():
    raise SystemExit("Build the backend and frontend first; see docs/testing.md.")
folder = Path(tempfile.mkdtemp(prefix="rapidmaker-browser-"))
api = None
browser = None
created = False
failed = True
# SIGTERM must also run cleanup; otherwise a stopped test leaves its private DB/server behind.
def interrupted(_signal, _frame):
    raise SystemExit(143)
signal.signal(signal.SIGTERM, interrupted)
try:
    sql("CREATE DATABASE " + name)
    created = True
    env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "LD_LIBRARY_PATH")}
    env.update(DATABASE_URL=parsed._replace(path="/" + name).geturl(), MODEL_STORAGE_DIR=str(folder / "models"), API_PORT="18436", AUTH_MODE="localhost", AUTH_ORIGIN="http://localhost:18437", AUTH_CODE_KEY=secrets.token_hex(32), MAIL_MODE="disabled", MAIL_INBOUND_MODE="disabled", LOG_LEVEL="warn")
    rates = dict(version="browser-test-only", setup_grosze=1000, handling_per_plate_grosze=500, machine_per_hour_grosze=200, energy_per_kwh_grosze=120, printer_watts=150, loss_bps=500, margin_bps=3000, vat_bps=2300, minimum_net_grosze=2000)
    env["PRICING_RATES_JSON"] = json.dumps(rates)
    if sandbox:
        if not provider_values.get("STRIPE_SECRET_KEY", "").startswith(("sk_test_", "rk_test_")):
            raise RuntimeError("Stripe test key required; live keys refused")
        env.update(provider_values, MAIL_MODE="test", MAIL_TEST_TO="void@sieciowiec.xyz", MAIL_INBOUND_MODE="test")
    log = folder / "api.jsonl"
    with log.open("wb") as output:
        os.chmod(log, 0o600)
        api = subprocess.Popen([str(binary)], cwd=root, env=env, stdin=subprocess.DEVNULL, stdout=output, stderr=output, start_new_session=True)
    for _ in range(100):
        if api.poll() is not None:
            raise RuntimeError("Isolated API failed to start; inspect private local test report.")
        try:
            req = urllib.request.Request("http://127.0.0.1:18436/api/openapi.json", headers={"Host": "localhost:18437"})
            with urllib.request.urlopen(req, timeout=1):
                break
        except (urllib.error.URLError, TimeoutError):
            time.sleep(0.1)
    else:
        raise RuntimeError("Isolated API startup timeout")
    # Explicit fixture costs only in this disposable database; every color remains stock=false.
    sql("UPDATE catalog_materials SET cost_net_grosze_per_kg=10000,production_enabled=true WHERE id IN ('GFA00','GFB00','GFU01')", name)
    browser_env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "DISPLAY", "PLAYWRIGHT_CHROMIUM_EXECUTABLE")}
    if "PLAYWRIGHT_CHROMIUM_EXECUTABLE" not in browser_env and Path("/usr/bin/chromium").is_file():
        browser_env["PLAYWRIGHT_CHROMIUM_EXECUTABLE"] = "/usr/bin/chromium"
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
    if browser and browser.poll() is None:
        os.killpg(browser.pid, signal.SIGINT)
        try:
            browser.wait(timeout=30)
        except subprocess.TimeoutExpired:
            os.killpg(browser.pid, signal.SIGKILL)
            browser.wait()
    if api and api.poll() is None:
        os.killpg(api.pid, signal.SIGTERM)
        try:
            api.wait(timeout=30)
        except subprocess.TimeoutExpired:
            os.killpg(api.pid, signal.SIGKILL)
            api.wait()
    if created:
        sql("DROP DATABASE " + name + " WITH (FORCE)")
    if failed and (folder / "api.jsonl").exists():
        report = root / "frontend/tests/artifacts/integration-api.jsonl"
        report.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(folder / "api.jsonl", report)
        report.chmod(0o600)
        print("Private API failure report:", report)
    # This directory belongs exclusively to this run. Never touches developer uploads.
    shutil.rmtree(folder)
