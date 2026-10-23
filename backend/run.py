#!/usr/bin/env python3
"""Run the local sandbox. Read only provider variables; never evaluate shell text."""
import os
import shlex
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
allowed = {"STRIPE_SECRET_KEY", "STRIPE_PUBLISHABLE_KEY", "FURGONETKA_SANDBOX_TOKEN", "FURGONETKA_SENDER_JSON", "DATABASE_URL", "PRICING_RATES_JSON", "ADMIN_API_TOKEN", "MODEL_STORAGE_DIR"}
allowed.update({"MAIL_MODE", "MAIL_SMTP_HOST", "MAIL_SMTP_PORT", "MAIL_USER", "MAIL_PASSWORD", "MAIL_FROM", "MAIL_REPLY_TO", "MAIL_TEST_TO"})
allowed.update({"AUTH_MODE", "AUTH_ORIGIN", "AUTH_CODE_KEY"})
for name in (".env",):
    path = root / name
    if not path.exists():
        continue
    if path.stat().st_mode & 0o077:
        raise SystemExit(f"Restrict {name} permissions with chmod 600 first.")
    for line in path.read_text().splitlines():
        key, sep, raw = line.strip().removeprefix("export ").partition("=")
        if sep and key in allowed and key not in os.environ:
            values = shlex.split(raw, comments=True)
            if len(values) > 1:
                raise SystemExit(f"Quote the value of {key} in {name}.")
            os.environ[key] = values[0] if values else ""

os.chdir(root)
binary = root / "backend/target/debug/rapidmaker-api"
os.execv(binary, [str(binary), *sys.argv[1:]])
