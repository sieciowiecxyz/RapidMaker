import { expect, test } from './fixtures';
import { request as httpRequest } from 'node:http';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
test.use({ screenshot: 'off', trace: 'off', video: 'off' });
for (const phase of ['INSERT', 'COMMIT'] as const)
  test(`an HTTP disconnect during ${phase} keeps model rows and files coherent`, async ({
    request,
  }) => {
    const database = process.env.RAPIDMAKER_TEST_DATABASE!,
      root = process.env.RAPIDMAKER_TEST_MODEL_ROOT!;
    expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
    expect(root).toMatch(/^\/tmp\/rapidmaker-browser-[^/]+\/models$/);
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
    const source = readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl'));
    const guest = randomUUID().replaceAll('-', '') + randomUUID().replaceAll('-', '');
    const owner = createHash('sha256').update(guest).digest('hex');
    const origin = 'http://localhost:18437';
    const directories = readdirSync(root).sort();
    sql(`CREATE FUNCTION browser_hold_commit() RETURNS trigger LANGUAGE plpgsql AS $$
    BEGIN PERFORM pg_sleep(2); RETURN NEW; END $$;
    ${
      phase === 'COMMIT'
        ? 'CREATE CONSTRAINT TRIGGER browser_hold_commit AFTER INSERT ON private_models DEFERRABLE INITIALLY DEFERRED'
        : 'CREATE TRIGGER browser_hold_commit BEFORE INSERT ON private_models'
    }
    FOR EACH ROW EXECUTE FUNCTION browser_hold_commit();`);
    const pending = httpRequest('http://127.0.0.1:18436/api/models', {
      method: 'POST',
      headers: {
        Host: 'localhost:18437',
        Origin: origin,
        'x-model-filename': 'disconnected.stl',
        'Content-Type': 'application/octet-stream',
        'Content-Length': source.length,
        Cookie: 'rapidmaker_guest=' + guest,
      },
    });
    pending.on('error', () => {});
    pending.on('response', (response) => response.resume());
    try {
      pending.end(source);
      const blocked = () =>
        Number(
          sql(
            `SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND ${phase === 'COMMIT' ? "query='COMMIT'" : "query LIKE 'INSERT INTO private_models%'"} AND wait_event='PgSleep'`,
          ),
        );
      await expect.poll(blocked, { timeout: 10_000, intervals: [20, 30, 50] }).toBe(1);
      const added = readdirSync(root).filter((id) => !directories.includes(id));
      expect(added).toHaveLength(1);
      const id = added[0];
      expect(id).toMatch(/^[a-f0-9-]{36}$/);
      const before = ['model.stl', 'canonical.stl', 'preview.glb'].map((file) =>
        createHash('sha256')
          .update(readFileSync(resolve(root, id, file)))
          .digest('hex'),
      );
      pending.destroy();
      await expect.poll(blocked, { timeout: 10_000, intervals: [50, 100] }).toBe(0);
      const count = () =>
        Number(
          sql(`SELECT count(*) FROM private_models WHERE id='${id}' AND owner_hash='${owner}'`),
        );
      await expect.poll(count).toBe(phase === 'COMMIT' ? 1 : 0);
      const rows = count();
      const present = existsSync(resolve(root, id));
      await test.info().attach('disconnect-commit-evidence', {
        body: JSON.stringify({
          phase,
          databasePhaseObserved: true,
          clientDisconnected: true,
          rows,
          directoryPresent: present,
          coherent: (rows === 1) === present,
        }),
        contentType: 'application/json',
      });
      expect(present).toBe(phase === 'COMMIT');
      if (rows) {
        expect(present, 'Committed model must keep all files after client disconnect').toBe(true);
        expect(
          ['model.stl', 'canonical.stl', 'preview.glb'].map((file) =>
            createHash('sha256')
              .update(readFileSync(resolve(root, id, file)))
              .digest('hex'),
          ),
        ).toEqual(before);
        const metadata = await request.get('/api/models/' + id, {
          headers: { Cookie: 'rapidmaker_guest=' + guest },
        });
        expect(metadata.status()).toBe(200);
      }
    } finally {
      pending.destroy();
      sql(
        'DROP TRIGGER IF EXISTS browser_hold_commit ON private_models; DROP FUNCTION IF EXISTS browser_hold_commit();',
      );
    }
  });
