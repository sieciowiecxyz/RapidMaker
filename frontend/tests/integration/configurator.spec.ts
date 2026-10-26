import { expect, test, type Page } from './fixtures';
import { resolve } from 'node:path';
import { readFileSync } from 'node:fs';

const fixture = (name: string) => resolve('../backend/crates/geometry/tests/fixtures', name);
const draftKey = 'rapidmaker.storefront.draft.v1';

async function storefront(page: Page) {
  return page.evaluate((key) => JSON.parse(localStorage.getItem(key) ?? 'null'), draftKey);
}
async function cart(page: Page) {
  return page.evaluate(
    () => JSON.parse(localStorage.getItem('rapidmaker.frontend.demo.v1') ?? 'null')?.cart,
  );
}
async function readyAdvanced(page: Page) {
  await expect(page.locator('.summary')).toHaveAttribute('aria-busy', 'false');
  await expect(page.locator('.quote-state')).toHaveText('Cenę i zakres potwierdzi zespół.');
}
async function manualModel(page: Page) {
  await page.goto('/wycena');
  await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles(fixture('cube_20mm.stl'));
  await expect(page.getByRole('img', { name: /Model z pliku cube_20mm.stl/ })).toBeVisible();
  await page.getByRole('checkbox', { name: 'Mam nietypowe wymagania do omówienia' }).check();
  await page.getByRole('radio', { name: 'Wysyłka', exact: true }).check();
  await page.locator('input[name=material][value=GFA00]').check();
  await page.locator('input[name=color]').first().check();
  await page.locator('input[name=quality][value=standard]').check();
  await page.locator('input[name=strength][value=standard]').check();
  const advanced = page.getByRole('button', {
    name: 'Opcje zaawansowane: podpory, wypełnienie i wykończenie',
  });
  await expect(advanced).toBeEnabled();
  await advanced.click();
  await expect(page).toHaveURL(/\/wycena\/zaawansowana$/);
  await readyAdvanced(page);
}

