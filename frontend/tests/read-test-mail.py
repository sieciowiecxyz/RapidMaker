#!/usr/bin/env python3
"""Read only fresh test mail in the configured iCloud folder; stdout is private JSON."""
import email
from email.policy import default
from email.utils import parseaddr, formatdate, make_msgid
import imaplib
import json
from pathlib import Path
import re
import shlex
import sys
import time

root = Path(__file__).resolve().parents[2]
values = {}
for line in (root / '.env').read_text().splitlines():
    key, sep, raw = line.partition('=')
    if sep and key.strip().startswith('MAIL_'):
        parsed = shlex.split(raw, comments=True)
        if parsed: values[key.strip()] = parsed[0]
request = json.load(sys.stdin)
if request['mode'] == 'reply':
    import smtplib
    import ssl
    from email.message import EmailMessage
    message = EmailMessage()
    message['From'] = 'void@sieciowiec.xyz'
    message['To'] = 'void@sieciowiec.xyz'
    message['Subject'] = 'Re: Rapidmaker — test rozmowy'
    message['Date'] = formatdate(localtime=False)
    message['Message-ID'] = make_msgid(domain='sieciowiec.xyz')
    reference = request['message_id']
    if len(reference) > 512 or any(ord(c) < 32 for c in reference): raise SystemExit('Invalid test reference')
    message['In-Reply-To'] = reference
    message['References'] = reference
    if len(request['body']) > 2000: raise SystemExit('Test body too long')
    message.set_content(request['body'])
    try:
        with smtplib.SMTP('smtp.mail.me.com', 587, timeout=20) as smtp:
            smtp.starttls(context=ssl.create_default_context())
            smtp.login(values['MAIL_USER'], values['MAIL_PASSWORD'])
            smtp.send_message(message)
        print(json.dumps({'sent': True}))
    except Exception: raise SystemExit('Test reply transport failed; no credentials logged')
    sys.exit(0)
user = values.get('MAIL_IMAP_USER') or values.get('MAIL_USER')
password = values.get('MAIL_IMAP_PASSWORD') or values.get('MAIL_PASSWORD')
folder = values.get('MAIL_IMAP_MAILBOX', 'INBOX')
stage = 'connect'
try:
    with imaplib.IMAP4_SSL('imap.mail.me.com', 993, timeout=20) as imap:
        stage = 'login'
        imap.login(user, password)
        stage = 'select'
        if imap.select(folder, readonly=True)[0] != 'OK': raise RuntimeError('folder')
        stage = request['mode']
        if request['mode'] == 'watermark':
            status, data = imap.status(folder, '(UIDNEXT)')
            match = re.search(rb'UIDNEXT (\d+)', b' '.join(data))
            if status != 'OK' or not match: raise RuntimeError('cursor')
            print(json.dumps({'after': int(match[1]) - 1}))
        else:
            after = int(request['after'])
            deadline = time.monotonic() + 90
            while time.monotonic() < deadline:
                imap.noop()
                status, data = imap.uid('search', None, 'UID', f'{after + 1}:*')
                if status != 'OK': raise RuntimeError('search')
                uids = b' '.join(item for item in data if isinstance(item, bytes))
                for uid in [n for n in uids.split() if int(n) > after][-30:]:
                    status, sizes = imap.uid('fetch', uid, '(RFC822.SIZE)')
                    size = re.search(rb'RFC822.SIZE (\d+)', b' '.join(v for v in sizes if isinstance(v, bytes)))
                    if status != 'OK' or not size or int(size[1]) > 256 * 1024: continue
                    status, items = imap.uid('fetch', uid, '(BODY.PEEK[])')
                    raw = next((item[1] for item in items if isinstance(item, tuple)), None)
                    if status != 'OK' or not raw: continue
                    message = email.message_from_bytes(raw, policy=default)
                    if parseaddr(str(message.get('From', '')))[1].lower() != parseaddr(values.get('MAIL_FROM', ''))[1].lower(): continue
                    bodies = []
                    for part in message.walk():
                        if part.get_content_type() == 'text/plain' and part.get_content_disposition() != 'attachment':
                            bodies.append(part.get_content())
                    body = '\n'.join(bodies)
                    if request['mode'] == 'code':
                        match = re.search(r'Twój kod logowania:\s*(\d{8})', body)
                        if match:
                            print(json.dumps({'code': match[1]})); sys.exit(0)
                    elif request['mode'] == 'chat' and request['reference'] in body and request['marker'] in body:
                        print(json.dumps({'message_id': str(message.get('Message-ID', ''))})); sys.exit(0)
                    elif request['mode'] == 'offer' and request['reference'] in body and '/konto/zamowienia/' in body:
                        print(json.dumps({'received': True})); sys.exit(0)
                time.sleep(2)
            raise RuntimeError('mail_timeout')
except Exception as error:
    import traceback
    line = traceback.extract_tb(error.__traceback__)[-1].lineno
    raise SystemExit(f'Test mail unavailable ({stage}:{type(error).__name__}:line{line}); no credentials or mail body logged.')
