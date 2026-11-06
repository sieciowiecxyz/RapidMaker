import { expect, test, type APIRequestContext } from './fixtures';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import {
  closeSync,
  lstatSync,
  openSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { resolve } from 'node:path';

test.use({ screenshot: 'off', trace: 'off', video: 'off' });
const origin = 'http://localhost:18437';
const source = readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
const digest = (bytes: Buffer) => createHash('sha256').update(bytes).digest('hex');

test('real directory and persistence failures clean partial uploads and allow recovery', async ({
  request,
  playwright,
}, testInfo) => {
  const root = process.env.RAPIDMAKER_TEST_MODEL_ROOT ?? '';
  const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
  expect(root).toMatch(/^\/tmp\/rapidmaker-browser-[^/]+\/models$/);
  expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
  expect(realpathSync(root)).toBe(root);
  expect(lstatSync(root).isSymbolicLink()).toBe(false);
  expect(lstatSync(root).isDirectory()).toBe(true);
  const sql = (query: string) =>
    execFileSync(
      'podman',
      [
        'exec',
        process.env.TEST_PG_CONTAINER ?? 'rapidmaker-catalog-postgres',
        'psql',
        '-U',
        'rapidmaker',
        '-d',
        database,
        '-v',
        'ON_ERROR_STOP=1',
        '-At',
        '-c',
        query,
      ],
      { encoding: 'utf8', timeout: 15_000, stdio: 'pipe' },
    ).trim();
  const count = () => Number(sql('SELECT count(*) FROM private_models'));
  const directories = () => readdirSync(root).sort();
  const upload = () =>
    request.post('/api/models', {
      headers: {
        Origin: origin,
        'Content-Type': 'application/octet-stream',
        'x-model-filename': 'cube_20mm.stl',
      },
      data: source,
    });
  const files = ['canonical.stl', 'model.stl', 'preview.glb'];
  const snapshot = (id: string) => {
    expect(id).toMatch(/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/);
    const directory = resolve(root, id);
    expect(lstatSync(directory).mode & 0o777).toBe(0o700);
    expect(readdirSync(directory).sort()).toEqual(files);
    return files.map((file) => {
      const path = resolve(directory, file),
        stat = lstatSync(path);
      expect(stat.isFile()).toBe(true);
      expect(stat.mode & 0o077).toBe(0);
      return digest(readFileSync(path));
    });
  };
  const accessible = async (client: APIRequestContext, id: string) => {
    const original = await client.get(`/api/models/${id}/source`);
    expect(original.status()).toBe(200);
    expect(digest(await original.body())).toBe(digest(source));
    const preview = await client.get(`/api/models/${id}/preview.glb`);
    expect(preview.status()).toBe(200);
    const bytes = await preview.body();
    expect(bytes.subarray(0, 4).toString()).toBe('glTF');
    expect(bytes.readUInt32LE(4)).toBe(2);
    expect(bytes.readUInt32LE(8)).toBe(bytes.length);
  };
  const baseline = await upload();
  expect(baseline.status()).toBe(201);
  const { id } = await baseline.json();
  const saved = snapshot(id),
    baselineDirectories = directories(),
    baselineCount = count();
  expect(baselineDirectories).toContain(id);
  await accessible(request, id);
  const outsider = await playwright.request.newContext({ baseURL: origin });
  try {
    for (const path of ['source', 'preview.glb'])
      expect((await outsider.get(`/api/models/${id}/${path}`)).status()).toBe(401);
  } finally {
    await outsider.dispose();
  }

  // ENOTDIR is a real filesystem failure; this does not simulate or claim ENOSPC.
  const holding = `${root}-holding-${randomUUID()}`;
  let blocked = false,
    directoryStatus: number;
  renameSync(root, holding);
  try {
    const fd = openSync(root, 'wx', 0o600);
    blocked = true;
    try {
      writeFileSync(fd, 'isolated upload failure');
    } finally {
      closeSync(fd);
    }
    directoryStatus = (await upload()).status();
  } finally {
    if (blocked) unlinkSync(root);
    renameSync(holding, root);
  }
  expect(directoryStatus).toBe(500);
  expect(count()).toBe(baselineCount);
  expect(directories()).toEqual(baselineDirectories);
  expect(snapshot(id)).toEqual(saved);

  // Persistence is reached only after source, canonical STL and GLB are generated.
  let persistenceStatus: number;
  try {
    sql(`CREATE FUNCTION browser_reject_model() RETURNS trigger LANGUAGE plpgsql AS $$
      BEGIN RAISE EXCEPTION 'isolated upload persistence failure'; END $$;
      CREATE TRIGGER browser_reject_model BEFORE INSERT ON private_models
      FOR EACH ROW EXECUTE FUNCTION browser_reject_model();`);
    persistenceStatus = (await upload()).status();
  } finally {
    sql(
      'DROP TRIGGER IF EXISTS browser_reject_model ON private_models; DROP FUNCTION IF EXISTS browser_reject_model();',
    );
  }
  expect(persistenceStatus).toBe(503);
  expect(count()).toBe(baselineCount);
  expect(directories()).toEqual(baselineDirectories);
  expect(snapshot(id)).toEqual(saved);
  await accessible(request, id);

  const recovered = await upload();
  expect(recovered.status()).toBe(201);
  const recoveryId = (await recovered.json()).id;
  expect(recoveryId).not.toBe(id);
  snapshot(recoveryId);
  await accessible(request, recoveryId);
  await accessible(request, id);
  expect(snapshot(id)).toEqual(saved);
  expect(directories()).toEqual([...baselineDirectories, recoveryId].sort());
  expect(count()).toBe(baselineCount + 1);
  await testInfo.attach('upload-failure-statuses', {
    body: JSON.stringify({
      baseline: 201,
      directory: directoryStatus,
      persistence: persistenceStatus,
      recovery: 201,
      addedModelsAfterBaseline: 1,
    }),
    contentType: 'application/json',
  });
});
