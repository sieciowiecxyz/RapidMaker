<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from '../lib/api';
  import type { EmailChallenge } from '../lib/types';
  let { onSession }: { onSession: () => void | Promise<void> } = $props();
  let email = $state(new URLSearchParams(location.search).get('email') ?? '');
  let register = $state(new URLSearchParams(location.search).get('register') === '1');
  let operatorMode = $state(
    location.pathname.startsWith('/admin/') ||
      new URLSearchParams(location.search).get('operator') === '1',
  );
  let operatorKey = $state('');
  let challenge = $state<EmailChallenge | null>(null);
  let busy = $state(false),
    error = $state('');
  let accounts = $state<{ email: string; role: string }[]>([]);
  onMount(async () => {
    accounts = await api.getDemoAccounts();
  });
  async function signInOperator() {
    busy = true;
    error = '';
    const token = operatorKey;
    operatorKey = '';
    try {
      await api.loginOperator(token);
      await onSession();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function request() {
    busy = true;
    error = '';
    try {
      challenge = await api.requestLogin(email, register);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function confirm(expired = false) {
    if (!challenge) return;
    busy = true;
    error = '';
    try {
      await api.confirmLogin(challenge.id, expired);
      await onSession();
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
      {operatorMode
        ? 'Panel administratora.'
        : register
          ? 'Zacznijmy od emaila.'
          : 'Dobrze Cię widzieć.'}
    </h1>
    <p class="muted">
      {operatorMode
        ? 'Materiały, dostępność i koszty produkcji.'
        : 'Zamówienia, wiadomości i wszystkie szczegóły Twoich wydruków.'}
    </p>
  </div>
  <div class="panel stack login-panel">
    {#if error}<p class="alert error" role="alert">{error}</p>{/if}
    {#if operatorMode}
      <form
        class="stack"
        onsubmit={(event) => {
          event.preventDefault();
          signInOperator();
        }}
      >
        <label class="field-label" for="operator-key"
          >Klucz dostępu administratora
          <input
            id="operator-key"
            type="password"
            autocomplete="current-password"
            required
            minlength="32"
            maxlength="256"
            bind:value={operatorKey}
          />
        </label>
        <button type="submit" class="button primary" disabled={busy}
          >{busy ? 'Logowanie…' : 'Zaloguj się do panelu'}</button
        >
      </form>
      <button
        class="button ghost"
        onclick={() => {
          operatorMode = false;
          operatorKey = '';
          error = '';
        }}>Logowanie klienta</button
      >
    {:else if challenge}
      <h2>Sprawdź swoją skrzynkę</h2>
      <p>
        Link do {challenge.isNew ? 'utworzenia konta' : 'logowania'} przygotowaliśmy dla
        <strong>{challenge.email}</strong>.
      </p>
      <p class="muted">
        To lokalne demo. Email nie zostanie wysłany — potwierdź go przyciskiem poniżej.
      </p>
      <button class="button primary" disabled={busy} onclick={() => confirm()}
        >{busy ? 'Chwileczkę…' : 'Symuluj potwierdzenie emaila'}</button
      >
      <div class="toolbar">
        <button class="button secondary" disabled={busy} onclick={request}
          >Wyślij link ponownie</button
        ><button
          class="button ghost"
          disabled={busy}
          onclick={() => {
            challenge = null;
            error = '';
          }}>Zmień email</button
        >
      </div>
      <details>
        <summary>Sprawdź wygasły link</summary><button
          class="button secondary"
          disabled={busy}
          onclick={() => confirm(true)}>Symuluj wygaśnięcie linku</button
        >
      </details>
    {:else}
      <form
        class="stack"
        onsubmit={(event) => {
          event.preventDefault();
          request();
        }}
      >
        <label class="field-label" for="email"
          >Adres email<input
            id="email"
            type="email"
            autocomplete="email"
            required
            bind:value={email}
            placeholder="ty@przyklad.pl"
          /></label
        >
        <label class="choice"
          ><input type="checkbox" bind:checked={register} />Utwórz nowe konto klienta</label
        >
        <button type="submit" class="button primary" disabled={busy}
          >{busy
            ? 'Przygotowujemy link…'
            : register
              ? 'Utwórz konto przez email'
              : 'Wyślij link do logowania'}</button
        >
      </form>
      <p class="muted">Bez hasła. Wybór panelu następuje automatycznie po potwierdzeniu emaila.</p>
      <div class="demo-accounts">
        <span class="field-label">Wypróbuj konto demonstracyjne</span
        >{#each accounts as account}<button
            class="button secondary"
            onclick={() => {
              email = account.email;
              register = false;
            }}>{account.role === 'admin' ? 'Administrator' : 'Klient'} · {account.email}</button
          >{/each}
      </div>
    {/if}
    {#if !operatorMode}<button
        class="button ghost"
        onclick={() => {
          operatorMode = true;
          challenge = null;
          error = '';
        }}>Dostęp administratora</button
      >{/if}
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
  .choice {
    display: flex;
    gap: 12px;
    align-items: center;
    min-height: 44px;
  }
  .choice input {
    width: 18px;
    height: 18px;
  }
  .demo-accounts {
    display: grid;
    gap: 10px;
    border-top: 1px solid var(--color-border);
    padding-top: 22px;
  }
  .demo-accounts button {
    white-space: normal;
    overflow-wrap: anywhere;
  }
  details button {
    margin-top: 12px;
  }
</style>
