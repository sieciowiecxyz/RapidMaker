import { expect, test, type Page } from './fixtures';
import { createHash } from 'node:crypto';
import { readFileSync, realpathSync, statSync } from 'node:fs';
import { basename, dirname, resolve, sep } from 'node:path';

type ModelCase = { case: number; file: string; sha256: string; size_bytes: number };
// Private, opt-in downloads: no manifest means no additional default tests.
const manifest = process.env.E2E_CORPUS_MANIFEST;
const cases: ModelCase[] = manifest ? JSON.parse(readFileSync(manifest, 'utf8')) : [];
if (manifest && (cases.length !== 41 || new Set(cases.map((item) => item.case)).size !== 41))
  throw new Error('Corpus GUI requires the 41 accepted, unique cases.');
const selected = cases.filter((item) => [5, 9, 22, 25].includes(item.case));
if (
  manifest &&
  selected
    .map((item) => item.case)
    .sort((a, b) => a - b)
    .join(',') !== '5,9,22,25'
)
  throw new Error('Corpus GUI requires cases 5, 9, 22 and 25 exactly once.');

function corpusFile(item: ModelCase) {
  expect(Number.isSafeInteger(item.case) && item.case > 0).toBe(true);
  expect(Number.isSafeInteger(item.size_bytes) && item.size_bytes > 0).toBe(true);
  const root = realpathSync(dirname(resolve(manifest!)));
  const path = realpathSync(resolve(root, item.file));
  expect(path.startsWith(root + sep), 'Corpus file stays inside manifest directory').toBe(true);
  expect(path.toLowerCase()).toMatch(/\.(3mf|stl)$/);
  expect(statSync(path).isFile()).toBe(true);
  expect(statSync(path).size).toBeLessThanOrEqual(128 * 1024 * 1024);
  const bytes = readFileSync(path);
  expect(bytes.length).toBe(item.size_bytes);
  expect(createHash('sha256').update(bytes).digest('hex')).toBe(item.sha256);
  return path;
}

async function silhouette(page: Page) {
  const canvas = page.locator('.print-scene canvas');
  expect(
    await canvas.evaluate((node: HTMLCanvasElement) => {
      const gl = node.getContext('webgl');
      const program = gl?.getParameter(gl.CURRENT_PROGRAM);
      return (
        !!gl && !gl.isContextLost() && !!program && gl.getProgramParameter(program, gl.LINK_STATUS)
      );
    }),
    'Live, linked WebGL renderer',
  ).toBe(true);
  // Only this pixel probe changes CSS. Magenta excludes transparency and the black
  // ground shadow; canvas-only pixels and hidden overlays exclude text as evidence.
  const png = await canvas.screenshot({
    animations: 'disabled',
    style:
      '.chamber canvas { background: #ff00ff !important; } .chamber-watermark, .chamber-callout { visibility: hidden !important; }',
  });
  return page.evaluate(async (base64) => {
    const image = new Image();
    image.src = `data:image/png;base64,${base64}`;
    await image.decode();
    const probe = document.createElement('canvas');
    probe.width = probe.height = 96;
    const context = probe.getContext('2d')!;
    context.drawImage(image, 0, 0, 96, 96);
    const rgba = context.getImageData(0, 0, 96, 96).data;
    return Array.from({ length: 96 * 96 }, (_, index) => {
      const [r, g, b, a] = rgba.slice(index * 4, index * 4 + 4);
      return a > 240 && g > 5 && Math.max(r, g, b) - Math.min(r, g, b) < 45 ? 1 : 0;
    });
  }, png.toString('base64'));
}

