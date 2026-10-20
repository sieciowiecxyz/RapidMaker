<script lang="ts">
  import { onMount } from 'svelte';
  import Button from '../components/ui/Button.svelte';
  import Icon from '../components/Icon.svelte';
  import ModelArt from '../components/ModelArt.svelte';
  import { getMaterials } from '../lib/api';
  import { navigate } from '../lib/router';
  import type { Material } from '../lib/types';

  let materials = $state<Material[]>([]);
  let error = $state('');
  onMount(async () => {
    try {
      materials = (await getMaterials()).filter((item) => item.compatible).slice(0, 3);
    } catch {
      error = 'Nie udało się wczytać materiałów.';
    }
  });
</script>

<svelte:head><title>Rapidmaker — Twój projekt. Gotowy wydruk.</title></svelte:head>

<div class="home">
  <section class="intro" aria-labelledby="home-title">
    <div class="intro-copy">
      <p class="kicker">DRUK 3D NA ZAMÓWIENIE</p>
      <h1 id="home-title">Twój projekt.<br /><span>Gotowy wydruk.</span></h1>
      <p class="intro-description">
        Od jednej części do całej serii.<br />Dodaj model, dobierz materiał i sprawdź cenę.
      </p>
      <div class="intro-actions">
        <Button onclick={() => navigate('/wycena')}
          >Wyceń swój model <Icon name="arrow" size={18} /></Button
        >
        <a href="#jak-to-dziala">Jak to działa <span aria-hidden="true">↓</span></a>
      </div>
      <p class="formats">STL / STEP / 3MF <span>Bez zobowiązań</span></p>
    </div>
    <figure class="part-visual">
      <ModelArt color="var(--color-primary)" />
      <figcaption>
        <span>OD MODELU DO PRZEDMIOTU</span><span>FDM / WARSTWA PO WARSTWIE</span>
      </figcaption>
    </figure>
  </section>

  <section class="process" id="jak-to-dziala" aria-labelledby="process-title">
    <div class="section-intro">
      <p class="kicker">PROSTY PROCES</p>
      <h2 id="process-title">Trzy kroki.<br /> I masz to.</h2>
    </div>
    <ol class="process-list">
      {#each [{ title: 'Dodaj model', text: 'Prześlij pliki STL, STEP lub 3MF. Możesz zamówić kilka różnych części.' }, { title: 'Dopasuj wydruk', text: 'Wybierz materiał, kolor i liczbę sztuk. Sprawdź konfigurację oraz cenę.' }, { title: 'Wybierz odbiór', text: 'Kurier, Paczkomat lub odbiór osobisty. Status zamówienia znajdziesz w swoim panelu.' }] as step, i}
        <li>
          <span class="index">0{i + 1}</span>
          <div>
            <h3>{step.title}</h3>
            <p>{step.text}</p>
          </div>
        </li>
      {/each}
    </ol>
  </section>

  <section class="materials" aria-labelledby="materials-title">
    <div class="materials-heading">
      <div>
        <p class="kicker">MATERIAŁ MA ZNACZENIE</p>
        <h2 id="materials-title">Do Twojego zastosowania.</h2>
      </div>
      <a href="/wycena">Wszystkie materiały <span aria-hidden="true">↗</span></a>
    </div>
    <div class="material-list">
      {#each materials as material, i}
        <a class="material-row" href="/wycena" aria-label={`Wyceń wydruk — ${material.name}`}>
          <span class="index">0{i + 1}</span>
          <h3>{material.name}</h3>
          <p>{material.description}</p>
          <div class="swatches" aria-label="Przykładowe kolory">
            {#each material.colors.slice(0, 4) as color}<span
                style:background={color.hex}
                title={color.name}
              ></span>{/each}
          </div>
          <span class="row-arrow" aria-hidden="true">↗</span>
        </a>
      {/each}
    </div>
    {#if error}<p class="material-error" role="alert">
        {error} <a href="/wycena">Przejdź do wyceny ↗</a>
      </p>{/if}
  </section>
</div>

<style>
  .home {
    color: var(--color-text);
  }
  .intro {
    display: grid;
    grid-template-columns: 1.15fr 1fr;
    align-items: center;
    min-height: 660px;
    padding: 70px 0 80px;
    gap: 20px;
  }
  .kicker {
    font-size: 11px;
    letter-spacing: 0.17em;
    font-weight: 600;
    color: var(--color-muted);
    margin: 0 0 25px;
  }
  h1 {
    font-size: clamp(42px, 5.1vw, 74px);
    line-height: 1.05;
    font-weight: 550;
    letter-spacing: -0.065em;
    margin: 0;
  }
  h1 span {
    color: var(--color-accent);
  }
  .intro-description {
    font-size: 16px;
    line-height: 1.8;
    color: var(--color-muted);
    margin: 28px 0 32px;
  }
  .intro-actions {
    display: flex;
    align-items: center;
    gap: 26px;
  }
  .intro-actions a,
  .materials-heading > a {
    color: var(--color-text);
    font-size: 13px;
    text-decoration: none;
  }
  .intro-actions a span {
    margin-left: 10px;
  }
  .intro-actions a:hover,
  .materials-heading > a:hover {
    color: var(--color-accent);
  }
  .formats {
    display: flex;
    gap: 22px;
    color: var(--color-muted);
    font-size: 11px;
    letter-spacing: 0.08em;
    margin: 23px 0 0;
  }
  .formats span {
    letter-spacing: 0;
    color: var(--color-muted);
  }
  .part-visual {
    margin: 0;
    min-width: 0;
  }
  .part-visual :global(svg.model-art) {
    display: block;
    width: 100%;
    height: auto;
    max-width: 540px;
    margin: auto;
    filter: none;
    animation: none;
    transform: none;
  }
  figcaption {
    display: flex;
    justify-content: space-between;
    gap: 15px;
    margin: 0 30px;
    border-top: 1px solid var(--color-border);
    padding-top: 15px;
    color: var(--color-muted);
    font-size: 8px;
    letter-spacing: 0.12em;
  }
  .process {
    display: grid;
    grid-template-columns: 0.8fr 1.2fr;
    gap: 80px;
    border-top: 1px solid var(--color-border);
    padding: 65px 0 80px;
    scroll-margin-top: 100px;
  }
  h2 {
    font-size: clamp(26px, 3vw, 36px);
    line-height: 1.18;
    font-weight: 500;
    letter-spacing: -0.04em;
    margin: 0;
  }
  .section-intro .kicker,
  .materials-heading .kicker {
    margin-bottom: 19px;
  }
  .process-list {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .process-list li {
    display: flex;
    gap: 27px;
    padding: 22px 0;
  }
  .process-list li:not(:last-child) {
    border-bottom: 1px solid var(--color-border);
  }
  .process-list li:first-child {
    padding-top: 0;
  }
  .index {
    color: var(--color-muted);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    line-height: 25px;
  }
  h3 {
    margin: 0;
    font-size: 17px;
    line-height: 1.5;
    letter-spacing: -0.02em;
    font-weight: 500;
  }
  .process-list p {
    margin: 7px 0 0;
    color: var(--color-muted);
    font-size: 13px;
    line-height: 1.7;
    max-width: 420px;
  }
  .materials {
    border-top: 1px solid var(--color-border);
    padding: 65px 0 90px;
  }
  .materials-heading {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 30px;
    margin-bottom: 36px;
  }
  .materials-heading > a {
    padding-bottom: 4px;
    white-space: nowrap;
  }
  .materials-heading > a span {
    margin-left: 10px;
  }
  .material-row {
    display: grid;
    grid-template-columns: 35px 170px 1fr 95px 20px;
    align-items: center;
    gap: 20px;
    border-top: 1px solid var(--color-border);
    padding: 26px 0;
    text-decoration: none;
    color: inherit;
  }
  .material-row p {
    color: var(--color-muted);
    font-size: 13px;
    line-height: 1.6;
    margin: 0;
    max-width: 400px;
  }
  .material-row:hover h3,
  .material-row:hover .row-arrow {
    color: var(--color-accent);
  }
  .swatches {
    display: flex;
    justify-content: flex-end;
  }
  .swatches span {
    display: block;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    border: 2px solid var(--color-bg);
    margin-left: -3px;
    box-shadow: 0 0 0 1px var(--color-border);
  }
  .row-arrow {
    color: var(--color-muted);
    font-size: 20px;
    text-align: right;
  }
  .material-error {
    color: var(--color-muted);
    font-size: 13px;
  }
  .material-error a {
    color: var(--color-accent);
  }
  @media (max-width: 900px) {
    .intro {
      min-height: 570px;
      padding: 60px 0;
    }
    .intro-actions {
      align-items: flex-start;
      flex-direction: column;
      gap: 20px;
    }
    figcaption {
      margin: 0 12px;
    }
    figcaption span:last-child {
      display: none;
    }
    .process {
      gap: 45px;
    }
    .material-row {
      grid-template-columns: 25px 145px 1fr 20px;
      gap: 15px;
    }
    .swatches {
      display: none;
    }
  }
  @media (max-width: 620px) {
    .intro {
      grid-template-columns: 1fr;
      gap: 12px;
      padding: 45px 0 42px;
      min-height: 0;
    }
    h1 {
      font-size: clamp(40px, 10.8vw, 64px);
    }
    .kicker {
      font-size: 11px;
      margin-bottom: 23px;
    }
    .intro-description {
      margin: 23px 0 26px;
      font-size: 14px;
    }
    .intro-actions {
      flex-direction: row;
      align-items: center;
      gap: 20px;
    }
    .intro-actions :global(.button) {
      padding: 13px 17px;
      font-size: 12px;
    }
    .intro-actions a {
      font-size: 12px;
    }
    .intro-actions a span {
      margin-left: 3px;
    }
    .formats {
      margin-top: 20px;
      font-size: 11px;
    }
    .part-visual {
      max-width: 360px;
      width: 100%;
      margin: 8px auto 0;
    }
    .part-visual :global(svg.model-art) {
      margin-top: -18px;
      margin-bottom: -15px;
    }
    figcaption {
      margin: 0 18px;
      font-size: 11px;
    }
    figcaption span:last-child {
      display: block;
    }
    .process {
      grid-template-columns: 1fr;
      gap: 32px;
      padding: 40px 0;
    }
    .section-intro h2 br {
      display: none;
    }
    h2 {
      font-size: 27px;
    }
    .process-list li {
      padding: 19px 0;
      gap: 20px;
    }
    .process-list p {
      font-size: 13px;
    }
    .materials {
      padding: 40px 0 50px;
    }
    .materials-heading {
      align-items: flex-start;
      flex-direction: column;
      gap: 20px;
      margin-bottom: 27px;
    }
    .material-row {
      grid-template-columns: 25px 1fr 20px;
      gap: 12px;
      padding: 20px 0;
    }
    .material-row h3 {
      grid-column: 2;
    }
    .material-row p {
      grid-column: 2;
      grid-row: 2;
    }
    .material-row .row-arrow {
      grid-column: 3;
      grid-row: 1;
    }
    .material-row .index {
      grid-row: 1;
    }
  }
</style>
