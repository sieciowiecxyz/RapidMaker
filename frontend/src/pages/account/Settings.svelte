<script lang="ts">
  import { onMount } from 'svelte';
  import { getProfile, updateProfile } from '../../lib/api';
  import type { Profile } from '../../lib/types';
  import Button from '../../components/ui/Button.svelte';
  let { onSession = () => {} }: { onSession?: () => void } = $props();
  let profile = $state<Profile | null>(null);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state('');
  let success = $state('');
  async function load() {
    loading = true;
    error = '';
    try {
      profile = await getProfile();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }
  onMount(load);
  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (!profile) return;
    busy = true;
    error = '';
    success = '';
    try {
      profile = await updateProfile(profile);
      success = 'Zapisano preferencje powiadomień na koncie.';
      onSession();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<div class="page stack">
  <div class="page-heading">
    <div>
      <p class="eyebrow">Moje konto</p>
      <h1>Ustawienia</h1>
      <p class="muted">Twoje dane i powiadomienia, tak jak lubisz.</p>
    </div>
  </div>
  {#if loading}<div class="panel" role="status">Ładowanie ustawień…</div>{:else if !profile}<div
      class="panel stack"
    >
      <p class="alert error" role="alert">{error}</p>
      <Button onclick={load}>Spróbuj ponownie</Button>
    </div>{:else}
    <form onsubmit={save} class="stack">
      <section class="panel stack">
        <h2>Dane kontaktowe</h2>
        <p class="muted">
          Email jest powiązany z kontem. Pozostałe dane podajesz przy zamówieniu; książka adresowa
          nie jest jeszcze dostępna.
        </p>
        <div class="form-grid">
          <label
            >Imię i nazwisko<input
              disabled
              bind:value={profile.contact.name}
              autocomplete="name"
              required
            /></label
          ><label
            >Email do logowania<input value={profile.contact.email} type="email" disabled /><small
              class="muted">Adres email pozostaje bez zmian.</small
            ></label
          ><label
            >Telefon<input
              disabled
              bind:value={profile.contact.phone}
              autocomplete="tel"
              type="tel"
            /></label
          >
        </div>
      </section>
      <section class="panel stack">
        <h2>Adres dostawy</h2>
        <div class="form-grid">
          <label
            >Ulica i numer<input
              disabled
              bind:value={profile.contact.street}
              autocomplete="street-address"
            /></label
          ><label
            >Kod pocztowy<input
              disabled
              bind:value={profile.contact.postalCode}
              autocomplete="postal-code"
              placeholder="00-000"
              pattern={'[0-9]{2}-[0-9]{3}'}
            /></label
          ><label
            >Miejscowość<input
              disabled
              bind:value={profile.contact.city}
              autocomplete="address-level2"
            /></label
          ><label
            >Ulubiony punkt odbioru<input
              disabled
              bind:value={profile.contact.pickupPoint}
              placeholder="Nazwa lub kod punktu"
            /></label
          >
        </div>
      </section>
      <section class="panel stack">
        <h2>Dane do faktury VAT</h2>
        <div class="form-grid">
          <label
            >Nazwa firmy<input
              disabled
              bind:value={profile.contact.company}
              autocomplete="organization"
            /></label
          ><label
            >NIP<input
              disabled
              bind:value={profile.contact.nip}
              inputmode="numeric"
              placeholder="10 cyfr"
              pattern={'[0-9]{10}'}
            /></label
          >
        </div>
      </section>
      <section class="panel stack">
        <h2>Powiadomienia email</h2>
        <p class="muted">
          Wybierz wiadomości wysyłane na email konta. Potwierdzenia zakupu i płatności wysyłamy
          niezależnie od tych ustawień.
        </p>
        <div class="notification-grid">
          <label
            ><input type="checkbox" bind:checked={profile.notifications.message} /><span
              >Nowe wiadomości od obsługi</span
            ></label
          ><label
            ><input type="checkbox" bind:checked={profile.notifications.status} /><span
              >Zmiany statusu, wysyłka i gotowość do odbioru</span
            ></label
          >
        </div>
      </section>
      {#if error}<p class="alert error" role="alert">{error}</p>{/if}{#if success}<p
          class="alert success"
          role="status"
        >
          {success}
        </p>{/if}
      <div>
        <Button type="submit" disabled={busy}>{busy ? 'Zapisywanie…' : 'Zapisz ustawienia'}</Button>
      </div>
    </form>
    <section class="panel stack">
      <h2>Usunięcie konta</h2>
      <p class="muted">Usuwanie konta nie jest jeszcze dostępne w aplikacji.</p>
      <div><Button variant="ghost" disabled>Usuń konto</Button></div>
    </section>
  {/if}
</div>

<style>
  .notification-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .notification-grid label {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 44px;
  }
  .notification-grid input {
    width: 20px;
    height: 20px;
    min-height: 20px;
    accent-color: var(--color-primary);
  }
  @media (max-width: 650px) {
    .notification-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
