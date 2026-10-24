<script lang="ts">
  import { onMount } from 'svelte';
  import Passkeys from '../components/Passkeys.svelte';
  import * as auth from '../lib/session';
  import { apiRequest } from '../lib/http';
  import type { EmailChallenge } from '../lib/types';
  let { onSession }: { onSession: () => void | Promise<void> } = $props();
  const params = new URLSearchParams(location.search);
  let email = $state(params.get('email') ?? '');
  let register = $state(params.get('register') === '1');
  let challenge = $state<EmailChallenge | null>(null);
  let current = $state<auth.AuthState | null>(null);
  let password = $state(''),
    repeated = $state(''),
    code = $state('');
  let recovery = $state(false),
    busy = $state(true),
    error = $state(''),
    notice = $state('');
  const factor = $derived(current?.stage === 'second_factor');
  const setup = $derived(current?.stage === 'setup');
  const totp = $derived(factor && current?.second_factor === 'totp');
  async function advance(value: auth.AuthState | null) {
    current = value;
    if (value?.stage === 'authenticated') await onSession();
  }
  onMount(async () => {
    try {
      const value = await auth.getAuthState();
      if (value) email = value.user.email;
      if (value?.stage === 'authenticated' && params.has('reauth')) current = null;
      else await advance(value);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  });
  async function act(action: () => Promise<void>) {
    if (busy) return;
    busy = true;
    error = '';
    notice = '';
    try {
      await action();
    } catch (e) {
      error =
        e instanceof DOMException && e.name === 'NotAllowedError'
          ? 'Nie potwierdzono klucza dostępu. Spróbuj ponownie.'
          : (e as Error).message;
    } finally {
      busy = false;
    }
  }
  function credentials() {
    const value = password;
    password = '';
    repeated = '';
    return value;
  }
  async function passwordLogin() {
    if (busy) return;
    const value = credentials();
    await act(async () => advance(await auth.loginPassword(email, value)));
  }
  async function passkeyLogin() {
    if (busy || !email.trim()) {
      error = 'Podaj email konta.';
      return;
    }
    await act(async () => advance(await auth.loginPasskey(email)));
  }
  async function requestEmail() {
    await act(async () => {
      challenge = await auth.requestLogin(email, true);
      code = '';
    });
  }
  async function verify() {
    const value = code;
    code = '';
    await act(async () => {
      const result = challenge
        ? await auth.confirmLogin(challenge.id, value)
        : await auth.verifyFactor(value);
      challenge = null;
      recovery = false;
      await advance(result);
    });
  }
  async function setPassword() {
    if (busy) return;
    if (password !== repeated) {
      error = 'Hasła muszą być takie same.';
      return;
    }
    if (Array.from(password).length < 12) {
      error = 'Hasło musi mieć co najmniej 12 znaków.';
      return;
    }
    const value = credentials();
    await act(async () => {
      await apiRequest('/api/account/password', 'PUT', { password: value });
      await advance(await auth.getAuthState());
    });
  }
  async function changeAccount() {
    await act(async () => {
      if (current) await auth.logout();
      current = null;
      challenge = null;
      password = '';
      repeated = '';
      code = '';
      recovery = false;
    });
  }
  async function registeredPasskey() {
    await advance(await auth.getAuthState());
  }
</script>

<section class="page login-page">
  <div class="page-heading">
    <span class="badge">Twoje projekty, w jednym miejscu</span>
    <h1>
      {setup
        ? 'Zabezpiecz swoje konto.'
        : factor
          ? 'Jeszcze jeden krok.'
          : register
            ? 'Zacznijmy od emaila.'
            : 'Dobrze Cię widzieć.'}
    </h1>
    <p class="muted">
      {setup
        ? 'Email potwierdzony. Ustaw hasło lub dodaj klucz dostępu.'
        : 'Twoje zamówienia i wiadomości — bezpiecznie w jednym miejscu.'}
    </p>
  </div>
  <div class="panel stack login-panel" aria-busy={busy}>
    {#if error}<p class="alert error" role="alert">{error}</p>{/if}
    {#if notice}<p class="alert success" role="status">{notice}</p>{/if}
    {#if setup}
      <form
        class="stack"
        onsubmit={(e) => {
          e.preventDefault();
          void setPassword();
        }}
      >
        <label
          >Nowe hasło<input
            type="password"
            bind:value={password}
            minlength="12"
            maxlength="128"
            autocomplete="new-password"
            required
            disabled={busy}
          /></label
        >
        <label
          >Powtórz hasło<input
            type="password"
            bind:value={repeated}
            minlength="12"
            maxlength="128"
            autocomplete="new-password"
            required
            disabled={busy}
          /></label
        >
        <small class="muted">12–128 znaków. Użyj unikalnego hasła.</small>
        <button class="button primary" disabled={busy}>Ustaw hasło i przejdź dalej</button>
      </form>
      <Passkeys enrollmentMode onSession={registeredPasskey} />
    {:else if challenge || factor}
      <h2>
        {recovery
          ? 'Kod ratunkowy'
          : totp
            ? 'Kod z aplikacji uwierzytelniającej'
            : 'Sprawdź swoją skrzynkę'}
      </h2>
      <p class="muted">
        {recovery
          ? 'Wpisz jeden z zapisanych kodów. Każdy działa tylko raz.'
          : totp
            ? 'Wpisz aktualny 6-cyfrowy kod. Email nie zastępuje aplikacji.'
            : `Wysłaliśmy 8-cyfrowy kod na ${challenge?.email ?? current?.user.email ?? email}.`}
      </p>
      <form
        class="stack"
        onsubmit={(e) => {
          e.preventDefault();
          void verify();
        }}
      >
        <label for="login-code"
          >{recovery ? 'Kod ratunkowy' : totp ? 'Kod z aplikacji' : 'Kod z emaila'}
          <input
            id="login-code"
            type="text"
            inputmode={recovery ? 'text' : 'numeric'}
            autocomplete="one-time-code"
            pattern={recovery ? undefined : totp ? '[0-9]{6}' : '[0-9]{8}'}
            minlength={recovery ? 1 : totp ? 6 : 8}
            maxlength={recovery ? 128 : totp ? 6 : 8}
            bind:value={code}
            required
            disabled={busy}
          />
        </label>
        <button class="button primary" disabled={busy}
          >{busy ? 'Sprawdzanie…' : 'Potwierdź i zaloguj'}</button
        >
      </form>
      <div class="toolbar">
        {#if totp}<button
            class="button ghost"
            disabled={busy}
            onclick={() => {
              recovery = !recovery;
              code = '';
              error = '';
            }}>{recovery ? 'Wróć do kodu z aplikacji' : 'Użyj kodu ratunkowego'}</button
          >
        {:else}<button
            class="button secondary"
            disabled={busy}
            onclick={() =>
              challenge
                ? requestEmail()
                : act(async () => {
                    await auth.resendFactor();
                    notice = 'Wysłano nowy kod.';
                  })}>Wyślij kod ponownie</button
          >{/if}
        <button class="button ghost" disabled={busy} onclick={changeAccount}>Zmień konto</button>
      </div>
    {:else if register}
      <form
        class="stack"
        onsubmit={(e) => {
          e.preventDefault();
          void requestEmail();
        }}
      >
        <label for="email"
          >Adres email<input
            id="email"
            type="email"
            bind:value={email}
            autocomplete="email"
            maxlength="254"
            required
            disabled={busy}
          /></label
        >
        <button class="button primary" disabled={busy}>Potwierdź email</button>
      </form>
      <p class="muted">
        Najpierw potwierdzisz email, potem ustawisz hasło lub passkey. Jeśli konto ma już metodę
        logowania, użyj jej poniżej.
      </p>
      <button
        class="button ghost"
        disabled={busy}
        onclick={() => {
          register = false;
          error = '';
        }}>Mam już konto</button
      >
    {:else}
      <form
        class="stack"
        onsubmit={(e) => {
          e.preventDefault();
          void passwordLogin();
        }}
      >
        <label for="email"
          >Adres email<input
            id="email"
            type="email"
            bind:value={email}
            autocomplete="username webauthn"
            maxlength="254"
            required
            disabled={busy}
          /></label
        >
        <label for="password"
          >Hasło<input
            id="password"
            type="password"
            bind:value={password}
            autocomplete="current-password"
            maxlength="128"
            required
            disabled={busy}
          /></label
        >
        <button class="button primary" disabled={busy}
          >{busy ? 'Logowanie…' : 'Zaloguj hasłem'}</button
        >
        <button
          class="button secondary"
          type="button"
          disabled={busy || !email.trim()}
          onclick={passkeyLogin}>Zaloguj passkey</button
        >
      </form>
      <p class="muted">
        Po haśle lub passkey potwierdzisz logowanie kodem email. Jeśli masz aplikację
        uwierzytelniającą, użyjesz jej zamiast emaila.
      </p>
      <button
        class="button ghost"
        disabled={busy}
        onclick={() => {
          register = true;
          error = '';
        }}>Utwórz konto lub zabezpiecz dotychczasowe</button
      >
    {/if}
    <a href="/wycena">Możesz też zamówić jako gość →</a>
  </div>
</section>

<style>
  .login-page {
    max-width: 560px;
    margin-inline: auto;
  }
  .login-panel {
    max-width: 560px;
    margin: 24px auto;
    border: 0;
    padding: 0;
  }
  .page-heading {
    text-align: center;
  }
  label {
    display: grid;
    gap: 8px;
  }
  small {
    line-height: 1.6;
  }
</style>