for (const item of selected) {
  test(`corpus GUI ${item.case}: large 3MF, real upload and interactive mesh`, async ({
    page,
    context,
    browser,
  }, info) => {
    test.setTimeout(300_000);
    const path = corpusFile(item);
    expect(path.toLowerCase()).toMatch(/\.3mf$/);
    expect(statSync(path).size).toBeLessThanOrEqual(25 * 1024 * 1024);
    const name = basename(path);
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto('/wycena');
    const uploaded = page.waitForResponse(
      (r) => new URL(r.url()).pathname === '/api/models' && r.request().method() === 'POST',
      { timeout: 90_000 },
    );
    const previewed = page.waitForResponse(
      (r) => /\/api\/models\/[^/]+\/preview\.glb$/.test(new URL(r.url()).pathname),
      { timeout: 120_000 },
    );
    await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles(path);
    const upload = await uploaded;
    expect(upload.status()).toBe(201);
    expect((await previewed).status()).toBe(200);
    await expect
      .poll(() =>
        page.evaluate(() => {
          const draft = JSON.parse(
            localStorage.getItem('rapidmaker.storefront.draft.v1') ?? 'null',
          );
          return draft?.models?.[0]?.analysis?.id;
        }),
      )
      .toBeTruthy();
    // Large WebGL work can evict browser response bodies; use the actual saved UI ID.
    const id = await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1')!).models[0].analysis.id,
    );
    const model = await (await context.request.get(`/api/models/${encodeURIComponent(id)}`)).json();
    expect(model.filename).toBe(name);
    const previewUrl = new URL(
      `/api/models/${encodeURIComponent(model.id)}/preview.glb`,
      page.url(),
    ).href;
    const preview = await context.request.get(previewUrl);
    expect(preview.status()).toBe(200);
    const glb = await preview.body();
    expect(glb.subarray(0, 4).toString()).toBe('glTF');
    expect(glb.readUInt32LE(8)).toBe(glb.length);
    const outsider = await browser.newContext();
    try {
      expect([401, 403, 404]).toContain((await outsider.request.get(previewUrl)).status());
    } finally {
      await outsider.close();
    }
    const scene = page.locator('.print-scene');
    await expect(scene.locator('.chamber-visual')).toHaveAccessibleName(
      `Model z pliku ${name}. Obróć przeciągając myszą. Kolor: Czarny.`,
      { timeout: 90_000 },
    );
    await expect(scene.locator('.scene-caption')).toContainText('trójkątów');
    const caption = await scene.locator('.scene-caption').innerText();
    const count = caption.split('trójkątów')[0].replace(/\D/g, '');
    expect(
      Number(count),
      'Real large mesh, without requiring degenerate triangles',
    ).toBeGreaterThan(100_000);
    await expect(scene.locator('.chamber-callout')).toHaveText(name);
    await page.locator('input[name=material][value=pla]').check();
    await page.locator('input[name=color]').first().check();
    await page.locator('input[name=quality][value=standard]').check();
    const quoted =
      item.case === 9
        ? page.waitForResponse(
            (r) => r.url().endsWith('/api/quotes/estimate') && r.request().method() === 'POST',
          )
        : undefined;
    const cartQuoted =
      item.case === 9
        ? page.waitForResponse(
            (r) =>
              new URL(r.url()).pathname === '/api/checkout/quote' &&
              r.request().method() === 'POST',
            { timeout: 180_000 },
          )
        : undefined;
    await page.locator('input[name=strength][value=standard]').check();
    if (quoted) {
      expect((await quoted).status()).toBe(202);
      const server = await cartQuoted!;
      expect(server.status()).toBe(200);
      const quote = await server.json();
      expect(Number.isSafeInteger(quote.print_grosze)).toBe(true);
      expect(quote.print_grosze).toBeGreaterThan(0);
      await expect(page.locator('.price')).toHaveText(
        new Intl.NumberFormat('pl-PL', { style: 'currency', currency: 'PLN' }).format(
          quote.print_grosze / 100,
        ),
      );
      await expect(
        page.getByRole('button', { name: 'Przejdź do zamówienia', exact: true }),
      ).toBeEnabled();
    } else {
      await expect(page.locator('.manual-heading')).toHaveText('Wymagana ręczna wycena');
      expect(model.dimensions_mm.some((axis: number) => axis > 500)).toBe(true);
      await expect(page.locator('.estimate-lines')).toContainText('model przekracza 500 mm');
      await expect(page.locator('.price')).toHaveCount(0);
    }
    for (const width of [1440, 390]) {
      await page.setViewportSize({ width, height: 900 });
      for (const theme of ['light', 'dark']) {
        if ((await page.locator('html').getAttribute('data-theme')) !== theme)
          await page
            .getByRole('button', {
              name: theme === 'light' ? 'Zmień na jasny motyw' : 'Zmień na ciemny motyw',
              exact: true,
            })
            .click();
        await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
        await scene.scrollIntoViewIfNeeded();
        await expect(scene.locator('.viewer-fallback')).toHaveCount(0);
        let before: number[] = [];
        await expect
          .poll(
            async () => {
              before = await silhouette(page);
              return before.reduce((sum, pixel) => sum + pixel, 0);
            },
            { message: 'Visible neutral mesh pixels, excluding labels and shadow' },
          )
          .toBeGreaterThan(60);
        await scene.getByRole('button', { name: 'Obróć model', exact: true }).click();
        await expect
          .poll(
            async () => {
              const after = await silhouette(page);
              expect(after.reduce<number>((sum, pixel) => sum + pixel, 0)).toBeGreaterThan(60);
              return after.filter((pixel, index) => pixel !== before[index]).length;
            },
            { message: 'Rotate changes the rendered mesh silhouette' },
          )
          .toBeGreaterThan(20);
        const filename = scene.locator('.chamber-filename');
        await expect(filename).toHaveText(name);
        await expect(filename).toHaveAttribute('title', name);
        if (item.case === 9 && width === 390) {
          await expect(filename).toHaveCSS('text-overflow', 'ellipsis');
          expect(await filename.evaluate((node) => node.scrollWidth > node.clientWidth)).toBe(true);
          expect((await filename.boundingBox())!.height).toBeLessThan(24);
        }
        // Authored UI, without the diagnostic CSS used for pixel probes above.
        await scene.screenshot({ path: info.outputPath(`${item.case}-${width}-${theme}.png`) });
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
          width,
        );
      }
    }
    info.annotations.push({
      type: 'ui-render',
      description: `Deep GUI mesh/privacy checks and four frames; ${item.case === 9 ? 'server price checked' : 'explicit manual quote above 500 mm'}`,
    });
  });
}

