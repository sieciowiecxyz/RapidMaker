<script lang="ts">
  import { onMount } from 'svelte';
  import gsap from 'gsap';
  import type { StlMesh } from '../lib/stlParser';
  import { sampleParts } from '../lib/partGeometry';
  import { createPartViewer, type ViewerState } from '../lib/partViewer';
  import { IconPlayerPause, IconPlayerPlay, IconRefresh, IconArrowDownRight, IconRotateClockwise, IconPlus, IconMinus } from '@tabler/icons-svelte';
  let { theme, material, color, colorName, quality, strength, strengthTrigger=0, mesh, fileName, meshKey }: { theme:'dark'|'light';material:string;color:string;colorName:string;quality:string;strength:string;strengthTrigger?:number;mesh?:StlMesh;fileName?:string;meshKey?:string } = $props();
  let canvas:HTMLCanvasElement,host:HTMLDivElement;
  const materialName=$derived(material === 'pa' ? 'PA-CF' : material === 'unselected' ? 'Nie wybrano' : material.toUpperCase());
  const sample=$derived(sampleParts[material]??sampleParts.pla);
  let paused=$state(false),reducedMotion=$state(false),unavailable=$state(false);
  let replay=()=>{},togglePause=()=>{},rotate=()=>{},zoomIn=()=>{},zoomOut=()=>{};
  let demonstrating=$state(false);
  const strengthName=$derived(({light:'Lekka',standard:'Standard',reinforced:'Wzmocniona'} as Record<string,string>)[strength]);
  let configure=(_theme:string,_material:string,_color:string,_quality:string,_strength:string,_mesh?:StlMesh,_meshKey?:string,_strengthTrigger?:number)=>{};
  $effect(()=>configure(theme,material,color,quality,strength,mesh,meshKey,strengthTrigger));
  onMount(()=>{
    let viewer:ReturnType<typeof createPartViewer>;
    try {viewer=createPartViewer(canvas);}catch {unavailable=true;return;}
    if(!viewer){unavailable=true;return;}
    const renderer=viewer;
    const reduced=matchMedia('(prefers-reduced-motion:reduce)');
    const state:ViewerState={yaw:-.45,pitch:.62,build:1,flex:0,layer:.13,sweep:1,r:.16,g:.18,b:.16,oldR:.16,oldG:.18,oldB:.16,explode:0,zoom:1,upperOpacity:1,density:.065,wall:4};
    let active=true,lastDraw=0,kind='',hex='',lastStrengthTrigger=strengthTrigger;
    let rotation:gsap.core.Tween|undefined,entrance:gsap.core.Tween|undefined,flex:gsap.core.Timeline|undefined,demonstration:gsap.core.Timeline|undefined;
    // Explicit redraws must work while IntersectionObserver catches up after scroll/resize.
    const draw=()=>{if(!document.hidden)renderer.draw(state);};
    const syncActivity=()=>{const stop=paused||demonstrating||!active||document.hidden||reduced.matches;rotation?.paused(stop);entrance?.paused(stop);flex?.paused(stop);};
    function start(){demonstration?.kill();demonstrating=false;state.explode=0;state.upperOpacity=1;rotation?.kill();entrance?.kill();reducedMotion=reduced.matches;if(reduced.matches){state.build=1;draw();return;}state.build=.04;entrance=gsap.to(state,{build:1,duration:1.3,ease:'power3.out',onUpdate:draw});rotation=gsap.to(state,{yaw:state.yaw+Math.PI*2,duration:55,ease:'none',repeat:-1});syncActivity();}
    function demonstrate(){
      demonstration?.kill();entrance?.kill();state.build=1;
      if(reduced.matches||document.documentElement.dataset.input==='keyboard'){state.explode=0;state.upperOpacity=1;demonstrating=false;syncActivity();draw();return;}
      demonstrating=true;syncActivity();
      demonstration=gsap.timeline({onUpdate:draw,onComplete:()=>{state.explode=0;state.upperOpacity=1;demonstrating=false;syncActivity();draw();}})
        .to(state,{explode:1,upperOpacity:0,duration:.48,ease:'power3.out'})
        .to(state,{explode:1,duration:.4})
        .to(state,{explode:0,zoom:1,upperOpacity:1,duration:.6,ease:'power3.inOut'});
    }
    configure=(_theme,nextMaterial,nextColor,nextQuality,nextStrength,nextMesh,nextMeshKey,nextStrengthTrigger)=>{
      const instant=reduced.matches||document.documentElement.dataset.input==='keyboard';
      const nextKind=nextMesh ? `upload:${nextMeshKey}` : nextMaterial;
      if(kind!==nextKind){demonstration?.kill();demonstrating=false;state.explode=0;state.upperOpacity=1;kind=nextKind;renderer.setPart(kind,nextMesh);state.zoom=1;entrance?.kill();flex?.kill();state.flex=0;if(!instant&&!paused){state.build=.12;entrance=gsap.to(state,{build:1,duration:.6,ease:'power3.out',onUpdate:draw});if(kind==='tpu')flex=gsap.timeline({onUpdate:draw}).to(state,{flex:1,duration:.45,ease:'power2.inOut'}).to(state,{flex:0,duration:.65,ease:'elastic.out(1,.5)'});}else state.build=1;}
      if(hex!==nextColor){hex=nextColor;state.oldR=state.r;state.oldG=state.g;state.oldB=state.b;const rgb=nextColor.replace('#','').match(/.{2}/g)?.map(v=>parseInt(v,16)/255)??[.16,.18,.16];gsap.killTweensOf(state,'r,g,b,sweep');if(instant){[state.r,state.g,state.b]=rgb;state.sweep=1;}else{state.sweep=0;gsap.to(state,{r:rgb[0],g:rgb[1],b:rgb[2],sweep:1,duration:.65,ease:'power3.out',onUpdate:draw});}}
      const nextDensity=({light:.035,standard:.065,reinforced:.115} as Record<string,number>)[nextStrength]??.065;
      const nextWall=({light:2,standard:4,reinforced:7} as Record<string,number>)[nextStrength]??4;
      gsap.to(state,{wall:nextWall,density:nextDensity,duration:instant?0:.35,ease:'power3.out',overwrite:'auto',onUpdate:draw});
      if(nextStrength && lastStrengthTrigger!==nextStrengthTrigger)demonstrate();lastStrengthTrigger=nextStrengthTrigger ?? 0;
      gsap.to(state,{layer:nextQuality==='fine'?.26:nextQuality==='draft'?.08:.13,duration:instant?0:.25,ease:'power3.out',overwrite:'auto',onUpdate:draw});syncActivity();draw();
    };
    configure(theme,material,color,quality,strength,mesh,meshKey,strengthTrigger);
    const resize=new ResizeObserver(()=>{const width=host.clientWidth,height=matchMedia('(max-width:760px)').matches?210:Math.max(320,Math.min(460,width*.83));const ratio=Math.min(devicePixelRatio,1.5);canvas.width=Math.round(width*ratio);canvas.height=Math.round(height*ratio);canvas.style.height=`${height}px`;draw();});resize.observe(host);
    zoomIn=()=>{state.zoom=Math.min(2.4,state.zoom*1.2);draw();};zoomOut=()=>{state.zoom=Math.max(.5,state.zoom/1.2);draw();};
    replay=()=>{paused=false;start();};togglePause=()=>{paused=!paused;if(!paused&&!reduced.matches){rotation?.kill();rotation=gsap.to(state,{yaw:state.yaw+Math.PI*2,duration:55,ease:'none',repeat:-1});}syncActivity();};
    rotate=()=>{paused=true;syncActivity();if(reduced.matches){state.yaw+=Math.PI/4;draw();}else gsap.to(state,{yaw:state.yaw+Math.PI/4,duration:.3,ease:'power3.out',overwrite:'auto',onUpdate:draw});};
    const tick=(time:number)=>{if((paused&&!demonstrating)||reduced.matches||document.hidden||!active||time-lastDraw<1/30)return;lastDraw=time;draw();};gsap.ticker.add(tick);
    const visible=new IntersectionObserver(([entry])=>{active=entry.isIntersecting;syncActivity();if(active)draw();},{threshold:.05});visible.observe(host);
    let dragging=false,lastX=0,lastY=0;
    const down=(event:PointerEvent)=>{if(event.pointerType==='mouse'||event.pointerType==='pen'){dragging=true;lastX=event.clientX;lastY=event.clientY;canvas.setPointerCapture(event.pointerId);}};
    const move=(event:PointerEvent)=>{if(!dragging)return;rotation?.pause();state.yaw+=(event.clientX-lastX)*.009;state.pitch=Math.max(-1.4,Math.min(1.4,state.pitch+(event.clientY-lastY)*.007));lastX=event.clientX;lastY=event.clientY;draw();};
    const up=()=>{if(dragging)paused=true;dragging=false;syncActivity();};
    canvas.addEventListener('pointerdown',down);canvas.addEventListener('pointermove',move);canvas.addEventListener('pointerup',up);canvas.addEventListener('pointercancel',up);document.addEventListener('visibilitychange',syncActivity);reduced.addEventListener('change',start);start();
    return()=>{demonstration?.kill();rotation?.kill();entrance?.kill();flex?.kill();gsap.killTweensOf(state);gsap.ticker.remove(tick);resize.disconnect();visible.disconnect();document.removeEventListener('visibilitychange',syncActivity);reduced.removeEventListener('change',start);canvas.removeEventListener('pointerdown',down);canvas.removeEventListener('pointermove',move);canvas.removeEventListener('pointerup',up);canvas.removeEventListener('pointercancel',up);renderer.dispose();configure=()=>{};};
  });
