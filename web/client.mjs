import {LatestQueue} from './scheduler.mjs';
/** Framework-free worker client. Opening transfers ownership of the input ArrayBuffer. */
export class SkaldDocument {
  constructor({timeoutMs=15000}={}){this.timeoutMs=timeoutMs;this.next=0;this.pending=new Map();this.spawn();this.queue=new LatestQueue(value=>this.request('render',value));}
  spawn(){this.worker=new Worker(new URL('./worker.mjs',import.meta.url),{type:'module'});
    this.worker.onmessage=({data})=>{const p=this.pending.get(data.id);if(!p)return;clearTimeout(p.timer);this.pending.delete(data.id);if(data.error)p.reject(new Error(data.error));else p.resolve(data.result);};
    this.worker.onerror=()=>this.fail(new Error('Rendering worker failed'));}
  fail(error){this.worker.terminate();for(const p of this.pending.values()){clearTimeout(p.timer);p.reject(error);}this.pending.clear();this.closed=true;this.queue?.close();}
  request(type,payload,transfer=[]){if(this.closed)return Promise.reject(new Error('Document closed'));const id=++this.next;return new Promise((resolve,reject)=>{const timer=setTimeout(()=>this.fail(new Error('Rendering deadline exceeded; worker terminated')),this.timeoutMs);this.pending.set(id,{resolve,reject,timer});this.worker.postMessage({id,type,...payload},transfer);});}
  open(buffer){return this.request('open',{buffer},[buffer]);}
  render(page,scale){return this.queue.submit({page,scale});}
  close(){this.fail(new DOMException('Document closed','AbortError'));}
}
