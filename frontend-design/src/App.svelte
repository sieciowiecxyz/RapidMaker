<script lang="ts">
  import { onMount, tick } from 'svelte';
  import gsap from 'gsap';
  import type { StlMesh } from './lib/stlParser';
  import { resolveColor, resolveFilament } from './lib/colorMatching';
  import { softValue } from './lib/motion';
  import FlowBackground from './components/FlowBackground.svelte';
  import FlowGlowDemo from './components/FlowGlowDemo.svelte';
  let glowMode=$state<'off'|'pulse'|'band'|'wave'|'comet'>('pulse');
  let glowPaused=$state(false);
  let glowShape=$state<'trail'|'bulge'|'premium'>('premium');
  let glowLength=$state<'short'|'medium'|'long'>('long');
  let glowSpeed=$state<'slow'|'normal'|'fast'>('slow');
  let glowPreview=$state(false);
  let pulseCycles=0;
  function nextGlowCycle(){
    if(glowPreview)return;
    if(glowMode==='wave'){glowMode='pulse';pulseCycles=0;}
    else if(++pulseCycles>=2)glowMode='wave';
  }
  function toggleGlowPreview(){
    glowPreview=!glowPreview;
    if(!glowPreview){glowMode='pulse';glowShape='premium';glowLength='long';glowSpeed='slow';glowPaused=false;pulseCycles=0;}
  }
  import PrintScene from './components/PrintScene.svelte';
  import PrintTransition from './components/PrintTransition.svelte';
  import OrderPage from './components/OrderPage.svelte';
  import ProductionControls from './components/ProductionControls.svelte';
  import { IconArrowUpRight, IconArrowRight, IconUpload, IconFile3d, IconX, IconCheck, IconSun, IconMoon, IconPhone, IconMail, IconMapPin, IconCube, IconRefresh } from '@tabler/icons-svelte';
  import Button from './components/ui/Button.svelte';
  import Input from './components/ui/Input.svelte';
  import Card from './components/ui/Card.svelte';
  import * as api from './lib/api';
  import { defaultSettings, emptySettings, normalizeSettings, validateModel, settingsSummary } from './lib/modelSettings';
  import type { Model, Material, Quote, PrintSettings } from './lib/types';

  let checkoutOpen = $state(false);
  let transitioning = $state(false);
  let orderDraft = $state<{models:Model[];materials:Material[];quote:Quote;delivery:string} | null>(null);
  let configuratorScroll = 0;
  async function focusOrder() { await tick(); document.getElementById('order-title')?.focus({preventScroll:true}); }
  function revealOrder() {
    checkoutOpen=true;
    if(location.hash!=='#zamowienie') history.pushState(null,'','#zamowienie');
    window.scrollTo({top:0,behavior:'instant'});
  }
  function finishOrderTransition() { transitioning=false; void focusOrder(); }
  async function backToConfiguration() {
    checkoutOpen=false;
    history.pushState(null,'','#wycena');
    await tick();
    window.scrollTo({top:configuratorScroll,behavior:'instant'});
    document.querySelector<HTMLButtonElement>('.quote-contact')?.focus({preventScroll:true});
  }
  let contactGlow = $state(false);
  let contactFrame = 0;
  let contactTimer: ReturnType<typeof setTimeout>;
  function highlightContact() {
    cancelAnimationFrame(contactFrame);
    clearTimeout(contactTimer);
    contactGlow = false;
    let previousY = window.scrollY, stableFrames = 0;
    const started = performance.now();
    const waitForArrival = (now: number) => {
      stableFrames = Math.abs(window.scrollY - previousY) < .5 ? stableFrames + 1 : 0;
      previousY = window.scrollY;
      if ((now - started > 120 && stableFrames >= 5) || now - started > 1800) {
        contactGlow = true;
        contactTimer = setTimeout(() => { contactGlow = false; }, 1500);
      } else contactFrame = requestAnimationFrame(waitForArrival);
    };
    contactFrame = requestAnimationFrame(waitForArrival);
  }
  let isMobile = $state(false);
  let theme = $state<'light' | 'dark'>('dark');
  let materials = $state<Material[]>([]);
  let models = $state<Model[]>([]);
  const uploadedFiles = new Map<string,File>();
  const meshCache = new Map<string,StlMesh>();
  let fileRevision = $state(0);
  type Preview = {status:'idle'|'loading'|'ready'|'error'|'unsupported';id:string;mesh?:StlMesh;error?:string};
  let stlPreview = $state<Preview>({status:'idle',id:''});
  let previewRetry = $state(0);
  let selectedId = $state('');
  let strengthTrigger = $state(0);
  let lastSettings = $state<PrintSettings>({...defaultSettings});
  const selectedModel = $derived(models.find(model => model.id === selectedId));
  const activeSettings = $derived(selectedModel?.settings ?? lastSettings);
  const material = $derived(activeSettings.material);
  const color = $derived(activeSettings.color);
  const customShade = $derived(activeSettings.customShade);
  const quality = $derived(activeSettings.quality);
  const strength = $derived(activeSettings.strength);
  const selectedIndex = $derived(models.findIndex(model => model.id === selectedId));
  const validationIssues = $derived(materials.length ? models.map(model => ({model,errors:validateModel(model,materials)})).filter(item => item.errors.length) : []);
  let settingsCopied = $state('');
  const selectedModelReady = $derived(materials.length > 0 && !!selectedModel && !validateModel({...selectedModel,quantity:1},materials).length);
  const incompleteSettingsIds = $derived(materials.length ? models.filter(model=>validateModel({...model,quantity:1},materials).length).map(model=>model.id) : []);
  function applySelectedToIncomplete() {
    if(!selectedModelReady || !selectedModel?.settings || !incompleteSettingsIds.length) return;
    const settings={...selectedModel.settings};
    const targets=new Set(incompleteSettingsIds);
    models=models.map(model=>targets.has(model.id)?{...model,settings:{...settings}}:model);
    lastSettings={...settings};
    settingsCopied=`Uzupełniono ustawienia ${targets.size} modeli z wybranego pliku.`;
  }
  const selectedIssues = $derived(validationIssues.find(item=>item.model.id===selectedId)?.errors ?? []);
  async function completeNextModel() {
    const target=validationIssues.find(item=>item.model.id===selectedId) ?? validationIssues[0];
    if(!target) return;
    selectModel(target.model.id);
    await tick();
    const heading=document.getElementById('model-settings');
    heading?.focus({preventScroll:true});
    heading?.scrollIntoView({behavior:matchMedia('(prefers-reduced-motion: reduce)').matches?'instant':'smooth',block:'start'});
  }
  let completedQuoteKey = $state('');
  function updateSettings(patch:Partial<PrintSettings>) {
    if(patch.strength)strengthTrigger++;
    settingsCopied='';
    const materialChanged=patch.material!==undefined&&patch.material!==activeSettings.material;
    const next=normalizeSettings({...activeSettings,...patch,...(materialChanged?{customShade:'',finish:''}:{})});
    lastSettings={...next};
    if(selectedModel) models=models.map(model=>model.id===selectedId?{...model,settings:{...next}}:model);
  }
  function selectModel(id:string) {
    selectedId=id;
    const settings=models.find(model=>model.id===id)?.settings;
    if(settings) lastSettings={...settings};
  }
  function removeModel(id:string) {
    uploadedFiles.delete(id);meshCache.delete(id);fileRevision++;
    const index=models.findIndex(model=>model.id===id);
    models=models.filter(model=>model.id!==id);
    if(selectedId===id) selectModel(models[Math.min(index,models.length-1)]?.id ?? '');
  }
  let delivery = $state('pickup');
  let manual = $state(false);
  let simulateError = $state(false);
  const quoteKey = $derived(JSON.stringify({models,delivery,manual,simulateError}));
  let quote = $state<Quote | null>(null);
  let busy = $state(false);
  let quoteError = $state('');
  let fileError = $state('');
  let dragging = $state(false);
  let contactContext = $state('');
  let fileInput: HTMLInputElement;
  let revision = 0;
  const currentMaterial = $derived(materials.find(m => m.id === material));
  const customChoice = $derived({name:resolveFilament(customShade)?.name??resolveColor(customShade)??'Wybierz odcień',hex:resolveColor(customShade)??'#292e2a'});
  const customColorRequest = $derived(models.some(model=>model.settings?.color==='custom'));
  const currentColor = $derived(color === 'custom' ? { id:'custom', name:`Niestandardowy: ${customChoice.name} (${customChoice.hex})`, hex:customChoice.hex, available:true } : (currentMaterial ?? materials[0])?.colors.find(c => c.id === color));
  const canContact = $derived(models.length > 0 && !!quote && !busy && !quoteError && !validationIssues.length && completedQuoteKey === quoteKey);
  const money = (value: number) => new Intl.NumberFormat('pl-PL', { style: 'currency', currency: 'PLN' }).format(value);

  onMount(() => {
    const syncRoute=()=>{
      if(transitioning)transitioning=false;
      checkoutOpen=location.hash==='#zamowienie' && !!orderDraft;
      if(checkoutOpen){window.scrollTo({top:0,behavior:'instant'});void focusOrder();}
      else if(location.hash==='#zamowienie')history.replaceState(null,'','#wycena');
    };
    syncRoute();
    window.addEventListener('popstate',syncRoute);
    window.addEventListener('hashchange',syncRoute);
    const mobile = matchMedia('(max-width: 760px)');
    const syncMobile = () => { isMobile = mobile.matches; };
    syncMobile(); mobile.addEventListener('change', syncMobile);
    api.getMaterials().then(value => { materials = value; });
    try { const saved = localStorage.getItem('rapidmaker-concept-theme'); if (saved === 'light' || saved === 'dark') theme = saved; } catch { /* An unavailable browser store does not block the prototype. */ }
    document.documentElement.dataset.theme = theme;
    const keyboard = () => { document.documentElement.dataset.input = 'keyboard'; };
    const pointer = () => { document.documentElement.dataset.input = 'pointer'; };
    window.addEventListener('keydown', keyboard);
    window.addEventListener('pointerdown', pointer);
    const media = gsap.matchMedia();
    media.add('(prefers-reduced-motion: no-preference)', () => {
      const timeline = gsap.timeline({ defaults: { ease: 'power4.out', clearProps: 'transform,opacity' } });
      timeline.from('.hero-word', { y: 22, opacity: .2, duration: .65, stagger: .08 })
        .from('.hero .quote-card', { y: 16, opacity: .45, duration: .5 }, .16);
    });
    return () => {
      window.removeEventListener('popstate',syncRoute);window.removeEventListener('hashchange',syncRoute);
      cancelAnimationFrame(contactFrame); clearTimeout(contactTimer);
      mobile.removeEventListener('change', syncMobile);
      media.revert();
      window.removeEventListener('keydown', keyboard); window.removeEventListener('pointerdown', pointer);
    };
  });


  $effect(() => {
    const key=quoteKey;
    const options = { models: models.map(model => ({...model,settings:model.settings?{...model.settings}:undefined})), delivery, manual, simulateError };
    const invalid=validationIssues.length>0;
    const requestId = ++revision;
    let cancelled = false;
    quoteError = '';
    contactContext = '';
    if (!options.models.length || invalid || !materials.length) { quote = null; completedQuoteKey=''; busy = false; return; }
    busy = true;
    api.getQuote(options).then(value => {
      if (!cancelled && requestId === revision) { quote = value; completedQuoteKey=key; }
    }).catch((error: unknown) => {
      if (!cancelled && requestId === revision) quoteError = error instanceof Error ? error.message : 'Nie udało się przeliczyć przykładu.';
    }).finally(() => { if (!cancelled && requestId === revision) busy = false; });
    return () => { cancelled = true; };
  });

  $effect(()=>{
    fileRevision; previewRetry;
    const id=selectedId,file=uploadedFiles.get(id);
    if(!file){stlPreview={status:'idle',id};return;}
    if(!/\.stl$/i.test(file.name)){stlPreview={status:'unsupported',id};return;}
    const cached=meshCache.get(id);
    if(cached){meshCache.delete(id);meshCache.set(id,cached);stlPreview={status:'ready',id,mesh:cached};return;}
    stlPreview={status:'loading',id};
    let cancelled=false;
    let worker:Worker;
    try{worker=new Worker(new URL('./lib/stl.worker.ts',import.meta.url),{type:'module'});}
    catch{stlPreview={status:'error',id,error:'Odczyt STL w tle jest niedostępny w tej przeglądarce.'};return;}
    worker.onmessage=(event:MessageEvent<{ok:boolean;mesh:StlMesh;error?:string}>)=>{
      if(cancelled)return;
      if(event.data.ok){
        const mesh=event.data.mesh;
        let size=mesh.vertices.byteLength+(mesh.sectionMask?.byteLength ?? 0);
        for(const cached of meshCache.values())size+=cached.vertices.byteLength+(cached.sectionMask?.byteLength ?? 0);
        while(size>48*1024*1024 && meshCache.size){
          const oldest=meshCache.keys().next().value!;
          size-=meshCache.get(oldest)!.vertices.byteLength+(meshCache.get(oldest)!.sectionMask?.byteLength ?? 0);meshCache.delete(oldest);
        }
        meshCache.set(id,mesh);stlPreview={status:'ready',id,mesh};
      }else stlPreview={status:'error',id,error:event.data.error ?? 'Nie udało się odczytać STL.'};
      worker.terminate();
    };
    worker.onerror=()=>{if(!cancelled)stlPreview={status:'error',id,error:'Nie udało się uruchomić odczytu STL. Spróbuj ponownie.'};worker.terminate();};
    worker.postMessage({file});
    return ()=>{cancelled=true;worker.terminate();};
  });

  function changeTheme() {
    theme = theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem('rapidmaker-concept-theme', theme); } catch { /* Optional preference only. */ }
  }
  async function useSample(count = 1) { uploadedFiles.clear();meshCache.clear();fileRevision++;settingsCopied=''; models = (await api.getSampleModels(count)).map(model=>({...model,settings:{...emptySettings}})); selectModel(models[0]?.id ?? ''); fileError = ''; simulateError = false; }
  function addFiles(files: FileList | File[] | null) {
    if (!files?.length) return;
    const incoming = Array.from(files);
    if (models.length + incoming.length > 10) { fileError = 'W próbce możesz dodać do 10 modeli. Usuń część plików i spróbuj ponownie.'; return; }
    const invalid = incoming.find(file => !/\.(stl|step|stp|3mf)$/i.test(file.name));
    if (invalid) { fileError = `Nieobsługiwany format: ${invalid.name}. Wybierz STL, STEP lub 3MF.`; return; }
    if (incoming.some(file => file.size === 0 || file.size > 25 * 1024 * 1024)) { fileError = 'Wybierz niepusty plik do 25 MB. W próbce nie analizujemy jego geometrii.'; return; }
    fileError = ''; simulateError = false; settingsCopied='';
    const added=incoming.map(file=>{const id=crypto.randomUUID();uploadedFiles.set(id,file);return {id,name:file.name,quantity:1,settings:{...emptySettings}};});
    models = [...models,...added];fileRevision++;
    selectModel(added[0].id);
    if (fileInput) fileInput.value = '';
  }
  function changeQuantity(id: string, quantity: number) { models = models.map(m => m.id === id ? { ...m, quantity } : m); }
  function prepareContact(event:MouseEvent) {
    if (!canContact || !quote || transitioning) return;
    if(customColorRequest){
      const body=['Cześć, proszę o dobór filamentu i wycenę dla poniższych modeli.','',...models.map(model=>`${model.name} · ${model.quantity} szt.\n${settingsSummary(model.settings!,materials)}`),'','Proszę o potwierdzenie odcienia, dostępności i ceny.','Pliki modeli dołączę do tej wiadomości.'].join('\n');
      location.href='mailto:rapidmaker@rapidmaker.pl?subject='+encodeURIComponent('Dobór koloru i wycena druku 3D')+'&body='+encodeURIComponent(body);return;
    }
    configuratorScroll=window.scrollY;
    orderDraft={models:models.map(model=>({...model,settings:{...model.settings!}})),materials:[...materials],quote:{...quote,lines:quote.lines.map(line=>({...line}))},delivery};
    if(event.detail===0 || matchMedia('(prefers-reduced-motion: reduce)').matches){revealOrder();void focusOrder();}
    else transitioning=true;
  }
