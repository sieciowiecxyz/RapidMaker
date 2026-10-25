<script lang="ts">
  import Passkeys from '../../components/Passkeys.svelte';
  import AccountSecurity from '../../components/AccountSecurity.svelte';
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
  let alive = true,
    generation = 0;
  async function load() {
    const current = ++generation;
    loading = true;
    error = '';
    try {
      const result = await getProfile();
      if (alive && current === generation) profile = result;
    } catch (e) {
      if (alive && current === generation) error = (e as Error).message;
    } finally {
      if (alive && current === generation) loading = false;
    }
  }
  onMount(() => {
    void load();
    return () => {
      alive = false;
      generation++;
    };
  });
  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (!profile || busy) return;
    const current = generation;
    busy = true;
    error = '';
    success = '';
    try {
      const result = await updateProfile(profile);
      if (!alive || current !== generation) return;
      profile = result;
      success = 'Zapisano preferencje powiadomień na koncie.';
      onSession();
    } catch (e) {
      if (alive && current === generation) error = (e as Error).message;
    } finally {
      if (alive && current === generation) busy = false;
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
        <h2>Twoje konto</h2>
        <p>{profile.contact.email}</p>
        <p class="muted">Dane odbiorcy i faktury podajesz przy zamówieniu.</p>
      </section>
      <section class="panel stack">
        <h2>Powiadomienia email</h2>
        <p class="muted">
          Wybierz wiadomości wysyłane na email konta. Potwierdzenia zakupu i płatności wysyłamy
          niezależnie od tych ustawień.
        </p>
        <div class="notification-grid">
          <label
            ><input
              type="checkbox"
              disabled={busy}
              bind:checked={profile.notifications.message}
            /><span>Nowe wiadomości od obsługi</span></label
          ><label
            ><input
              type="checkbox"
              disabled={busy}
              bind:checked={profile.notifications.status}
            /><span>Zmiany statusu, wysyłka i gotowość do odbioru</span></label
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
    <AccountSecurity />
    <Passkeys
      bootstrapMode={new URLSearchParams(location.search).get('bootstrap') === '1'}
      {onSession}
    />
    <section class="panel stack">
      <h2>Usunięcie konta</h2>
      <p class="muted">
        Napisz z adresu swojego konta na
        <a href="mailto:piotr@rapidmaker.pl?subject=Usuni%C4%99cie%20konta%20RapidMaker"
          >piotr@rapidmaker.pl</a
        >. Zamknięcie konta nie anuluje zamówień. Dane wymagane do rozliczeń lub trwających spraw
        zachowamy zgodnie z <a href="/prywatnosc">polityką prywatności</a>.
      </p>
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
