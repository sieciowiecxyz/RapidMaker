import { buildPart, type PartMesh } from './partMesh';
import { makeSectionMask } from './sectionMask';
const vertex=`
attribute vec3 aPosition;attribute vec3 aNormal;
uniform float uYaw,uPitch,uAspect,uFlex,uScale,uCap,uCut,uOffset,uMin,uMax;
varying vec3 vNormal,vPosition,vLocal;varying float vLayerSurface;
void main(){
 vec3 p=aPosition;if(uCap>.5)p.y=uCut;vLocal=p;vLayerSurface=1.-abs(aNormal.y);p.y+=uOffset;p*=uScale;
 p.y*=1.-uFlex*.28;
 p.xz*=1.+uFlex*.025;
 vec3 flexibleNormal=normalize(vec3(aNormal.x/(1.+uFlex*.025),aNormal.y/(1.-uFlex*.28),aNormal.z/(1.+uFlex*.025)));
 float c=cos(uYaw),s=sin(uYaw);mat3 yaw=mat3(c,0.,-s,0.,1.,0.,s,0.,c);
 float cp=cos(uPitch),sp=sin(uPitch);mat3 pitch=mat3(1.,0.,0.,0.,cp,sp,0.,-sp,cp);
 mat3 turn=pitch*yaw;vec3 view=turn*p;vNormal=turn*flexibleNormal;vPosition=view;
 float w=410.-view.z;
 gl_Position=vec4(view.x*2.65/uAspect,view.y*2.65,(1100./900.)*w-200000./900.,w);
}`;
const fragment=`
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform vec3 uColor,uOldColor;uniform float uSweep,uBuild,uMin,uMax,uLayer,uGround,uGloss,uSliced,uLow,uHigh,uClipSide,uCut,uCap,uDensity,uWall,uOpacity,uFinish;
uniform sampler2D uBoundary;
uniform vec3 uFilamentColors[8];uniform float uColorCount;
varying vec3 vNormal,vPosition,vLocal;varying float vLayerSurface;
vec3 remnantPigment(vec3 p){
 float height=(p.y-uMin)/max(uMax-uMin,1.);
 float blend=smoothstep(.1,.9,height*.85+p.x/240.+.075);
 return mix(vec3(1.,.361,.467),vec3(.710,.933,.294),blend);
}
vec3 filamentPigment(vec3 p){
 float height=clamp((p.y-uMin)/max(uMax-uMin,1.),0.,1.);
 // Coextruded Silk shows different pigments around the part, while gradient
 // filaments change along its height. This is an illustrative material preview.
 float t=uFinish==2.?(.5+.5*sin(atan(p.z,p.x)+height*.6)):height;
 float position=t*max(uColorCount-1.,1.);
 vec3 pigment=uFilamentColors[0];
 for(int i=0;i<7;i++){
   if(float(i+1)<uColorCount) pigment=mix(pigment,uFilamentColors[i+1],smoothstep(0.,1.,position-float(i)));
 }
 return pigment;
}
void main(){
 if(uGround>.5){float r=length(vLocal.xz/vec2(160.,125.));float alpha=exp(-r*r*3.8)*.36;gl_FragColor=vec4(0.,0.,0.,alpha);return;}
 if(uBuild<.999 && vLocal.y>mix(uMin,uMax,uBuild))discard;
 if(uCap>.5){
   vec2 p=vLocal.xz;
   vec2 cell=abs(fract(vec2(p.x+p.y,p.x-p.y)*uDensity)-.5);
   float line=1.-smoothstep(.055,.12,min(cell.x,cell.y));
   float edge=texture2D(uBoundary,(p+180.)/360.).r;
   float wall=smoothstep(1.-uWall/12.-.03,1.-uWall/12.+.03,edge);
   vec3 lattice=mix(vec3(.035,.055,.04),uColor*.7+vec3(.18),line);
   vec3 shell=(uFinish==6.?remnantPigment(vLocal):uColorCount>1.?filamentPigment(vLocal):uColor)*.75+vec3(.22);
   gl_FragColor=vec4(mix(lattice,shell,wall),uOpacity);return;
 }
 if(uSliced>.5){
   if(uClipSide<1.5 && vLocal.y>uHigh+.01)discard;
   if((uClipSide<.5||uClipSide>1.5) && vLocal.y<uLow-.01)discard;
 }
 float phase=vLocal.y*uLayer*6.283;
 float layerVisibility=clamp(vLayerSurface,0.,1.);
 #ifdef FILTER_LAYERS
 layerVisibility*=1.-smoothstep(.8,2.2,fwidth(phase));
 #endif
 vec3 n=normalize(vNormal+vec3(0.,sin(phase)*.24*layerVisibility,0.));
 vec3 view=normalize(vec3(0.,0.,410.)-vPosition);
 vec3 key=normalize(vec3(-.65,.85,1.2)),fill=normalize(vec3(.8,.25,.4));
 float diffuse=max(dot(n,key),0.);float bounce=max(dot(n,fill),0.);
 float spec=max(dot(n,normalize(key+view)),0.);
 float gloss=uFinish==1.||uFinish==3.?9.:uFinish==2.?95.:uGloss;
 float sheen=pow(spec,gloss);
 float rim=pow(1.-max(dot(n,view),0.),3.);
 float stripe=pow(.5+.5*cos(phase),2.5)*layerVisibility;
 vec3 base=mix(uColor,uOldColor,smoothstep(uSweep-1.5,uSweep+1.5,vLocal.y));
 // Object-space patterns stay attached to the part while it rotates.
 if(uFinish==3.){
   // Sparse mineral inclusions, anchored in object space rather than stripes.
   vec3 grainPosition=vLocal*.62;
   vec3 cell=floor(grainPosition);
   float seed=fract(sin(dot(cell,vec3(127.1,311.7,74.7)))*43758.5453);
   vec3 center=vec3(.25)+.5*fract(sin(vec3(dot(cell,vec3(12.1,47.7,113.1)),dot(cell,vec3(89.7,17.3,61.1)),dot(cell,vec3(43.7,93.1,29.3))))*43758.5453);
   float distanceToGrain=length(fract(grainPosition)-center);
   float radius=mix(.13,.31,seed);
   float softness=.055;
   #ifdef FILTER_LAYERS
   softness=max(softness,fwidth(distanceToGrain)*.8);
   #endif
   float mineral=(1.-smoothstep(radius-softness,radius+softness,distanceToGrain))*step(.65,seed);
   base=mix(base,base*.21,mineral*.85);
 }
 if(uColorCount>1.)base=filamentPigment(vLocal);
 if(uFinish==6.)base=remnantPigment(vLocal);
 base=pow(base,vec3(2.2));
 vec3 light=base*(.24+diffuse*1.55+bounce*.42)*(1.-stripe*.42);
 float specularStrength=uFinish==1.||uFinish==3.?.025:uFinish==2.?1.6:.22;
 light+=vec3(.8,.88,1.)*(sheen*specularStrength+rim*.11);
 if(uFinish==2.)light+=vec3(.85,.92,1.)*pow(spec,14.)*.25;
 if(uFinish==4.){
   vec3 cell=floor(vLocal*2.1);
   float grain=fract(sin(dot(cell,vec3(12.9898,78.233,37.719)))*43758.5453);
   float glitter=step(.965,grain)*pow(max(dot(n,normalize(key+view)),0.),7.);
   light+=vec3(1.,.88,.59)*glitter*1.8;
 }
 float cavity=smoothstep(uMin,uMin+18.,vLocal.y);light*=.76+cavity*.24;
 float cut=1.-smoothstep(.0,1.8,abs(vLocal.y-mix(uMin,uMax,uBuild)));
 light+=vec3(.30,.50,.05)*cut*(1.-step(.999,uBuild));
 light=light/(light+vec3(.65));light=pow(light,vec3(1./2.2));
 gl_FragColor=vec4(light,uOpacity*(uFinish==5.?(.80+rim*.15):1.));
}`;
export type ViewerState={yaw:number;pitch:number;build:number;flex:number;layer:number;sweep:number;r:number;g:number;b:number;oldR:number;oldG:number;oldB:number;explode:number;zoom:number;upperOpacity:number;density:number;wall:number;finish:number;colorStops:number[]};
export function createPartViewer(canvas:HTMLCanvasElement){
 const gl=canvas.getContext('webgl',{alpha:true,antialias:true,premultipliedAlpha:false,stencil:true,powerPreference:'low-power'});
 if(!gl)return null;
 const compile=(type:number,source:string)=>{const shader=gl.createShader(type)!;gl.shaderSource(shader,source);gl.compileShader(shader);if(!gl.getShaderParameter(shader,gl.COMPILE_STATUS)){const error=gl.getShaderInfoLog(shader);gl.deleteShader(shader);throw new Error(error??'Nie udało się uruchomić podglądu.');}return shader;};
 const layerFilter=gl.getExtension('OES_standard_derivatives');
 const filteredFragment=(layerFilter?'#extension GL_OES_standard_derivatives : enable\n#define FILTER_LAYERS\n':'')+fragment;
 const vs=compile(gl.VERTEX_SHADER,vertex),fs=compile(gl.FRAGMENT_SHADER,filteredFragment),program=gl.createProgram()!;gl.attachShader(program,vs);gl.attachShader(program,fs);gl.linkProgram(program);if(!gl.getProgramParameter(program,gl.LINK_STATUS))throw new Error('Nie udało się uruchomić podglądu.');gl.useProgram(program);
 const buffer=gl.createBuffer()!;const ground=gl.createBuffer()!;
 gl.bindBuffer(gl.ARRAY_BUFFER,ground);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([-240,-95,-200,0,1,0,240,-95,-200,0,1,0,240,-95,200,0,1,0,-240,-95,-200,0,1,0,240,-95,200,0,1,0,-240,-95,200,0,1,0]),gl.STATIC_DRAW);
 const cap=gl.createBuffer()!;
 gl.bindBuffer(gl.ARRAY_BUFFER,cap);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([
 -180,0,-180,0,1,0,180,0,-180,0,1,0,180,0,180,0,1,0,-180,0,-180,0,1,0,180,0,180,0,1,0,-180,0,180,0,1,0
 ]),gl.STATIC_DRAW);
 const position=gl.getAttribLocation(program,'aPosition'),normal=gl.getAttribLocation(program,'aNormal');gl.enableVertexAttribArray(position);gl.enableVertexAttribArray(normal);
 const uniform=(name:string)=>gl.getUniformLocation(program,name);
 const names=['uYaw','uPitch','uAspect','uFlex','uColor','uOldColor','uSweep','uBuild','uMin','uMax','uLayer','uGround','uGloss','uScale','uSliced','uLow','uHigh','uClipSide','uOffset','uCut','uCap','uDensity','uWall','uOpacity','uBoundary','uFinish','uFilamentColors[0]','uColorCount'];const u=Object.fromEntries(names.map(name=>[name,uniform(name)]));
 let count=0,min=-80,max=82,kind='';let lost=false;
 const boundaries:WebGLTexture[]=[];const layers=2;
 const onLost=(event:Event)=>{event.preventDefault();lost=true;};canvas.addEventListener('webglcontextlost',onLost);
 function setPart(material:string,external?:PartMesh){
   if(kind===material)return;kind=material;const mesh=external ?? buildPart(material);min=mesh.minY;max=mesh.maxY;count=mesh.vertices.length/6;
   gl!.bindBuffer(gl!.ARRAY_BUFFER,buffer);gl!.bufferData(gl!.ARRAY_BUFFER,mesh.vertices,gl!.STATIC_DRAW);
   for(const texture of boundaries)gl!.deleteTexture(texture);boundaries.length=0;
   for(let i=1;i<layers;i++){
     const texture=gl!.createTexture()!;gl!.bindTexture(gl!.TEXTURE_2D,texture);gl!.texParameteri(gl!.TEXTURE_2D,gl!.TEXTURE_MIN_FILTER,gl!.LINEAR);gl!.texParameteri(gl!.TEXTURE_2D,gl!.TEXTURE_MAG_FILTER,gl!.LINEAR);gl!.texParameteri(gl!.TEXTURE_2D,gl!.TEXTURE_WRAP_S,gl!.CLAMP_TO_EDGE);gl!.texParameteri(gl!.TEXTURE_2D,gl!.TEXTURE_WRAP_T,gl!.CLAMP_TO_EDGE);
     if(mesh.sectionMask)gl!.texImage2D(gl!.TEXTURE_2D,0,gl!.RGBA,512,512,0,gl!.RGBA,gl!.UNSIGNED_BYTE,mesh.sectionMask);
     else gl!.texImage2D(gl!.TEXTURE_2D,0,gl!.RGBA,gl!.RGBA,gl!.UNSIGNED_BYTE,makeSectionMask(mesh.vertices,min+(max-min)*i/layers));
     boundaries.push(texture);
   }
 }
 const bind=(buf:WebGLBuffer)=>{gl.bindBuffer(gl.ARRAY_BUFFER,buf);gl.vertexAttribPointer(position,3,gl.FLOAT,false,24,0);gl.vertexAttribPointer(normal,3,gl.FLOAT,false,24,12);};
 function draw(state:ViewerState){if(lost)return;gl!.viewport(0,0,canvas.width,canvas.height);gl!.clearColor(0,0,0,0);gl!.clear(gl!.COLOR_BUFFER_BIT|gl!.DEPTH_BUFFER_BIT|gl!.STENCIL_BUFFER_BIT);gl!.useProgram(program);
 gl!.uniform1f(u.uColorCount,state.colorStops.length/3);
 gl!.uniform3fv(u['uFilamentColors[0]'],new Float32Array([...state.colorStops,...Array(24-state.colorStops.length).fill(0)]));
 gl!.uniform1f(u.uFinish,state.finish);gl!.uniform1f(u.uOpacity,1);gl!.depthMask(true);gl!.uniform1f(u.uSliced,state.explode>.001?1:0);gl!.uniform1f(u.uClipSide,0);gl!.uniform1f(u.uOffset,0);gl!.uniform1f(u.uCap,0);gl!.uniform1f(u.uDensity,state.density);gl!.uniform1f(u.uWall,state.wall);gl!.uniform1i(u.uBoundary,0);
 gl!.uniform1f(u.uScale,(kind==='pla'?1.38:kind==='tpu'?1.4:1.04)*(1-state.explode*.16)*state.zoom);gl!.uniform1f(u.uYaw,state.yaw);gl!.uniform1f(u.uPitch,state.pitch);gl!.uniform1f(u.uAspect,canvas.width/canvas.height);gl!.uniform1f(u.uFlex,kind==='tpu'?state.flex:0);
 gl!.uniform3f(u.uColor,state.r,state.g,state.b);gl!.uniform3f(u.uOldColor,state.oldR,state.oldG,state.oldB);gl!.uniform1f(u.uSweep,min+(max-min+8)*state.sweep);gl!.uniform1f(u.uBuild,state.build);gl!.uniform1f(u.uMin,min);gl!.uniform1f(u.uMax,max);gl!.uniform1f(u.uLayer,state.layer);gl!.uniform1f(u.uGloss,kind==='pa'?65:kind==='tpu'?25:45);
 gl!.enable(gl!.BLEND);gl!.blendFunc(gl!.SRC_ALPHA,gl!.ONE_MINUS_SRC_ALPHA);gl!.disable(gl!.DEPTH_TEST);gl!.uniform1f(u.uGround,1);bind(ground);gl!.drawArrays(gl!.TRIANGLES,0,6);
 gl!.enable(gl!.DEPTH_TEST);gl!.uniform1f(u.uGround,0);
 if(state.explode<=.001){
   bind(buffer);
   // Keep only the nearest skin when showing translucent filament.
   if(state.finish===5){gl!.colorMask(false,false,false,false);gl!.drawArrays(gl!.TRIANGLES,0,count);gl!.colorMask(true,true,true,true);gl!.depthMask(false);gl!.depthFunc(gl!.LEQUAL);}
   gl!.drawArrays(gl!.TRIANGLES,0,count);gl!.depthFunc(gl!.LESS);
 }
 else{
   let opaque=true;
   const renderCap=(height:number,side:number,texture:WebGLTexture)=>{
     gl!.stencilMask(255);gl!.clear(gl!.STENCIL_BUFFER_BIT);gl!.enable(gl!.STENCIL_TEST);gl!.stencilFunc(gl!.ALWAYS,0,255);gl!.uniform1f(u.uClipSide,side);
     gl!.stencilOpSeparate(gl!.FRONT,gl!.KEEP,gl!.KEEP,gl!.DECR_WRAP);gl!.stencilOpSeparate(gl!.BACK,gl!.KEEP,gl!.KEEP,gl!.INCR_WRAP);
     gl!.colorMask(false,false,false,false);gl!.depthMask(false);gl!.disable(gl!.DEPTH_TEST);bind(buffer);gl!.drawArrays(gl!.TRIANGLES,0,count);
     gl!.colorMask(true,true,true,true);gl!.depthMask(opaque);gl!.enable(gl!.DEPTH_TEST);gl!.stencilFunc(gl!.NOTEQUAL,0,255);gl!.stencilOp(gl!.KEEP,gl!.KEEP,gl!.KEEP);
     gl!.activeTexture(gl!.TEXTURE0);gl!.bindTexture(gl!.TEXTURE_2D,texture);bind(cap);gl!.uniform1f(u.uCut,height);gl!.uniform1f(u.uCap,1);gl!.drawArrays(gl!.TRIANGLES,0,6);gl!.uniform1f(u.uCap,0);gl!.disable(gl!.STENCIL_TEST);
   };
   for(let i=0;i<layers;i++){
     const low=min+(max-min)*i/layers,high=min+(max-min)*(i+1)/layers;
     const opacity=i===layers-1?state.upperOpacity:1;opaque=opacity>.999;gl!.uniform1f(u.uOpacity,opacity);gl!.depthMask(opaque);
     if(opacity<=.001)continue;
     gl!.uniform1f(u.uOffset,i*28*state.explode);gl!.uniform1f(u.uLow,low);gl!.uniform1f(u.uHigh,high);gl!.uniform1f(u.uClipSide,0);bind(buffer);
     // Resolve the nearest surface before blending, so translucent walls do not stack.
     if(!opaque){gl!.colorMask(false,false,false,false);gl!.depthMask(true);gl!.drawArrays(gl!.TRIANGLES,0,count);gl!.colorMask(true,true,true,true);gl!.depthMask(false);gl!.depthFunc(gl!.LEQUAL);}
     gl!.drawArrays(gl!.TRIANGLES,0,count);
     if(i<layers-1)renderCap(high,1,boundaries[i]);
     if(i>0)renderCap(low,2,boundaries[i-1]);
     gl!.depthFunc(gl!.LESS);
   }
 }
 gl!.depthMask(true);
 }
 return {setPart,draw,dispose(){canvas.removeEventListener('webglcontextlost',onLost);gl.deleteBuffer(buffer);gl.deleteBuffer(ground);gl.deleteBuffer(cap);for(const texture of boundaries)gl.deleteTexture(texture);gl.deleteProgram(program);gl.deleteShader(vs);gl.deleteShader(fs);},get triangles(){return count/3;}};
}
