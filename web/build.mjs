import {build} from 'esbuild';
import {readFile, writeFile, readdir, mkdir} from 'node:fs/promises';
const root = new URL('../', import.meta.url);
await mkdir(new URL('assets/vendor/',root),{recursive:true});
await Promise.all(['playground','math','math-latex'].map(name => build({entryPoints:[new URL(`${name}.js`,import.meta.url).pathname],bundle:true,minify:true,format:name === 'math-latex' ? 'esm' : 'iife',target:'es2022',outfile:new URL(`assets/vendor/${name}.js`,root).pathname,legalComments:'eof'})));
const licenses = [];
async function scan(directory) {
  const entries = await readdir(directory,{withFileTypes:true});
  entries.sort((a,b)=>a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
  for (const entry of entries) {
    if (!entry.isDirectory() || entry.name.startsWith('.') || entry.name === 'esbuild' || entry.name === '@esbuild') continue;
    const path = new URL(entry.name+'/',directory);
    if (entry.name.startsWith('@')) {await scan(path);continue;}
    const pkg = JSON.parse(await readFile(new URL('package.json',path),'utf8'));
    for (const name of ['LICENSE','LICENSE.txt','LICENSE.md']) {
      try { licenses.push(`${pkg.name} ${pkg.version}\n${await readFile(new URL(name,path),'utf8')}`);break; } catch (error) {if(error.code!=='ENOENT')throw error;}
    }
  }
}
await scan(new URL('node_modules/',import.meta.url));
await writeFile(new URL('assets/vendor/licenses.txt',root),licenses.join('\n\n────────\n\n'));
