<script lang="ts">
  import gsap from 'gsap';
  import { slide } from 'svelte/transition';
  import { IconCheck } from '@tabler/icons-svelte';
  import { customColors } from '../lib/colors';
  let { value = $bindable() }: { value:string } = $props();
  const choice=$derived(customColors.find(shade=>shade.id===value));
  const step=360/customColors.length;
  const wheelGradient=`conic-gradient(from 0deg, ${[...customColors,customColors[0]].map((shade,i)=>`${shade.hex} ${i*step}deg`).join(', ')})`;
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
  <fieldset class="wheel-field">
    <legend class="sr-only">Wybierz orientacyjny odcień</legend>
    <div class="wheel-stage" transition:bloom|global use:petals>
      <div class="wheel-disc" style:background={wheelGradient} aria-hidden="true"></div>
      <div class="wheel-center" aria-hidden="true"><span style={`background:${choice?.hex ?? '#626861'}`}></span><strong>{choice?.name ?? 'Wybierz odcień'}</strong><small>{choice ? 'Wybrany odcień' : 'Kliknij kolor na kole'}</small></div>
      {#each customColors as shade,i}
        {@const angle=(-90+i*step)*Math.PI/180}
        {@const x=Math.cos(angle)*103}
        {@const y=Math.sin(angle)*103}
        <label class="wheel-point" class:selected={value===shade.id} style={`left:calc(50% + ${x}px);top:calc(50% + ${y}px);--pigment:${shade.hex}`} data-x={x} data-y={y} title={shade.name}>
          <input class="choice-input" type="radio" name="custom-shade" value={shade.id} bind:group={value} aria-label={shade.name}/>
          <span class="wheel-sample" aria-hidden="true"><IconCheck size={17} stroke={2}/></span>
          <span class="sr-only">{shade.name}</span>
        </label>
      {/each}
    </div>
  </fieldset>
  <p class="wheel-hint">Wybierz kolor z koła. Dokładny odcień i dostępność potwierdzimy przy wycenie.</p>
</div>
