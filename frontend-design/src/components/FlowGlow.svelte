<script lang="ts">
  import {onMount,tick} from 'svelte';
  import gsap from 'gsap';
  let {paths,mode,length,speed,paused=false,premium=false,oncycle}:{paths:string[];mode:'pulse'|'band'|'wave'|'comet';length:'short'|'medium'|'long';speed:'slow'|'normal'|'fast';paused?:boolean;premium?:boolean;oncycle?:()=>void}=$props();
  let host:SVGGElement;
  type XY={x:number;y:number};
  type Track={points:XY[];total:number;halo:SVGPathElement;soft:SVGPathElement;core:SVGPathElement;gradients:SVGLinearGradientElement[]};
  let configure=(_paths:string[])=>{};
  $effect(()=>{premium;configure(paths);});
  onMount(()=>{
    let tracks:Track[]=[],alive=true,phase=0,last=0,drawn=0;
    const reduced=matchMedia('(prefers-reduced-motion:reduce)');
    configure=(curves)=>{void tick().then(()=>{
      if(!alive)return;
      tracks=curves.filter(Boolean).map((curve,i)=>{
        const source=document.createElementNS('http://www.w3.org/2000/svg','path');source.setAttribute('d',curve);
        const total=source.getTotalLength();
        const points=Array.from({length:513},(_,j)=>{const p=source.getPointAtLength(total*j/512);return {x:p.x,y:p.y};});
        const lane=host.querySelectorAll<SVGGElement>('.bulge-lane')[i];
        return {points,total,halo:lane.querySelector<SVGPathElement>('.halo')!,soft:lane.querySelector<SVGPathElement>('.soft')!,core:lane.querySelector<SVGPathElement>('.core')!,gradients:Array.from(lane.querySelectorAll<SVGLinearGradientElement>('linearGradient'))};
      });
      phase=tracks.length?Math.min(.8,(window.scrollY+350)/tracks[0].total):0;
      draw();
    });};
    const point=(track:Track,distance:number)=>{
      const position=Math.max(0,Math.min(512,distance/track.total*512)),index=Math.min(511,Math.floor(position)),t=position-index,a=track.points[index],b=track.points[index+1];
      return {x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t};
    };
    function draw(){
      if(reduced.matches)return;
      const extent=({short:85,medium:180,long:340})[length]*(mode==='comet'?1.4:1);
      for(let i=0;i<tracks.length;i++){
        const track=tracks[i],center=((phase+(mode==='wave'?i*.025:0))%1)*track.total;
        const halfWidth=premium?(mode==='band'?1.1:mode==='wave'?1.8:2.6):mode==='band'?3.8:mode==='wave'?6:9;
        const upper:XY[]=[],lower:XY[]=[],spine:XY[]=[];
        for(let j=0;j<=32;j++){
          const t=j/32,d=center+(t-.5)*extent,p=point(track,d),a=point(track,d-2),b=point(track,d+2);
          const tangent=Math.hypot(b.x-a.x,b.y-a.y)||1;
          const envelope=mode==='comet'?Math.pow(t,1.7)*Math.pow(Math.sin(Math.PI*t),.7)*2:Math.pow(Math.sin(Math.PI*t),2);
          const radius=.25+halfWidth*envelope;
          const nx=-(b.y-a.y)/tangent,ny=(b.x-a.x)/tangent;
          spine.push(p);
          upper.push({x:p.x+nx*radius,y:p.y+ny*radius});lower.push({x:p.x-nx*radius,y:p.y-ny*radius});
        }
        const outline=[...upper,...lower.reverse()];
        const d=outline.map((p,j)=>`${j?'L':'M'}${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' ')+' Z';
        for(const node of [track.halo,track.soft])node.setAttribute('d',d);
        track.core.setAttribute('d',premium?spine.map((p,j)=>`${j?'L':'M'}${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' '):d);
        for(const gradient of track.gradients){const a=point(track,center-extent/2),b=point(track,center+extent/2);gradient.setAttribute('x1',String(a.x));gradient.setAttribute('y1',String(a.y));gradient.setAttribute('x2',String(b.x));gradient.setAttribute('y2',String(b.y));}
      }
    }
    const update=(time:number)=>{
      const delta=last?Math.min(.1,time-last):0;last=time;
      if(paused||document.hidden||reduced.matches||document.documentElement.dataset.theme==='light')return;
      const nextPhase=phase+delta/({slow:28,normal:18,fast:10})[speed];
      phase=nextPhase%1;
      if(nextPhase>=1)oncycle?.();
      if(time-drawn<1/30)return;drawn=time;draw();
    };
    configure(paths);gsap.ticker.add(update);
    return()=>{alive=false;configure=()=>{};gsap.ticker.remove(update);};
  });
