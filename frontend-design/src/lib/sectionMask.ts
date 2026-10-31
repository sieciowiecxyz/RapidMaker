// A small, cached boundary-distance texture makes the illustrative shell follow real slice edges.
export function makeSectionMask(vertices:Float32Array,height:number):HTMLCanvasElement {
  const canvas=document.createElement('canvas');canvas.width=canvas.height=512;
  const ctx=canvas.getContext('2d')!;
  drawSectionMask(vertices,height,ctx);
  return canvas;
}
export function drawSectionMask(vertices:Float32Array,height:number,ctx:CanvasRenderingContext2D|OffscreenCanvasRenderingContext2D){
  ctx.fillStyle='#000';ctx.fillRect(0,0,512,512);
  const path=new Path2D(),map=(value:number)=>(value+180)/360*512;
  for(let t=0;t<vertices.length;t+=18){
    const intersections:number[][]=[];
    for(let i=0;i<3;i++){
      const a=t+i*6,b=t+((i+1)%3)*6,ay=vertices[a+1],by=vertices[b+1];
      if((ay<height&&by>=height)||(by<height&&ay>=height)){
        const fraction=(height-ay)/(by-ay);
        intersections.push([vertices[a]+(vertices[b]-vertices[a])*fraction,vertices[a+2]+(vertices[b+2]-vertices[a+2])*fraction]);
      }
    }
    if(intersections.length===2){path.moveTo(map(intersections[0][0]),map(intersections[0][1]));path.lineTo(map(intersections[1][0]),map(intersections[1][1]));}
  }
  ctx.lineCap='round';ctx.lineJoin='round';
  for(let radius=12;radius>=.5;radius-=.5){const value=Math.round((1-radius/12)*255);ctx.strokeStyle=`rgb(${value} ${value} ${value})`;ctx.lineWidth=radius*2/360*512;ctx.stroke(path);}
}
