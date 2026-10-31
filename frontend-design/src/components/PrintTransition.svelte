<script lang="ts">
  import { onMount } from 'svelte';
  import gsap from 'gsap';
  let { oncovered, oncomplete }: { oncovered:()=>void; oncomplete:()=>void } = $props();
  let root: HTMLDivElement;
  let skip: HTMLButtonElement;
  let finish=()=>{};
  onMount(()=>{
    const previousOverflow=document.body.style.overflow;
    document.body.style.overflow='hidden';
    skip.focus({preventScroll:true});
    const rows=root.querySelectorAll<HTMLElement>('.print-wipe-row');
    const head=root.querySelector<HTMLElement>('.print-wipe-head')!;
    let covered=false,finished=false;
    const cover=()=>{if(!covered){covered=true;oncovered();}};
    const timeline=gsap.timeline({onComplete:()=>{if(!finished){finished=true;oncomplete();}}});
    const width=window.innerWidth, height=window.innerHeight;
    gsap.set(head,{x:-90,y:height/rows.length-110});
    rows.forEach((row,i)=>{
      const time=i*.14;
      timeline.to(row,{scaleX:1,duration:.14,ease:'none'},time);
      timeline.to(head,{x:i%2?-90:width-90,y:(i+1)*height/rows.length-110,duration:.14,ease:'none'},time);
    });
    timeline.to(head,{opacity:0,duration:.1},1.12)
      .call(cover,[],1.16)
      .to(root,{opacity:0,duration:.3,ease:'power4.out'},1.28);
    timeline.timeScale(1.15);
    finish=()=>{if(finished)return;finished=true;timeline.kill();cover();oncomplete();};
    const onKey=(event:KeyboardEvent)=>{if(event.key==='Escape'){event.preventDefault();finish();}else if(event.key==='Tab'){event.preventDefault();skip.focus();}};
    window.addEventListener('keydown',onKey);
    return ()=>{timeline.kill();document.body.style.overflow=previousOverflow;window.removeEventListener('keydown',onKey);};
  });
</script>

<div class="print-wipe" bind:this={root}>
  <div class="print-wipe-layers" aria-hidden="true">
    {#each Array(8) as _,i}<div class="print-wipe-row" style:transform-origin={i%2?'right center':'left center'}></div>{/each}
  </div>
  <div class="print-wipe-head" aria-hidden="true">
    <svg viewBox="0 0 180 136" fill="none"><path d="M54 0v22m72-22v22" stroke="#a7b5a2" stroke-width="9"/><rect x="26" y="20" width="128" height="71" rx="9" fill="#141d15" stroke="#edf4e9" stroke-width="2"/><rect x="35" y="29" width="110" height="53" rx="4" fill="#202e1d"/><circle cx="90" cy="55" r="22" stroke="#758469" stroke-width="2"/><path d="m90 35 9 15 12 5-16 5-5 15-6-15-15-5 16-5Z" fill="#a7b5a2"/><circle cx="90" cy="55" r="6" fill="#141d15"/><path d="M63 92h54v16H63z" fill="#a7b5a2"/><path d="M70 108h40l-13 18H83z" fill="#e6b45a" stroke="#0b100c" stroke-width="2"/><path d="M87 126h6v10h-6z" fill="#edf4e9"/><circle cx="42" cy="36" r="3" fill="#b5ee4b"/><circle cx="138" cy="36" r="3" fill="#b5ee4b"/></svg>
  </div>
  <div class="print-wipe-label" role="status">Przygotowujemy miejsce dla Twojego projektu.</div>
  <button class="print-wipe-skip" bind:this={skip} onclick={()=>finish()}>Pomiń animację <span aria-hidden="true">↗</span></button>
</div>

<style>
  .print-wipe{position:fixed;inset:0;z-index:1000;isolation:isolate;cursor:progress}
  .print-wipe-layers{position:absolute;inset:0;display:grid;grid-template-rows:repeat(8,1fr)}
  .print-wipe-row{transform:scaleX(0);background:#b5ee4b;background-image:repeating-linear-gradient(0deg,transparent 0 5px,#111a0812 5px 6px);margin-bottom:-1px}
  .print-wipe-head{position:absolute;left:0;top:0;width:180px;height:136px;filter:drop-shadow(0 8px 8px #0005);will-change:transform;pointer-events:none}
  .print-wipe-head svg{width:100%;height:100%}
  .print-wipe-label{position:absolute;left:24px;bottom:24px;background:#0b100c;color:#edf4e9;padding:10px 16px;font-size:13px}
  .print-wipe-skip{position:absolute;right:24px;top:24px;min-height:44px;padding:10px 16px;background:#0b100c;color:#edf4e9;border:1px solid #758469;font-size:13px;cursor:pointer}
  .print-wipe-skip span{margin-left:14px;color:#b5ee4b}
  @media(max-width:760px){.print-wipe-label{left:16px;bottom:16px;max-width:calc(100% - 32px);font-size:12px}.print-wipe-head{width:140px}.print-wipe-skip{right:16px;top:16px}}
</style>
