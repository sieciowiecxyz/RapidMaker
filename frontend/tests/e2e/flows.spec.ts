import { test, expect, type Page } from '@playwright/test';

async function login(page: Page, role: 'client' | 'admin') {
  await page.goto('/logowanie');
  await page.getByLabel('Adres email').fill(`${role}@rapidmaker.test`);
  await page.getByRole('button', { name: 'Wyślij link do logowania', exact: true }).click();
  await page.getByRole('button', { name: 'Symuluj potwierdzenie emaila', exact: true }).click();
  await expect(page).toHaveURL(role === 'admin' ? /\/admin\/zamowienia$/ : /\/konto\/zamowienia$/);
}
async function sample(page: Page) {
  await page.goto('/wycena/zaawansowana');
  await page.getByRole('button', { name: 'Lub wypróbuj przykładowy model →' }).click();
  await expect(page.getByRole('link', { name: 'Przejdź do zamówienia →' })).toBeVisible();
}
async function screenshot(page: Page, name: string, mobile = false) {
  if (mobile) await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({
    path: new URL(`../artifacts/${name}.png`, import.meta.url).pathname,
    fullPage: true,
  });
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
  ).toBeTruthy();
}

test('mixed parts keep independent quantities and unknown geometry blocks payment', async ({ page }) => {
  await sample(page);
  await page.getByLabel('Liczba sztuk tego modelu', { exact: true }).fill('3');
  await page.getByLabel('Liczba sztuk tego modelu', { exact: true }).press('Tab');
  await expect(page.locator('.summary')).toContainText('3 szt.');
  await page.getByLabel('Wybierz modele').setInputFiles({
    name: 'obudowa.step', mimeType: 'application/octet-stream', buffer: Buffer.from('metadata only'),
  });
  await expect(page.getByRole('heading', { name: 'obudowa.step', exact: true })).toBeVisible();
  await expect(page.getByLabel('Liczba sztuk tego modelu', { exact: true }).nth(0)).toHaveValue('3');
  await expect(page.getByLabel('Liczba sztuk tego modelu', { exact: true }).nth(1)).toHaveValue('1');
  await expect(page.locator('.summary')).toContainText('masa');
  await expect(page.locator('.summary')).not.toContainText('16,99');
  await screenshot(page, 'mixed-parts-desktop');
  await page.getByRole('link', { name: /Poproś o wycenę|Przejdź do zamówienia/ }).click();
  await expect(page.getByRole('button', { name: 'Przejdź do płatności →' })).toBeDisabled();
});

test('untrusted Stripe return never confirms payment', async ({ page }) => {
  await sample(page);
  await page.goto('/zamowienie?stripe_session=cs_test_forged');
  await expect(page.getByRole('alert')).toContainText(/sesj/);
  await expect(page.getByText('Stripe potwierdził płatność testową.', { exact: false })).toHaveCount(0);
  await page.goto('/zamowienie?stripe_cancelled=1');
  await expect(page.getByRole('status')).toContainText('anulowana');
});

