import { expect, test, type APIRequestContext } from './fixtures';
import { accounts, login } from './helpers';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';

test.use({ screenshot: 'off', trace: 'off' });
const origin = 'http://localhost:18437';

// A schema error alone does not prove authorization; use deserializable bodies.
function sample(schema: any, definitions: any): unknown {
  if (schema.$ref) return sample(definitions[schema.$ref.split('/').pop()], definitions);
  if (schema.enum) return schema.enum[0];
  switch (schema.type) {
    case 'object':
      return Object.fromEntries(
        (schema.required ?? []).map((key: string) => [
          key,
          sample(schema.properties[key], definitions),
        ]),
      );
    case 'array':
      return [];
    case 'boolean':
      return false;
    case 'integer':
    case 'number':
      return 1;
    case 'string':
      return 'test';
    default:
      throw new Error('Unhandled request schema in the authorization probe');
  }
}

function retainedState() {
  const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
  expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]{32}$/);
  const tables = [
    'orders',
    'quotes',
    'payment_attempts',
    'payment_refunds',
    'shipments',
    'shipment_packages',
    'shipment_actions',
    'order_messages',
    'email_outbox',
    'slicer_jobs',
    'checkout_drafts',
    'order_file_holds',
    'order_files',
    'private_models',
    'catalog_materials',
    'catalog_colors',
    'pricing_config',
  ];
  const state = execFileSync(
    'podman',
    [
      'exec',
      process.env.TEST_PG_CONTAINER ?? 'rapidmaker-catalog-postgres',
      'psql',
      '-U',
      'rapidmaker',
      '-d',
      database,
      '-At',
      '-v',
      'ON_ERROR_STOP=1',
      '-c',
      `SELECT jsonb_build_array(${tables.map((table) => `(SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]'::jsonb) FROM ${table} t)`).join(',')})`,
    ],
    { encoding: 'utf8', timeout: 15_000 },
  ).trim();
  return createHash('sha256').update(state).digest('hex');
}

test('documented operator operations reject guests, clients and pending admin MFA and retain business data', async ({
  page,
  browser,
  playwright,
}) => {
  test.skip(!process.env.RAPIDMAKER_TEST_ACCOUNTS, 'Requires --local-accounts');
  const client = accounts().find(
    (account) => account.email === 'browser-security@example.invalid',
  )!;
  const admin = accounts().find((account) => account.role === 'admin')!;
  expect(client.totp_uri && admin.totp_uri).toBeTruthy();
  await login(page, client);
  const me = await (await page.request.get('/api/auth/me')).json();
  expect(me.user.id).toBe(client.id);
  expect(me.csrf_token).toEqual(expect.any(String));
  const preferences = await (await page.request.get('/api/account/notifications')).json();
  expect(
    (
      await page.request.put('/api/account/notifications', {
        headers: { Origin: origin, 'X-CSRF-Token': me.csrf_token },
        data: preferences,
      })
    ).status(),
  ).toBe(200);
  const guest = await playwright.request.newContext({ baseURL: origin });
  const pendingAdmin = await playwright.request.newContext({ baseURL: origin });
  const operator = await browser.newContext();
  try {
    const uploaded = await guest.post('/api/models', {
      headers: {
        Origin: origin,
        'Content-Type': 'application/octet-stream',
        'X-Model-Filename': 'cube.stl',
      },
      data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
    });
    expect(uploaded.status()).toBe(201);
    const model = await uploaded.json();
    const primary = await pendingAdmin.post('/api/auth/password/login', {
      headers: { Origin: origin },
      data: { email: admin.email, password: admin.password },
    });
    expect(primary.status()).toBe(200);
    const pending = await primary.json();
    expect(pending.stage).toBe('second_factor');
    expect(pending.csrf_token).toEqual(expect.any(String));
    const operatorPage = await operator.newPage();
    await login(operatorPage, admin);
    expect((await operator.request.get('/api/admin/pricing')).status()).toBe(200);
    const operatorMe = await (await operator.request.get('/api/auth/me')).json();
    expect(operatorMe.csrf_token).toEqual(expect.any(String));
    const schema = await (await guest.get('/api/openapi.json')).json();
    const operations = Object.entries(schema.paths).flatMap(([path, methods]) =>
      path.startsWith('/api/admin/') && path !== '/api/admin/session'
        ? Object.entries(methods as Record<string, any>)
            .filter(([method]) => ['get', 'post', 'put', 'patch', 'delete'].includes(method))
            .map(([method, operation]) => ({
              path,
              method: method.toUpperCase(),
              id: operation.operationId,
              input: operation.requestBody?.content?.['application/json']?.schema,
            }))
        : [],
    );
    expect(operations.length).toBeGreaterThan(0);
    const before = retainedState();
    const evidence: { actor: string; operation: string; method: string; status: number }[] = [];
    const actors: {
      name: string;
      request: APIRequestContext;
      csrf: string;
      status: number;
      mutationsOnly?: boolean;
      origin?: string;
    }[] = [
      { name: 'guest', request: guest, csrf: '', status: 401 },
      { name: 'client', request: page.request, csrf: me.csrf_token, status: 403 },
      { name: 'pending-admin', request: pendingAdmin, csrf: pending.csrf_token, status: 401 },
      {
        name: 'admin-no-csrf',
        request: operator.request,
        csrf: '',
        status: 403,
        mutationsOnly: true,
      },
      {
        name: 'admin-wrong-origin',
        request: operator.request,
        csrf: operatorMe.csrf_token,
        status: 403,
        mutationsOnly: true,
        origin: 'https://evil.example.invalid',
      },
    ];
    for (const actor of actors) {
      if (!actor.mutationsOnly) {
        const state = await (await actor.request.get('/api/admin/session')).json();
        expect(state).toMatchObject({ authenticated: false, admin_verified: false, session: null });
      }
      for (const operation of operations.filter(
        (operation) => !actor.mutationsOnly || operation.method !== 'GET',
      )) {
        const url = operation.path.replace(/\{([^}]+)\}/g, (_, key: string) =>
          key === 'kind'
            ? 'source'
            : key === 'code'
              ? '10101'
              : '11111111-1111-4111-8111-111111111111',
        );
        const response = await actor.request.fetch(url, {
          method: operation.method,
          headers: {
            Origin: actor.origin ?? origin,
            ...(actor.csrf ? { 'X-CSRF-Token': actor.csrf } : {}),
          },
          ...(operation.method === 'GET'
            ? {}
            : { data: sample(operation.input, schema.components.schemas) }),
        });
        evidence.push({
          actor: actor.name,
          operation: operation.id,
          method: operation.method,
          status: response.status(),
        });
        expect(response.status(), `${actor.name} ${operation.id}`).toBe(actor.status);
      }
    }
    expect(retainedState()).toBe(before);
    await test.info().attach('operator-authorization-matrix', {
      body: JSON.stringify({
        operations: operations.length,
        attempts: evidence,
        retainedStateUnchanged: true,
      }),
      contentType: 'application/json',
    });
    expect(
      (await guest.delete(`/api/models/${model.id}`, { headers: { Origin: origin } })).status(),
    ).toBe(204);
  } finally {
    await operator.close();
    await pendingAdmin.dispose();
    await guest.dispose();
  }
});
