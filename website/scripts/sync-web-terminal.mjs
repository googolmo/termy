// Copies the built @termysh/core and @termysh/web bundles into
// public/web-terminal/ for the /demo page. The site's Docker and Vercel builds
// only see website/, so the bundle is vendored rather than linked.
//
//   cd ../packages && bun run build && cd ../website && bun scripts/sync-web-terminal.mjs
//
// Once @termysh/web is on npm, the demo can depend on it directly instead.
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const website = join(dirname(fileURLToPath(import.meta.url)), '..');
const packages = join(website, '..', 'packages');
const out = join(website, 'public', 'web-terminal');
mkdirSync(out, { recursive: true });

copyFileSync(join(packages, 'core', 'dist', 'index.js'), join(out, 'core.js'));
copyFileSync(join(packages, 'core', 'dist', 'termy.wasm'), join(out, 'termy.wasm'));
const web = readFileSync(join(packages, 'web', 'dist', 'index.js'), 'utf8');
writeFileSync(join(out, 'web.js'), web.replaceAll('from "@termysh/core"', 'from "./core.js"'));

const version = JSON.parse(readFileSync(join(packages, 'web', 'package.json'), 'utf8')).version;
writeFileSync(join(out, 'VERSION'), `${version}\n`);
console.log(`synced @termysh/web ${version} to public/web-terminal/`);
