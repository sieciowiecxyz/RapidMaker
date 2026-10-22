import { test, expect } from '@playwright/test';
import { writeFile } from 'node:fs/promises';

const part = { id: 'bracket', quantity: 1, dimensions_mm: [120, 80, 45], weight_g: 60 };

test('locker selection, parcel contents and Stripe return keep the checkout usable', async ({page}) => {
  await page.goto('/wycena/zaawansowana');
  await page.getByRole('button',{name:'Wypróbuj przykład'}).click();
  await page.getByRole('link',{name:'Dalej: wysyłka →'}).click();
  await page.getByLabel('Imię i nazwisko',{exact:true}).fill('Test odbiorcy');
  await page.getByLabel('Email',{exact:true}).fill('void@sieciowiec.xyz');
  await page.getByLabel('Telefon',{exact:true}).fill('600700800');
  await page.getByRole('radio',{name:/Paczkomat InPost/}).check();
  await page.getByLabel('Znajdź Paczkomat',{exact:true}).fill('31-021 Kraków');
  await page.getByRole('button',{name:'Znajdź punkty',exact:true}).click();
  const point=page.locator('.point-list button').first();
  await expect(point).toBeVisible();
  await point.click();
  await page.getByRole('button',{name:'Podsumowanie →'}).click();
  await expect(page.getByRole('button',{name:'Przejdź do płatności →'})).toBeEnabled();
  await page.getByText('Paczka 1',{exact:true}).click();
  await expect(page.getByRole('list',{name:'Zawartość paczki 1'})).toContainText('Uchwyt na słuchawki.stl');
  await page.screenshot({path:'tests/artifacts/checkout-locker-desktop.png',fullPage:true});
  await page.setViewportSize({width:390,height:844});
  await page.screenshot({path:'tests/artifacts/checkout-locker-mobile.png',fullPage:true});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBeTruthy();
  await page.route('https://checkout.stripe.com/**',r=>r.fulfill({status:200,body:'Stripe test session'}));
  let session: {session_id:string;url:string};
  await page.route('**/api/checkout/session',async route=>{ const response=await route.fetch();session=await response.json();await route.fulfill({response}); });
  await page.getByRole('button',{name:'Przejdź do płatności →'}).click();
  await expect(page).toHaveURL(new RegExp('^https://checkout[.]stripe[.]com/'));
  expect(session.session_id).toMatch(/^cs_test_/);
  await writeFile('/tmp/rapidmaker-last-stripe-session.json',JSON.stringify(session),{mode:0o600});
  await page.goto('/zamowienie?stripe_session='+session.session_id);
  await expect(page.getByRole('heading',{name:'Płatność jeszcze przed Tobą.'})).toBeVisible();
  await expect(page.getByRole('link',{name:'Wróć do płatności →'})).toHaveAttribute('href',session.url);
  await page.goto('/zamowienie?stripe_cancelled=1');
  await expect(page.getByLabel('Email',{exact:true})).toHaveValue('void@sieciowiec.xyz');
  await expect(page.getByRole('radio',{name:/Paczkomat InPost/})).toBeChecked();
  await expect(page.getByRole('status')).toContainText('zachowane');
  await expect(page.getByRole('link',{name:'Wróć do rozpoczętej płatności →'})).toBeVisible();
  await page.goto('/logowanie?register=1&email=void%40sieciowiec.xyz');
  await page.getByRole('button',{name:'Utwórz konto przez email',exact:true}).click();
  await page.getByRole('button',{name:'Symuluj potwierdzenie emaila',exact:true}).click();
  await expect(page).toHaveURL(/konto\/zamowienia$/);
  const order = await page.evaluate((sessionId) => {
    const state = JSON.parse(localStorage.getItem('rapidmaker.frontend.demo.v1')!);
    return state.orders.find((order: any) => order.sandboxPayment?.sessionId === sessionId);
  }, session.session_id);
  expect(order.payment).toBe('unpaid');
  expect(order.sandboxPayment.amountGrosze).toBe(Math.round(order.quote.total * 100));
  await page.goto('/konto/zamowienia/' + order.id);
  await expect(page.getByRole('heading',{name:order.number,exact:true})).toBeVisible();
  await expect(page.getByText('Scenariusz płatności demo',{exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Sprawdź płatność',exact:true}).click();
  await expect(page.getByRole('status')).toContainText('Sprawdzono status');
  await page.screenshot({path:'tests/artifacts/stripe-order-mobile.png',fullPage:true});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBeTruthy();
  // Syntetyczny status sprawdza lokalne przypisanie; nie jest dowodem wpłaty u dostawcy.
  await page.route('**/api/checkout/session/' + session.session_id,async route=> {
    const response = await route.fetch();
    const status = await response.json();
    await route.fulfill({response,json:{...status,status:'paid',checkout_status:'complete'}});
  });
  await page.getByRole('button',{name:'Sprawdź płatność',exact:true}).click();
  await expect(page.getByText('Opłacone',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Wyloguj',exact:true}).click();
  await page.goto('/logowanie');
  await page.getByRole('button',{name:/^Administrator ·/}).click();
  await page.getByRole('button',{name:'Wyślij link do logowania',exact:true}).click();
  await page.getByRole('button',{name:'Symuluj potwierdzenie emaila',exact:true}).click();
  await expect(page).toHaveURL(/admin\/zamowienia$/);
  await page.goto('/admin/zamowienia/' + order.id);
  await expect(page.getByRole('heading',{name:order.number,exact:true})).toBeVisible();
  await expect(page.getByText(order.sandboxPayment.offer.name,{exact:true})).toBeVisible();
  await page.setViewportSize({width:1440,height:1000});
  await page.screenshot({path:'tests/artifacts/stripe-order-admin-desktop.png',fullPage:true});
});

test('Rust packs 1000 pieces and mixed parts using dimensions and mass', async ({ request }) => {
  const one = await request.post('/api/shipping/estimate', { data: { parts: [part], delivery: 'courier' } });
  expect(one.ok()).toBeTruthy();
  const small = await one.json();
  const bulk = await (await request.post('/api/shipping/estimate', {
    data: { parts: [{ ...part, quantity: 1000 }], delivery: 'courier' },
  })).json();
  expect(small.status).toBe('estimated');
  expect(bulk.status).toBe('estimated');
  expect(bulk.parcels.length).toBeGreaterThan(small.parcels.length);
  expect(bulk.parcels.flatMap((p: any) => p.placements)).toHaveLength(1000);
  expect(bulk.packaging_grosze).toBeGreaterThan(small.packaging_grosze);
  const mixed = await (await request.post('/api/shipping/estimate', {
    data: { parts: [part, { ...part, id: 'cover', quantity: 7, dimensions_mm: [90, 70, 12], weight_g: 15 }], delivery: 'courier' },
  })).json();
  expect(mixed.parcels.flatMap((p: any) => p.placements)).toHaveLength(8);
  expect(mixed.parcels.every((p: any) => p.weight_g <= 20000)).toBeTruthy();
});

test('missing data, oversize, forged prices and cross-origin calls cannot open checkout', async ({ request }) => {
  for (const bad of [{ ...part, dimensions_mm: null }, { ...part, dimensions_mm: [1800, 1800, 1800] }]) {
    const result = await (await request.post('/api/checkout/quote', { data: { parts: [bad], delivery: 'courier' } })).json();
    expect(result.packing.status).toBe('manual_review');
    expect(result.offers).toHaveLength(0);
    const pay = await request.post('/api/checkout/session', { data: { quote_id: result.id, offer_id: 'pickup' } });
    expect(pay.ok()).toBeFalsy();
  }
  const tampered = await request.post('/api/checkout/quote', { data: { parts: [part], delivery: 'pickup', print_grosze: 1 } });
  expect(tampered.ok()).toBeFalsy();
  const foreign = await request.post('/api/checkout/quote', { headers: { Origin: 'https://evil.example' }, data: { parts: [part], delivery: 'pickup' } });
  expect(foreign.status()).toBe(403);
  const manual = await (await request.post('/api/checkout/quote', { data: { parts: [part], delivery: 'pickup', manual: true } })).json();
  expect(manual.offers).toHaveLength(0);
  const unknown = await request.get('/api/checkout/session/cs_test_forged');
  expect(unknown.status()).toBe(404);
});

test('Stripe sandbox creates one session per server quote and verifies unpaid state', async ({ request }) => {
  const quote = await (await request.post('/api/checkout/quote', { data: { parts: [part], delivery: 'pickup' } })).json();
  test.skip(!quote.stripe_configured, 'Requires an actual server-side Stripe test key.');
  expect(quote.production_demo).toBeTruthy();
  expect(quote.offers).toEqual([{ id: 'pickup', name: 'Odbiór osobisty', price_grosze: 0 }]);
  const input = { data: { quote_id: quote.id, offer_id: 'pickup' } };
  const first = await request.post('/api/checkout/session', input);
  expect(first.ok()).toBeTruthy();
  const checkout = await first.json();
  expect(checkout.session_id).toMatch(/^cs_test_/);
  expect(checkout.url).toMatch(/^https:\/\/checkout.stripe.com\//);
  const second = await (await request.post('/api/checkout/session', input)).json();
  expect(second.session_id).toBe(checkout.session_id);
  const state = await (await request.get(`/api/checkout/session/${checkout.session_id}`)).json();
  expect(state).toMatchObject({ status: 'unpaid', livemode: false, currency: 'pln', quote_id: quote.id, total_grosze: quote.print_grosze + quote.packaging_grosze });
});

test('Furgonetka sandbox prices the full parcel plan and Stripe uses that offer', async ({ request }) => {
  const contact = { name: 'Test odbiorcy', email: 'void@sieciowiec.xyz', phone: '600700800',
    street: 'Floriańska 15', postalCode: '31-021', city: 'Kraków' };
  const getQuote = async (quantity: number) => (await request.post('/api/checkout/quote', {
    data: { parts: [{ ...part, quantity }], delivery: 'courier', contact },
  })).json();
  const small = await getQuote(1);
  test.skip(!small.furgonetka_configured, 'Requires an actual Furgonetka sandbox token and sender.');
  expect(small.offers.length).toBeGreaterThan(0);
  const bulk = await getQuote(1000);
  expect(bulk.packing.parcels.length).toBeGreaterThan(small.packing.parcels.length);
  if (bulk.offers.length) {
    for (const offer of bulk.offers) {
      const single = small.offers.find((o: any) => o.id === offer.id);
      if (single) expect(offer.price_grosze).toBeGreaterThan(single.price_grosze);
    }
  } else expect(bulk.reason).toBeTruthy();
  test.skip(!small.stripe_configured, 'Requires a Stripe test key to verify the combined amount.');
  const session = await (await request.post('/api/checkout/session', {
    data: { quote_id: small.id, offer_id: small.offers[0].id },
  })).json();
  expect(session.session_id).toMatch(/^cs_test_/);
  const status = await (await request.get(`/api/checkout/session/${session.session_id}`)).json();
  expect(status.total_grosze).toBe(small.print_grosze + small.packaging_grosze + small.offers[0].price_grosze);
  expect(status.status).toBe('unpaid');
});