</script>

<FlowBackground oncycle={nextGlowCycle} shape={glowShape} mode={glowMode} paused={glowPaused} length={glowLength} speed={glowSpeed}/>
{#if glowPreview}<FlowGlowDemo bind:shape={glowShape} bind:mode={glowMode} bind:paused={glowPaused} bind:length={glowLength} bind:speed={glowSpeed}/>{/if}
<div inert={transitioning} aria-busy={transitioning}>
<a class="skip-link button button-primary" href={checkoutOpen?'#zamowienie':'#main'}>Przejdź do treści</a>
<div class="prototype-bar">Propozycja nowej strony. Podgląd STL działa lokalnie. Pliki pozostają w przeglądarce, ceny są przykładowe.</div>
<header class="header shell">
  <a class="brand" href="#main" aria-label="RapidMaker, początek strony"><img src="/logo.svg" alt="" width="30" height="40" /><span>rapidmaker</span></a>
  <nav aria-label="Główna nawigacja">{#if checkoutOpen}<button class="order-nav-back" onclick={backToConfiguration}>Wróć do konfiguracji</button>{:else}<a href="#oferta">Oferta</a><a href="#wycena">Wycena</a><a href="#kontakt" onclick={highlightContact}>Kontakt</a>{/if}</nav>
  <div class="header-actions"><button class="icon-button" onclick={changeTheme} aria-label={theme === 'dark' ? 'Zmień na jasny motyw' : 'Zmień na ciemny motyw'}><span class="t-icon-swap" data-state={theme === 'dark' ? 'a' : 'b'} aria-hidden="true"><span class="t-icon" data-icon="a"><IconSun size={18} stroke={1.6} /></span><span class="t-icon" data-icon="b"><IconMoon size={18} stroke={1.6} /></span></span></button></div>
</header>

{#snippet scenePreview()}
  {#if selectedModel && uploadedFiles.has(selectedId) && stlPreview.id===selectedId}
    {#if stlPreview.status==='ready' && stlPreview.mesh}
      <PrintScene colorStops={color === 'custom' ? resolveFilament(customShade)?.hexes : undefined} finish={color === 'any' ? 'remnant-gradient' : color === 'custom' ? activeSettings.finish || resolveFilament(customShade)?.series || '' : ''} {theme} material={material || 'unselected'} color={currentColor?.hex ?? '#87908a'} colorName={color === 'custom' ? (customShade ? customChoice.name : 'Wybierz odcień') : currentColor?.name ?? 'Nie wybrano'} {quality} {strength} {strengthTrigger} mesh={stlPreview.mesh} meshKey={selectedId} fileName={selectedModel.name}/>
    {:else}
      <div class="print-scene chamber empty-model-preview"><div class="chamber-top"><span><i></i> Podgląd</span><span>STL / plik klienta</span></div><div class="empty-preview-content" aria-live="polite"><IconCube size={54} stroke={1}/>
        {#if stlPreview.status==='loading'}<h3>Przygotowuję model…</h3><p>{selectedModel.name}</p><p>Możesz w tym czasie wybrać ustawienia wydruku.</p>
        {:else if stlPreview.status==='error'}<h3>Nie można wyświetlić modelu</h3><p>{stlPreview.error}</p><Button variant="secondary" onclick={()=>{previewRetry++;}}>Spróbuj ponownie</Button>
        {:else if stlPreview.status==='unsupported'}<h3>Podgląd dostępny dla STL</h3><p>Ten plik możesz skonfigurować, ale podgląd STEP i 3MF wymaga konwersji. Do podglądu dodaj eksport STL.</p>
        {:else}<h3>Przygotowuję podgląd…</h3>{/if}
      </div></div>
    {/if}
  {:else if models.length && !material}
    <div class="print-scene chamber empty-model-preview"><div class="chamber-top"><span><i></i> Podgląd</span><span>FDM / próbka</span></div><div class="empty-preview-content"><IconCube size={54} stroke={1}/><h3>Wybierz materiał</h3><p>Podgląd próbki pojawi się po wybraniu materiału dla tego modelu.</p></div></div>
  {:else}
    <PrintScene colorStops={color === 'custom' ? resolveFilament(customShade)?.hexes : undefined} finish={color === 'any' ? 'remnant-gradient' : color === 'custom' ? activeSettings.finish || resolveFilament(customShade)?.series || '' : ''} {theme} material={material || 'pla'} color={currentColor?.hex ?? '#626861'} colorName={color === 'custom' ? (customShade ? customChoice.name : 'Wybierz odcień') : currentColor?.name ?? 'Nie wybrano'} {quality} {strength} {strengthTrigger} />
  {/if}
{/snippet}
{#snippet mobilePreview()}{#if isMobile}{@render scenePreview()}{/if}{/snippet}

<div hidden={checkoutOpen}>
<main id="main" class="shell">
  <section class="hero" aria-labelledby="hero-title">
    <div class="hero-copy">
      <h1 id="hero-title"><span class="hero-word">Twój <span class="coral-highlight">pomysł.</span></span><span class="hero-word hero-accent">Realna część.</span></h1>
      <p class="intro"><strong class="intro-lead">Prototyp, część zamienna czy <span>krótka seria.</span></strong><span class="intro-detail">Wydrukujemy Twój model i pomożemy dobrać materiał do jego zadania.</span></p>
      <div class="capabilities"><span><IconCheck size={15} stroke={2} /> Druk FDM</span><span><IconCheck size={15} stroke={2} /> Projektowanie 3D</span><span><IconCheck size={15} stroke={2} /> Odbiór w Piasecznie</span></div>
    </div>

    <div id="wycena">
      <Card class="quote-card">
        <div class="quote-heading"><div><h2>Skonfiguruj wydruk.</h2><p>Materiał. Kolor. Detal. Nadaj mu formę.</p></div><div class="quote-icon"><IconCube size={22} stroke={1.5} /></div></div>
        <input bind:this={fileInput} type="file" class="sr-only" tabindex="-1" aria-label="Dodaj modele STL, STEP lub 3MF" accept=".stl,.step,.stp,.3mf" multiple onchange={event => addFiles(event.currentTarget.files)} />
        <button type="button" class:dragging class="dropzone" onclick={() => fileInput.click()} ondragover={event => { event.preventDefault(); dragging = true; }} ondragleave={() => { dragging = false; }} ondrop={event => { event.preventDefault(); dragging = false; addFiles(event.dataTransfer?.files ?? null); }}>
          <span class="drop-title"><IconUpload size={19} stroke={1.7} /> Dodaj modele lub przeciągnij pliki</span><p>STL, STEP, 3MF / do 25 MB na plik</p>
        </button>
        <div class="sample-link"><Button variant="text" onclick={() => useSample()}>{models.length ? 'Wczytaj ponownie przykład' : 'Nie masz pliku? Wypróbuj przykład'}<IconArrowRight size={15} stroke={1.7} /></Button><Button variant="text" onclick={() => useSample(10)}>Pokaż przykład: 10 STL</Button></div>
        {#if fileError}<p class="error" role="alert">{fileError}</p>{/if}
        {#if models.length}
          <div class="models-summary"><strong>Modele ({models.length}/10)</strong><span>{models.reduce((sum, model) => sum + (model.quantity || 0), 0)} szt. łącznie</span></div>
          <div class="models" class:models-batch={models.length > 4} role="region" aria-label="Lista dodanych modeli">
            {#each models as model (model.id)}
              {@const issues = materials.length ? validateModel(model,materials) : []}
              <div class="model-row" class:model-selected={selectedId===model.id} class:model-invalid={issues.length>0}>
                <button class="model-select" aria-pressed={selectedId===model.id} aria-label={`Konfiguruj ${model.name}`} onclick={()=>selectModel(model.id)}>
                  <IconFile3d size={23} stroke={1.5} />
                  <span><span class="model-name">{model.name}</span><span class="model-meta">{model.settings ? settingsSummary(model.settings,materials) : 'Wybierz ustawienia'}</span><span class="model-state">{issues.length ? 'Uzupełnij ustawienia' : selectedId===model.id ? 'Edytujesz ten model' : 'Ustawienia kompletne'}</span></span>
                </button>
                <label class="quantity"><span class="sr-only">Liczba sztuk: {model.name}</span><Input type="number" min="1" max="999" step="1" value={model.quantity} oninput={event => changeQuantity(model.id, event.currentTarget.valueAsNumber)} /></label>
                <button class="remove" aria-label={`Usuń ${model.name}`} onclick={()=>removeModel(model.id)}><IconX size={17} stroke={1.7} /></button>
              </div>
            {/each}
          </div>
          <div class="model-editor-heading" id="model-settings" tabindex="-1"><div><span class="field-label">Ustawienia modelu {selectedIndex+1}/{models.length}</span><strong>{selectedModel?.name}</strong></div>{#if selectedIndex < models.length-1}<Button variant="text" onclick={()=>selectModel(models[selectedIndex+1].id)}>Następny model<IconArrowRight size={15}/></Button>{/if}</div>
          <p class="model-inheritance">{#if selectedIssues.length}Do uzupełnienia: {selectedIssues.map(error=>error==='dostępny kolor'?'kolor':error).join(', ')}.{:else}Ustawienia zapisane dla tego modelu. Możesz przejść do kolejnego.{/if}</p>
        {/if}
        <ProductionControls {materials} bind:finish={()=>activeSettings.finish??'',value=>updateSettings({finish:value})} bind:material={()=>material,value=>updateSettings({material:value})} bind:color={()=>color,value=>updateSettings({color:value})} bind:customShade={()=>customShade,value=>updateSettings({customShade:value})} bind:quality={()=>quality,value=>updateSettings({quality:value})} bind:strength={()=>strength,value=>updateSettings({strength:value})} preview={mobilePreview} />
        <fieldset class="delivery"><legend class="field-label">Dostawa</legend><div class="delivery-options"><label class="radio-label"><input type="radio" bind:group={delivery} value="pickup" /> Odbiór osobisty</label><label class="radio-label"><input type="radio" bind:group={delivery} value="shipping" /> Wysyłka</label></div></fieldset>
        <label class="check-label"><input type="checkbox" bind:checked={manual} /> Mam nietypowe wymagania do omówienia</label>

        <div class="quote-result" aria-busy={busy}>
          <div role="status" aria-live="polite" aria-atomic="true">
            {#if settingsCopied}<p class="settings-copied"><IconCheck size={16}/> {settingsCopied}</p>{/if}
            {#if !models.length}<p class="price-label">Przykładowa wycena</p><p class="empty-price">Dodaj model, żeby zacząć.</p>
            {:else if validationIssues.length}
              <div class="configuration-progress">
                <div class="configuration-progress-title"><strong>Dokończ konfigurację</strong><span>{models.length-validationIssues.length}/{models.length} gotowych</span></div>
                <div class="configuration-progress-track" role="progressbar" aria-label="Modele z kompletem ustawień" aria-valuemin="0" aria-valuemax={models.length} aria-valuenow={models.length-validationIssues.length}>{#each models as model}<span class:ready={!validateModel(model,materials).length} aria-hidden="true"></span>{/each}</div>
                {#if selectedModelReady && incompleteSettingsIds.length}
                  <p>Skopiuj ustawienia wybranego modelu do nieuzupełnionych plików. Gotowe konfiguracje i liczby sztuk pozostają bez zmian.</p>
                  <Button variant="secondary" class="complete-model" onclick={applySelectedToIncomplete}>Uzupełnij dla wszystkich<IconArrowRight size={16}/></Button>
                {:else}
                  <p>Dokończ ustawienia wybranego modelu, aby skopiować je do nieuzupełnionych plików.</p>
                  <Button variant="secondary" class="complete-model" onclick={completeNextModel}>Uzupełnij ustawienia<IconArrowRight size={16}/></Button>
                {/if}
              </div>
            {:else if quoteError}<p class="error">{quoteError}</p><Button variant="text" onclick={() => { simulateError = false; models = [...models]; }}><IconRefresh size={15} /> Ponów przeliczenie</Button>
            {:else if quote?.manual && !busy}<p class="manual-heading">{customColorRequest?'Kolor dobieramy indywidualnie':'Wymagana ręczna wycena'}</p><p class="estimate-lines">{customColorRequest?'Przygotujemy e-mail z wybranym odcieniem i ustawieniami modeli. Cenę potwierdzimy po doborze filamentu.':'Omówimy wymagania, cenę i termin przed realizacją.'}</p>
            {:else}<div class="price-row"><div><p class="price-label">Przykładowa cena łącznie</p><p class="price" class:stale={busy && !!quote}><span class="t-text-soft" use:softValue={quote ? money(quote.total) : 'Przeliczanie…'}></span></p></div>{#if busy}<span class="processing">{quote ? 'Cena nieaktualna' : 'Przeliczanie'}</span>{:else}<IconCheck size={21} stroke={1.7} />{/if}</div>{#if quote}<p class="estimate-lines">{models.reduce((sum, m) => sum + (m.quantity || 0), 0)} szt. / {models.length} modeli z osobnymi ustawieniami / dostawa {money(quote.delivery)}</p>{/if}{/if}
          </div>
          <Button class="quote-contact" disabled={!canContact} onclick={prepareContact}>{customColorRequest?'Potwierdź kolor przez e-mail':'Porozmawiajmy o realizacji'}<IconArrowUpRight size={17} stroke={1.7} /></Button>
          <p class="estimate-note">Symulacja interfejsu. Cena nie wynika z geometrii pliku i nie stanowi oferty. Termin i koszt potwierdzimy indywidualnie.</p>
        </div>
      </Card>
    </div>
    {#if !isMobile}{@render scenePreview()}{/if}
  </section>

  <section id="oferta" class="content-section" aria-labelledby="services-title">
    <div class="section-head"><h2 id="services-title">Części, które mają zadanie.</h2></div>
    <div class="services"><div class="services-main service"><h3>Druk FDM i krótkie serie</h3><p>Prototypy, obudowy, uchwyty i części zamienne. Dobieramy materiał oraz sposób wykonania do zastosowania elementu.</p><div class="material-list"><span>PLA</span><span>PETG</span><span>ASA</span><span>TPU</span></div></div><div><div class="service"><h3>Projektowanie i skanowanie 3D</h3><p>Nie masz gotowego pliku? Opowiedz, co część ma robić. Pomożemy przygotować model i dostosować go do druku.</p></div><div class="service"><h3>Więcej technologii</h3><p>Projekty w MJF i SLA realizujemy przez partnerów. Skontaktuj się z nami, żeby ustalić odpowiedni proces.</p></div></div></div>
  </section>

  <section id="kontakt" class="content-section" aria-labelledby="contact-title"><div class="contact-grid"><div><h2 id="contact-title" tabindex="-1">Porozmawiajmy<br />o Twojej części.</h2><p class="contact-lead">Prześlij model lub opisz, czego potrzebujesz. Ustalimy materiał, cenę i termin realizacji.</p>{#if contactContext}<p class="contact-context">{contactContext}</p>{/if}<div class="contact-links" class:contact-glow={contactGlow}><a class="contact-link" href="tel:+48537320816"><IconPhone size={21} stroke={1.6} />537 320 816<IconArrowUpRight size={16} /></a><a class="contact-link" href="mailto:rapidmaker@rapidmaker.pl?subject=Wycena%20druku%203D"><IconMail size={21} stroke={1.6} />rapidmaker@rapidmaker.pl<IconArrowUpRight size={16} /></a></div><div class="address"><p><strong>Pracownia RapidMaker</strong></p><p>Zielony Krąg 1, 05-500 Piaseczno</p><small>Pon.-pt. 9:00-17:00. Wizyty po umówieniu.</small></div></div><div class="map"><iframe title="Mapa dojazdu do RapidMaker w Piasecznie" src="https://www.google.com/maps/embed?pb=!1m18!1m12!1m3!1d2452.44230244811!2d20.994547177129274!3d52.07167776932459!2m3!1f0!2f0!3f0!3m2!1i1024!2i768!4f13.1!3m3!1m2!1s0x47192fc6e84dd8ff%3A0xf49d7b6dd1af3606!2sRapid%20Maker!5e0!3m2!1spl!2spl!4v1687527970408!5m2!1spl!2spl" loading="lazy" referrerpolicy="no-referrer-when-downgrade"></iframe><div class="map-footer"><span>Odbiór osobisty po umówieniu</span><a href="https://www.google.com/maps/dir/?api=1&destination=ul.+Zielony+Kr%C4%85g+1,+05-500+Piaseczno" target="_blank" rel="noreferrer">Wyznacz trasę</a></div></div></div></section>

  <details class="prototype-tools"><summary>Narzędzia podglądu</summary><div class="specimen-tools"><Button variant="text" onclick={toggleGlowPreview}>{glowPreview?'Wróć do automatycznego światła':'Porównaj animacje tła'}</Button><Button variant="text" onclick={changeTheme}>Zmień motyw</Button><Button variant="text" onclick={async () => { if (!models.length) await useSample(); simulateError = true; document.getElementById('wycena')?.scrollIntoView(); }}>Symuluj błąd wyceny</Button></div></details>
</main>
<footer class="shell"><span>RapidMaker / Piaseczno</span><span>Próbka projektu. Wycena i kontakt teraz, zakup w kolejnym etapie.</span><a href="#main">Wróć na górę</a></footer>

</div>
{#if orderDraft}<div hidden={!checkoutOpen}><OrderPage draft={orderDraft} onback={backToConfiguration}/></div>{/if}
</div>
{#if transitioning}<PrintTransition oncovered={revealOrder} oncomplete={finishOrderTransition}/>{/if}