test('storefront navigation, theme, sample and invalid upload actions have observable results', async ({
  page,
}, info) => {
  await page.goto('/');
  await page.screenshot({ path: info.outputPath('storefront-desktop.png') });
  await page.getByRole('link', { name: 'RapidMaker, początek strony' }).click();
  await expect(page).toHaveURL(/#main$/);
  await page.getByRole('link', { name: 'Przejdź do treści', exact: true }).focus();
  await page.getByRole('link', { name: 'Przejdź do treści', exact: true }).press('Enter');
  await page.getByRole('button', { name: 'Zmień na jasny motyw' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.getByRole('button', { name: 'Zmień na ciemny motyw' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  for (const [name, hash] of [
    ['Oferta', '#oferta'],
    ['Wycena', '#wycena'],
    ['Kontakt', '#kontakt'],
  ] as const) {
    await page
      .getByRole('navigation', { name: 'Główna nawigacja' })
      .getByRole('link', { name, exact: true })
      .click();
    await expect(page).toHaveURL(new RegExp(`${hash}$`));
  }
  await expect(page.getByRole('link', { name: '537 320 816', exact: true })).toHaveAttribute(
    'href',
    'tel:+48537320816',
  );
  await expect(
    page.getByRole('link', { name: 'piotr@rapidmaker.pl', exact: true }),
  ).toHaveAttribute('href', /^mailto:piotr@rapidmaker\.pl/);
  await expect(page.getByRole('link', { name: 'Wyznacz trasę' })).toHaveAttribute(
    'href',
    /^https:\/\/www\.google\.com\/maps/,
  );
  await page.getByRole('link', { name: 'Wróć na górę' }).click();
  await expect(page).toHaveURL(/#main$/);
  await page.getByRole('button', { name: /Nie masz pliku\? Wypróbuj przykład/ }).click();
  await expect(
    page.getByRole('button', { name: 'Konfiguruj uchwyt-montazowy.stl', exact: true }),
  ).toBeVisible();
  await expect(page.getByText('Obiekt pokazowy.', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: /Wczytaj ponownie przykład/ }).click();
  await expect(
    page.getByRole('region', { name: 'Lista dodanych modeli' }).locator('.model-row'),
  ).toHaveCount(1);
  await page.getByRole('button', { name: 'Usuń uchwyt-montazowy.stl', exact: true }).click();
  await expect(page.getByText('Dodaj model, żeby zacząć.', { exact: true })).toBeVisible();
  const upload = page.getByLabel('Dodaj modele STL, STEP lub 3MF');
  for (const [name, buffer, message] of [
    ['not-model.txt', Buffer.from('not a model'), 'Nieobsługiwany format: not-model.txt'],
    ['empty.stl', Buffer.alloc(0), 'Wybierz niepusty plik do 25 MB.'],
    ['large.stl', Buffer.alloc(25 * 1024 * 1024 + 1), 'Wybierz niepusty plik do 25 MB.'],
  ] as const) {
    await upload.setInputFiles({ name, mimeType: 'application/octet-stream', buffer });
    await expect(page.getByRole('alert')).toContainText(message);
  }
  await upload.setInputFiles(
    Array.from({ length: 11 }, (_, index) => ({
      name: `part-${index}.stl`,
      mimeType: 'application/octet-stream',
      buffer: Buffer.from('solid invalid\nendsolid'),
    })),
  );
  await expect(page.getByRole('alert')).toContainText('Możesz dodać do 10 modeli.');
  const transfer = await page.evaluateHandle(
    (bytes) => {
      const data = new DataTransfer();
      data.items.add(
        new File([new Uint8Array(bytes)], 'dragged.stl', { type: 'application/octet-stream' }),
      );
      return data;
    },
    [...readFileSync(fixture('cube_20mm.stl'))],
  );
  const drop = page.locator('.dropzone');
  await drop.dispatchEvent('dragover', { dataTransfer: transfer });
  await expect(drop).toHaveClass(/dragging/);
  await drop.dispatchEvent('dragleave', { dataTransfer: transfer });
  await expect(drop).not.toHaveClass(/dragging/);
  await drop.dispatchEvent('drop', { dataTransfer: transfer });
  await expect(page.getByRole('img', { name: /Model z pliku dragged.stl/ })).toBeVisible();
  page.once('dialog', (dialog) => dialog.dismiss());
  await page.getByRole('button', { name: /Wczytaj ponownie przykład/ }).click();
  await expect(
    page.getByRole('button', { name: 'Konfiguruj dragged.stl', exact: true }),
  ).toBeVisible();
  page.once('dialog', (dialog) => dialog.accept());
  await page.getByRole('button', { name: /Wczytaj ponownie przykład/ }).click();
  await expect(
    page.getByRole('button', { name: 'Konfiguruj uchwyt-montazowy.stl', exact: true }),
  ).toBeVisible();
  await page.getByRole('link', { name: 'Regulaminy i prywatność', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Dokumenty i zasady' })).toBeVisible();
  await page.goBack();
  await page.getByRole('link', { name: 'Moje zamówienia', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Dobrze Cię widzieć.' })).toBeVisible();
});

test('all material colors and print choices persist per model, including copying and quantity validation', async ({
  page,
}) => {
  test.setTimeout(180_000);
  await page.goto('/wycena');
  const catalog = await (await page.request.get('/api/catalog/materials')).json();
  await page
    .getByLabel('Dodaj modele STL, STEP lub 3MF')
    .setInputFiles([fixture('cube_20mm.stl'), fixture('simple_model.3mf')]);
  await expect(page.getByRole('img', { name: /cube_20mm.stl.*Biały podgląd/ })).toBeVisible();
  await expect
    .poll(async () =>
      (await storefront(page)).models.every(
        (model: { status: string }) => model.status === 'ready',
      ),
    )
    .toBe(true);
  for (const [name, value] of [
    ['Wysyłka', 'shipping'],
    ['Odbiór osobisty', 'pickup'],
  ] as const) {
    await page.getByRole('radio', { name, exact: true }).check();
    await expect.poll(async () => (await storefront(page)).delivery).toBe(value);
  }
  await page.getByRole('button', { name: 'Uzupełnij ustawienia', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Konfiguruj cube_20mm.stl', exact: true }),
  ).toHaveAttribute('aria-pressed', 'true');
  // Leave the second part incomplete during the sweep: no unnecessary slicer call per color.
  for (const family of catalog.materials as { id: string; colors: { code: string }[] }[]) {
    await page.locator(`input[name=material][value="${family.id}"]`).check();
    await expect(page.locator('input[name=color]')).toHaveCount(family.colors.length);
    for (const color of family.colors) {
      const option = page.locator(`input[name=color][value="${color.code}"]`);
      await option.check();
      await expect(option).toBeChecked();
      await expect
        .poll(async () => (await storefront(page)).models[0].settings.color)
        .toBe(color.code);
    }
  }
  for (const quality of await page
    .locator('input[name=quality]')
    .evaluateAll((options) => options.map((option) => (option as HTMLInputElement).value))) {
    await page.locator(`input[name=quality][value="${quality}"]`).check();
    await expect
      .poll(async () => (await storefront(page)).models[0].settings.quality)
      .toBe(quality);
  }
  for (const strength of ['light', 'standard', 'reinforced']) {
    await page.locator(`input[name=strength][value="${strength}"]`).check();
    await expect
      .poll(async () => (await storefront(page)).models[0].settings.strength)
      .toBe(strength);
  }
  const quantity = page.getByRole('spinbutton', { name: 'Liczba sztuk: cube_20mm.stl' });
  for (const invalid of ['0', '1001']) {
    await quantity.fill(invalid);
    await expect(page.getByText(/Do uzupełnienia: liczba sztuk od 1 do 1000/)).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Przejdź do dostawy', exact: true }),
    ).toBeDisabled();
  }
  await quantity.fill('1000');
  await expect(
    page.getByText('Ustawienia zapisane dla tego modelu.', { exact: false }),
  ).toBeVisible();
  await quantity.fill('2');
  await page.getByRole('checkbox', { name: 'Mam nietypowe wymagania do omówienia' }).check();
  await page.getByRole('button', { name: 'Uzupełnij dla wszystkich', exact: true }).click();
  await expect
    .poll(async () => {
      const models = (await storefront(page)).models;
      return models[0].settings.color === models[1].settings.color && models[1].quantity === 1;
    })
    .toBe(true);
  await page.getByRole('button', { name: 'Następny model', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Konfiguruj simple_model.3mf', exact: true }),
  ).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'Konfiguruj cube_20mm.stl', exact: true }).click();
  await page.getByRole('button', { name: 'Usuń simple_model.3mf', exact: true }).click();
  await quantity.fill('1');
  await page.locator('input[name=quality][value=standard]').check();
  await page.locator('input[name=strength][value=standard]').check();
  for (const family of catalog.materials as { id: string }[]) {
    await page.locator(`input[name=material][value="${family.id}"]`).check();
    await page.locator('input[name=color]').first().check();
    const requested = page.waitForResponse(
      (response) =>
        response.url().endsWith('/api/quotes/estimate') && response.request().method() === 'POST',
    );
    await page.getByRole('checkbox', { name: 'Mam nietypowe wymagania do omówienia' }).uncheck();
    const response = await requested;
    expect(response.request().postDataJSON().material_id).toBe(family.id);
    expect(response.status()).toBe(202);
    await expect(page.getByRole('button', { name: 'Przejdź do dostawy', exact: true })).toBeEnabled(
      { timeout: 90_000 },
    );
    await expect(page.locator('.price')).toContainText(/\d.*zł/);
    await page.getByRole('checkbox', { name: 'Mam nietypowe wymagania do omówienia' }).check();
  }
  await page.reload();
  await expect(quantity).toHaveValue('1');
  await expect(page.locator('input[name=material]:checked')).toHaveValue(
    catalog.materials.at(-1).id,
  );
});

test('viewer mouse controls and reduced-motion mobile controls remain usable', async ({
  page,
}, info) => {
  await page.goto('/wycena');
  await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles(fixture('cube_20mm.stl'));
  await expect(page.getByRole('img', { name: /cube_20mm.stl.*Biały podgląd/ })).toBeVisible();
  for (const name of ['Przybliż model', 'Oddal model', 'Obróć model']) {
    const button = page.getByRole('button', { name, exact: true });
    await expect(button).toBeEnabled();
    await button.click();
  }
  await expect(
    page.getByRole('button', { name: 'Wznów animację druku', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Wznów animację druku', exact: true }).click();
  await page.getByRole('button', { name: 'Wstrzymaj animację druku', exact: true }).click();
  await page.getByRole('button', { name: 'Odtwórz animację druku', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Wstrzymaj animację druku', exact: true }),
  ).toBeVisible();
  const canvas = page.locator('.chamber-visual canvas');
  await canvas.scrollIntoViewIfNeeded();
  const box = (await canvas.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 40, box.y + box.height / 2 + 15);
  await page.mouse.up();
  await expect(
    page.getByRole('button', { name: 'Wznów animację druku', exact: true }),
  ).toBeVisible();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(
    page.getByRole('button', { name: 'Odtwórz animację druku', exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole('button', { name: /(?:Wstrzymaj|Wznów) animację druku/ }),
  ).toBeDisabled();
  await page.getByRole('button', { name: 'Obróć model', exact: true }).click();
  const scale = () =>
    page.locator('.chamber-visual canvas').evaluate((canvas) => {
      const gl = (canvas as HTMLCanvasElement).getContext('webgl');
      const program = gl?.getParameter(gl.CURRENT_PROGRAM);
      return program ? gl!.getUniform(program, gl!.getUniformLocation(program, 'uScale')) : null;
    });
  const beforeScale = await scale();
  const before = await page
    .locator('.chamber-visual')
    .screenshot({ path: 'tests/artifacts/integration/viewer-before-zoom.png' });
  await page.getByRole('button', { name: 'Przybliż model', exact: true }).click();
  expect(beforeScale).toBeGreaterThan(0);
  await expect
    .poll(scale, { message: 'Zoom must reach the real WebGL renderer' })
    .toBeGreaterThan(beforeScale);
  // Reduced motion renders on demand; wait for the GPU/compositor to present that draw.
  await expect
    .poll(
      async () => {
        const after = await page
          .locator('.chamber-visual')
          .screenshot({ path: 'tests/artifacts/integration/viewer-after-zoom.png' });
        return !after.equals(before);
      },
      { timeout: 5_000, message: 'Zoom must visibly change the model, not only its WebGL uniform' },
    )
    .toBe(true);
  await page.getByRole('button', { name: 'Oddal model', exact: true }).click();
  await page.screenshot({ path: info.outputPath('configurator-mobile.png') });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('advanced controls save every offered value and survive return to the storefront', async ({
  page,
}) => {
  test.setTimeout(240_000);
  await manualModel(page);
  await page.getByText('Więcej ustawień i wymagania', { exact: true }).click();
  await expect(page.getByLabel('Rysunek techniczny · PDF')).toBeDisabled();
  // Each non-default advanced preference is deliberately manual; no invented server price.
  for (const label of [
    'Orientacja wydruku',
    'Podpory',
    'Wykończenie',
    'Pasowanie części',
    'Termin',
  ]) {
    const select = page.getByRole('combobox', { name: label, exact: true });
    const values = await select
      .locator('option')
      .evaluateAll((options) => options.map((option) => (option as HTMLOptionElement).value));
    for (const value of values) {
      await select.selectOption(value);
      await readyAdvanced(page);
      await expect(select).toHaveValue(value);
    }
  }
  const units = page.getByRole('combobox', { name: 'Jednostki pliku STL', exact: true });
  for (const value of ['cm', 'in', 'mm']) {
    await units.selectOption(value);
    await readyAdvanced(page);
    await expect.poll(async () => (await cart(page)).models[0].units).toBe(value);
  }
  for (const material of ['PLA', 'ABS', 'TPU']) {
    await page
      .locator('.material-list')
      .getByRole('button', { name: new RegExp(`^${material}`) })
      .click();
    await readyAdvanced(page);
    const colors = page.getByRole('group', { name: 'Kolor materiału' }).getByRole('button');
    for (let index = 0; index < (await colors.count()); index++) {
      await colors.nth(index).click();
      await readyAdvanced(page);
      await expect(colors.nth(index)).toHaveAttribute('aria-pressed', 'true');
    }
  }
  for (let index = 0; index < (await page.locator('.quality-choices button').count()); index++) {
    const button = page.locator('.quality-choices button').nth(index);
    await button.click();
    await readyAdvanced(page);
    await expect(button).toHaveAttribute('aria-pressed', 'true');
  }
  for (let index = 0; index < (await page.locator('.strength-choices button').count()); index++) {
    const button = page.locator('.strength-choices button').nth(index);
    await button.click();
    await readyAdvanced(page);
    await expect(button).toHaveAttribute('aria-pressed', 'true');
  }
  await page.getByRole('button', { name: /Własne ściany i wypełnienie/ }).click();
  await readyAdvanced(page);
  for (const [name, value] of [
    ['Liczba ścian', '8'],
    ['Wypełnienie (%)', '100'],
    ['Inserty gwintowane · liczba', '3'],
  ] as const) {
    await page.getByRole('spinbutton', { name, exact: true }).fill(value);
    await page.getByRole('spinbutton', { name, exact: true }).press('Tab');
    await readyAdvanced(page);
    await expect(page.getByRole('spinbutton', { name, exact: true })).toHaveValue(value);
  }
  for (const [name, invalid, previous] of [
    ['Liczba ścian', '9', '8'],
    ['Wypełnienie (%)', '101', '100'],
    ['Inserty gwintowane · liczba', '-1', '3'],
  ] as const) {
    const input = page.getByRole('spinbutton', { name, exact: true });
    await input.fill(invalid);
    await input.press('Tab');
    await expect(page.getByRole('alert')).toContainText(
      'Wybierz 2–8 ścian, 10–100% wypełnienia i 0–100 insertów.',
    );
    await expect(page.getByRole('link', { name: 'Dalej: wysyłka →', exact: true })).toHaveCount(0);
    await page.getByRole('button', { name: 'Spróbuj ponownie', exact: true }).click();
    await readyAdvanced(page);
    await expect(input).toHaveValue(previous);
  }
  for (const value of ['gyroid', 'grid']) {
    await page.getByRole('combobox', { name: 'Wzór wypełnienia', exact: true }).selectOption(value);
    await readyAdvanced(page);
  }
  for (const value of ['M3', 'M4', 'M5']) {
    await page.getByRole('combobox', { name: 'Rozmiar insertów', exact: true }).selectOption(value);
    await readyAdvanced(page);
  }
  await page
    .getByRole('textbox', { name: 'Ważne wymagania', exact: true })
    .fill('Próba E2E: ważne wymiary i obciążenie.');
  await page.getByRole('textbox', { name: 'Ważne wymagania', exact: true }).press('Tab');
  await readyAdvanced(page);
  await units.selectOption('cm');
  await readyAdvanced(page);
  await page
    .locator('.quality-choices')
    .getByRole('button', { name: /^Robocza/ })
    .click();
  await readyAdvanced(page);
  await page.getByRole('link', { name: '← Wróć do konfiguratora', exact: true }).click();
  await expect(page).toHaveURL(/\/wycena\?restore=cart$/);
  await expect(page.locator('input[name=quality][value=draft]')).toBeChecked();
  await expect(page.getByRole('radio', { name: 'Wysyłka', exact: true })).toBeChecked();
  await expect(page.getByRole('button', { name: /Opcje zaawansowane: podpory/ })).toBeEnabled();
  await page.getByRole('button', { name: /Opcje zaawansowane: podpory/ }).click();
  await readyAdvanced(page);
  await page.getByText('Więcej ustawień i wymagania', { exact: true }).click();
  await expect(units).toHaveValue('cm');
  await expect(
    page.locator('.quality-choices').getByRole('button', { name: /^Robocza/ }),
  ).toHaveAttribute('aria-pressed', 'true');
  const saved = (await cart(page)).models[0];
  expect(saved.advanced).toMatchObject({
    wallCount: 8,
    infill: 100,
    insertCount: 3,
    insertSize: 'M5',
  });
  expect(saved.notes).toContain('Próba E2E');
  await page.getByRole('link', { name: 'Dalej: wysyłka →', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Dokąd wysłać wydruk?', exact: true }),
  ).toBeVisible();
});

test('real connection failures show recoverable quote and preview actions', async ({
  page,
  context,
}) => {
  await page.goto('/wycena');
  await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles(fixture('cube_20mm.stl'));
  await expect(page.getByRole('img', { name: /cube_20mm.stl.*Biały podgląd/ })).toBeVisible();
  await page.locator('input[name=material][value=GFA00]').check();
  await page.locator('input[name=color]').first().check();
  await page.locator('input[name=quality][value=standard]').check();
  await context.setOffline(true);
  try {
    await page.locator('input[name=strength][value=standard]').check();
    await expect(page.getByRole('button', { name: 'Ponów przeliczenie' })).toBeVisible();
  } finally {
    await context.setOffline(false);
  }
  await page.getByRole('button', { name: 'Ponów przeliczenie' }).click();
  await expect(page.getByRole('button', { name: 'Przejdź do dostawy', exact: true })).toBeEnabled({
    timeout: 90_000,
  });
  await context.setOffline(true);
  // A failed upload must stay visibly invalid; never display the sample as the uploaded geometry.
  try {
    await page
      .getByLabel('Dodaj modele STL, STEP lub 3MF')
      .setInputFiles(fixture('simple_model.3mf'));
    await expect(page.getByText('Nie można wyświetlić modelu', { exact: true })).toBeVisible();
  } finally {
    await context.setOffline(false);
  }
  // An interrupted upload is not a valid model: removal and a fresh upload are the recovery action.
  await page.getByRole('button', { name: 'Usuń simple_model.3mf', exact: true }).click();
  await page
    .getByLabel('Dodaj modele STL, STEP lub 3MF')
    .setInputFiles(fixture('simple_model.3mf'));
  await expect(page.getByRole('img', { name: /Model z pliku simple_model.3mf/ })).toBeVisible();
  // Network fault injection only: no fake API response, model or price.
  await page.route('**/api/models/*/preview.glb', (route) => route.abort('failed'));
  await page.getByLabel('Dodaj modele STL, STEP lub 3MF').setInputFiles(fixture('step_block.step'));
  await expect(page.getByRole('button', { name: 'Spróbuj ponownie', exact: true })).toBeVisible();
  await page.unroute('**/api/models/*/preview.glb');
  await page.getByRole('button', { name: 'Spróbuj ponownie', exact: true }).click();
  await expect(page.getByRole('img', { name: /Model z pliku step_block.step/ })).toBeVisible();
});

test('advanced upload, sample, model selection and removal retain the correct part', async ({
  page,
}) => {
  await manualModel(page);
  await page
    .getByLabel('Wybierz modele', { exact: true })
    .setInputFiles(fixture('simple_model.3mf'));
  await readyAdvanced(page);
  await expect(page.locator('.parts .part-row')).toHaveCount(2);
  await expect(page.locator('.editor-heading h2')).toHaveText('simple_model.3mf');
  const uploaded = (await cart(page)).models.find(
    (model: { name: string }) => model.name === 'simple_model.3mf',
  );
  const metadata = await page.request.get(`/api/models/${uploaded.analysis.id}`);
  expect(metadata.status()).toBe(200);
  expect((await metadata.json()).filename).toBe('simple_model.3mf');
  await page.getByRole('button', { name: 'Wypróbuj przykład', exact: true }).click();
  await readyAdvanced(page);
  await expect(page.locator('.parts .part-row')).toHaveCount(3);
  await expect(page.locator('.editor-heading h2')).toHaveText('Uchwyt na słuchawki.stl');
  await page.getByRole('button', { name: 'Usuń Uchwyt na słuchawki.stl', exact: true }).click();
  await readyAdvanced(page);
  await expect(page.locator('.parts .part-row')).toHaveCount(2);
  await page.locator('.parts .part-row').filter({ hasText: 'cube_20mm.stl' }).click();
  await expect(page.locator('.editor-heading h2')).toHaveText('cube_20mm.stl');
  await page.locator('.parts .part-row').filter({ hasText: 'simple_model.3mf' }).click();
  await page.getByRole('button', { name: 'Usuń simple_model.3mf', exact: true }).click();
  await readyAdvanced(page);
  await expect(page.locator('.editor-heading h2')).toHaveText('cube_20mm.stl');
  await page.getByRole('button', { name: 'Usuń cube_20mm.stl', exact: true }).click();
  await expect(
    page.getByText('Dodaj model, aby skonfigurować wydruk.', { exact: true }),
  ).toBeVisible();
  await expect(page.locator('.parts .part-row')).toHaveCount(0);
  await page
    .getByLabel('Wybierz modele', { exact: true })
    .setInputFiles({ name: 'invalid.txt', mimeType: 'text/plain', buffer: Buffer.from('bad') });
  await expect(page.getByRole('alert')).toContainText('invalid.txt: wybierz STL, STEP lub 3MF.');
});
