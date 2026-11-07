import { expect, test } from './fixtures';
import {
  constants,
  copyFileSync,
  lstatSync,
  readFileSync,
  readlinkSync,
  realpathSync,
  renameSync,
  statSync,
  unlinkSync,
} from 'node:fs';
import { createHash } from 'node:crypto';
import { basename, dirname, join, resolve } from 'node:path';

test('binary replacement preserves native worker uploads and private previews', async ({
  request,
  playwright,
}, testInfo) => {
  const modelRoot = process.env.RAPIDMAKER_TEST_MODEL_ROOT!;
  const build = process.env.RAPIDMAKER_TEST_API_BUILD_BINARY!;
  const pid = Number(process.env.RAPIDMAKER_TEST_API_PID);
  const folder = dirname(modelRoot);
  expect(Number.isSafeInteger(pid) && pid > 1).toBe(true);
  expect(dirname(folder)).toBe('/tmp');
  expect(basename(folder)).toMatch(/^rapidmaker-browser-/);
  expect(realpathSync(folder)).toBe(folder);
  expect(realpathSync(modelRoot)).toBe(modelRoot);
  expect(lstatSync(folder).uid).toBe(process.getuid!());
  expect(lstatSync(folder).mode & 0o077).toBe(0);
  expect(build).toBe(join(folder, 'rapidmaker-api-build'));
  expect(realpathSync(build)).toBe(build);
  expect(lstatSync(build).isFile()).toBe(true);
  const hash = (path: string) => createHash('sha256').update(readFileSync(path)).digest('hex');
  const processState = () => {
    const proc = `/proc/${pid}`;
    const fields = readFileSync(`${proc}/stat`, 'utf8')
      .split(/\)(.*)/s)[1]
      .trim()
      .split(/\s+/);
    const exe = `${proc}/exe`;
    return {
      pid,
      start: fields[19],
      exe: readlinkSync(exe),
      inode: statSync(exe).ino,
      sha256: hash(exe),
    };
  };
  const before = processState();
  const buildBefore = statSync(build).ino;
  const bytes = readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
  const upload = () =>
    request.post('/api/models', {
      headers: {
        Origin: 'http://localhost:18437',
        'Content-Type': 'application/octet-stream',
        'x-model-filename': 'cube_20mm.stl',
      },
      data: bytes,
    });
  const evidence: Record<string, unknown> = { before, buildBefore };
  const replacement = join(folder, 'rapidmaker-api-build-replacement');
  let replacementCreated = false;
  try {
    const first = await upload();
    evidence.firstStatus = first.status();
    expect(first.status()).toBe(201);
    const firstModel = await first.json();
    copyFileSync(build, replacement, constants.COPYFILE_EXCL);
    replacementCreated = true;
    expect(hash(replacement)).toBe(before.sha256);
    renameSync(replacement, build);
    replacementCreated = false;
    evidence.buildAfter = statSync(build).ino;
    expect(statSync(build).ino).not.toBe(buildBefore);
    expect(processState().start).toBe(before.start);
    const second = await upload();
    evidence.secondStatus = second.status();
    evidence.after = processState();
    expect(second.status()).toBe(201);
    const secondModel = await second.json();
    expect(processState()).toEqual(before);
    expect(before.exe).toMatch(new RegExp(`^${folder}/runtime/pin-[^/]+/rapidmaker-api$`));
    const foreign = await playwright.request.newContext({ baseURL: 'http://localhost:18437' });
    try {
      // Establish a distinct real guest owner; an absent session correctly returns 401.
      const foreignUpload = await foreign.post('/api/models', {
        headers: {
          Origin: 'http://localhost:18437',
          'Content-Type': 'application/octet-stream',
          'x-model-filename': 'cube_20mm.stl',
        },
        data: bytes,
      });
      expect(foreignUpload.status()).toBe(201);
      for (const model of [firstModel, secondModel]) {
        const path = `/api/models/${model.id}/preview.glb`;
        const preview = await request.get(path);
        expect(preview.status()).toBe(200);
        expect((await preview.body()).subarray(0, 4).toString()).toBe('glTF');
        expect((await foreign.get(path)).status()).toBe(404);
      }
      evidence.previews = 'both owner 200 GLB / foreign 404';
    } finally {
      await foreign.dispose();
    }
  } finally {
    if (replacementCreated) unlinkSyncIfPresent(replacement);
    await testInfo.attach('binary-replacement-evidence', {
      body: JSON.stringify(evidence, null, 2),
      contentType: 'application/json',
    });
  }
});

function unlinkSyncIfPresent(path: string) {
  try {
    unlinkSync(path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
  }
}
