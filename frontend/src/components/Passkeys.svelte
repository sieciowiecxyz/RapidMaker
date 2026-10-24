<script lang="ts">
  import { onMount } from 'svelte';
  import { getSession, loginPasskey } from '../lib/session';
  import { apiRequest, clearAuthToken } from '../lib/http';
  import { listPasskeys, registerPasskey, removePasskey, type Passkey } from '../lib/passkeys';
  let {
    bootstrapMode = false,
    onSession = () => {},
  }: { bootstrapMode?: boolean; onSession?: () => void | Promise<void> } = $props();
  let admin = $state(false),
    loaded = $state(false),
    busy = $state(false);
  let keys = $state<Passkey[]>([]),
    name = $state(''),
    bootstrap = $state(''),
    error = $state(''),
    success = $state('');
  async function load() {
    const session = await getSession();
    admin = session?.role === 'admin' && !!session.adminVerified;
    if (admin) keys = (await listPasskeys()).passkeys;
  }
  onMount(async () => {
    try {
      await load();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loaded = true;
    }
  });
  async function act(action: () => Promise<void>, message = '') {
    if (busy) return;
    busy = true;
    error = '';
    success = '';
    try {
      await action();
      await load();
      success = message;
    } catch (e) {
      error =
        e instanceof DOMException && e.name === 'NotAllowedError'
          ? 'Nie potwierdzono klucza. Spróbuj ponownie.'
          : (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function add() {
    if (busy) return;
    if (!name.trim()) {
      error = 'Podaj nazwę klucza.';
      return;
    }
    const grant = bootstrap;
    bootstrap = '';
    await act(async () => {
      await registerPasskey(name, grant || undefined);
      name = '';
      await onSession();
    }, 'Klucz dostępu został zapisany.');
  }
  async function remove(key: Passkey) {
    if (!confirm(`Usunąć klucz „${key.name}”?`)) return;
    await act(async () => {
      await removePasskey(key.id);
    }, 'Klucz został usunięty.');
  }
  async function logoutAll() {
    if (!confirm('Wylogować wszystkie urządzenia, także to?')) return;
    busy = true;
    error = '';
    try {
      await apiRequest('/api/auth/logout-all', 'POST', {});
      clearAuthToken();
      location.assign('/logowanie');
    } catch (e) {
      error = (e as Error).message;
      busy = false;
    }
  }
</script>

{#if loaded}
  <section class="panel stack" aria-labelledby="security-heading">
    <div class="toolbar">
      <h2 id="security-heading">Bezpieczeństwo konta</h2>
      <button class="button ghost" disabled={busy} onclick={logoutAll}
        >Wyloguj wszystkie urządzenia</button
      >
    </div>
    {#if error}<p class="alert error" role="alert">{error}</p>{/if}
    {#if success}<p class="alert success" role="status">{success}</p>{/if}
    {#if admin}
      <p class="muted">Klucze dostępu administratora. Zachowaj co najmniej jeden klucz zapasowy.</p>
      <ul class="keys">
        {#each keys as key (key.id)}<li>
            <span
              ><strong>{key.name}</strong><small class="muted"
                >Dodano {new Date(key.created_at * 1000).toLocaleDateString('pl-PL')}</small
              ></span
            ><button
              class="button ghost"
              disabled={busy || keys.length < 2}
              onclick={() => remove(key)}>Usuń klucz</button
            >
          </li>{/each}
      </ul>
      <button
        class="button secondary"
        disabled={busy}
        onclick={() =>
          act(async () => {
            await loginPasskey();
          }, 'Potwierdzono dostęp. Możesz zarządzać kluczami.')}>Potwierdź dostęp passkey</button
      >
      <form
        class="key-form"
        onsubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <label
          >Nazwa nowego klucza<input
            bind:value={name}
            required
            maxlength="64"
            autocomplete="off"
            placeholder="Np. klucz zapasowy"
          /></label
        ><button class="button primary" disabled={busy || keys.length >= 5}
          >{busy ? 'Zapisywanie…' : 'Dodaj klucz dostępu'}</button
        >
      </form>
    {:else if bootstrapMode}
      <h3>Pierwszy klucz lub odzyskanie dostępu</h3>
      <p class="muted">
        Wklej jednorazowe zezwolenie z lokalnego narzędzia operatora. Sam email nie daje dostępu
        administratora. Zezwolenie nie jest zapisywane w przeglądarce.
      </p>
      <form
        class="stack"
        onsubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <label
          >Nazwa klucza<input bind:value={name} required maxlength="64" autocomplete="off" /></label
        ><label
          >Jednorazowe zezwolenie<input
            type="password"
            bind:value={bootstrap}
            required
            autocomplete="off"
          /></label
        ><button class="button primary" disabled={busy}
          >{busy ? 'Potwierdzanie…' : 'Zarejestruj klucz operatora'}</button
        >
      </form>
    {/if}
  </section>
{/if}

<style>
  h2 {
    margin: 0;
  }
  .keys {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .keys li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 14px 0;
  }
  .keys li + li {
    border-top: 1px solid var(--color-border);
  }
  .keys small {
    display: block;
    margin-top: 4px;
  }
  .key-form {
    display: flex;
    align-items: end;
    gap: 16px;
  }
  label {
    display: grid;
    gap: 8px;
    flex: 1;
  }
  .key-form button {
    flex-shrink: 0;
  }
  @media (max-width: 640px) {
    .key-form {
      display: grid;
    }
    .toolbar {
      align-items: start;
    }
  }
</style>
