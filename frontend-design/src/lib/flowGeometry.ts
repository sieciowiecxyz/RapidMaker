type Point={x:number;y:number};
/** Arc-length lookup for the generated M/C contours, without synchronous SVG geometry reads. */
export function sampleFlowPath(path:string,count=512):{points:Point[];total:number}{
  const values=path.match(/-?\d*\.?\d+(?:e[-+]?\d+)?/gi)?.map(Number)??[];
  if(values.length<8||(values.length-2)%6!==0||!values.every(Number.isFinite))return {points:[],total:0};
  const polyline=[{x:values[0],y:values[1]}],distances=[0];
  let start=polyline[0],total=0;
  for(let i=2;i<values.length;i+=6){
    for(let j=1;j<=48;j++){
      const t=j/48,u=1-t;
      const p={x:u*u*u*start.x+3*u*u*t*values[i]+3*u*t*t*values[i+2]+t*t*t*values[i+4],y:u*u*u*start.y+3*u*u*t*values[i+1]+3*u*t*t*values[i+3]+t*t*t*values[i+5]};
      const previous=polyline[polyline.length-1];total+=Math.hypot(p.x-previous.x,p.y-previous.y);polyline.push(p);distances.push(total);
    }
    start=polyline[polyline.length-1];
  }
  let segment=1;
  const points=Array.from({length:count+1},(_,i)=>{
    const distance=total*i/count;
    while(segment<distances.length-1&&distances[segment]<distance)segment++;
    const a=polyline[segment-1],b=polyline[segment],t=(distance-distances[segment-1])/(distances[segment]-distances[segment-1]||1);
    return {x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t};
  });
  return {points,total};
}
