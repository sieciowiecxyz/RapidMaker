<script lang="ts">
  import { onMount } from 'svelte';
  import { apiRequest, ApiError, clearAuthToken } from '../lib/http';
  type Security = { has_password: boolean; totp_enabled: boolean };
  let security = $state<Security | null>(null);
  let loading = $state(true),
    busy = $state(false),
    reauth = $state(false);
  let password = $state(''),
    repeat = $state(''),
    code = $state('');
  let setup = $state<{ secret: string; otpauth_uri: string } | null>(null);
  let recoveryCodes = $state<string[]>([]);
  let error = $state(''),
    success = $state('');
  let alive = true,
    generation = 0;
  const reauthUrl = '/logowanie?reauth=1&next=%2Fkonto%2Fustawienia';
  function failed(cause: unknown) {
    error = cause instanceof Error ? cause.message : 'Nie udało się zapisać zmiany.';
    reauth = cause instanceof ApiError && cause.status === 403;
  }
  async function load() {
    const current = ++generation;
    loading = true;
    error = '';
    try {
      const result = await apiRequest<Security>('/api/account/security');
      if (alive && current === generation) {
        security = result;
        if (result.totp_enabled) {
          setup = null;
          code = '';
        }
      }
    } catch (cause) {
      if (alive && current === generation) failed(cause);
    } finally {
      if (alive && current === generation) loading = false;
    }
  }
  onMount(() => {
    void load();
    return () => {
      alive = false;
      generation++;
      password = repeat = code = '';
      setup = null;
      recoveryCodes = [];
    };
  });
  async function act(action: () => Promise<void>) {
    if (busy) return;
    busy = true;
    error = '';
    success = '';
    reauth = false;
    const current = generation;
    try {
      await action();
    } catch (cause) {
      if (alive && current === generation) failed(cause);
    } finally {
      if (alive) busy = false;
    }
  }
  async function savePassword() {
    const characters = Array.from(password).length;
    if (characters < 12 || characters > 128 || new TextEncoder().encode(password).length > 512) {
      error = 'Hasło musi mieć od 12 do 128 znaków, maksymalnie 512 bajtów UTF-8.';
      return;
    }
    if (password !== repeat) {
      error = 'Wpisane hasła są różne.';
      return;
    }
    await act(async () => {
      try {
        await apiRequest('/api/account/password', 'PUT', { password });
        if (!alive) return;
        security = security && { ...security, has_password: true };
        success = 'Hasło zapisane.';
      } finally {
        if (alive) password = repeat = '';
      }
    });
  }
  async function startTotp() {
    await act(async () => {
      const result = await apiRequest<{ secret: string; otpauth_uri: string }>(
        '/api/account/totp/start',
        'POST',
        {},
      );
      if (!alive) return;
      if (!result.secret || !result.otpauth_uri?.startsWith('otpauth://totp/'))
        throw new Error('Nieprawidłowe dane konfiguracji aplikacji uwierzytelniającej.');
      setup = result;
      code = '';
      recoveryCodes = [];
    });
  }
  async function confirmTotp() {
    if (!/^[0-9]{6}$/.test(code)) {
      error = 'Wpisz 6 cyfr z aplikacji.';
      return;
    }
    await act(async () => {
      const result = await apiRequest<{ recovery_codes: string[] }>(
        '/api/account/totp/confirm',
        'POST',
        { code },
      );
      if (!alive) return;
      recoveryCodes = result.recovery_codes;
      setup = null;
      code = '';
      security = security && { ...security, totp_enabled: true };
      success = 'Aplikacja uwierzytelniająca jest włączona. Zapisz kody zapasowe.';
    });
  }
  async function disableTotp() {
    if (
      !confirm(
        'Wyłączyć aplikację uwierzytelniającą? Drugi etap logowania znów będzie wymagał kodu email.',
      )
    )
      return;
    await act(async () => {
      await apiRequest('/api/account/totp', 'DELETE');
      if (!alive) return;
      security = security && { ...security, totp_enabled: false };
      setup = null;
      recoveryCodes = [];
      code = '';
      success = 'Wyłączono aplikację. Drugi etap logowania wymaga kodu email.';
    });
  }
  async function logoutAll() {
    if (!confirm('Wylogować wszystkie urządzenia, także to?')) return;
    await act(async () => {
      await apiRequest('/api/auth/logout-all', 'POST', {});
      clearAuthToken();
      if (alive) location.assign('/logowanie');
    });
  }
</script>

