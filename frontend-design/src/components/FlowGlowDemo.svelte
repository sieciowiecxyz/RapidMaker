<script lang="ts">
  import {IconPlayerPause,IconPlayerPlay,IconChevronDown,IconChevronUp} from '@tabler/icons-svelte';
  let {mode=$bindable('pulse'),paused=$bindable(false),shape=$bindable('bulge'),length=$bindable('medium'),speed=$bindable('normal')}:{mode:'off'|'pulse'|'band'|'wave'|'comet';paused?:boolean;shape?:'trail'|'bulge'|'premium';length?:'short'|'medium'|'long';speed?:'slow'|'normal'|'fast'}=$props();
  let open=$state(true);
  const options=[{id:'pulse',name:'Impuls',detail:'Jedna koralowa smuga po centralnej linii.'},{id:'band',name:'Pas',detail:'Szeroki pas światła na sąsiednich liniach.'},{id:'wave',name:'Fala',detail:'Limonkowe impulsy przesunięte w czasie.'},{id:'comet',name:'Kometa',detail:'Limonkowa główka i zwężający się koralowy ogon.'},{id:'off',name:'Wył.',detail:'Obecne, nieruchome tło.'}] as const;
</script>

<aside class="glow-demo" aria-label="Porównanie animacji tła">
  <div class="demo-heading"><span>Światło na liniach <small>Podgląd</small></span><button type="button" aria-label={open?'Zwiń wybór animacji':'Rozwiń wybór animacji'} aria-expanded={open} onclick={()=>open=!open}>{#if open}<IconChevronDown size={16}/>{:else}<IconChevronUp size={16}/>{/if}</button></div>
  {#if open}
    <div class="shape-options" aria-label="Rodzaj poświaty"><button type="button" class:selected={shape==='trail'} aria-pressed={shape==='trail'} onclick={()=>{shape='trail';paused=false;}}>Smuga <small>Poprzednia</small></button><button type="button" class:selected={shape==='bulge'} aria-pressed={shape==='bulge'} onclick={()=>{shape='bulge';paused=false;}}>Zgrubienie</button><button type="button" class:selected={shape==='premium'} aria-pressed={shape==='premium'} onclick={()=>{shape='premium';paused=false;}}>Glow <small>Premium</small></button></div>
    <div class="demo-options" aria-label="Wariant światła">{#each options as option}<button type="button" class:selected={mode===option.id} aria-pressed={mode===option.id} onclick={()=>{mode=option.id;paused=false;}}>{option.name}</button>{/each}<button class="pause" type="button" disabled={mode==='off'} aria-label={paused?'Wznów światło':'Wstrzymaj światło'} onclick={()=>paused=!paused}>{#if paused}<IconPlayerPlay size={16}/>{:else}<IconPlayerPause size={16}/>{/if}</button></div>
    <div class="tuning"><span>Długość</span><div aria-label="Długość smugi">{#each [{id:'short',name:'Krótka'},{id:'medium',name:'Średnia'},{id:'long',name:'Długa'}] as choice}<button type="button" class:selected={length===choice.id} aria-pressed={length===choice.id} onclick={()=>{length=choice.id as typeof length;paused=false;}}>{choice.name}</button>{/each}</div></div>
    <div class="tuning"><span>Prędkość</span><div aria-label="Prędkość światła">{#each [{id:'slow',name:'Wolno'},{id:'normal',name:'Normalnie'},{id:'fast',name:'Szybko'}] as choice}<button type="button" class:selected={speed===choice.id} aria-pressed={speed===choice.id} onclick={()=>{speed=choice.id as typeof speed;paused=false;}}>{choice.name}</button>{/each}</div></div>
    <p>{options.find(o=>o.id===mode)?.detail}</p>
    <p class="reduced-note">Animacja wyłączona zgodnie z ustawieniami ograniczonego ruchu.</p>
  {/if}
</aside>

<style>
 .glow-demo{position:fixed;left:20px;bottom:20px;z-index:20;width:370px;max-width:calc(100vw - 32px);background:var(--surface);color:var(--ink);border:1px solid var(--control-line);box-shadow:0 10px 32px #0005;padding:12px 14px;border-radius:var(--control-radius)}
 .demo-heading{display:flex;align-items:center;justify-content:space-between;gap:12px;font:600 13px var(--display)}.demo-heading small{font:400 10px var(--body);color:var(--muted);margin-left:8px}.demo-heading button{display:grid;place-items:center;width:28px;height:28px;border:0;background:none;color:var(--muted);cursor:pointer}
 .demo-options{display:flex;gap:5px;margin-top:9px}.demo-options button{min-height:38px;flex:1;border:1px solid var(--line);background:transparent;color:var(--ink);font-size:10px;font-weight:600;cursor:pointer}.demo-options button.selected{color:var(--accent);background:var(--soft);border-color:var(--accent)}.demo-options button.pause{flex:0 0 34px;display:grid;place-items:center}.demo-options button:disabled{opacity:.4;cursor:default}
 .shape-options{display:flex;gap:6px;margin-top:10px}.shape-options button{flex:1;min-height:42px;border:1px solid var(--control-line);background:none;color:var(--ink);font-size:12px;font-weight:650;cursor:pointer}.shape-options button.selected{border-color:var(--accent);background:var(--soft);box-shadow:inset 0 -2px var(--accent)}.shape-options small{font-size:9px;font-weight:400;color:var(--muted);margin-left:5px}
 .tuning{display:flex;align-items:center;justify-content:space-between;gap:10px;margin-top:10px}.tuning>span{font-size:10px;color:var(--muted)}.tuning>div{display:flex;gap:3px}.tuning button{min-height:30px;min-width:59px;padding:5px 8px;border:1px solid var(--line);background:transparent;color:var(--muted);font-size:10px;cursor:pointer}.tuning button.selected{background:var(--soft);border-color:var(--accent);color:var(--accent)}
 p{font-size:10px;line-height:1.5;color:var(--muted);margin-top:9px}.reduced-note{display:none}button:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
 @media(hover:hover) and (pointer:fine){button:hover{border-color:var(--accent);color:var(--accent)}}
 @media(prefers-reduced-motion:reduce){.reduced-note{display:block;color:var(--accent)}}
 @media(max-width:760px){.glow-demo{left:16px;bottom:12px;width:340px}}
</style>
