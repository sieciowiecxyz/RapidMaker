import { expect, test } from './fixtures';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync, realpathSync, watch } from 'node:fs';
import { resolve } from 'node:path';

test.use({ screenshot: 'off', trace: 'off', video: 'off' });

test('exhausted model quotas refuse work before creating conversion files', async ({ request }) => {
  const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
  const root = process.env.RAPIDMAKER_TEST_MODEL_ROOT ?? '';
  expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
  expect(root).toMatch(/^\/tmp\/rapidmaker-browser-[^/]+\/models$/);
  expect(realpathSync(root)).toBe(root);
  const sql = (query: string) => execFileSync('podman', ['exec', process.env.TEST_PG_CONTAINER ?? 'rapidmaker-catalog-postgres', 'psql', '-U', 'rapidmaker', '-d', database, '-v', 'ON_ERROR_STOP=1', '-Atc', query], { encoding: 'utf8' }).trim();
  const cookie = 'ab'.repeat(32), owner = createHash('sha256').update(cookie).digest('hex');
  const input = readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
  const quotaMessage = 'Przekroczono limit zapisanych modeli.';
  const headers = { Origin: 'http://localhost:18437', 'Content-Type': 'application/octet-stream', 'x-model-filename': 'quota.stl', Cookie: `rapidmaker_guest=${cookie}` };
  const observations: { name: string; directoriesTouched: number; status: number }[] = [];
  const cases = [
    { name: 'owner-count', count: 20, owner: `'${owner}'`, source: '1', mesh: '84', beforeWork: true },
    { name: 'global-count', count: 1000, owner: "lpad(to_hex(g),64,'0')", source: '1', mesh: '84', beforeWork: true },
    { name: 'owner-bytes', count: 2, owner: `'${owner}'`, source: '96*1024*1024', mesh: '32*1024*1024', beforeWork: true },
    { name: 'global-bytes', count: 64, owner: "lpad(to_hex(g),64,'0')", source: '96*1024*1024', mesh: '32*1024*1024', beforeWork: true },
    // The original fits exactly; derived files must still fail the final atomic check.
    { name: 'derived-bytes', count: 2, owner: `'${owner}'`, source: '96*1024*1024', mesh: `32*1024*1024-CASE WHEN g=1 THEN ${input.length} ELSE 0 END`, beforeWork: false },
  ];
  try {
    for (const scenario of cases) {
      sql('DELETE FROM private_models');
      sql(`INSERT INTO private_models(id,owner_hash,filename,extension,source_bytes,mesh_bytes,preview_bytes,sha256,dimensions_mm,expires_at) SELECT gen_random_uuid()::text,${scenario.owner},'quota.stl','stl',${scenario.source},${scenario.mesh},0,repeat('a',64),ARRAY[20.,20.,20.],extract(epoch from now())::bigint+3600 FROM generate_series(1,${scenario.count}) g`);
      const touched = new Set<string>();
      const watcher = watch(root, (_, name) => {
        if (name && /^[a-f0-9-]{36}$/.test(name.toString())) touched.add(name.toString());
      });
      try {
        const response = await request.post('/api/models', { headers, data: input });
        expect(response.status()).toBe(429);
        expect((await response.json()).message).toContain(quotaMessage);
        await new Promise(resolveWait => setTimeout(resolveWait, 25));
        observations.push({ name: scenario.name, directoriesTouched: touched.size, status: response.status() });
        expect(readdirSync(root)).toHaveLength(0);
        expect(Number(sql('SELECT count(*) FROM private_models'))).toBe(scenario.count);
      } finally { watcher.close(); }
    }
    sql('DELETE FROM private_models');
    const uploaded = await request.post('/api/models', { headers, data: input });
    expect(uploaded.status()).toBe(201);
    const model = await uploaded.json();
    const preview = await request.get(model.preview_url);
    expect(preview.status()).toBe(200);
    expect((await preview.body()).subarray(0,4).toString()).toBe('glTF');
    expect(Number(sql('SELECT count(*) FROM private_models'))).toBe(1);
  } finally {
    await test.info().attach('quota-admission-evidence', { body: JSON.stringify(observations), contentType: 'application/json' });
  }
  for (const scenario of cases) {
    const observed = observations.find(item => item.name === scenario.name)!;
    expect(observed.directoriesTouched, scenario.name).toBe(scenario.beforeWork ? 0 : 1);
  }
});
