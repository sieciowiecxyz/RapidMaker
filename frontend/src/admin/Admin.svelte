<script lang="ts">
  import { onMount, tick } from 'svelte';
  import {
    IconLayoutList,
    IconCube,
    IconMessage,
    IconArrowUpRight,
    IconSearch,
    IconX,
    IconChevronRight,
    IconArrowRight,
    IconCheck,
    IconClock,
    IconPackage,
    IconSun,
    IconMoon,
    IconDownload,
    IconCreditCard,
    IconFileInvoice,
  } from '@tabler/icons-svelte';
  import Finance from './Finance.svelte';
  import {
    total,
    pieces,
    money,
    csvCell,
    orderFromServer,
    nextStatus,
    type ServerOrder,
    type Message,
    type OrderFile,
    type Order,
    type Stage,
  } from './data';
  import { getSession, logout } from '../lib/session';
  import { apiRequest, ApiError } from '../lib/http';
  let authorized = $state(false);
  let loading = $state(true);
  let busy = $state(false);
  let orders = $state<Order[]>([]);
  let messagesLoading = $state(false);
  let fileError = $state('');
  let filesLoading = $state(false);
  let detailGeneration = 0;
  let authGeneration = 0;
  async function signOut() {
    try {
      await logout();
      expired();
    } catch (error) {
      failed(error);
    }
  }
  function expired() {
    authGeneration++;
    detailGeneration++;
    authorized = false;
    orders = [];
    selectedId = null;
    location.assign('/logowanie?operator=1&next=%2Fadmin.html');
  }
  function failed(error: unknown) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403)) {
      expired();
      return;
    }
    feedback = error instanceof Error ? error.message : 'Nie udało się połączyć z serwerem.';
  }
  async function load() {
    const generation = ++authGeneration;
    loading = true;
    storageError = '';
    try {
      const session = await getSession();
      if (generation !== authGeneration) return;
      if (session?.role !== 'admin' || !session.adminVerified) {
        expired();
        return;
      }
      authorized = true;
      const data = await apiRequest<{ orders: ServerOrder[] }>('/api/admin/orders');
      if (generation !== authGeneration) return;
      const previous = new Map(orders.map((o) => [o.id, o]));
      orders = data.orders.map((row) => {
        const order = orderFromServer(row),
          old = previous.get(order.id);
        if (old) {
          order.draft = old.draft;
          order.messages = old.messages;
          order.nextAfter = old.nextAfter;
          order.message = old.message;
          enrichFiles(order, old.files);
        }
        return order;
      });
    } catch (error) {
      if (error instanceof ApiError && (error.status === 401 || error.status === 403)) {
        expired();
        return;
      }
      storageError = error instanceof Error ? error.message : 'Nie udało się pobrać zamówień.';
    } finally {
      loading = false;
    }
  }
  let view = $state('Zamówienia');
  let filter = $state('Wszystkie');
  let query = $state('');
  let selectedId = $state<string | null>(null);
  let tab = $state('Modele i wycena');
  let feedback = $state('');
  let storageError = $state('');
  let light = $state(false);
  let panel = $state<HTMLElement>();
  let opener: HTMLElement | null = null;
  let selected = $derived(orders.find((o) => o.id === selectedId));
  let actionCount = $derived(orders.filter((o) => o.attention).length);
  let visible = $derived(
    orders
      .filter((o) =>
        [o.id, o.client, ...o.models.map((m) => m.name)]
          .join(' ')
          .toLocaleLowerCase('pl')
          .includes(query.toLocaleLowerCase('pl')),
      )
      .filter(
        (o) =>
          filter === 'Wszystkie' ||
          (filter === 'Wymaga reakcji' && !!o.attention) ||
          (filter === 'Nowe' && o.status === 'Nowe') ||
          (filter === 'W produkcji' && ['Przygotowanie', 'Drukowanie'].includes(o.status)) ||
          (filter === 'Do wysłania' && o.status === 'Pakowanie'),
      ),
  );
  const production: Stage[] = ['Przygotowanie', 'Drukowanie', 'Pakowanie'];
  const statusTone = (status: Stage) =>
    ({
      Nowe: 'new',
      Przygotowanie: 'preparing',
      Drukowanie: 'printing',
      Pakowanie: 'packing',
      Wysłane: 'sent',
      Zakończone: 'complete',
      Anulowane: 'waiting',
    })[status];
  const swatch = (color: string) =>
    /^#[0-9a-f]{6}$/i.test(color)
      ? color
      : ({ Czarny: '#17191b', Biały: '#f3f1e7', Zielony: '#427c58' }[color] ?? '#858e88');
  const fmt = (d: string) =>
    d
      ? new Intl.DateTimeFormat('pl-PL', { day: 'numeric', month: 'short' }).format(
          new Date(d + 'T12:00:00'),
        )
      : 'Nie ustalono';
  onMount(() => {
    void load();
    window.addEventListener('rapidmaker:session-expired', expired);
    return () => {
      authGeneration++;
      detailGeneration++;
      window.removeEventListener('rapidmaker:session-expired', expired);
    };
  });
  function enrichFiles(o: Order, data: OrderFile[]) {
    o.files = data;
    for (const m of o.models) {
      const matching = data.filter((f) => f.model_id === m.id);
      const f = matching[0];
      if (!f) continue;
      m.name = f.filename;
      if (matching.length !== 1) {
        m.material = 'Kilka konfiguracji';
        continue;
      }
      m.material = f.material_id;
      m.color = f.color_code;
      m.quality =
        f.options?.quality === 'fine'
          ? 'Dokładny · 0,12 mm'
          : f.options?.quality === 'standard'
            ? 'Standard · 0,20 mm'
            : '—';
      m.strength = f.options?.wall_loops ? f.options.wall_loops + ' ściany' : '—';
    }
  }
  async function files(o: Order, generation: number) {
    filesLoading = true;
    try {
      const data = await apiRequest<{ files: OrderFile[] }>(
        `/api/admin/orders/${encodeURIComponent(o.id)}/files`,
      );
      if (generation !== detailGeneration) return;
      enrichFiles(o, data.files);
    } catch (error) {
      if (generation === detailGeneration) {
        fileError = error instanceof Error ? error.message : 'Nie udało się pobrać plików.';
        failed(error);
      }
    } finally {
      if (generation === detailGeneration) filesLoading = false;
    }
  }
  async function messages(o: Order, more = false) {
    if (messagesLoading) return;
    const generation = detailGeneration;
    messagesLoading = true;
    try {
      const data = await apiRequest<{ messages: Message[]; next_after: number | null }>(
        `/api/admin/orders/${encodeURIComponent(o.id)}/messages?after=${more ? (o.messages.at(-1)?.sequence ?? 0) : 0}&limit=50`,
      );
      if (generation !== detailGeneration) return;
      o.messages = more ? [...o.messages, ...data.messages] : data.messages;
      o.nextAfter = data.next_after;
      const last = o.messages.at(-1);
      if (last) {
        o.message = last.body;
        await apiRequest(`/api/admin/orders/${encodeURIComponent(o.id)}/messages/read`, 'POST', {
          sequence: last.sequence,
        });
      }
    } catch (error) {
      if (generation === detailGeneration) failed(error);
    } finally {
      if (generation === detailGeneration) messagesLoading = false;
    }
  }
  async function open(o: Order, event?: MouseEvent) {
    opener = (event?.currentTarget as HTMLElement) ?? null;
    selectedId = o.id;
    messagesLoading = false;
    tab = 'Modele i wycena';
    feedback = '';
    fileError = '';
    const generation = ++detailGeneration;
    void files(o, generation);
    await tick();
    panel?.focus();
  }
  async function openFinance(o: Order, e?: MouseEvent) {
    await open(o, e);
    tab = 'Rozliczenie';
  }
  function close() {
    detailGeneration++;
    selectedId = null;
    messagesLoading = false;
    opener?.focus();
  }
  async function updateStatus(o: Order, status: string) {
    const next = nextStatus(o);
    if (busy || !next || next.state !== status) return;
    busy = true;
    try {
      await apiRequest(`/api/admin/orders/${encodeURIComponent(o.id)}/status`, 'POST', { status });
      await load();
      feedback = 'Status zapisany.';
    } catch (error) {
      failed(error);
    } finally {
      busy = false;
    }
  }
  async function advance(o: Order) {
    const next = nextStatus(o);
    if (next) await updateStatus(o, next.state);
  }
  async function send(o: Order) {
    const body = o.draft.trim();
    if (!body || busy) return;
    busy = true;
    feedback = '';
    try {
      await apiRequest(`/api/admin/orders/${encodeURIComponent(o.id)}/messages`, 'POST', { body });
      o.draft = '';
      await messages(o, true);
      feedback = 'Wiadomość wysłana.';
    } catch (error) {
      failed(error);
    } finally {
      busy = false;
    }
  }
  function exportCsv() {
    const lines = [
      ['Zamówienie', 'Klient', 'Status', 'Płatność', 'Termin', 'Wartość'],
      ...visible.map((o) => [
        o.id,
        o.client,
        o.status,
        o.paid ? 'Opłacone' : 'Nieopłacone',
        o.due,
        String(total(o)),
      ]),
    ];
    const url = URL.createObjectURL(
      new Blob(['\uFEFF' + lines.map((r) => r.map(csvCell).join(';')).join('\n')], {
        type: 'text/csv;charset=utf-8',
      }),
    );
    const a = document.createElement('a');
    a.href = url;
    a.download = 'rapidmaker-zamowienia.csv';
    a.click();
    URL.revokeObjectURL(url);
  }
  function theme() {
    light = !light;
    document.documentElement.dataset.theme = light ? 'light' : 'dark';
  }
  function keyboard(e: KeyboardEvent) {
    if (e.key === 'Escape' && selected) close();
  }
