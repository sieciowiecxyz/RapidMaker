import { expect, test, type Page } from './fixtures';
import { accounts, login } from './helpers';
import { checkSvgGraphics } from './svg-graphics';

const routes = [
  ['/', /Twój pomysł/i],
  ['/wycena', /Twój pomysł/i],
  ['/logowanie', 'Dobrze Cię widzieć.'],
  ['/logowanie?register=1', 'Zacznijmy od emaila.'],
  ['/regulaminy', 'Dokumenty i zasady'],
  ['/regulamin', 'Regulamin'],
  ['/prywatnosc', 'Polityka prywatności'],
] as const;

function watchAssets(page: Page) {
  const failedAssets: string[] = [];
  page.on('response', (response) => {
    if (
      ['image', 'font', 'stylesheet', 'script'].includes(response.request().resourceType()) &&
      response.status() >= 400
    )
      failedAssets.push(new URL(response.url()).pathname);
  });
  page.on('requestfailed', (request) => {
    if (['image', 'font', 'stylesheet', 'script'].includes(request.resourceType()))
      failedAssets.push(new URL(request.url()).pathname);
  });
  return failedAssets;
}

async function assetsAndLayout(page: Page, path: string, width: number) {
  await page.evaluate(() => document.fonts.ready);
  for (const image of await page.locator('img').all()) {
    if (await image.isVisible()) await image.scrollIntoViewIfNeeded();
    await expect
      .poll(
        () =>
          image.evaluate(
            (node) => node instanceof HTMLImageElement && node.complete && node.naturalWidth > 0,
          ),
        { message: `${path}: image decodes` },
      )
      .toBe(true);
  }
  const fonts = await page.evaluate(() => ({
    body: getComputedStyle(document.body).fontFamily,
    heading: getComputedStyle(document.querySelector('h1')!).fontFamily,
    loaded: [...document.fonts]
      .filter((font) => font.status === 'loaded')
      .map((font) => font.family),
  }));
  expect(fonts.body).toContain('Manrope');
  const display = ['/', '/wycena', '/admin.html'].includes(path) ? 'Oxanium' : 'Manrope';
  expect(fonts.heading).toContain(display);
  expect(fonts.loaded.some((font) => font.includes('Manrope'))).toBe(true);
  expect(fonts.loaded.some((font) => font.includes('Oxanium'))).toBe(true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
    width,
  );
  // Approved UI: 20 visible storefront icons; admin has one additional desktop control.
  const minimumPainted = ['/', '/wycena'].includes(path)
    ? 20
    : path === '/admin.html'
      ? width === 390
        ? 10
        : 11
      : 1;
  const svgCounts = await checkSvgGraphics(page, path, minimumPainted);
  test.info().annotations.push({
    type: 'svg-rendering',
    description: JSON.stringify({
      path,
      width,
      theme: await page.locator('html').getAttribute('data-theme'),
      ...svgCounts,
    }),
  });
}

test('SVG paint proof rejects blank graphics and accepts a stroked minus', async ({ page }) => {
  await page.setContent(`
    <svg width="24" height="24" viewBox="0 0 24 24">
      <rect width="24" height="24" fill="none" />
    </svg>
  `);
  await expect(checkSvgGraphics(page, 'blank negative control')).rejects.toThrow(
    'SVG descendants must paint visible pixels',
  );
  await expect(page.locator('[data-e2e-svg]')).toHaveCount(0);
  await page.setContent(`
    <svg width="24" height="24" viewBox="0 0 24 24">
      <path d="M4 12H20" fill="none" stroke="black" stroke-width="2" />
    </svg>
  `);
  expect(await checkSvgGraphics(page, 'minus positive control')).toEqual({
    total: 1,
    hidden: 0,
    geometry: 1,
    painted: 1,
    largeGeometryOnly: 0,
  });
  await page.addStyleTag({ content: 'svg { display: none; }' });
  await expect(checkSvgGraphics(page, 'hidden negative control')).rejects.toThrow(
    'expected visible painted SVGs',
  );
  await page.setContent('<p>No graphics</p>');
  await expect(checkSvgGraphics(page, 'missing negative control')).rejects.toThrow(
    'expected visible painted SVGs',
  );
});

