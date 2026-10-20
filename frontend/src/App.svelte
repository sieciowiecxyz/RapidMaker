<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Header from './components/Header.svelte';
  import DemoTools from './components/DemoTools.svelte';
  import Home from './pages/Home.svelte';
  import Login from './pages/Login.svelte';
  import Configure from './pages/configure/Configure.svelte';
  import Checkout from './pages/checkout/Checkout.svelte';
  import AccountOrders from './pages/account/Orders.svelte';
  import AccountOrder from './pages/account/Order.svelte';
  import AccountSettings from './pages/account/Settings.svelte';
  import AdminOrders from './pages/admin/Orders.svelte';
  import AdminOrder from './pages/admin/Order.svelte';
  import Production from './pages/admin/Production.svelte';
  import Materials from './pages/admin/Materials.svelte';
  import AdminSettings from './pages/admin/Settings.svelte';
  import { getSession, logout } from './lib/api';
  import { navigate } from './lib/router';
  import type { Session } from './lib/types';
  let path = $state(location.pathname.replace(/\/$/, '') || '/');
  let query = $state(location.search);
  let session = $state<Session | null>(null);
  let ready = $state(false);
  let generation = $state(0);
  const adminArea = $derived(path.startsWith('/admin/'));
  const accountArea = $derived(path.startsWith('/konto/'));
  const protectedArea = $derived(adminArea || accountArea);
  const allowed = $derived(!adminArea || session?.role === 'admin');
  const orderId = $derived(path.split('/')[3] || '');
  const tabs = $derived(
    adminArea
      ? [
          ['Zamówienia', '/admin/zamowienia'],
          ['Produkcja', '/admin/produkcja'],
          ['Materiały i ceny', '/admin/materialy'],
          ['Ustawienia', '/admin/ustawienia'],
        ]
      : [
          ['Zamówienia', '/konto/zamowienia'],
          ['Ustawienia konta', '/konto/ustawienia'],
        ],
  );
  const titles: Record<string, string> = {
    '/': 'Druk 3D na zamówienie',
    '/wycena': 'Wycena wydruku',
    '/logowanie': 'Logowanie',
    '/zamowienie': 'Zamówienie',
  };
  const pageTitle = $derived(
    protectedArea && !session
      ? 'Logowanie'
      : !allowed
        ? 'Brak dostępu'
        : (titles[path] ??
          tabs.find(([, href]) => path === href || path.startsWith(href + '/'))?.[0] ??
          'Nie znaleziono strony'),
  );
  async function refreshSession() {
    session = await getSession();
    ready = true;
  }
  async function signedIn() {
    await refreshSession();
    navigate(session?.role === 'admin' ? '/admin/zamowienia' : '/konto/zamowienia');
  }
  async function signedOut() {
    await logout();
    await refreshSession();
    navigate('/');
  }
  async function demoChanged() {
    await refreshSession();
    generation++;
  }
  async function routeChanged() {
    path = location.pathname.replace(/\/$/, '') || '/';
    query = location.search;
    await tick();
    const target = location.hash ? document.getElementById(location.hash.slice(1)) : null;
    if (target instanceof HTMLDetailsElement) target.open = true;
    if (target) target.scrollIntoView({ behavior: 'instant' });
    else window.scrollTo({ top: 0, behavior: 'instant' });
  }
  function localLink(event: MouseEvent) {
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey ||
      event.altKey
    )
      return;
    const anchor = (event.target as Element).closest('a');
    if (!anchor || anchor.target || anchor.hasAttribute('download')) return;
    const url = new URL(anchor.href);
    if (url.origin !== location.origin) return;
    event.preventDefault();
    navigate(url.pathname + url.search + url.hash);
  }
  onMount(() => {
    void refreshSession();
    void routeChanged();
    window.addEventListener('popstate', routeChanged);
    window.addEventListener('rapidmaker:navigate', routeChanged);
    document.addEventListener('click', localLink);
    return () => {
      window.removeEventListener('popstate', routeChanged);
      window.removeEventListener('rapidmaker:navigate', routeChanged);
      document.removeEventListener('click', localLink);
    };
  });
</script>

<svelte:head><title>{pageTitle} — Rapidmaker</title></svelte:head>

<a class="skip-link" href="#tresc">Przejdź do treści</a>
<div class="site-shell" id="top">
  <Header {session} onLogout={signedOut} />
  {#if protectedArea && session && allowed}
    <nav class="account-tabs" aria-label={adminArea ? 'Panel administratora' : 'Panel klienta'}>
      {#each tabs as [label, href]}<a
          {href}
          aria-current={path.startsWith(href) ? 'page' : undefined}>{label}</a
        >{/each}
    </nav>
  {/if}
  <main id="tresc" class="app-main">
    {#key path + query + generation}
      {#if !ready}<div class="page" role="status">Wczytywanie…</div>
      {:else if protectedArea && !session}<Login onSession={signedIn} />
      {:else if !allowed}<section class="page empty-state">
          <h1>Ten panel jest dla obsługi.</h1>
          <p>Przejdź do swoich zamówień albo zaloguj się kontem administratora.</p>
          <a class="button primary" href="/konto/zamowienia">Moje zamówienia</a>
        </section>
      {:else if path === '/'}<Home />
      {:else if path === '/logowanie'}<Login onSession={signedIn} />
      {:else if path === '/wycena'}<Configure />
      {:else if path === '/zamowienie'}<Checkout />
      {:else if path === '/konto/zamowienia'}<AccountOrders />
      {:else if accountArea && path.startsWith('/konto/zamowienia/') && orderId}<AccountOrder
          id={orderId}
        />
      {:else if path === '/konto/ustawienia'}<AccountSettings onSession={refreshSession} />
      {:else if path === '/admin/zamowienia'}<AdminOrders />
      {:else if adminArea && path.startsWith('/admin/zamowienia/') && orderId}<AdminOrder
          id={orderId}
        />
      {:else if path === '/admin/produkcja'}<Production />
      {:else if path === '/admin/materialy'}<Materials />
      {:else if path === '/admin/ustawienia'}<AdminSettings />
      {:else}<section class="page empty-state">
          <h1>Nie znaleźliśmy tej strony.</h1>
          <a class="button primary" href="/">Wróć na stronę główną</a>
        </section>{/if}
    {/key}
  </main>
  {#if new URLSearchParams(query).has('devtools')}<DemoTools onChange={demoChanged} />{/if}
  <footer>
    <a class="brand footer-brand" href="/">rapidmaker<span class="brand-dot">.</span></a><span
      >Pomysły mają kształt. Nadajmy im formę.</span
    >
  </footer>
</div>
