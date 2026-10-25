<script lang="ts">
  import { onMount } from 'svelte';
  import { getLegalDocuments, type LegalDocuments } from '../lib/legal';
  let { path }: { path: string } = $props();
  let documents = $state<LegalDocuments | null>(null);
  let error = $state('');
  const hub = $derived(path === '/regulaminy');
  const title = $derived(
    hub ? 'Dokumenty i zasady' : path === '/regulamin' ? 'Regulamin' : 'Polityka prywatności',
  );
  const text = $derived(
    documents ? (path === '/regulamin' ? documents.terms : documents.privacy) : '',
  );
  onMount(() => {
    void load();
  });
  async function load() {
    error = '';
    try {
      documents = await getLegalDocuments();
    } catch {
      error = 'Nie udało się wczytać dokumentów. Spróbuj ponownie.';
    }
  }
  function download() {
    if (!documents) return;
    const url = URL.createObjectURL(new Blob([text], { type: 'text/plain;charset=utf-8' }));
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `rapidmaker-${path.slice(1)}-${documents.version}.txt`;
    anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
</script>

<section class="page legal-page">
  <a class="back" href={hub ? '/' : '/regulaminy'}
    >← {hub ? 'Strona główna' : 'Wszystkie dokumenty'}</a
  >
  <header class="page-heading"><h1>{title}</h1></header>
  {#if hub}
    <p>Prosto o zamówieniach, koncie i Twoich danych.</p>
    <nav class="documents" aria-label="Dokumenty">
      <a href="/regulamin"
        ><strong>Regulamin</strong><span>Zamówienia, płatności, dostawa i reklamacje →</span></a
      >
      <a href="/prywatnosc"
        ><strong>Polityka prywatności</strong><span
          >Dane, okresy przechowywania i Twoje prawa →</span
        ></a
      >
    </nav>
    <p>
      Pytania, reklamacje lub prośba o usunięcie konta: <a href="mailto:piotr@rapidmaker.pl"
        >piotr@rapidmaker.pl</a
      >.
    </p>
  {:else if error}
    <p role="alert">{error}</p>
    <button class="button secondary" onclick={load}>Spróbuj ponownie</button>
  {:else if !documents}<p role="status">Wczytywanie dokumentu…</p>
  {:else}
    <div class="document-tools">
      <span>Wersja: {documents.version}</span><button class="text-button" onclick={download}
        >Pobierz dokument (.txt) ↓</button
      >
    </div>
    <article aria-label={title}>
      {#each text.split(/\n\s*\n/) as paragraph}
        {#if /^\d+[.)]\s/.test(paragraph) && !paragraph.includes('\n')}<h2>{paragraph}</h2>
        {:else}<p>{paragraph}</p>{/if}
      {/each}
    </article>
  {/if}
</section>

<style>
  .legal-page {
    max-width: 880px;
    padding-block: 40px 72px;
  }
  .page-heading {
    margin-block: 24px;
  }
  .documents {
    display: grid;
    gap: 16px;
    margin-block: 32px;
  }
  .documents a {
    display: grid;
    gap: 8px;
    padding: 24px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--card-radius);
    text-decoration: none;
  }
  .documents span,
  .document-tools {
    color: var(--muted);
  }
  .document-tools {
    display: flex;
    gap: 16px;
    align-items: center;
    flex-wrap: wrap;
    margin-bottom: 32px;
    font-size: 12px;
  }
  article {
    font-size: 15px;
    line-height: 1.85;
    overflow-wrap: anywhere;
  }
  article p {
    white-space: pre-line;
    margin: 0 0 20px;
  }
  article h2 {
    font-size: 21px;
    margin: 36px 0 16px;
  }
</style>
