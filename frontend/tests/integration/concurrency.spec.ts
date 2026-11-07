import { expect, test } from './fixtures';
import { readFileSync, readdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { execFileSync } from 'node:child_process';

// Linux-only evidence for this runner-owned API. RSS sums may double-count shared pages;
// sampled peaks are observations, not a hard memory bound or simultaneous HWM sums.
function resourceMeter() {
  const pid = Number(process.env.RAPIDMAKER_TEST_API_PID);
  if (!Number.isSafeInteger(pid) || pid <= 0) throw new Error('Missing runner-owned API PID');
  const begin = Date.now();
  let phase = 'baseline_before_uploads';
  let identity: string | undefined;
  const evidence = {
    pid,
    interval_ms: 100,
    started_at: new Date(begin).toISOString(),
    identity_before: '',
    identity_after: '',
    sample_count: 0,
    elapsed_ms: 0,
    transient_missing_reads: 0,
    read_errors: [] as string[],
    note: 'KiB; tree RSS includes API; shared pages may be double-counted; sampled peak is not a hard bound; HWM is API-only',
    phases: {} as Record<string, Record<string, number>>,
  };
  const proc = <T>(path: string, required: boolean, operation: (path: string) => T) => {
    try {
      return operation(path);
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (!required && ['ENOENT', 'ESRCH'].includes(code ?? '')) {
        evidence.transient_missing_reads++;
        return undefined;
      }
      evidence.read_errors.push(`${path}: ${code ?? 'unknown'}`);
      throw new Error(`Resource measurement failed: ${path}: ${code}`);
    }
  };
  const read = (path: string, required: boolean) =>
    proc(path, required, (p) => readFileSync(p, 'utf8'));
  const sample = () => {
    const stat = read(`/proc/${pid}/stat`, true)!;
    const start = stat.slice(stat.lastIndexOf(')') + 2).split(' ')[19];
    if (!start || !/^\d+$/.test(start)) throw new Error('Invalid API stat starttime');
    identity ??= start;
    evidence.identity_before = identity;
    evidence.identity_after = start;
    if (start !== identity) throw new Error('Runner API process identity changed');
    const seen = new Set<number>([pid]);
    let apiRss = 0,
      apiHwm = 0,
      treeRss = 0,
      rssRead = 0;
    for (const child of seen) {
      const status = read(`/proc/${child}/status`, child === pid);
      if (status === undefined) continue;
      const rss = status.match(/^VmRSS:\s+(\d+) kB$/m);
      if (rss) {
        treeRss += Number(rss[1]);
        rssRead++;
      } else if (child === pid) throw new Error('API VmRSS missing');
      else evidence.transient_missing_reads++; // Exited/zombie children have no RSS field.
      if (child === pid) {
        apiRss = Number(rss![1]);
        const hwm = status.match(/^VmHWM:\s+(\d+) kB$/m);
        if (!hwm) throw new Error('API VmHWM missing');
        apiHwm = Number(hwm[1]);
      }
      const tids = proc(`/proc/${child}/task`, child === pid, (p) => readdirSync(p)) ?? [];
      for (const tid of tids) {
        const children = read(`/proc/${child}/task/${tid}/children`, false);
        for (const value of children?.trim().split(/\s+/) ?? []) if (value) seen.add(Number(value));
      }
    }
    const elapsed = Date.now() - begin;
    const values = {
      api_rss_kib: apiRss,
      api_hwm_kib: apiHwm,
      tree_rss_kib: treeRss,
      descendants: seen.size - 1,
      rss_read_count: rssRead,
    };
    const previous = evidence.phases[phase];
    const summary = previous ?? { samples: 0, first_ms: elapsed, last_ms: elapsed, max_gap_ms: 0 };
    summary.max_gap_ms = Math.max(summary.max_gap_ms, elapsed - summary.last_ms);
    summary.last_ms = elapsed;
    summary.samples++;
    for (const [key, value] of Object.entries(values))
      summary[key + '_max'] = Math.max(summary[key + '_max'] ?? 0, value);
    evidence.phases[phase] = summary;
    evidence.sample_count++;
    evidence.elapsed_ms = elapsed;
    return { elapsed_ms: elapsed, ...values };
  };
  return {
    evidence,
    sample,
    phase: (value: string) => {
      phase = value;
    },
  };
}

function fingerprints(pid: number) {
  return Object.fromEntries(
    Object.entries({
      binary: `/proc/${pid}/exe`,
      spec: resolve('tests/integration/concurrency.spec.ts'),
      runner: resolve('tests/run-integration.py'),
    }).map(([name, path]) => [name, createHash('sha256').update(readFileSync(path)).digest('hex')]),
  );
}

test('concurrent guests deduplicate their own slicer jobs without sharing models or prices', async ({
  playwright,
}, testInfo) => {
  test.setTimeout(240_000);
  const clients = await Promise.all(
    Array.from({ length: 8 }, () =>
      playwright.request.newContext({ baseURL: 'http://localhost:18437' }),
    ),
  );
  const headers = { Origin: 'http://localhost:18437' };
  const samples: { elapsed_ms: number; latency_ms: number; status: number; running: number }[] = [];
  let stopProbe = false;
  let probe: Promise<void> | undefined;
  let probeFailure: unknown;
  let started = 0;
  let acceptanceMs = 0;
  let completedMs = 0;
  let deduplicatedJobs = 0;
  let resources: ReturnType<typeof resourceMeter> | undefined;
  let resourceTimer: ReturnType<typeof setInterval> | undefined;
  let resourceFailure: unknown;
  let baseline, recovery;
  let hashesBefore, hashesAfter;
  let recoveryProbeStatus: number | undefined;
  try {
    resources = resourceMeter();
    hashesBefore = fingerprints(resources.evidence.pid);
    baseline = resources.sample();
    resourceTimer = setInterval(() => {
      try {
        resources!.sample();
      } catch (error) {
        resourceFailure = error;
        clearInterval(resourceTimer);
      }
    }, 100);
    resources.phase('uploads');
    const models: { id: string; preview_url: string }[] = [];
    // Upload sequentially: this test exercises the quote queue, not conversion admission limits.
    for (const client of clients) {
      const uploaded = await client.post('/api/models', {
        headers: {
          ...headers,
          'Content-Type': 'application/octet-stream',
          'x-model-filename': 'cube_20mm.stl',
        },
        data: readFileSync(resolve('../backend/crates/geometry/tests/fixtures/cube_20mm.stl')),
      });
      expect(uploaded.status()).toBe(201);
      models.push(await uploaded.json());
    }
    expect(new Set(models.map((model) => model.id)).size).toBe(clients.length);
    for (const model of models)
      expect(model.id).toMatch(/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/);
    const modelIds = models.map((model) => `'${model.id}'`).join(',');
    const ownJobs = `FROM slicer_jobs WHERE model_id IN (${modelIds})`;
    const catalog = await clients[0].get('/api/catalog/materials');
    expect(catalog.status()).toBe(200);
    const material = (await catalog.json()).materials.find(
      (value: { id: string }) => value.id === 'GFA00',
    );
    expect(material?.colors.length).toBeGreaterThan(0);
    const input = (index: number, modelId = models[index].id) => ({
      model_id: modelId,
      material_id: material.id,
      color_code: material.colors[0].code,
      options: { quantity: index + 1, quality: 'standard', wall_loops: 3, infill_percent: 20 },
    });
    const database = process.env.RAPIDMAKER_TEST_DATABASE ?? '';
    expect(database).toMatch(/^rapidmaker_test_browser_[a-f0-9]+$/);
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
          '-At',
          '-v',
          'ON_ERROR_STOP=1',
          '-c',
          query,
        ],
        { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], timeout: 15_000 },
      ).trim();
    resources.phase('jobs');
    started = Date.now();
    const pending = clients.map((client, index) =>
      Promise.all(
        Array.from({ length: 2 }, () =>
          client.post('/api/quotes/estimate', { headers, data: input(index) }),
        ),
      ),
    );
    const heartbeat = Date.now();
    expect((await clients[0].get('/api/catalog/materials')).status()).toBe(200);
    expect(Date.now() - heartbeat).toBeLessThan(2000);
    const pairs = await Promise.all(pending);
    acceptanceMs = Date.now() - started;
    expect(acceptanceMs).toBeLessThan(5000);
    const jobs = [];
    for (const pair of pairs) {
      expect(pair.map((response) => response.status())).toEqual([202, 202]);
      const [first, duplicate] = await Promise.all(pair.map((response) => response.json()));
      expect(duplicate.id).toBe(first.id);
      jobs.push(first);
    }
    deduplicatedJobs = new Set(jobs.map((job) => job.id)).size;
    expect(deduplicatedJobs).toBe(clients.length);
    expect(jobs.some((job) => ['queued', 'running'].includes(job.status))).toBe(true);

    await expect
      .poll(() => Number(sql(`SELECT count(*) ${ownJobs} AND status='running'`)), {
        timeout: 10_000,
        intervals: [100],
      })
      .toBe(1);
    // Sample the cohort until all real jobs finish; the single worker is bounded in DB.
    probe = (async () => {
      while (!stopProbe) {
        const heartbeat = Date.now();
        const response = await clients[0].get('/api/catalog/materials');
        const latency = Date.now() - heartbeat;
        samples.push({
          elapsed_ms: Date.now() - started,
          latency_ms: latency,
          status: response.status(),
          running: Number(sql(`SELECT count(*) ${ownJobs} AND status='running'`)),
        });
        if (!stopProbe) await new Promise((resolve) => setTimeout(resolve, 250));
      }
    })().catch((error: unknown) => {
      probeFailure = error;
    });
    const results = await Promise.all(
      clients.map(async (client, index) => {
        const foreign = (index + 1) % clients.length;
        for (const path of [
          `/api/models/${models[foreign].id}`,
          models[foreign].preview_url,
          jobs[foreign].poll_url,
        ])
          expect((await client.get(path)).status()).toBe(404);
        expect(
          (
            await client.post('/api/quotes/estimate', {
              headers,
              data: input(index, models[foreign].id),
            })
          ).status(),
        ).toBe(404);
        let job = jobs[index];
        await expect
          .poll(
            async () => {
              const response = await client.get(job.poll_url);
              expect(response.status()).toBe(200);
              job = await response.json();
              return job.status;
            },
            { timeout: 180_000, intervals: [250, 1000] },
          )
          .toMatch(/^(succeeded|failed)$/);
        expect(job.status, job.message ?? 'Real slicer must complete the owned job').toBe(
          'succeeded',
        );
        expect(job.result).toMatchObject({
          id: jobs[index].id,
          model_id: models[index].id,
          quantity: index + 1,
          currency: 'PLN',
          options: input(index).options,
        });
        expect(job.result.measurement.quantity).toBe(index + 1);
        expect(job.result.measurement.time_seconds).toBeGreaterThan(0);
        expect(job.result.measurement.filament_grams).toBeGreaterThan(0);
        for (const amount of Object.values(job.result.price)) {
          expect(Number.isSafeInteger(amount)).toBe(true);
          expect(amount).toBeGreaterThanOrEqual(0);
        }
        expect(job.result.price.gross_grosze).toBeGreaterThan(0);
        return job.result;
      }),
    );
    completedMs = Date.now() - started;
    resources.sample();
    resources.phase('recovery');
    stopProbe = true;
    await probe;
    expect(probeFailure).toBeUndefined();
    expect(samples.some((sample) => sample.running === 1)).toBe(true);
    for (const sample of samples) {
      expect(sample.status).toBe(200);
      expect(sample.latency_ms).toBeLessThan(2000);
      expect(sample.running).toBeGreaterThanOrEqual(0);
      expect(sample.running).toBeLessThanOrEqual(1);
    }
    // Every foreign snapshot now exists: rejection must prove ownership, not a pending result.
    for (const [index, client] of clients.entries()) {
      const foreign = (index + 1) % clients.length;
      const checkout = (quoteId: string, modelIndex = index) =>
        client.post('/api/checkout/quote', {
          headers,
          data: {
            delivery: 'pickup',
            parts: [
              {
                id: models[modelIndex].id,
                analysis_id: models[modelIndex].id,
                quantity: modelIndex + 1,
                quote_id: quoteId,
              },
            ],
          },
        });
      expect((await checkout(jobs[foreign].id, foreign)).status()).toBe(409);
      const owned = await checkout(results[index].id);
      expect(owned.status()).toBe(200);
      expect((await owned.json()).print_grosze).toBe(results[index].price.gross_grosze);
      const cached = await client.post('/api/quotes/estimate', { headers, data: input(index) });
      expect(cached.status()).toBe(202);
      expect(await cached.json()).toMatchObject({
        id: jobs[index].id,
        status: 'succeeded',
        result: results[index],
      });
    }
    expect(
      sql(
        `SELECT (SELECT count(*) ${ownJobs}), (SELECT count(*) FROM quotes WHERE request->>'model_id' IN (${modelIds})), max(attempts) ${ownJobs}`,
      ),
    ).toBe(`${clients.length}|${clients.length}|1`);
    await expect
      .poll(
        () => {
          recovery = resources!.sample();
          return recovery.descendants;
        },
        { timeout: 10_000, intervals: [100, 250] },
      )
      .toBe(0);
    recoveryProbeStatus = (await clients[0].get('/api/catalog/materials')).status();
    expect(recoveryProbeStatus).toBe(200);
    hashesAfter = fingerprints(resources.evidence.pid);
    expect(hashesAfter).toEqual(hashesBefore);
    expect(resources.evidence.identity_after).toBe(resources.evidence.identity_before);
    expect(resourceFailure).toBeUndefined();
    expect(resources.evidence.sample_count).toBeGreaterThan(1);
  } finally {
    clearInterval(resourceTimer);
    stopProbe = true;
    await probe;
    const latencies = samples.map((sample) => sample.latency_ms).sort((a, b) => a - b);
    try {
      await testInfo.attach('eight-client-queue-evidence', {
        contentType: 'application/json',
        body: JSON.stringify({
          resources: {
            ...resources?.evidence,
            baseline,
            recovery,
            recovery_probe_status: recoveryProbeStatus,
            failure: resourceFailure instanceof Error ? resourceFailure.message : resourceFailure,
            sha256_before: hashesBefore,
            sha256_after: hashesAfter,
          },
          clients: clients.length,
          estimate_requests: clients.length * 2,
          deduplicated_jobs: deduplicatedJobs,
          acceptance_ms: acceptanceMs,
          completion_ms: completedMs,
          max_heartbeat_ms: latencies.at(-1) ?? null,
          p95_heartbeat_ms: latencies[Math.ceil(latencies.length * 0.95) - 1] ?? null,
          max_sampled_running: samples.length
            ? Math.max(...samples.map((sample) => sample.running))
            : null,
          probe_failed: probeFailure !== undefined,
          samples,
        }),
      });
    } finally {
      await Promise.all(clients.map((client) => client.dispose()));
    }
  }
});
