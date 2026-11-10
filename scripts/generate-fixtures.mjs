import {spawnSync} from 'node:child_process';import {readdirSync} from 'node:fs';import {resolve,delimiter} from 'node:path';
const skald=resolve(process.argv[2]??'../skald-pdf');
const modules=['skald-core','skald-fonts','skald-layout','skald-image','skald-brotli'];
const cp=modules.flatMap(m=>readdirSync(`${skald}/${m}/build/libs`).filter(f=>f.endsWith('.jar')&&!f.includes('sources')&&!f.includes('javadoc')).map(f=>`${skald}/${m}/build/libs/${f}`)).join(delimiter);
const result=spawnSync('java',['--enable-native-access=ALL-UNNAMED','--class-path',cp,'examples/GenerateFixtures.java','examples/pdfs'],{stdio:'inherit'});process.exit(result.status??1);
