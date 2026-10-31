import earcut from 'earcut';
type P=[number,number];type V=[number,number,number];
export type PartMesh={vertices:Float32Array;minY:number;maxY:number;sectionMask?:Uint8Array};
const circle=(r:number,cx=0,cy=0,n=64):P[]=>Array.from({length:n},(_,i)=>[cx+Math.cos(i/n*Math.PI*2)*r,cy+Math.sin(i/n*Math.PI*2)*r]);
function roundedRect(w:number,h:number,r:number):P[]{return [[w-r,h-r],[-w+r,h-r],[-w+r,-h+r],[w-r,-h+r]].flatMap(([x,y],i)=>Array.from({length:12},(_,j)=>{const a=(i*90+j/11*90)*Math.PI/180;return [x+Math.cos(a)*r,y+Math.sin(a)*r] as P;}));}
function roundedPolygon(points:P[],fraction=.18):P[]{return points.flatMap((p,i)=>{const a=points[(i+points.length-1)%points.length],b=points[(i+1)%points.length];const start:P=[p[0]+(a[0]-p[0])*fraction,p[1]+(a[1]-p[1])*fraction],end:P=[p[0]+(b[0]-p[0])*fraction,p[1]+(b[1]-p[1])*fraction];return Array.from({length:10},(_,j)=>{const t=j/9,u=1-t;return [u*u*start[0]+2*u*t*p[0]+t*t*end[0],u*u*start[1]+2*u*t*p[1]+t*t*end[1]] as P;});});}
function hull(points:P[]):P[]{const sorted=[...points].sort((a,b)=>a[0]-b[0]||a[1]-b[1]);const cross=(a:P,b:P,c:P)=>(b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]);const lower:P[]=[],upper:P[]=[];for(const p of sorted){while(lower.length>1&&cross(lower.at(-2)!,lower.at(-1)!,p)<=0)lower.pop();lower.push(p);}for(const p of sorted.reverse()){while(upper.length>1&&cross(upper.at(-2)!,upper.at(-1)!,p)<=0)upper.pop();upper.push(p);}return [...lower.slice(0,-1),...upper.slice(0,-1)];}
export function buildPart(material:string):PartMesh {
  const data:number[]=[];
  const normal=(a:V,b:V,c:V):V=>{const u=b.map((v,i)=>v-a[i]),v=c.map((q,i)=>q-a[i]);const n:V=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];const l=Math.hypot(...n)||1;return n.map(v=>v/l) as V;};
  const tri=(a:V,b:V,c:V,n?:V)=>{const geometric=normal(a,b,c),face=n??geometric;const reverse=geometric.reduce((sum,v,i)=>sum+v*face[i],0)<0;for(const p of reverse?[a,c,b]:[a,b,c])data.push(...p,...face);};
  function extrude(outer:P[],holes:P[][],height:number,centerY=0,bevel=2){
    const contours=[outer,...holes];const center=(ring:P[]):P=>[ring.reduce((s,p)=>s+p[0],0)/ring.length,ring.reduce((s,p)=>s+p[1],0)/ring.length];
    const inset=(ring:P[],amount:number):P[]=>{const c=center(ring);return ring.map(p=>{const d=Math.hypot(p[0]-c[0],p[1]-c[1])||1;return [p[0]+(c[0]-p[0])*amount/d,p[1]+(c[1]-p[1])*amount/d];});};
    const edge=contours.map((r,i)=>inset(r,i===0?bevel:-bevel));
    const flat=edge.flat().flat(),starts:number[]=[];let index=edge[0].length;for(const ring of edge.slice(1)){starts.push(index);index+=ring.length;}
    const points=edge.flat();const indices=earcut(flat,starts,2);
    for(let i=0;i<indices.length;i+=3){const p=indices.slice(i,i+3).map(j=>points[j]);tri([p[0][0],centerY+height/2,p[0][1]],[p[1][0],centerY+height/2,p[1][1]],[p[2][0],centerY+height/2,p[2][1]],[0,1,0]);tri([p[2][0],centerY-height/2,p[2][1]],[p[1][0],centerY-height/2,p[1][1]],[p[0][0],centerY-height/2,p[0][1]],[0,-1,0]);}
    contours.forEach((ring,ringIndex)=>{
      const e=edge[ringIndex];
      const bands=[{ring:e,y:centerY-height/2},{ring,y:centerY-height/2+bevel},{ring,y:centerY+height/2-bevel},{ring:e,y:centerY+height/2}];
      for(let k=0;k<bands.length-1;k++)for(let i=0;i<ring.length;i++){
        const j=(i+1)%ring.length;const a:V=[bands[k].ring[i][0],bands[k].y,bands[k].ring[i][1]],b:V=[bands[k].ring[j][0],bands[k].y,bands[k].ring[j][1]],c:V=[bands[k+1].ring[j][0],bands[k+1].y,bands[k+1].ring[j][1]],d:V=[bands[k+1].ring[i][0],bands[k+1].y,bands[k+1].ring[i][1]];
        let n=normal(a,b,c);const mid:P=[(a[0]+b[0])/2,(a[2]+b[2])/2],origin=center(ring);const outward=(mid[0]-origin[0])*n[0]+(mid[1]-origin[1])*n[2];if((outward<0&&ringIndex===0)||(outward>0&&ringIndex>0))n=n.map(v=>-v) as V;tri(a,b,c,n);tri(a,c,d,n);
      }
    });
  }
  function revolve(profile:P[]){const count=96;for(let row=0;row<profile.length-1;row++){const [r0,y0]=profile[row],[r1,y1]=profile[row+1];for(let i=0;i<count;i++){const a=i/count*Math.PI*2,b=(i+1)/count*Math.PI*2;const points:V[]=[[Math.cos(a)*r0,y0,Math.sin(a)*r0],[Math.cos(b)*r0,y0,Math.sin(b)*r0],[Math.cos(b)*r1,y1,Math.sin(b)*r1],[Math.cos(a)*r1,y1,Math.sin(a)*r1]];const slope=(r0-r1)/(y1-y0||.001);const n=(angle:number):V=>{const len=Math.hypot(1,slope);return [Math.cos(angle)/len,slope/len,Math.sin(angle)/len];};for(const idx of [0,2,1,0,3,2])data.push(...points[idx],...n(idx===0||idx===3?a:b));}}}
  if(material==='pla'){
    const profile:P[]=[[0,-80],[43,-80],[47,-77],[48,-72],[47,-67],[42,-64],[41,-59],[36,-56],[34,-52],[31,-48],[28,-41],[24,-31],[20,-18],[17,-5],[16,7],[23,9],[25,13],[24,17],[17,21],[12,24],[12,28]];
    for(let i=0;i<=36;i++){const angle=-Math.PI/2+i/36*Math.PI;profile.push([Math.max(.1,Math.cos(angle)*27),55+Math.sin(angle)*27]);}revolve(profile);
  }else if(material==='petg'){
    const gear:P[]=Array.from({length:96},(_,i)=>{const a=i/96*Math.PI*2,r=[87,87,103,105,103,87][i%6];return [Math.cos(a)*r,Math.sin(a)*r];});extrude(gear,[circle(25),...Array.from({length:6},(_,i)=>circle(10,Math.cos(i/6*Math.PI*2)*56,Math.sin(i/6*Math.PI*2)*56))],36,0,2);
    // Raised hub starts at the gear top; its bore wall does not overlap the main body.
    extrude(circle(40),[circle(25)],12,24,2);
  }else if(material==='pa'){
    // Three socket bosses and a recessed web, proportioned from the supplied reference.
    const bosses:P[]=[[-72,-53],[72,-53],[0,91]];
    const outline:P[]=[];
    const radius=36;
    for(let i=0;i<3;i++){
      const prev=bosses[(i+2)%3],c=bosses[i],next=bosses[(i+1)%3];
      const incoming:P=[c[0]-prev[0],c[1]-prev[1]],outgoing:P=[next[0]-c[0],next[1]-c[1]];
      let start=Math.atan2(-incoming[0],incoming[1]),end=Math.atan2(-outgoing[0],outgoing[1]);while(end<start)end+=Math.PI*2;
      for(let j=0;j<=28;j++){const angle=start+(end-start)*j/28;outline.push([c[0]+Math.cos(angle)*radius,c[1]+Math.sin(angle)*radius]);}
      const length=Math.hypot(...outgoing),normal:P=[outgoing[1]/length,-outgoing[0]/length];
      const from=outline.at(-1)!,to:P=[next[0]+normal[0]*radius,next[1]+normal[1]*radius];
      const control:P=[(c[0]+next[0])/2+normal[0]*3,(c[1]+next[1])/2+normal[1]*3];
      for(let j=1;j<16;j++){const t=j/16,u=1-t;outline.push([u*u*from[0]+2*u*t*control[0]+t*t*to[0],u*u*from[1]+2*u*t*control[1]+t*t*to[1]]);}
    }
    const bores=bosses.map(([x,y])=>circle(25.6,x,y));
    extrude(outline,bores,24,-21,3);
    const pocket=roundedPolygon([[-41,-29],[41,-29],[0,49]],.28);
    extrude(outline,[...bores,pocket],26,4,3);
    for(let i=0;i<bosses.length;i++){
      const [x,y]=bosses[i],height=i===2?59:78,center=i===2?-3:6;
      extrude(circle(36.5,x,y),[circle(25,x,y)],height,center,2.5);
      // A second shoulder inside each bore makes the sockets read as engineered seats.
      extrude(circle(25.3,x,y),[circle(22,x,y)],7,center+height/2-15,1);
    }
  }else if(material==='tpu'){
    // Symmetrical, hollow bellows coupling based on the reference photograph.
    const outer:P[]=[[31,-64],[32,-62],[32,-58],[30,-56],[30,-42]];
    for(let fold=0;fold<5;fold++){
      const y=-40+fold*16;
      outer.push([30,y],[31,y+2],[46,y+6],[48,y+8],[46,y+10],[31,y+14],[30,y+16]);
    }
    outer.push([30,56],[32,58],[32,62],[31,64]);
    const inner:P[]=outer.slice().reverse().map(([r,y])=>[r-3,y]);
    const profile:P[]=[...outer,...inner,outer[0]],count=96;
    const surfaceNormal=(index:number,angle:number):V=>{
      const before=profile[Math.max(0,index-1)],after=profile[Math.min(profile.length-1,index+1)];
      const dr=after[0]-before[0],dy=after[1]-before[1],length=Math.hypot(dr,dy)||1;
      return [Math.cos(angle)*dy/length,-dr/length,Math.sin(angle)*dy/length];
    };
    for(let row=0;row<profile.length-1;row++)for(let i=0;i<count;i++){
      const [r0,y0]=profile[row],[r1,y1]=profile[row+1],a=i/count*Math.PI*2,b=(i+1)/count*Math.PI*2;
      const points:V[]=[[Math.cos(a)*r0,y0,Math.sin(a)*r0],[Math.cos(b)*r0,y0,Math.sin(b)*r0],[Math.cos(b)*r1,y1,Math.sin(b)*r1],[Math.cos(a)*r1,y1,Math.sin(a)*r1]];
      for(const index of [0,2,1,0,3,2])data.push(...points[index],...surfaceNormal(row+(index>=2?1:0),index===0||index===3?a:b));
    }
  }else{
    extrude(roundedRect(96,72,14),[],12,-28,3);
    extrude(roundedRect(96,72,14),[roundedRect(84,60,8)],58,5,2);
    for(const [x,y] of [[-73,-49],[73,-49],[-73,49],[73,49]])extrude(circle(9,x,y),[circle(3,x,y)],48,0,1);
  }
  let minY=Infinity,maxY=-Infinity;for(let i=1;i<data.length;i+=6){minY=Math.min(minY,data[i]);maxY=Math.max(maxY,data[i]);}
  return {vertices:new Float32Array(data),minY,maxY};
}
