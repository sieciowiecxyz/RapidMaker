import { expect, test } from '@playwright/test';

for (const width of [1440, 390]) {
  test(`homepage img elements load in both themes at ${width}px`, async ({ page }, info) => {
    await page.setViewportSize({ width, height: 900 });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    // Asset regression: authentication/catalog integration has separate real-API suites.
    await page.route('**/api/auth/me', (route) => route.fulfill({ status: 401, json: {} }));
    await page.route('**/api/catalog/materials', (route) =>
      route.fulfill({ json: { materials: [] } }),
    );
    await page.goto('/');
    await page.evaluate(() => document.fonts.ready);
    for (const theme of ['dark', 'light']) {
      if (theme === 'light')
        await page.getByRole('button', { name: 'Zmień na jasny motyw' }).click();
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
      const images = page.locator('img');
      expect(await images.count()).toBeGreaterThan(0);
      for (const image of await images.all()) {
        if (await image.isVisible()) await image.scrollIntoViewIfNeeded();
        await expect
          .poll(
            () =>
              image.evaluate(
                (node) =>
                  node instanceof HTMLImageElement && node.complete && node.naturalWidth > 0,
              ),
            {
              message: `Image must load: ${await image.getAttribute('src')}`,
            },
          )
          .toBe(true);
      }
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
        width,
      );
      await page.evaluate(() => scrollTo(0, 0));
      await page.screenshot({ path: info.outputPath(`${theme}.png`) });
    }
    expect(errors).toEqual([]);
  });
}