</script>

<g class="bulges" class:premium bind:this={host} data-mode={mode} aria-hidden="true">
  {#each paths.filter(Boolean) as _,i}
    {@const coral=mode==='pulse'||mode==='comet'||i===Math.floor(paths.length/2)}
    <g class="bulge-lane" style={`--pigment:${premium?`url(#flow-premium-${i})`:mode==='comet'?`url(#flow-comet-${i})`:coral?'#FF5C77':'#B5EE4B'};--hot:url(#flow-hot-${i})`}>
      {#if premium}<defs>
        <linearGradient id={`flow-premium-${i}`} gradientUnits="userSpaceOnUse"><stop offset="0" stop-color={coral?'#FF5C77':'#B5EE4B'} stop-opacity="0"/><stop offset=".22" stop-color={coral?'#FF5C77':'#B5EE4B'} stop-opacity=".3"/><stop offset=".65" stop-color={coral?'#FF5C77':'#B5EE4B'}/><stop offset=".85" stop-color={mode==='comet'?'#B5EE4B':coral?'#FF92A5':'#DAFF97'}/><stop offset="1" stop-color={mode==='comet'?'#B5EE4B':coral?'#FF5C77':'#B5EE4B'} stop-opacity="0"/></linearGradient>
        <linearGradient id={`flow-hot-${i}`} gradientUnits="userSpaceOnUse"><stop offset="0" stop-color={coral?'#FFD0D9':'#ECFFC9'} stop-opacity="0"/><stop offset=".45" stop-color={coral?'#FFD0D9':'#ECFFC9'} stop-opacity=".45"/><stop offset=".8" stop-color="#FFFCEB"/><stop offset="1" stop-color="#FFFCEB" stop-opacity="0"/></linearGradient>
      </defs>{/if}
      {#if mode==='comet'&&!premium}<defs><linearGradient id={`flow-comet-${i}`} gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#FF5C77" stop-opacity="0"/><stop offset=".45" stop-color="#FF5C77"/><stop offset="1" stop-color="#B5EE4B"/></linearGradient></defs>{/if}
      <path class="halo"/><path class="soft"/><path class="core"/>
    </g>
  {/each}
</g>

<style>
 .bulge-lane path{fill:var(--pigment);stroke:var(--pigment);stroke-linejoin:round}
 .halo{stroke-width:12;opacity:.055}.soft{stroke-width:5;opacity:.14}.core{stroke-width:0;opacity:.9}
 .bulges[data-mode=band] .halo{opacity:.025}.bulges[data-mode=band] .soft{opacity:.07}.bulges[data-mode=band] .core{opacity:.65}
 .bulges.premium .halo{stroke-width:7;opacity:.65;filter:blur(6px)}.bulges.premium .soft{stroke-width:1;opacity:.8;filter:blur(1.5px)}.bulges.premium .core{fill:none;stroke:var(--hot);stroke-width:1.05;stroke-linecap:round;opacity:.95}
 .bulges.premium[data-mode=band] .halo{opacity:.26}.bulges.premium[data-mode=band] .soft{opacity:.6}.bulges.premium[data-mode=band] .core{opacity:.8}
 @media(prefers-reduced-motion:reduce){.bulges{display:none}}
</style>