<section class="panel stack" aria-labelledby="account-security-heading">
  <div class="toolbar">
    <h2 id="account-security-heading">Bezpieczeństwo konta</h2>
    <button class="button ghost" disabled={busy || loading} onclick={load}>Odśwież stan</button>
    <button class="button ghost" disabled={busy} onclick={logoutAll}
      >Wyloguj wszystkie urządzenia</button
    >
  </div>
  <p class="muted">
    Logujesz się hasłem lub kluczem dostępu, a potem potwierdzasz dostęp kodem email albo aplikacją
    uwierzytelniającą.
  </p>
  {#if error}<p class="alert error" role="alert">{error}</p>{/if}
  {#if reauth}<p class="muted">
      Ta zmiana wymaga ponownego potwierdzenia logowania. <a href={reauthUrl}
        >Zaloguj się ponownie</a
      >, a następnie powtórz zmianę.
    </p>{/if}
  {#if success}<p class="alert success" role="status">{success}</p>{/if}
  {#if loading}<p role="status">Ładowanie zabezpieczeń…</p>
  {:else if security}
    <section class="security-section stack">
      <h3>{security.has_password ? 'Zmień hasło' : 'Ustaw hasło'}</h3>
      <p class="muted">12–128 znaków. Możesz używać menedżera haseł i wklejać hasło.</p>
      <form
        onsubmit={(event) => {
          event.preventDefault();
          void savePassword();
        }}
        class="stack"
      >
        <div class="password-fields">
          <label
            >Nowe hasło<input
              type="password"
              autocomplete="new-password"
              bind:value={password}
              maxlength="512"
              required
              disabled={busy}
            /></label
          ><label
            >Powtórz hasło<input
              type="password"
              autocomplete="new-password"
              bind:value={repeat}
              maxlength="512"
              required
              disabled={busy}
            /></label
          >
        </div>
        <div><button class="button secondary" disabled={busy}>Zapisz hasło</button></div>
      </form>
    </section>
    <section class="security-section stack">
      <div class="toolbar">
        <h3>Aplikacja uwierzytelniająca</h3>
        <span class="muted">{security.totp_enabled ? 'Włączona' : 'Wyłączona'}</span>
      </div>
      <p class="muted">
        Po włączeniu aplikacji kod z emaila przestaje działać przy logowaniu. Użyjesz kodu z
        aplikacji albo jednorazowego kodu zapasowego.
      </p>
      {#if setup}
        <p>Dodaj konto w swojej aplikacji, wpisując ten klucz:</p>
        <code class="secret">{setup.secret}</code>
        <p><a href={setup.otpauth_uri}>Otwórz w aplikacji uwierzytelniającej</a></p>
        <p class="muted">
          Klucz jest poufny. Nie wysyłaj go innym osobom. Włączenie wymaga potwierdzenia kodem.
        </p>
        <form
          class="totp-form"
          onsubmit={(event) => {
            event.preventDefault();
            void confirmTotp();
          }}
        >
          <label
            >Kod z aplikacji<input
              inputmode="numeric"
              autocomplete="one-time-code"
              pattern={'[0-9]{6}'}
              maxlength="6"
              bind:value={code}
              required
              disabled={busy}
            /></label
          ><button class="button secondary" disabled={busy}>Potwierdź włączenie</button>
        </form>
      {:else if security.totp_enabled}
        <div>
          <button class="button ghost" disabled={busy} onclick={disableTotp}
            >Wyłącz aplikację</button
          >
        </div>
      {:else}
        <div>
          <button class="button secondary" disabled={busy} onclick={startTotp}
            >Włącz aplikację</button
          >
        </div>
      {/if}
      {#if recoveryCodes.length}
        <div class="stack" role="status">
          <h3>Kody zapasowe — zapisz teraz</h3>
          <p class="muted">
            Każdy kod działa tylko raz. Nie pokażemy ich ponownie. Zachowaj je w bezpiecznym miejscu
            poza tym urządzeniem.
          </p>
          <ul class="recovery-codes">
            {#each recoveryCodes as backup}<li><code>{backup}</code></li>{/each}
          </ul>
          <div>
            <button class="button secondary" onclick={() => (recoveryCodes = [])}
              >Zapisałem kody — ukryj</button
            >
          </div>
        </div>
      {/if}
    </section>
  {:else}<div>
      <button
        class="button secondary"
        onclick={() => {
          loading = true;
          error = '';
          void load();
        }}>Spróbuj ponownie</button
      >
    </div>{/if}
</section>

<style>
  .security-section {
    gap: 16px;
  }
  .password-fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  label {
    display: grid;
    gap: 8px;
    min-width: 0;
  }
  .totp-form {
    display: flex;
    align-items: end;
    flex-wrap: wrap;
    gap: 16px;
  }
  .totp-form label {
    max-width: 200px;
  }
  .secret {
    overflow-wrap: anywhere;
    font-size: 16px;
    user-select: all;
  }
  .recovery-codes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px 24px;
    list-style: none;
    padding: 0;
    margin: 0;
    font-size: 14px;
  }
  @media (max-width: 650px) {
    .password-fields,
    .recovery-codes {
      grid-template-columns: 1fr;
    }
  }
</style>
