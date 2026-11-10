import {SkaldDocument} from './client.mjs';
import * as pdfjs from '/node_modules/pdfjs-dist/build/pdf.mjs';
pdfjs.GlobalWorkerOptions.workerSrc='/node_modules/pdfjs-dist/build/pdf.worker.mjs';
const fixtures=['invoice-flate','invoice-brotli','unicode-brotli','vectors-brotli','raster-flate','jpeg-flate','report-brotli'];
const paint=frame=>{const canvas=document.createElement('canvas');canvas.width=frame.width;canvas.height=frame.height;canvas.getContext('2d',{alpha:false}).putImageData(new ImageData(new Uint8ClampedArray(frame.rgba),frame.width,frame.height),0,0);return canvas;};
const makeCanvas=(width,height)=>{const canvas=document.createElement('canvas');canvas.width=width;canvas.height=height;return canvas;};
window.runBenchmarks=async(rounds=9)=>{const records=[];for(const fixture of fixtures){const input=await fetch(`/examples/pdfs/${fixture}.pdf`).then(r=>r.arrayBuffer());let checksum;
  for(let iteration=-2;iteration<rounds;iteration++){for(const engine of (iteration%2===0?['skald','pdfjs-display','pdfjs-print']:['pdfjs-print','pdfjs-display','skald'])){
    const start=performance.now();let render,warm,canvas,close,pixelMae=null;
    if(engine==='skald'){const doc=new SkaldDocument();await doc.open(input.slice(0));render=async()=>paint(await doc.render(0,1));close=()=>doc.close();}
    else{const task=pdfjs.getDocument({data:new Uint8Array(input.slice(0)),wasmUrl:'/node_modules/pdfjs-dist/wasm/'});const doc=await task.promise;const page=await doc.getPage(1);const viewport=page.getViewport({scale:1});render=async()=>{const c=makeCanvas(Math.ceil(viewport.width),Math.ceil(viewport.height));await page.render({canvasContext:c.getContext('2d',{alpha:false}),viewport,background:'#ffffff',intent:engine.endsWith('print')?'print':'display'}).promise;return c;};close=()=>task.destroy();}
    try{canvas=await render();const first=performance.now()-start;const pixels=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data;let ink=0;for(let i=0;i<pixels.length;i+=4)if(pixels[i]<240)ink++;if(ink===0)throw Error(`${engine} produced blank ${fixture}`);
      if(engine==='skald'){checksum=Array.from(pixels);}else if(checksum){let mae=0;for(let i=0;i<pixels.length;i++)mae+=Math.abs(pixels[i]-checksum[i]);mae/=pixels.length;pixelMae=mae;}
      const warmStart=performance.now();await render();warm=performance.now()-warmStart;
      if(iteration>=0)records.push({fixture,engine,iteration,sourceBytes:input.byteLength,firstPageMs:first,warmPageMs:warm,width:canvas.width,height:canvas.height,pixelMae});
    }finally{await close();}
  }}
}return {userAgent:navigator.userAgent,hardwareConcurrency:navigator.hardwareConcurrency,scale:1,rounds,records};};
window.benchmarkReady=true;
