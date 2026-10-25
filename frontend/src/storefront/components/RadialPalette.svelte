<script lang="ts">
  import { untrack } from 'svelte';
  import gsap from 'gsap';
  import { slide } from 'svelte/transition';
  import { IconCheck, IconChevronLeft, IconChevronRight } from '@tabler/icons-svelte';
  import { customColors } from '../lib/colors';
  let { value = $bindable() }: { value:string } = $props();
  const seriesOptions=[...new Set(customColors.filter(c=>c.series!=='Orientacyjne').map(c=>c.series)), 'Orientacyjne'];
  let series=$state(untrack(()=>customColors.find(c=>c.id===value && c.series!=='Orientacyjne')?.series ?? 'PLA Basic'));
  let page=$state(0);
  const group=$derived(customColors.filter(c=>c.series===series));
  const pages=$derived(Math.ceil(group.length/12));
  const visible=$derived(group.slice(page*12,(page+1)*12));
  const choice=$derived(customColors.find(shade=>shade.id===value));
  const step=$derived(360/visible.length);
  const wheelGradient=$derived(`conic-gradient(from 0deg, ${[...visible,visible[0]].map((shade,i)=>`${shade.hex} ${i*step}deg`).join(', ')})`);
  const ease=gsap.parseEase('power3.out');
  const duration=()=>matchMedia('(prefers-reduced-motion:reduce)').matches||document.documentElement.dataset.input==='keyboard'?0:420;
  function bloom(node:HTMLElement){
    const trigger=node.closest('.color-field')?.querySelector('.pigment-sample.custom')?.getBoundingClientRect();
    const box=node.getBoundingClientRect();
    const dx=trigger?trigger.left+trigger.width/2-box.left-box.width/2:0;
    const dy=trigger?trigger.top+trigger.height/2-box.top-box.height/2:-140;
    return {duration:duration(),easing:ease,css:(t:number,u:number)=>`opacity:${Math.min(1,t*3)};transform:translate(${dx*u}px,${dy*u}px) scale(${.16+.84*t});`};
  }
  function petals(node:HTMLElement){
    if(!duration())return;
    const points=node.querySelectorAll<HTMLElement>('.wheel-point');
    const tween=gsap.from(points,{x:(_,el:HTMLElement)=>-Number(el.dataset.x),y:(_,el:HTMLElement)=>-Number(el.dataset.y),scale:.7,opacity:0,duration:.38,delay:.06,stagger:.014,ease:'power3.out',clearProps:'transform,opacity'});
    return {destroy(){tween.kill();}};
  }
</script>

<div class="radial-palette-panel" transition:slide|global={{duration:duration(),easing:ease}}>
  <div class="palette-series" aria-label="Seria kolorów PLA">{#each seriesOptions as option}<button type="button" class:active={series===option} aria-pressed={series===option} onclick={()=>{series=option;page=0;}}>{option}</button>{/each}</div>
  <div class="palette-page"><span>{series} · {group.length} odcieni</span><div><button type="button" disabled={page===0} aria-label="Poprzednie kolory" onclick={()=>page--}><IconChevronLeft size={16}/></button><span>{page+1} / {pages}</span><button type="button" disabled={page>=pages-1} aria-label="Następne kolory" onclick={()=>page++}><IconChevronRight size={16}/></button></div></div>
  <fieldset class="wheel-field">
    <legend class="sr-only">Wybierz orientacyjny odcień</legend>
    {#key series+page}<div class="wheel-stage" transition:bloom|global use:petals>
      <div class="wheel-disc" style:background={wheelGradient} aria-hidden="true"></div>
      <div class="wheel-center" aria-hidden="true"><span style={`background:${choice?.hex ?? '#626861'}`}></span><strong>{choice?.name ?? 'Wybierz odcień'}</strong><small>{choice ? 'Wybrany odcień' : 'Kliknij kolor na kole'}</small></div>
      {#each visible as shade,i}
        {@const angle=(-90+i*step)*Math.PI/180}
        {@const x=Math.cos(angle)*103}
        {@const y=Math.sin(angle)*103}
        <label class="wheel-point" class:selected={value===shade.id} style={`left:calc(50% + ${x}px);top:calc(50% + ${y}px);--pigment:${shade.hexes.length>1?`linear-gradient(135deg,${shade.hexes.join(',')})`:shade.hex}`} data-x={x} data-y={y} title={shade.name+' · '+shade.code}>
          <input class="choice-input" type="radio" name="custom-shade" value={shade.id} bind:group={value} aria-label={shade.name}/>
          <span class="wheel-sample" aria-hidden="true"><IconCheck size={17} stroke={2}/></span>
          <span class="sr-only">{shade.name}</span>
        </label>
      {/each}
    </div>{/key}
  </fieldset>
  <p class="wheel-hint">Odcienie Bambu Lab PLA. Kolor na ekranie jest orientacyjny; dostępność potwierdzimy przy wycenie.</p>
</div>

<style>
 .palette-series{display:flex;gap:6px;overflow-x:auto;padding:12px 8px;scrollbar-width:thin;max-width:100%}
 .palette-series button{padding:7px 10px;white-space:nowrap;border:1px solid var(--line);border-radius:3px;background:var(--surface);color:var(--muted);font-size:11px;flex-shrink:0;cursor:pointer}
 .palette-series button.active{color:var(--accent);border-color:var(--accent);background:var(--soft)}
 .palette-page{display:flex;align-items:center;justify-content:space-between;padding:0 10px;color:var(--muted);font-size:11px;gap:10px}
 .palette-page>div{display:flex;align-items:center;gap:10px}.palette-page button{display:grid;place-items:center;width:32px;height:32px;border:1px solid var(--line);background:var(--surface);color:var(--ink);cursor:pointer;border-radius:3px}.palette-page button:disabled{opacity:.4;cursor:default}
 .palette-series button:active,.palette-page button:active:not(:disabled){transform:scale(.97)}
 .wheel-field{min-width:0}.wheel-sample{background:var(--pigment)}
</style>
