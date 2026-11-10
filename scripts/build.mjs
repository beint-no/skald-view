import { spawnSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
function run(command,args){const r=spawnSync(command,args,{stdio:'inherit'});if(r.status!==0)process.exit(r.status??1);}
run('cargo',['build','--locked','--release','--target','wasm32-unknown-unknown','--lib']);
mkdirSync('web/pkg',{recursive:true});
run('wasm-bindgen',['target/wasm32-unknown-unknown/release/skald_view.wasm','--out-dir','web/pkg','--target','web','--no-typescript']);

run(process.execPath,["scripts/licenses.mjs"]);
