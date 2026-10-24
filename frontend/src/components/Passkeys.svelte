<script lang="ts">
  import { onMount } from 'svelte';
  import { getSession } from '../lib/session';
  import { ApiError, apiRequest } from '../lib/http';
  import { listPasskeys, registerPasskey, removePasskey, type Passkey } from '../lib/passkeys';
  let {
    bootstrapMode = false,
    enrollmentMode = false,
    onSession = () => {},
  }: {
    bootstrapMode?: boolean;
    enrollmentMode?: boolean;
    onSession?: () => void | Promise<void>;
  } = $props();
  let authenticated = $state(false),
    loaded = $state(false),
    busy = $state(false);
  let keys = $state<Passkey[]>([]),
    name = $state(''),
    bootstrap = $state(''),
    error = $state(''),
    success = $state('');
  let reauth = $state(false),
    hasPassword = $state(false);
  let alive = true,
    generation = 0;
  const reauthUrl = '/logowanie?reauth=1&next=%2Fkonto%2Fustawienia';
  function failed(cause: unknown) {
    error =
      cause instanceof DOMException && cause.name === 'NotAllowedError'
        ? 'Nie potwierdzono klucza. Spróbuj ponownie.'
        : cause instanceof Error
          ? cause.message
          : 'Nie udało się zapisać klucza.';
    reauth = cause instanceof ApiError && cause.status === 403;
  }
  async function load() {
    const current = ++generation;
    const session = await getSession();
    if (!alive || current !== generation) return;
    authenticated = !!session;
    if (session) {
      const [result, security] = await Promise.all([
        listPasskeys(),
        apiRequest<{ has_password: boolean }>('/api/account/security'),
      ]);
      if (alive && current === generation) {
        keys = result.passkeys;
        hasPassword = security.has_password === true;
      }
    }
  }
  onMount(() => {
    void (async () => {
      try {
        await load();
      } catch (cause) {
        if (alive) failed(cause);
      } finally {
        if (alive) loaded = true;
      }
    })();
    return () => {
      alive = false;
      generation++;
      bootstrap = '';
      keys = [];
    };
  });
  async function act(action: () => Promise<void>, message = '') {
    if (busy) return;
    busy = true;
    error = '';
    success = '';
    reauth = false;
    try {
      await action();
      if (!alive) return;
      await load();
      if (alive) success = message;
    } catch (cause) {
      if (alive) failed(cause);
    } finally {
      if (alive) busy = false;
    }
  }
  async function add(useBootstrap = false) {
    if (busy) return;
    if (!name.trim()) {
      error = 'Podaj nazwę klucza.';
      return;
    }
    const grant = useBootstrap ? bootstrap : '';
    if (useBootstrap) bootstrap = '';
    await act(async () => {
      await registerPasskey(name, grant || undefined);
      if (!alive) return;
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
</script>

{#if loaded}
  <section class="panel stack" aria-labelledby="passkeys-heading">
    <h2 id="passkeys-heading">Klucze dostępu</h2>
    {#if error}<p class="alert error" role="alert">{error}</p>{/if}
    {#if reauth}<p class="muted">
        Zarządzanie kluczami wymaga świeżego logowania. <a href={reauthUrl}>Zaloguj się ponownie</a
        >, a następnie powtórz zmianę.
      </p>{/if}
    {#if success}<p class="alert success" role="status">{success}</p>{/if}
    {#if authenticated || enrollmentMode}
      <p class="muted">
        Klucz dostępu zastępuje hasło. Drugi etap logowania nadal wymaga kodu email lub aplikacji
        uwierzytelniającej. Zachowaj zapasową metodę logowania.
      </p>
      {#if authenticated}<ul class="keys">
          {#each keys as key (key.id)}<li>
              <span
                ><strong>{key.name}</strong><small class="muted"
                  >Dodano {new Date(key.created_at * 1000).toLocaleDateString('pl-PL')}</small
                ></span
              ><button
                class="button ghost"
                disabled={busy || (keys.length < 2 && !hasPassword)}
                onclick={() => remove(key)}>Usuń klucz</button
              >
            </li>{/each}
        </ul>
        <p class="muted">
          <a href={reauthUrl}>Potwierdź logowanie ponownie</a> przed zmianą kluczy. Serwer chroni ostatnią
          dostępną metodę logowania.
        </p>{/if}
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
    {/if}
    {#if bootstrapMode}
      <h3>Pierwszy klucz lub odzyskanie dostępu</h3>
      <p class="muted">
        Wklej jednorazowe zezwolenie z lokalnego narzędzia operatora. Zezwolenie nie jest zapisywane
        w przeglądarce. O dalszym potwierdzeniu dostępu decyduje aktualny etap logowania.
      </p>
      <form
        class="stack"
        onsubmit={(e) => {
          e.preventDefault();
          void add(true);
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
    {:else if !authenticated && !enrollmentMode}<p class="muted">
        <a href="/logowanie?next=%2Fkonto%2Fustawienia">Zaloguj się</a>, aby zarządzać kluczami
        dostępu.
      </p>{/if}
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
  }
</style>
