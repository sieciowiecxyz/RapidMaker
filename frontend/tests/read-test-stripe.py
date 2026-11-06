#!/usr/bin/env python3
"""Independent test-only Stripe evidence; two fixed GETs, sanitized JSON only."""
import json
from pathlib import Path
import re
import shlex
import sys
import urllib.request


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('Redirect refused')


def identifier(value, pattern):
    if not isinstance(value, str) or not re.fullmatch(pattern, value):
        raise ValueError('Invalid identifier')
    return value


def main():
    request = json.loads(sys.stdin.read(4097))
    if set(request) != {'payment_intent_id', 'attempt_id', 'order_id'}:
        raise ValueError('Invalid request')
    uuid = r'[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}'
    pi_id = identifier(request['payment_intent_id'], r'pi_[A-Za-z0-9]{1,100}')
    attempt_id = identifier(request['attempt_id'], uuid)
    order_id = identifier(request['order_id'], uuid)
    key = ''
    for line in (Path(__file__).resolve().parents[2] / '.env').read_text().splitlines():
        name, sep, raw = line.partition('=')
        if sep and name.strip() == 'STRIPE_SECRET_KEY':
            parts = shlex.split(raw, comments=True)
            key = parts[0] if len(parts) == 1 else ''
    identifier(key, r'sk_test_[A-Za-z0-9]+')
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

    def get(path):
        req = urllib.request.Request('https://api.stripe.com/v1/' + path,
                                     headers={'Authorization': 'Bearer ' + key}, method='GET')
        with opener.open(req, timeout=15) as response:
            body = response.read(128 * 1024 + 1)
            if response.status != 200 or len(body) > 128 * 1024:
                raise ValueError('Invalid response')
            return json.loads(body)

    def payment_fields(value):
        for field in ('amount',):
            if type(value[field]) is not int or value[field] < 0:
                raise ValueError('Invalid amount')
        return {'amount': value['amount'],
                'currency': identifier(value['currency'], r'[a-z]{3}'),
                'status': identifier(value['status'], r'[a-z_]{1,40}')}

    pi = get('payment_intents/' + pi_id)
    if (pi['id'] != pi_id or pi['object'] != 'payment_intent'
            or pi['livemode'] is not False or pi['metadata']['quote_id'] != attempt_id
            or type(pi['amount_received']) is not int or pi['amount_received'] < 0):
        raise ValueError('Payment correlation failed')
    result = {**payment_fields(pi), 'payment_intent_id': pi_id,
              'amount_received': pi['amount_received'], 'quote_id': attempt_id}
    refunds = get('refunds?payment_intent=' + pi_id + '&limit=100')
    if refunds['object'] != 'list' or refunds['has_more'] is not False or len(refunds['data']) > 100:
        raise ValueError('Incomplete refunds')
    result['refunds'] = []
    for refund in refunds['data']:
        if (refund['object'] != 'refund' or refund['payment_intent'] != pi_id
                or refund['metadata']['order_id'] != order_id):
            raise ValueError('Refund correlation failed')
        result['refunds'].append({**payment_fields(refund),
            'id': identifier(refund['id'], r're_[A-Za-z0-9]{1,100}'),
            'ledger_refund_id': identifier(refund['metadata']['refund_id'], uuid)})
    print(json.dumps(result))


if __name__ == '__main__':
    try:
        main()
    except Exception:
        raise SystemExit('Independent test Stripe evidence unavailable; no provider payload logged') from None