</script>

<svelte:window onkeydown={keyboard} />
<svelte:head><title>{view} · RapidMaker Admin</title></svelte:head>
{#if authorized}
  <div class="admin-shell">
    <aside class="sidebar">
      <a class="brand" href="/"
        ><img src="/logo.svg" alt="" />RapidMaker<span>Panel operatora</span></a
      >
      <nav aria-label="Panel administratora">
        {#each [{ name: 'Zamówienia', icon: IconLayoutList }, { name: 'Produkcja', icon: IconCube }, { name: 'Wiadomości', icon: IconMessage }, { name: 'Płatności', icon: IconCreditCard }, { name: 'Faktury', icon: IconFileInvoice }] as item}
          <button
            class:active={view === item.name}
            onclick={() => {
              view = item.name;
              selectedId = null;
            }}
            ><item.icon size={19} />{item.name}{#if item.name === 'Zamówienia'}<span
                class="nav-count">{orders.length}</span
              >{/if}</button
          >
        {/each}
      </nav>
      <div class="side-bottom">
        <a href="/admin/materialy">Materiały i ceny</a>
        <a href="/admin/ustawienia">Ustawienia wyceny</a>
        <button class="text-button" onclick={signOut}>Wyloguj</button>
        <a href="/" target="_blank" rel="noreferrer">Otwórz stronę <IconArrowUpRight size={17} /></a
        >
        <div class="operator">
          <span class="avatar">RM</span>
          <div><strong>RapidMaker</strong><small>Administrator</small></div>
        </div>
      </div>
    </aside>
    <main>
      <header class="topbar">
        <span>Pracownia <span class="slash">/</span> {view}</span>
        <div>
          <button class="text-button" disabled={loading} onclick={load}>Odśwież</button><button
            class="icon"
            onclick={theme}
            aria-label={light ? 'Włącz ciemny motyw' : 'Włącz jasny motyw'}
            >{#if light}<IconMoon size={18} />{:else}<IconSun size={18} />{/if}</button
          >
        </div>
      </header>
      <div class="workspace">
        <div class="page-heading">
          <div>
            <h1>{view}</h1>
            <p>
              {view === 'Zamówienia'
                ? 'Od pierwszego pliku do gotowej części. Wszystko w jednym miejscu.'
                : view === 'Płatności'
                  ? 'Rozliczenia zamówień. Kwoty i płatności pod kontrolą.'
                  : view === 'Faktury'
                    ? 'Dokumenty powiązane z zamówieniami, od szkicu do wystawienia.'
                    : view === 'Produkcja'
                      ? 'Kolejka pracy i postęp realizacji poszczególnych modeli.'
                      : 'Zapytania klientów przypisane do konkretnych zamówień.'}
            </p>
          </div>
          {#if view === 'Zamówienia'}<button class="secondary" onclick={exportCsv}
              ><IconDownload size={17} />Eksport CSV</button
            >{/if}
        </div>
        {#if storageError}<p class="warning" role="alert">{storageError}</p>{/if}
        {#if loading}<p role="status">Pobieranie zamówień…</p>{/if}
        {#if view === 'Zamówienia'}
          <div class="attention-strip">
            <div>
              <span class="signal"></span><strong>Wymaga reakcji: {actionCount}</strong><span
                class="strip-detail">Pozycje wymagające sprawdzenia przez operatora.</span
              >
            </div>
            <button class="text-button" onclick={() => (filter = 'Wymaga reakcji')}
              >Sprawdź kolejkę <IconArrowRight size={17} /></button
            >
          </div>
          <div class="list-tools">
            <div class="filters" aria-label="Filtr zamówień">
              {#each ['Wszystkie', 'Wymaga reakcji', 'Nowe', 'W produkcji', 'Do wysłania'] as f}<button
                  class:chosen={filter === f}
                  aria-pressed={filter === f}
                  onclick={() => (filter = f)}
                  >{f}{#if f === 'Wszystkie'}<span>{orders.length}</span>{/if}</button
                >{/each}
            </div>
            <label class="search"
              ><IconSearch size={18} /><input
                bind:value={query}
                placeholder="Szukaj numeru, klienta, pliku…"
                aria-label="Szukaj zamówień"
              />{#if query}<button
                  class="icon"
                  onclick={() => (query = '')}
                  aria-label="Wyczyść wyszukiwanie"><IconX size={16} /></button
                >{/if}</label
            >
          </div>
          <div class="table-scroll">
            <table>
              <thead
                ><tr
                  ><th>Zamówienie / klient</th><th>Modele</th><th>Realizacja</th><th>Płatność</th
                  ><th>Termin</th><th class="amount">Wartość</th><th
                    ><span class="sr-only">Szczegóły</span></th
                  ></tr
                ></thead
              ><tbody>
                {#each visible as o}<tr class:selected={selectedId === o.id}
                    ><td
                      ><button class="order-link" onclick={(e) => open(o, e)}
                        >{o.id}<IconArrowUpRight size={14} /></button
                      ><strong class="client">{o.client}</strong><small
                        >{fmt(o.date)} · formularz strony</small
                      ></td
                    ><td
                      ><strong
                        >{o.models.length}
                        {o.models.length === 1 ? 'model' : 'modele'}
                        <span class="muted">/ {pieces(o)} szt.</span></strong
                      >
                      <div class="material-colors">
                        {#each [...new Set(o.models.map((m) => m.material))] as material}<span
                            class="material-tag">{material}</span
                          >{/each}{#each [...new Set(o.models.map((m) => m.color))] as color}<span
                            class="color-label"
                            ><span
                              class="color-swatch"
                              style:background={swatch(color)}
                              aria-hidden="true"
                            ></span>{color}</span
                          >{/each}
                      </div></td
                    ><td
                      ><span
                        class="status"
                        data-status={statusTone(o.status)}
                        class:printing={o.status === 'Drukowanie'}
                        class:new-order={o.status === 'Nowe'}
                        >{#if o.status === 'Drukowanie'}<span class="signal"
                          ></span>{/if}{o.status}</span
                      >{#if o.attention}<small class="attention">{o.attention}</small>{/if}</td
                    ><td
                      ><span class="payment" class:paid={o.paid}
                        >{#if o.paid}<IconCheck size={14} />{:else}<IconClock
                            size={14}
                          />{/if}{o.payment}</span
                      ></td
                    ><td><strong>{fmt(o.due)}</strong><small>{o.delivery}</small></td><td
                      class="amount">{money(total(o))}</td
                    ><td
                      ><button
                        class="icon"
                        onclick={(e) => open(o, e)}
                        aria-label={`Otwórz ${o.id}`}><IconChevronRight size={18} /></button
                      ></td
                    ></tr
                  >{/each}
              </tbody>
            </table>
            {#if !visible.length}<div class="empty">
                <IconSearch size={28} />
                <h2>Brak pasujących zamówień</h2>
                <p>Zmień frazę lub pokaż wszystkie zamówienia.</p>
                <button
                  class="secondary"
                  onclick={() => {
                    query = '';
                    filter = 'Wszystkie';
                  }}>Wyczyść filtry</button
                >
              </div>{/if}
          </div>
          <footer class="list-footer">
            <span>{visible.length} z {orders.length} zamówień</span><span>Kwoty zamówień · PLN</span
            >
          </footer>
        {:else if view === 'Produkcja'}
          <div class="board">
            {#each production as stage}<section class="lane">
                <h2>
                  <span class="status" data-status={statusTone(stage)}>{stage}</span><span
                    >{orders.filter((o) => o.status === stage).length}</span
                  >
                </h2>
                {#each orders.filter((o) => o.status === stage) as o}<article
                    class="production-card"
                  >
                    <button class="order-link" onclick={(e) => open(o, e)}
                      >{o.id}<IconArrowUpRight size={15} /></button
                    >
                    <h3>{o.client}</h3>
                    <p>
                      {[...new Set(o.models.map((m) => m.material))].join(', ')} · {pieces(o)} szt.
                    </p>
                    <div class="production-progress">
                      <span>{pieces(o)} szt. w zamówieniu</span><span>{fmt(o.due)}</span>
                    </div>
                    <button
                      class="secondary"
                      disabled={busy || !nextStatus(o)}
                      onclick={() => advance(o)}
                      >{nextStatus(o)
                        ? 'Dalej: ' + nextStatus(o)!.label
                        : 'Nadanie w logistyce'}<IconArrowRight size={15} /></button
                    >
                  </article>{:else}<p class="lane-empty">Brak zleceń na tym etapie.</p>{/each}
              </section>{/each}
          </div>
        {:else if view === 'Płatności' || view === 'Faktury'}
          {#key view}<Finance {orders} mode={view} onopen={openFinance} />{/key}
        {:else}
          <div class="inbox">
            {#each orders as o}<button
                class="message-row"
                onclick={async (e) => {
                  await open(o, e);
                  tab = 'Wiadomości';
                  void messages(o);
                }}
                ><span class="avatar"
                  >{o.client
                    .split(' ')
                    .map((w) => w[0])
                    .slice(0, 2)
                    .join('')}</span
                ><span
                  ><strong>{o.client} <small>{o.id}</small></strong><span>{o.message}</span></span
                ><IconChevronRight size={20} /></button
              >{/each}
          </div>
        {/if}
      </div>
    </main>
    {#if selected}
      <aside class="detail" aria-label={`Szczegóły ${selected.id}`} tabindex="-1" bind:this={panel}>
        <div class="detail-header">
          <div>
            <span class="muted">Zamówienie</span>
            <h2>{selected.id}</h2>
          </div>
          <button class="icon" aria-label="Zamknij szczegóły" onclick={close}
            ><IconX size={21} /></button
          >
        </div>
        <div class="detail-person">
          <h3>{selected.client}</h3>
          <span>{selected.email}</span><small>Złożono {fmt(selected.date)}</small>
        </div>
        <div class="detail-summary">
          <label
            >Etap realizacji<select
              value={selected.state}
              disabled={busy || !nextStatus(selected)}
              onchange={(e) => updateStatus(selected!, e.currentTarget.value)}
              ><option value={selected.state}>{selected.status}</option
              >{#if nextStatus(selected)}<option value={nextStatus(selected)!.state}
                  >{nextStatus(selected)!.label}</option
                >{/if}</select
            ></label
          ><label
            >Termin realizacji<input
              type="date"
              value={selected.due}
              disabled
              title="Planowanie terminów nie jest jeszcze dostępne"
            /></label
          >
        </div>
        <div class="detail-tabs" aria-label="Sekcje zamówienia">
          {#each ['Modele i wycena', 'Pliki', 'Rozliczenie', 'Wiadomości', 'Notatki', 'Historia'] as t}<button
              class:chosen={tab === t}
              aria-pressed={tab === t}
              onclick={() => {
                tab = t;
                if (t === 'Wiadomości') void messages(selected!);
              }}>{t}</button
            >{/each}
        </div>
        <div class="detail-body">
          {#if tab === 'Modele i wycena'}
            <div class="section-title">
              <h3>Pliki do realizacji</h3>
              <span
                >{selected.models.length}
                {selected.models.length === 1 ? 'model' : 'modele'} · {pieces(selected)} szt.</span
              >
            </div>
            {#each selected.models as m}<section class="model">
                <div class="model-name">
                  <span class="file-icon"><IconCube size={24} /></span>
                  <div>
                    <strong>{m.name}</strong><small>{m.qty} szt.</small>
                    <div class="material-colors">
                      <span class="material-tag">{m.material}</span><span class="color-label"
                        ><span
                          class="color-swatch"
                          style:background={swatch(m.color)}
                          aria-hidden="true"
                        ></span>{m.color}</span
                      >
                    </div>
                  </div>
                </div>
                <dl>
                  <div>
                    <dt>Jakość</dt>
                    <dd>{m.quality}</dd>
                  </div>
                  <div>
                    <dt>Wytrzymałość</dt>
                    <dd>{m.strength}</dd>
                  </div>
                </dl>
                <div class="model-controls">
                  <label
                    >Cena / szt. (zł)<input
                      type="number"
                      value={m.price}
                      disabled
                      title="Cena z zapisanej wyceny"
                    /></label
                  ><label
                    >Gotowe / {m.qty}<input
                      type="number"
                      disabled
                      placeholder="—"
                      title="Postęp pojedynczych sztuk nie jest obsługiwany"
                    /></label
                  ><strong>{money(m.amount)}</strong>
                </div>
              </section>{/each}
            <div class="payment-box">
              <div><strong>Płatność</strong><small>{selected.payment}</small></div>
              <span class="muted">Status potwierdza operator płatności</span>
            </div>
            <section class="delivery">
              <h3><IconPackage size={18} />{selected.delivery}</h3>
              <p>{selected.address}</p>
            </section>
          {:else if tab === 'Rozliczenie'}
            {#key selected.id}<Finance
                {orders}
                {selected}
                mode="Rozliczenie"
                onopen={openFinance}
              />{/key}
          {:else if tab === 'Pliki'}
            <div class="section-title">
              <h3>Pliki zamówienia</h3>
              <span>{selected.models.length} pozycji</span>
            </div>
            {#if fileError}<p class="warning" role="alert">{fileError}</p>{/if}
            {#each selected.files as file}
              <section class="order-file">
                <div class="model-name">
                  <span class="file-icon"><IconCube size={24} /></span>
                  <div>
                    <strong>{file.filename}</strong><small
                      >{file.available ? 'Plik źródłowy' : 'Plik nie jest już dostępny'}</small
                    >
                  </div>
                </div>
                <div class="file-actions">
                  {#if file.available && file.source_url?.startsWith(`/api/admin/orders/${selected.id}/files/`)}<a
                      class="secondary"
                      href={file.source_url}
                      download={file.filename}><IconDownload size={16} />Pobierz</a
                    >{/if}<button
                    class="secondary"
                    disabled
                    title="Zmiana plików zamówienia nie jest dostępna">Zmień plik</button
                  >
                </div>
              </section>
            {:else}<p class="helper">
                {filesLoading ? 'Pobieranie plików…' : 'Brak dostępnych plików zamówienia.'}
              </p>{/each}
          {:else if tab === 'Wiadomości'}
            <h3>Rozmowa z klientem</h3>
            {#each selected.messages as message}<div class="bubble">
                <strong>{message.operator ? 'RapidMaker' : selected.client}</strong><small
                  >{new Date(message.created_at * 1000).toLocaleString('pl-PL')}</small
                >
                <p style="white-space:pre-wrap">{message.body}</p>
              </div>{:else}<p class="helper">
                {messagesLoading ? 'Pobieranie rozmowy…' : 'Nie ma jeszcze wiadomości.'}
              </p>{/each}
            {#if selected.nextAfter !== null}<button
                class="secondary"
                disabled={messagesLoading}
                onclick={() => messages(selected!, true)}>Pokaż kolejne</button
              >{/if}
            <form
              onsubmit={(e) => {
                e.preventDefault();
                void send(selected!);
              }}
            >
              <label class="message-label"
                >Twoja odpowiedź<textarea
                  rows="5"
                  maxlength="4000"
                  bind:value={selected.draft}
                  placeholder="Napisz odpowiedź dotyczącą realizacji…"></textarea></label
              ><button class="primary" disabled={busy || messagesLoading || !selected.draft.trim()}
                >Wyślij wiadomość</button
              >
            </form>
          {:else if tab === 'Notatki'}
            <h3>Notatki wewnętrzne</h3>
            <p class="helper">Notatki wewnętrzne nie są jeszcze obsługiwane przez serwer.</p>
            <textarea rows="4" disabled aria-label="Notatka wewnętrzna"></textarea>
          {:else}<h3>Historia realizacji</h3>
            <ol class="history">
              {#each selected.history as h}<li>{h}</li>{/each}
            </ol>{/if}
        </div>
        <div class="detail-footer">
          <p role="status">{feedback}</p>
          <div>
            <span>Kwota zamówienia<strong>{money(total(selected))}</strong></span><button
              class="primary"
              disabled={busy || !nextStatus(selected)}
              onclick={() => advance(selected!)}
              >{nextStatus(selected)
                ? 'Dalej: ' + nextStatus(selected)!.label
                : selected.status}<IconArrowRight size={17} /></button
            >
          </div>
        </div>
      </aside>
    {/if}
  </div>
{:else}<main class="workspace">
    <p role="status">
      {loading ? 'Sprawdzanie dostępu…' : storageError || 'Zaloguj się jako administrator.'}
    </p>
  </main>{/if}
