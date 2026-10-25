<script lang="ts">
  let { account = false, preparing = false }: { account?: boolean; preparing?: boolean } = $props();
</script>

<div class="legal-notice">
  <p>
    {account
      ? 'Tworząc konto, akceptujesz'
      : preparing
        ? 'Przygotowując płatność za ofertę, akceptujesz'
        : 'Klikając „Zamawiam i płacę”, akceptujesz'}
    <a href="/regulamin" target="_blank" rel="noopener">Regulamin</a>. Informacje o danych:
    <a href="/prywatnosc" target="_blank" rel="noopener">Polityka prywatności</a>.
  </p>
  {#if !account}
    <p>
      Produkcja do 14 dni kalendarzowych od potwierdzenia płatności, chyba że oferta określa inny
      termin. Czas dostawy podajemy osobno.
    </p>
    <p>
      Wydruk wykonujemy według Twojej specyfikacji — bez zwykłego prawa do 14-dniowego odstąpienia
      bez przyczyny. Zachowujesz prawo do reklamacji.
    </p>
  {/if}
</div>

<style>
  .legal-notice {
    font-size: 12px;
    line-height: 1.65;
    color: var(--muted);
  }
  p {
    margin: 0;
  }
  p + p {
    margin-top: 8px;
  }
  a {
    color: var(--ink);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
</style>