for (const width of [1440, 390]) {
  test(`public routes preserve theme, load assets and fit ${width}px`, async ({ page }, info) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width, height: 900 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    const failedAssets = watchAssets(page);
    for (const theme of ['light', 'dark']) {
      await page.goto('/');
      if ((await page.locator('html').getAttribute('data-theme')) !== theme)
        await page
          .getByRole('button', {
            name: theme === 'light' ? 'Zmień na jasny motyw' : 'Zmień na ciemny motyw',
          })
          .click();
      for (const [index, [path, heading]] of routes.entries()) {
        await page.goto(path, { waitUntil: 'domcontentloaded' });
        await expect(page.getByRole('heading', { level: 1 }).first()).toHaveText(heading);
        await expect(
          page.locator('html'),
          `${path}: saved theme survives direct navigation`,
        ).toHaveAttribute('data-theme', theme);
        await assetsAndLayout(page, path, width);
        await page.evaluate(() => scrollTo({ top: 0, behavior: 'instant' }));
        await page.screenshot({ path: info.outputPath(`${index}-${theme}.png`) });
      }
    }
    expect(failedAssets).toEqual([]);
  });
}

for (const width of [1440, 390]) {
  test(`authenticated client and admin routes preserve theme and fit ${width}px`, async ({
    page,
    context,
  }, info) => {
    test.skip(!process.env.RAPIDMAKER_TEST_ACCOUNTS, 'Requires --local-accounts');
    test.setTimeout(180_000);
    await page.setViewportSize({ width, height: 900 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    const failedAssets = watchAssets(page);
    for (const role of ['client', 'admin']) {
      await context.clearCookies();
      const account = accounts().find(
        (value) =>
          value.email ===
          (role === 'admin' ? 'browser-admin@example.invalid' : 'browser-other@example.invalid'),
      )!;
      await login(page, account);
      const paths =
        role === 'client'
          ? [
              ['/konto/zamowienia', 'Moje zamówienia'],
              ['/konto/ustawienia', 'Ustawienia'],
            ]
          : [
              ['/admin.html', 'Zamówienia'],
              ['/admin/zamowienia', 'Zamówienia'],
              ['/admin/produkcja', 'Produkcja'],
              ['/admin/materialy', 'Materiały i kolory'],
              ['/admin/ustawienia', 'Ustawienia wyceny'],
            ];
      for (const theme of ['light', 'dark']) {
        await page.goto('/');
        if ((await page.locator('html').getAttribute('data-theme')) !== theme)
          await page
            .getByRole('button', {
              name: theme === 'light' ? 'Zmień na jasny motyw' : 'Zmień na ciemny motyw',
            })
            .click();
        for (const [index, [path, heading]] of paths.entries()) {
          await page.goto(path);
          await expect(page.getByRole('heading', { level: 1 }).first()).toHaveText(heading);
          await expect(
            page.getByRole('status').filter({ hasText: /Ładowanie|Wczytywanie/ }),
          ).toHaveCount(0);
          await expect(
            page.getByRole('button', { name: 'Zaloguj hasłem', exact: true }),
          ).toHaveCount(0);
          await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
          await expect(page.getByRole('alert')).toHaveCount(0);
          await assetsAndLayout(page, path, width);
          await page.screenshot({ path: info.outputPath(`${role}-${index}-${theme}.png`) });
          if (path === '/admin.html') {
            const opposite = theme === 'light' ? 'ciemny' : 'jasny';
            await page
              .getByRole('button', { name: `Włącz ${opposite} motyw`, exact: true })
              .click();
            await page.reload();
            await expect(page.locator('html')).toHaveAttribute(
              'data-theme',
              theme === 'light' ? 'dark' : 'light',
            );
            await page
              .getByRole('button', {
                name: `Włącz ${theme === 'light' ? 'jasny' : 'ciemny'} motyw`,
                exact: true,
              })
              .click();
            await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
          }
        }
      }
    }
    expect(failedAssets).toEqual([]);
  });
}

test('theme still works when browser storage is unavailable', async ({ page }) => {
  await page.addInitScript(() => {
    for (const key of ['getItem', 'setItem'])
      Object.defineProperty(Storage.prototype, key, {
        value() {
          throw new DOMException('Unavailable', 'SecurityError');
        },
      });
  });
  await page.goto('/logowanie');
  await expect(
    page.getByRole('heading', { name: 'Dobrze Cię widzieć.', exact: true }),
  ).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.goto('/');
  await page.getByRole('button', { name: 'Zmień na jasny motyw', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});
