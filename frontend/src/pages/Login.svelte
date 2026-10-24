<script lang="ts">
  import Passkeys from '../components/Passkeys.svelte';
  import { onMount } from 'svelte';
  import * as auth from '../lib/session';
  import type { EmailChallenge } from '../lib/types';
  let { onSession }: { onSession: () => void | Promise<void> } = $props();
  let email = $state(new URLSearchParams(location.search).get('email') ?? '');
  let register = $state(new URLSearchParams(location.search).get('register') === '1');
  let challenge = $state<EmailChallenge | null>(null);
  let pendingAdmin = $state(false);
  let code = $state('');
  let busy = $state(true),
    error = $state('');

  onMount(async () => {
    try {
      const current = await auth.getSession();
      pendingAdmin = current?.role === 'admin' && !current.adminVerified;
      if (current && !pendingAdmin) await onSession();
    } catch {
      error = 'Nie udało się sprawdzić sesji. Spróbuj ponownie.';
    } finally {
      busy = false;
    }
  });
  async function request() {
    busy = true;
    error = '';
    try {
      challenge = await auth.requestLogin(email, register);
      code = '';
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function confirm() {
    if (!challenge) return;
    busy = true;
    error = '';
    try {
      const current = await auth.confirmLogin(challenge.id, code);
      code = '';
      challenge = null;
      pendingAdmin = current.role === 'admin' && !current.adminVerified;
      if (!pendingAdmin) await onSession();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function passkey() {
    busy = true;
    error = '';
    try {
      await auth.loginPasskey();
      pendingAdmin = false;
      await onSession();
    } catch (e) {
      error =
        e instanceof DOMException && e.name === 'NotAllowedError'
          ? 'Nie potwierdzono passkey. Spróbuj ponownie swoim kluczem administratora.'
          : `${(e as Error).message} Jeśli konto nie ma passkey, operator musi skonfigurować go lokalnie zgodnie z instrukcją administratora.`;
    } finally {
      busy = false;
    }
  }
  async function changeAccount() {
    busy = true;
    error = '';
    try {
      await auth.logout();
      pendingAdmin = false;
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<section class="page login-page">
  <div class="page-heading">
    <span class="badge">Twoje projekty, w jednym miejscu</span>
    <h1>
      {pendingAdmin
        ? 'Potwierdź dostęp.'
        : register
          ? 'Zacznijmy od emaila.'
          : 'Dobrze Cię widzieć.'}
    </h1>
    <p class="muted">Zamówienia, wiadomości i wszystkie szczegóły Twoich wydruków.</p>
  </div>
  <div class="panel stack login-panel">
    {#if error}<p class="alert error" role="alert">{error}</p>{/if}
    {#if pendingAdmin}
      <h2>Klucz dostępu administratora</h2>
      <p class="muted">
        Potwierdź logowanie swoim passkey, używając PIN-u lub biometrii urządzenia.
      </p>
      <button class="button primary" disabled={busy} onclick={passkey}
        >{busy ? 'Potwierdzanie…' : 'Potwierdź passkey'}</button
      >
      <details>
        <summary>Pierwszy klucz lub odzyskanie dostępu</summary><Passkeys
          bootstrapMode
          {onSession}
        />
      </details>
      <button class="button ghost" disabled={busy} onclick={changeAccount}
        >Zaloguj na inne konto</button
      >
    {:else if challenge}
      <h2>Sprawdź swoją skrzynkę</h2>
      <p>Kod do logowania wysłaliśmy na <strong>{challenge.email}</strong>.</p>
      <form
        class="stack"
        onsubmit={(event) => {
          event.preventDefault();
          confirm();
        }}
      >
        <label class="field-label" for="login-code"
          >Kod z emaila
          <input
            id="login-code"
            type="text"
            inputmode="numeric"
            autocomplete="one-time-code"
            pattern={'[0-9]{8}'}
            minlength="8"
            maxlength="8"
            required
            bind:value={code}
          />
        </label>
        <p class="muted">Wpisz 8 cyfr. Kod jest ważny przez 10 minut.</p>
        <button type="submit" class="button primary" disabled={busy}
          >{busy ? 'Logowanie…' : 'Zaloguj się'}</button
        >
      </form>
      <div class="toolbar">
        <button class="button secondary" disabled={busy} onclick={request}
          >Wyślij kod ponownie</button
        >
        <button
          class="button ghost"
          disabled={busy}
          onclick={() => {
            challenge = null;
            code = '';
            error = '';
          }}>Zmień email</button
        >
      </div>
    {:else}
      <form
        class="stack"
        onsubmit={(event) => {
          event.preventDefault();
          request();
        }}
      >
        <label class="field-label" for="email"
          >Adres email
          <input
            id="email"
            type="email"
            autocomplete="email"
            required
            maxlength="254"
            bind:value={email}
            placeholder="ty@przyklad.pl"
          />
        </label>
        <button type="submit" class="button primary" disabled={busy}
          >{busy ? 'Wysyłamy kod…' : 'Wyślij kod do logowania'}</button
        >
      </form>
      <p class="muted">
        Bez hasła. Przy pierwszym logowaniu utworzymy konto. Właściwy panel wybierzemy
        automatycznie.
      </p>
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
</style>
