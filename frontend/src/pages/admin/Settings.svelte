<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getSystemSettings,
    updateSystemSettings,
    retryJob,
    getIntegrationStatus,
  } from '../../lib/api';
  import type { SystemSettings } from '../../lib/types';
  import Button from '../../components/ui/Button.svelte';
  let settings = $state<SystemSettings | null>(null);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state('');
  let success = $state('');
  let integrations = $state<{ stripe_configured: boolean; furgonetka_configured: boolean } | null>(
    null,
  );
  async function load() {
    loading = true;
    error = '';
    try {
      settings = await getSystemSettings();
      integrations = await getIntegrationStatus().catch(() => null);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }
  onMount(load);
  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (!settings) return;
    busy = true;
    error = '';
    success = '';
    try {
      settings = await updateSystemSettings({
        setupFee: settings.setupFee,
        minimumPrice: settings.minimumPrice,
        electricityPerKwh: settings.electricityPerKwh,
      });
      success = 'Zapisano parametry.';
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function retry(id: string) {
    busy = true;
    error = '';
    success = '';
    try {
      settings = await retryJob(id);
      success = 'Zapisano prośbę o ponowienie.';
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
      <p class="eyebrow">Panel zespołu</p>
      <h1>Ustawienia</h1>
      <p class="muted">Cennik roboczy, zadania i stan integracji.</p>
    </div>
  </div>
  {#if error}<p class="alert error" role="alert">{error}</p>{/if}{#if success}<p
      class="alert success"
      role="status"
    >
      {success}
    </p>{/if}
  {#if loading}<div class="panel" role="status">Ładowanie ustawień…</div>{:else if !settings}<Button
      onclick={load}>Spróbuj ponownie</Button
    >{:else}
    <form class="panel settings-list" onsubmit={save}>
      <div class="settings-heading">
        <h2>Parametry wyceny</h2>
        <Button type="submit" disabled={busy}>Zapisz zmiany</Button>
      </div>
      <p class="settings-note muted">
        Ustawienia robocze. Wycena online korzysta obecnie z oddzielnego cennika serwera.
      </p>
      <div class="setting-rows">
        <label
          >Opłata startowa (zł)<input
            type="number"
            min="0"
            step="0.01"
            bind:value={settings.setupFee}
            required
          /></label
        ><label
          >Minimalna cena zamówienia (zł)<input
            type="number"
            min="0"
            step="0.01"
            bind:value={settings.minimumPrice}
            required
          /></label
        ><label
          >Koszt energii (zł / kWh)<input
            type="number"
            min="0"
            step="0.01"
            bind:value={settings.electricityPerKwh}
            required
          /></label
        >
      </div>
    </form>
    <section class="panel stack">
      <h2>Zadania wymagające uwagi</h2>
      {#each settings.jobs as job (job.id)}<article class="job">
          <div>
            <span class="badge"
              >{job.kind === 'conversion'
                ? 'Konwersja'
                : job.kind === 'quote'
                  ? 'Wycena'
                  : 'Email'}</span
            >
            <h3>{job.label}</h3>
            <p class="muted">
              {job.status === 'failed'
                ? 'Niepowodzenie · można ponowić'
                : 'Przekazano do ponowienia'}
            </p>
          </div>
          <Button
            variant="secondary"
            disabled={busy || job.status === 'retried'}
            onclick={() => retry(job.id)}
            >{job.status === 'retried' ? 'Ponowiono' : 'Ponów zadanie'}</Button
          >
        </article>{:else}<p class="empty-state">Brak nieudanych zadań.</p>{/each}
    </section>
    <section class="panel stack">
      <h2>Integracje</h2>
      <div class="integration">
        <strong>Stripe · płatności</strong><span class="badge"
          >{integrations === null
            ? 'Nie udało się sprawdzić'
            : integrations.stripe_configured
              ? 'Sandbox skonfigurowany'
              : 'Brak konfiguracji'}</span
        >
      </div>
      <div class="integration">
        <strong>Furgonetka · przesyłki</strong><span class="badge"
          >{integrations === null
            ? 'Nie udało się sprawdzić'
            : integrations.furgonetka_configured
              ? 'Sandbox skonfigurowany'
              : 'Brak konfiguracji'}</span
        >
      </div>
      <div class="integration">
        <strong>iCloud · email</strong><span class="badge">Wysyłka wyłączona</span>
      </div>
    </section>
  {/if}
</div>

<style>
  .settings-list {
    padding-top: 20px;
    padding-bottom: 8px;
  }
  .settings-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
  }
  .settings-heading h2 {
    margin: 0;
    font-size: 18px;
  }
  .settings-note {
    margin: 12px 0;
    font-size: 12px;
  }
  .setting-rows label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 14px 0;
    border-top: 1px solid var(--color-border);
    font-size: 14px;
  }
  .setting-rows input {
    width: 160px;
    flex-shrink: 0;
    min-height: 42px;
    text-align: right;
  }
  @media (max-width: 600px) {
    .setting-rows input {
      width: 105px;
    }
    .settings-heading {
      flex-wrap: wrap;
    }
  }

  .job {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    border-bottom: 1px solid var(--color-border);
    padding: 16px 0;
  }
  .job h3 {
    font-size: 1rem;
    overflow-wrap: anywhere;
    margin: 12px 0 6px;
  }
  .job p {
    margin: 0;
  }
  .integration {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 20px;
    padding: 12px 0;
    border-bottom: 1px solid var(--color-border);
  }
  @media (max-width: 650px) {
    .job,
    .integration {
      align-items: start;
      flex-direction: column;
    }
  }
</style>