// Every remaining manifest entry exercises the authored form; outcomes are observed,
// not copied from the API corpus report. Rejections are UI evidence, never prices.
for (const item of cases.filter((entry) => ![5, 9, 22, 25].includes(entry.case))) {
  test(`corpus GUI ${item.case}: real form render or explicit rejection`, async ({
    page,
    context,
  }, info) => {
    test.setTimeout(120_000);
    const path = corpusFile(item),
      name = basename(path);
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.goto('/wycena');
    let posts = 0;
    const isUpload = (request: { url(): string; method(): string }) =>
      new URL(request.url()).pathname === '/api/models' && request.method() === 'POST';
    page.on('request', (request) => {
      if (isUpload(request)) posts++;
    });
    const oversized = item.size_bytes > 25 * 1024 * 1024;
    if ([24, 39].includes(item.case)) expect(oversized).toBe(false);
    const uploaded = oversized
      ? undefined
      : page.waitForResponse((response) => isUpload(response.request()), { timeout: 90_000 });
    await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles(path);
    if (oversized) {
      // The alert is written by the synchronous input handler: it has settled here.
      await expect(page.getByRole('alert')).toHaveText('Wybierz niepusty plik do 25 MB.');
      expect(posts).toBe(0);
      await expect(page.locator('.quote-contact')).toBeDisabled();
      info.annotations.push({
        type: 'ui-size-rejection',
        description: '25 MiB form limit; no upload POST',
      });
      return;
    }
    const upload = await uploaded!;
    if ([24, 39].includes(item.case)) expect(upload.status()).toBe(201);
    if (item.case === 32) expect(upload.status()).toBe(422);
    if (upload.status() === 422) {
      // Chromium can evict large upload response bodies from its inspector cache.
      // Reproduce the same rejection through the owner's HTTP client, then match the UI.
      const rejected = await context.request.post('/api/models', {
        headers: {
          Origin: 'http://localhost:18437',
          'Content-Type': 'application/octet-stream',
          'x-model-filename': encodeURIComponent(name),
        },
        data: readFileSync(path),
        timeout: 90_000,
      });
      expect(rejected.status()).toBe(422);
      const error = await rejected.json();
      // Case 32 contains an invalid 3DManufacturing:3mfVersion metadata prefix.
      // It fits the byte budget now, but must still fail strict package validation.
      if (item.case === 32) {
        expect(error.message).toBe(
          'Nie można odczytać geometrii. Wybierz poprawny STL, STEP lub 3MF.',
        );
      } else expect(error.code).toBe('model_resource_limit');
      expect(typeof error.message).toBe('string');
      expect(error.message.length).toBeGreaterThan(0);
      await expect(page.getByRole('alert')).toHaveText(error.message);
      await expect(page.getByRole('status').filter({ hasText: 'Wysyłamy model' })).toHaveCount(0);
      for (const selector of [
        'input[name=material][value=pla]',
        'input[name=color][value=black]',
        'input[name=quality][value=standard]',
        'input[name=strength][value=standard]',
      ]) {
        await page.locator(selector).check();
        await expect(page.locator(selector)).toBeChecked();
      }
      await expect(page.locator('.quote-result')).toHaveAttribute('aria-busy', 'false');
      await expect(page.locator('.quote-result .error')).toHaveText(error.message);
      await expect(page.locator('.price')).toHaveCount(0);
      await expect(page.locator('.quote-contact')).toBeDisabled();
      // Local STL rendering is covered by positive cases; this branch tests server rejection.
      // 3MF cannot invent a real mesh when the backend has rejected it.
      if (/\.3mf$/i.test(path))
        await expect(page.locator('.chamber-visual[aria-label^="Model z pliku"]')).toHaveCount(0);

      info.annotations.push({
        type: item.case === 32 ? 'ui-invalid-rejection' : 'ui-resource-rejection',
        description:
          'HTTP 422 shown; completed print settings cannot enable checkout; no price claimed',
      });
      return;
    }
    expect(upload.status()).toBe(201);
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1') ?? 'null')
              ?.models?.[0]?.analysis?.id,
        ),
      )
      .toBeTruthy();
    const id = await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem('rapidmaker.storefront.draft.v1')!).models[0].analysis.id,
    );
    const metadata = await context.request.get(`/api/models/${encodeURIComponent(id)}`);
    expect(metadata.status()).toBe(200);
    const model = await metadata.json();
    expect(model.id).toBe(id);
    expect(model.filename).toBe(name);
    expect(model.dimensions_mm).toHaveLength(3);
    expect(model.dimensions_mm.every((axis: number) => Number.isFinite(axis) && axis > 0)).toBe(
      true,
    );
    const preview = await context.request.get(`/api/models/${encodeURIComponent(id)}/preview.glb`);
    expect(preview.status()).toBe(200);
    const glb = await preview.body();
    expect(glb.subarray(0, 4).toString()).toBe('glTF');
    expect(glb.readUInt32LE(8)).toBe(glb.length);
    const scene = page.locator('.print-scene');
    await expect(scene.locator('.chamber-visual')).toHaveAccessibleName(
      `Model z pliku ${name}. Obróć przeciągając myszą. Kolor: Czarny.`,
      { timeout: 90_000 },
    );
    await expect(scene.locator('.scene-caption')).toContainText('trójkątów');
    const renderedTriangles = Number(
      (await scene.locator('.scene-caption').innerText()).split('trójkątów')[0].replace(/\D/g, ''),
    );
    expect(renderedTriangles).toBeGreaterThanOrEqual(1);
    if ([24, 39].includes(item.case)) {
      const gltf = JSON.parse(glb.subarray(20, 20 + glb.readUInt32LE(12)).toString('utf8'));
      const binary = 28 + glb.readUInt32LE(12);
      let sourceTriangles = 0,
        zeroArea = 0;
      // Independent wire oracle supports legacy u32 and compact chunk-local u16 indices.
      for (const primitive of gltf.meshes[0].primitives) {
        const positions = gltf.accessors[primitive.attributes.POSITION];
        const indices = gltf.accessors[primitive.indices];
        const positionStart =
          binary + gltf.bufferViews[positions.bufferView].byteOffset + (positions.byteOffset ?? 0);
        const indexStart =
          binary + gltf.bufferViews[indices.bufferView].byteOffset + (indices.byteOffset ?? 0);
        expect([5123, 5125]).toContain(indices.componentType);
        const indexBytes = indices.componentType === 5123 ? 2 : 4;
        sourceTriangles += indices.count / 3;
        for (let i = 0; i < indices.count / 3; i++) {
          const points = [0, 1, 2].map((corner) => {
            const address = indexStart + (i * 3 + corner) * indexBytes;
            const index = indexBytes === 2 ? glb.readUInt16LE(address) : glb.readUInt32LE(address);
            const offset = positionStart + index * 12;
            return [0, 4, 8].map((axis) => glb.readFloatLE(offset + axis));
          });
          const [a, b, c] = points;
          const u = b.map((value, axis) => value - a[axis]);
          const v = c.map((value, axis) => value - a[axis]);
          if (
            u[1] * v[2] === u[2] * v[1] &&
            u[2] * v[0] === u[0] * v[2] &&
            u[0] * v[1] === u[1] * v[0]
          )
            zeroArea++;
        }
      }
      // The canonical GLB retains zero-area faces; the viewer omits them.
      expect(renderedTriangles).toBe(sourceTriangles - zeroArea);
      info.annotations.push({
        type: 'near-byte-budget',
        description: JSON.stringify({ sourceTriangles, renderedTriangles, zeroArea }),
      });
      expect(renderedTriangles).toBeGreaterThan(1_000_000);
      expect(renderedTriangles).toBeLessThanOrEqual(1_048_574);
    }
    for (const width of [1440, 390]) {
      await page.setViewportSize({ width, height: 900 });
      for (const theme of ['light', 'dark']) {
        if ((await page.locator('html').getAttribute('data-theme')) !== theme)
          await page
            .getByRole('button', {
              name: theme === 'light' ? 'Zmień na jasny motyw' : 'Zmień na ciemny motyw',
              exact: true,
            })
            .click();
        await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
        await scene.scrollIntoViewIfNeeded();
        await expect(scene.locator('.viewer-fallback')).toHaveCount(0);
        await expect(scene.locator('.chamber-filename')).toHaveText(name);
        await expect(scene.locator('.chamber-filename')).toHaveAttribute('title', name);
        let before: number[] = [];
        await expect
          .poll(
            async () => {
              before = await silhouette(page);
              return before.reduce((sum, pixel) => sum + pixel, 0);
            },
            { message: 'Real mesh has nonblank pixels, including sparse assemblies' },
          )
          .toBeGreaterThan(0);
        await scene.getByRole('button', { name: 'Przybliż model', exact: true }).click();
        await expect
          .poll(
            async () => {
              const after = await silhouette(page);
              expect(after.some(Boolean)).toBe(true);
              return after.filter((pixel, index) => pixel !== before[index]).length;
            },
            { message: 'Zoom changes mesh pixels, including rotationally symmetric models' },
          )
          .toBeGreaterThan(0);
        await scene.getByRole('button', { name: 'Oddal model', exact: true }).click();
        await scene.screenshot({ path: info.outputPath(`${item.case}-${width}-${theme}.png`) });
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
          width,
        );
      }
    }
    info.annotations.push({
      type: 'ui-render',
      description:
        'Real upload, metadata, owner GLB, mesh and zoom; four normal scene frames; no price claim',
    });
  });
}