test('client gate, conversation, persisted notification preferences and reorder', async ({
  page,
}) => {
  await page.goto('/logowanie');
  await expect(page.getByRole('button', { name: /Klient ·/ })).toBeVisible();
  await screenshot(page, 'login-mobile', true);
  await login(page, 'client');
  await page.goto('/admin/zamowienia');
  await expect(page.getByRole('heading', { name: 'Ten panel jest dla obsługi.' })).toBeVisible();
  await page.goto('/konto/zamowienia/demo-order-1');
  await page.getByLabel('Wiadomość', { exact: true }).fill('Czy powierzchnia będzie gładka?');
  await page.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
  await expect(page.getByText('Czy powierzchnia będzie gładka?', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Wyloguj', exact: true }).click();
  await login(page, 'admin');
  await page.goto('/admin/zamowienia/demo-order-1');
  await expect(page.getByText('Czy powierzchnia będzie gładka?', { exact: true })).toBeVisible();
  await page
    .getByLabel('Wiadomość', { exact: true })
    .fill('Tak, ustalimy powierzchnię przed drukiem.');
  await page.getByRole('button', { name: 'Wyślij wiadomość', exact: true }).click();
  await expect(
    page.getByText('Tak, ustalimy powierzchnię przed drukiem.', { exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Wyloguj', exact: true }).click();
  await login(page, 'client');
  await page.goto('/konto/zamowienia/demo-order-1');
  await expect(
    page.getByText('Tak, ustalimy powierzchnię przed drukiem.', { exact: true }),
  ).toBeVisible();
  await screenshot(page, 'client-mobile', true);
  await page.goto('/konto/ustawienia');
  await page.getByLabel('Nowa wiadomość od obsługi', { exact: true }).uncheck();
  await page.getByRole('button', { name: 'Zapisz ustawienia', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Zapisano ustawienia');
  await page.reload();
  await expect(page.getByLabel('Nowa wiadomość od obsługi', { exact: true })).not.toBeChecked();
  await page.goto('/konto/zamowienia/demo-order-1');
  await page.getByRole('button', { name: 'Zamów ponownie ↗' }).click();
  await expect(page).toHaveURL(/\/wycena(?:\?restore=cart)?$/);
  await expect(page.getByRole('link', { name: 'Przejdź do zamówienia →' })).toBeVisible();
});

test('admin production, shipping, refund, manual offer and failed job retry', async ({ page }) => {
  await login(page, 'admin');
  await screenshot(page, 'admin-desktop');
  await page.goto('/admin/zamowienia/demo-order-1');
  await page.getByLabel('Status', { exact: true }).selectOption('printing');
  await page.getByLabel('Prywatne notatki zespołu').fill('Sprawdzić krawędzie przed pakowaniem.');
  await page.getByRole('button', { name: 'Zapisz zmiany', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Zapisano ustalenia');
  await page.getByRole('button', { name: 'Przygotuj etykietę demo', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Przygotowano etykietę');
  await page.getByRole('button', { name: /Symuluj zwrot/ }).click();
  await expect(page.getByRole('status')).toContainText('Zasymulowano pełny zwrot');
  await expect(page.getByRole('button', { name: /Symuluj zwrot/ })).toHaveCount(0);
  await page.goto('/admin/zamowienia/demo-order-3');
  await page.getByLabel('Łączna cena oferty (zł)').fill('155');
  await page.getByLabel('Powód zmiany ceny').fill('Dodatkowe przygotowanie powierzchni.');
  await page.getByRole('button', { name: 'Zapisz i udostępnij ofertę', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Oferta demo gotowa');
  await page.goto('/admin/materialy');
  const pla = page
    .locator('form')
    .filter({ has: page.getByRole('heading', { name: 'Bambu PLA Basic', exact: true }) });
  const petg = page
    .locator('form')
    .filter({ has: page.getByRole('heading', { name: 'Bambu PETG HF', exact: true }) });
  await petg.getByLabel('Koszt filamentu / kg (zł)').fill('123');
  await pla.getByRole('checkbox', { name: /Leśny zielony/ }).uncheck();
  await pla.getByRole('button', { name: 'Zapisz materiał', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Zapisano');
  await expect(petg.getByLabel('Koszt filamentu / kg (zł)')).toHaveValue('123');
  await page.reload();
  await expect(
    page
      .locator('form')
      .filter({ has: page.getByRole('heading', { name: 'Bambu PLA Basic', exact: true }) })
      .getByRole('checkbox', { name: /Leśny zielony/ }),
  ).not.toBeChecked();
  await page.goto('/admin/ustawienia');
  await page.getByRole('button', { name: 'Ponów zadanie', exact: true }).first().click();
  await expect(page.getByRole('button', { name: 'Ponowiono', exact: true })).toBeDisabled();
});

test('invalid upload and empty/error lists provide visible recovery states', async ({ page }) => {
  await page.goto('/wycena/zaawansowana');
  await expect(
    page.getByRole('button', { name: 'Lub wypróbuj przykładowy model →' }),
  ).toBeEnabled();
  await page.getByLabel('Wybierz modele').setInputFiles({
    name: 'danger.exe',
    mimeType: 'application/octet-stream',
    buffer: Buffer.from('bad extension'),
  });
  await expect(page.getByRole('alert')).toContainText('wybierz STL, STEP lub 3MF');
  await login(page, 'client');
  await page.goto('/konto/zamowienia?devtools=1');
  await page.getByText('Scenariusze interfejsu', { exact: true }).click();
  await page.getByLabel('Stan danych').selectOption('empty');
  await expect(page.getByRole('heading', { name: 'Twój pierwszy projekt czeka' })).toBeVisible();
  await page.getByLabel('Stan danych').selectOption('error');
  await expect(page.getByRole('alert')).toBeVisible();
  await page.getByLabel('Stan danych').selectOption('normal');
  await expect(page.getByRole('link', { name: /RM-26041/ })).toBeVisible();
});

test('large manual order cannot pay and can be claimed after email confirmation', async ({
  page,
}) => {
  await sample(page);
  await page.getByLabel('Liczba sztuk tego modelu', { exact: true }).fill('51');
  await page.getByLabel('Liczba sztuk tego modelu', { exact: true }).press('Tab');
  await expect(page.getByRole('link', { name: 'Poproś o wycenę →' })).toBeVisible();
  await expect(page.locator('.summary')).not.toContainText('16,99');
  await page.getByRole('link', { name: 'Poproś o wycenę →' }).click();
  await page.getByLabel('Imię i nazwisko', { exact: true }).fill('Julia Nowa');
  await page.getByLabel('Email', { exact: true }).fill('julia@example.test');
  await page.getByLabel('Telefon', { exact: true }).fill('501234567');
  await page.getByRole('radio', { name: /Odbiór osobisty/ }).check();
  await expect(page.getByRole('button', { name: /Zamawiam i płacę/ })).toHaveCount(0);
  await page.getByRole('button', { name: 'Poproś o ręczną wycenę →', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Prośba o wycenę zapisana' })).toBeVisible();
  const number = await page.getByText(/^RM-\d+$/).innerText();
  await page.getByRole('link', { name: 'Utwórz konto przez email →' }).click();
  await expect(page.getByLabel('Adres email')).toHaveValue('julia@example.test');
  await page.getByRole('button', { name: 'Utwórz konto przez email', exact: true }).click();
  await page.getByText('Sprawdź wygasły link', { exact: true }).click();
  await page.getByRole('button', { name: 'Symuluj wygaśnięcie linku' }).click();
  await expect(page.getByRole('alert')).toContainText('Link wygasł');
  await page.getByRole('button', { name: 'Wyślij link ponownie' }).click();
  await page.getByRole('button', { name: 'Symuluj potwierdzenie emaila' }).click();
  await expect(page).toHaveURL(/\/konto\/zamowienia$/);
  await expect(page.getByRole('link', { name: new RegExp(number) })).toBeVisible();
});

test('opting out of invoice discards company data before manual request', async ({ page }) => {
  await sample(page);
  await page.getByLabel('Liczba sztuk tego modelu', { exact: true }).fill('51');
  await page.getByLabel('Liczba sztuk tego modelu', { exact: true }).press('Tab');
  await page.getByRole('link', { name: 'Poproś o wycenę →' }).click();
  await page.getByLabel('Imię i nazwisko', { exact: true }).fill('Jan Faktura');
  await page.getByLabel('Email', { exact: true }).fill('invoice@example.test');
  await page.getByLabel('Telefon', { exact: true }).fill('501234567');
  await page.getByRole('radio', { name: /Odbiór osobisty/ }).check();
  await page.getByLabel('Chcę fakturę na firmę').check();
  await page.getByLabel('Nazwa firmy', { exact: true }).fill('NIE ZAPISUJ TEJ FIRMY');
  await page.getByLabel('NIP', { exact: true }).fill('1234567890');
  await page.getByLabel('Chcę fakturę na firmę').uncheck();
  await page.getByRole('button', { name: 'Poproś o ręczną wycenę →', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Prośba o wycenę zapisana' })).toBeVisible();
  const number = await page.getByText(/^RM-\d+$/).innerText();
  await login(page, 'admin');
  await page.getByLabel('Szukaj', { exact: true }).fill(number);
  await page.getByRole('button', { name: 'Otwórz ↗', exact: true }).click();
  await expect(page.getByRole('heading', { name: number, exact: true })).toBeVisible();
  await expect(page.getByText(/NIE ZAPISUJ TEJ FIRMY/)).toHaveCount(0);
});

test('advanced FDM requirements survive manual quote checkout', async ({ page }) => {
  await sample(page);
  await page.getByLabel('Jakość', { exact: true }).selectOption('fine');
  await expect(page.getByRole('link', { name: 'Przejdź do zamówienia →' })).toBeVisible();
  await expect(page.locator('.summary')).toContainText('Wydruk i przygotowanie');
  await page.getByText('Opcje techniczne i wymagania', { exact: true }).click();
  await page.getByLabel('Wytrzymałość', { exact: true }).selectOption('strong');
  await expect(page.getByLabel('Liczba ścian', { exact: true })).toHaveValue('5');
  await expect(page.getByLabel('Wypełnienie (%)', { exact: true })).toHaveValue('40');
  await expect(page.getByRole('link', { name: 'Przejdź do zamówienia →' })).toBeVisible();
  await page.getByLabel('Liczba ścian', { exact: true }).fill('6');
  await page.getByLabel('Liczba ścian', { exact: true }).press('Tab');
  await expect(page.getByLabel('Wytrzymałość', { exact: true })).toHaveValue('custom');
  await expect(page.getByRole('link', { name: 'Przejdź do zamówienia →' })).toBeVisible();
  await page.getByLabel('Wykończenie', { exact: true }).selectOption('sanded');
  await expect(page.getByRole('link', { name: 'Poproś o wycenę →' })).toBeVisible();
  await page.getByLabel('Rysunek techniczny · PDF do 10 MB').setInputFiles({
    name: 'tolerancje.pdf',
    mimeType: 'application/pdf',
    buffer: Buffer.from('%PDF-demo metadata only'),
  });
  await expect(page.getByRole('button', { name: 'Usuń rysunek', exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Poproś o wycenę →' })).toBeVisible();
  await screenshot(page, 'advanced-mobile', true);
  await page.getByRole('link', { name: 'Poproś o wycenę →' }).click();
  await page.getByText('Parametry modeli', { exact: true }).click();
  await expect(
    page.getByText('Ściany: 6 · wypełnienie: 40% · gyroid', { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText('Wykończenie: szlifowanie — zapytanie', { exact: true }),
  ).toBeVisible();
  await page.getByLabel('Imię i nazwisko', { exact: true }).fill('Ala Techniczna');
  await page.getByLabel('Email', { exact: true }).fill('technical@example.test');
  await page.getByLabel('Telefon', { exact: true }).fill('501234567');
  await page.getByRole('radio', { name: /Odbiór osobisty/ }).check();
  await page.getByRole('button', { name: 'Poproś o ręczną wycenę →', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Prośba o wycenę zapisana' })).toBeVisible();
  const number = await page.getByText(/^RM-\d+$/).innerText();
  await login(page, 'admin');
  await page.getByLabel('Szukaj', { exact: true }).fill(number);
  await page.getByRole('button', { name: 'Otwórz ↗', exact: true }).click();
  await expect(
    page.getByText('Ściany: 6 · wypełnienie: 40% · gyroid', { exact: true }),
  ).toBeVisible();
  await expect(page.getByText(/Rysunek: tolerancje.pdf/)).toBeVisible();
});

test('order list actions open the right sections and reorder on mobile', async ({ page }) => {
  await login(page, 'client');
  const list = page.getByRole('list', { name: 'Twoje zamówienia', exact: true });
  await expect(list.getByRole('listitem')).toHaveCount(4);
  await page.screenshot({ path: 'tests/artifacts/orders-list-desktop.png', fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: 'tests/artifacts/orders-list-mobile.png', fullPage: true });
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
  ).toBeTruthy();
  await list
    .getByRole('listitem', { name: 'RM-26044', exact: true })
    .getByRole('link', { name: 'Opłać →', exact: true })
    .click();
  await expect(page).toHaveURL(/demo-order-4#platnosc$/);
  await expect(page.locator('#platnosc')).toBeInViewport();
  await expect(page.getByRole('button', { name: 'Opłać / ponów płatność' })).toBeVisible();
  await page.goto('/konto/zamowienia');
  await list
    .getByRole('listitem', { name: 'RM-26041', exact: true })
    .getByRole('link', { name: /Wiadomości/ })
    .click();
  await expect(page).toHaveURL(/demo-order-1#rozmowa$/);
  await expect(page.locator('#rozmowa')).toBeInViewport();
  await page.goto('/konto/zamowienia');
  await list
    .getByRole('listitem', { name: 'RM-26041', exact: true })
    .getByRole('button', { name: 'Zamów ponownie', exact: true })
    .click();
  await expect(page).toHaveURL(/\/wycena(?:\?restore=cart)?$/);
  await expect(page.getByRole('link', { name: 'Przejdź do zamówienia →' })).toBeVisible();
});