</script>

<div class="print-scene chamber" bind:this={host}>
  <div class="chamber-top"><span><i></i> Podgląd</span><span>{mesh ? 'Twój plik / 3D' : 'FDM / próbka'}</span></div>
  <div class="chamber-visual" role="img" aria-label={mesh ? `Model z pliku ${fileName}. Obróć przeciągając myszą. Kolor: ${colorName}.` : `Pokazowa część: ${sample.name}, ${materialName}, ${colorName}, jakość ${quality === 'fine' ? 'dokładna' : quality === 'standard' ? 'standardowa' : quality === 'draft' ? 'robocza' : 'nie wybrano'}. ${demonstrating ? `Poglądowe wypełnienie i ścianki: ${strengthName}.` : ''} Nie przedstawia wgranego pliku.`}>
    <span class="chamber-watermark" aria-hidden="true">{mesh && material === 'unselected' ? '3D' : materialName}</span>
    <canvas bind:this={canvas} aria-hidden="true"></canvas>{#if unavailable}<p class="viewer-fallback">Podgląd 3D jest niedostępny na tym urządzeniu. Konfigurator działa dalej.</p>{/if}
    <span class="chamber-callout"><IconArrowDownRight size={16} stroke={1.5} />{fileName ?? sample.name}</span>
  </div>
  {#if demonstrating}<div class="strength-demonstration" role="status"><span>Wypełnienie i ścianki · {strengthName}</span><small>Pokaz poglądowy</small></div>{/if}
  <div class="chamber-data"><div><span>Materiał</span><strong>{materialName}</strong></div><div><span>Kolor</span><strong>{colorName}</strong></div><div><span>Warstwy</span><strong>{quality === 'fine' ? 'Dokładne' : quality === 'standard' ? 'Standard' : quality === 'draft' ? 'Robocze' : 'Nie wybrano'}</strong></div></div>
  <div class="scene-caption"><span>{#if mesh}{mesh.triangleCount.toLocaleString('pl-PL')} trójkątów · podgląd geometrii. Przekrój i wypełnienie poglądowe; nie są wynikiem slicera.{:else}{sample.application}. Obiekt pokazowy.{/if}</span><div class="scene-actions"><button onclick={()=>zoomOut()} disabled={unavailable} aria-label="Oddal model"><IconMinus size={16}/></button><button onclick={()=>zoomIn()} disabled={unavailable} aria-label="Przybliż model"><IconPlus size={16}/></button><button onclick={()=>rotate()} disabled={unavailable} aria-label="Obróć model"><IconRotateClockwise size={16}/></button><button onclick={()=>togglePause()} disabled={reducedMotion || unavailable} aria-label={paused?'Wznów animację druku':'Wstrzymaj animację druku'}>{#if paused}<IconPlayerPlay size={16} />{:else}<IconPlayerPause size={16} />{/if}</button><button onclick={()=>replay()} disabled={reducedMotion || unavailable} aria-label="Odtwórz animację druku"><IconRefresh size={16}/></button></div></div>
</div>
