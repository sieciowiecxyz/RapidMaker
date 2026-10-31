<script lang="ts">
  import { onMount } from 'svelte';
  type Point={x:number;y:number;spread:number};
  let width=$state(1800),height=$state(2000);
  let contours=$state<string[]>([]);
  let accent=$state('');
  function smooth(points:{x:number;y:number}[], contactApproachX?:number){
    let d=`M ${points[0].x} ${points[0].y}`;
    for(let i=0;i<points.length-1;i++){
      const p0=points[Math.max(0,i-1)],p1=points[i],p2=points[i+1],p3=points[Math.min(points.length-1,i+2)];
      // Preserve the approved contact curve from the wider "before" variant.
      const startX=i===4 && contactApproachX!==undefined ? contactApproachX : p0.x;
      const endX=i===3 && contactApproachX!==undefined ? contactApproachX : p1.x;
      d+=` C ${p1.x+(p2.x-startX)/6} ${p1.y+(p2.y-p0.y)/6}, ${p2.x-(p3.x-endX)/6} ${p2.y-(p3.y-p1.y)/6}, ${p2.x} ${p2.y}`;
    }
    return d;
  }
  onMount(()=>{
    let frame=0,active=true;
    const measure=()=>{
      frame=0;if(!active)return;
      const w=document.documentElement.clientWidth,h=document.body.scrollHeight;
      const rect=(selector:string)=>{
        const e=document.querySelector<HTMLElement>(selector);
        if(!e || !e.getClientRects().length)return null;
        const r=e.getBoundingClientRect();return {left:r.left,right:r.right,top:r.top+window.scrollY,bottom:r.bottom+window.scrollY,height:r.height};
      };
      const heading=rect('.hero-copy h1'),scene=rect('.chamber'),cta=rect('.quote-contact'),card=rect('.quote-card'),contact=rect('#contact-title');
      const order=rect('.order-page');
      let points:Point[];
      if(order){
        points=[{x:w*.1,y:-240,spread:1},{x:Math.max(18,order.left-32),y:order.top+150,spread:.6},{x:Math.min(w-12,order.right+25),y:h*.6,spread:.5},{x:w*.75,y:h+320,spread:1.2}];
      }else if(heading&&scene&&cta&&card&&contact){
        const mobile=w<760;
        points=[
          {x:Math.max(-100,heading.left-220),y:-240,spread:1.3},
          {x:Math.max(12,heading.left-42),y:heading.top+heading.height*.55,spread:1.05},
          {x:Math.max(10,scene.left-(mobile?10:46)),y:scene.top+scene.height*.4,spread:1.15},
          {x:mobile?Math.min(w-10,card.right+10):(cta.left+cta.right)/2,y:cta.top+cta.height*.5,spread:.9},
          {x:Math.max(10,contact.left-(mobile?10:36)),y:contact.top+contact.height*.5,spread:1.0},
          {x:w*.72,y:h+350,spread:1.25},
        ];
      }else return;
      const contactApproachX=!order && card ? Math.min(w-10,card.right+(w<760?10:45)) : undefined;
      const paths=Array.from({length:35},(_,i)=>{
        const offset=(i-17)*(w<760?14:24);
        return smooth(points.map(p=>({x:p.x+offset*p.spread,y:p.y})),contactApproachX===undefined?undefined:contactApproachX+offset*points[3].spread);
      });
      const coral=smooth(points.map(p=>({x:p.x,y:p.y})),contactApproachX);
      if(JSON.stringify(paths)!==JSON.stringify(contours)){width=w;height=h;contours=paths;accent=coral;}
    };
    const schedule=()=>{if(!frame&&active)frame=requestAnimationFrame(measure);};
    const observer=new ResizeObserver(schedule);
    observer.observe(document.body);
    const main=document.getElementById('main');if(main)observer.observe(main);
    window.addEventListener('resize',schedule);
    void document.fonts.ready.then(schedule);schedule();
    return()=>{active=false;observer.disconnect();cancelAnimationFrame(frame);window.removeEventListener('resize',schedule);};
  });
</script>
<svg class="flow-background" viewBox={`0 0 ${width} ${height}`} width={width} height={height} aria-hidden="true" focusable="false">
  <g fill="none" stroke="#B5EE4B" stroke-width=".8" opacity=".115">{#each contours as path}<path d={path}/>{/each}</g>
  <path d={accent} fill="none" stroke="#FF5C77" stroke-width="1.1" opacity=".32"/>
</svg>
