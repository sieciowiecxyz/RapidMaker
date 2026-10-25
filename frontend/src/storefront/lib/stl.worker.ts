import { drawSectionMask } from './sectionMask';
import { parseStl } from './stlParser';
import { parsePreviewGlb } from './glbParser';
self.onmessage=async(event:MessageEvent<{file?:File;buffer?:ArrayBuffer;format?:'glb'}>)=>{
  try{
    const mesh=event.data.format === 'glb' && event.data.buffer ? parsePreviewGlb(event.data.buffer) : parseStl(await event.data.file!.arrayBuffer());
    if(typeof OffscreenCanvas!=='undefined'){
      const canvas=new OffscreenCanvas(512,512),ctx=canvas.getContext('2d');
      if(ctx){drawSectionMask(mesh.vertices,(mesh.minY+mesh.maxY)/2,ctx);mesh.sectionMask=new Uint8Array(ctx.getImageData(0,0,512,512).data.buffer);}
    }
    const transfer:Transferable[]=[mesh.vertices.buffer];
    if(mesh.sectionMask)transfer.push(mesh.sectionMask.buffer);
    self.postMessage({ok:true,mesh},{transfer});
  }catch(error){self.postMessage({ok:false,error:error instanceof Error?error.message:'Nie udało się przygotować podglądu modelu.'});}
};
