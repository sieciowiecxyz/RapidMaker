import { expect, test, type Page } from './fixtures';

// Authored examples, deliberately independent of renderer source and uploaded meshes.
const examples = [
  ['pla', 'PLA', 'Figurka szachowa'],
  ['petg', 'PETG', 'Zębatka'],
  ['asa', 'ASA', 'Obudowa'],
  ['pa', 'PA-CF', 'Ramię wahacza'],
  ['tpu', 'TPU', 'Mieszek elastyczny'],
] as const;

async function silhouette(page: Page) {
  const canvas = page.locator('.print-scene canvas');
  const live = await canvas.evaluate((node: HTMLCanvasElement) => {
    const gl = node.getContext('webgl');
    const program = gl?.getParameter(gl.CURRENT_PROGRAM);
    return (
      !!gl && !gl.isContextLost() && !!program && gl.getProgramParameter(program, gl.LINK_STATUS)
    );
  });
  expect(live, 'The visible canvas has a live, linked WebGL renderer').toBe(true);
  // Screenshot pixels work with preserveDrawingBuffer=false. Magenta distinguishes the
  // transparent canvas and black ground shadow from the neutral, opaque black model.
  // Screenshot-only CSS excludes labels/watermarks: text cannot make a blank mesh pass.
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
    const pixels = context.getImageData(0, 0, 96, 96).data;
    return Array.from({ length: 96 * 96 }, (_, index) => {
      const [r, g, b] = pixels.slice(index * 4, index * 4 + 3);
      return g > 5 && Math.max(r, g, b) - Math.min(r, g, b) < 45 ? 1 : 0;
    });
  }, png.toString('base64'));
}

for (const width of [1440, 390]) {
  test(`all five procedural showcase meshes render in both themes at ${width}px`, async ({
    page,
  }, info) => {
    test.setTimeout(180_000);
    await page.setViewportSize({ width, height: 900 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    const failedAssets: string[] = [];
    const staticTypes = ['image', 'font', 'stylesheet', 'script'];
    page.on('response', (response) => {
      if (staticTypes.includes(response.request().resourceType()) && response.status() >= 400)
        failedAssets.push(`${response.status()} ${new URL(response.url()).pathname}`);
    });
    page.on('requestfailed', (request) => {
      if (staticTypes.includes(request.resourceType()))
        failedAssets.push(new URL(request.url()).pathname);
    });
    await page.goto('/wycena');
    await page
      .getByRole('button', { name: 'Nie masz pliku? Wypróbuj przykład', exact: true })
      .click();
    await expect(
      page.getByRole('button', { name: 'Konfiguruj uchwyt-montazowy.stl', exact: true }),
    ).toBeVisible();
    for (const theme of ['light', 'dark']) {
      if ((await page.locator('html').getAttribute('data-theme')) !== theme)
        await page
          .getByRole('button', {
            name: theme === 'light' ? 'Zmień na jasny motyw' : 'Zmień na ciemny motyw',
            exact: true,
          })
          .click();
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
      const masks: number[][] = [];
      for (const [material, label, name] of examples) {
        await test.step(`${theme}: ${label} / ${name}`, async () => {
          await page.locator(`input[name=material][value=${material}]`).check();
          await page.locator('input[name=color][value=black]').check();
          await page.locator('input[name=quality][value=standard]').check();
          await page.locator('input[name=strength][value=standard]').check();
          await expect(page.locator(`input[name=material][value=${material}]`)).toBeChecked();
          await expect(page.locator('input[name=color][value=black]')).toBeChecked();
          const scene = page.locator('.print-scene');
          await expect(scene.locator('.chamber-visual')).toHaveAccessibleName(
            `Pokazowa część: ${name}, ${label}, Czarny, jakość standardowa. Nie przedstawia wgranego pliku.`,
          );
          await expect(scene.locator('.chamber-callout')).toHaveText(name);
          await expect(scene.locator('.viewer-fallback')).toHaveCount(0);
          await expect(scene.locator('.scene-caption')).toContainText('Obiekt pokazowy.');
          await expect(
            scene.getByRole('button', { name: 'Odtwórz animację druku' }),
          ).toBeDisabled();
          await scene.scrollIntoViewIfNeeded();
          let mask: number[] = [];
          await expect
            .poll(
              async () => {
                mask = await silhouette(page);
                return (
                  mask.reduce((sum, pixel) => sum + pixel, 0) > 60 &&
                  masks.every(
                    (previous) =>
                      mask.filter((pixel, index) => pixel !== previous[index]).length > 20,
                  )
                );
              },
              { message: `${label}: visible model and changed silhouette` },
            )
            .toBe(true);
          masks.push(mask);
          await scene.screenshot({ path: info.outputPath(`${width}-${theme}-${material}.png`) });
          for (const image of await page.locator('img:visible').all()) {
            await image.scrollIntoViewIfNeeded();
            await expect
              .poll(() =>
                image.evaluate(
                  (node: HTMLImageElement) =>
                    node.complete && node.naturalWidth > 0 && node.naturalHeight > 0,
                ),
              )
              .toBe(true);
          }
          await page.evaluate(() => document.fonts.ready);
          const fonts = await page.evaluate(() => ({
            body: getComputedStyle(document.body).fontFamily,
            heading: getComputedStyle(document.querySelector('h1')!).fontFamily,
            loaded: [...document.fonts]
              .filter((font) => font.status === 'loaded')
              .map((font) => font.family),
          }));
          expect(fonts.body).toContain('Manrope');
          expect(fonts.heading).toContain('Oxanium');
          for (const family of ['Manrope', 'Oxanium'])
            expect(fonts.loaded.some((font) => font.includes(family))).toBe(true);
          expect(
            await page.evaluate(() => document.documentElement.scrollWidth),
          ).toBeLessThanOrEqual(width);
          expect(failedAssets).toEqual([]);
        });
      }
    }
  });
}
