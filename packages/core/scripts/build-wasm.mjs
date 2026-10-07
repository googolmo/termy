// Builds crates/wasm and writes the wasm-bindgen glue to src/wasm/.
// The wasm-bindgen CLI version must match the crate's pinned dependency.
import { execFileSync } from 'node:child_process'
import { copyFileSync, mkdirSync, statSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const repo = join(root, '..', '..')
const out = join(root, 'src', 'wasm')
const run = (cmd, args) => execFileSync(cmd, args, { cwd: repo, stdio: 'inherit' })

run('cargo', ['build', '-p', 'termy_wasm', '--target', 'wasm32-unknown-unknown', '--profile', 'wasm'])
mkdirSync(out, { recursive: true })
run('wasm-bindgen', [
  'target/wasm32-unknown-unknown/wasm/termy_wasm.wasm',
  '--target', 'web',
  '--out-dir', out,
  '--out-name', 'termy_wasm',
  '--omit-default-module-path',
])

const wasm = join(out, 'termy_wasm_bg.wasm')
const features = ['bulk-memory', 'nontrapping-float-to-int', 'sign-ext', 'mutable-globals', 'reference-types', 'multivalue']
try {
  execFileSync('wasm-opt', ['-Os', ...features.map((f) => `--enable-${f}`), wasm, '-o', wasm], { stdio: 'inherit' })
} catch (error) {
  // Release builds set TERMY_REQUIRE_WASM_OPT so an unoptimized module never ships.
  if (process.env.TERMY_REQUIRE_WASM_OPT) throw error
  console.warn('wasm-opt unavailable; shipping the unoptimized module')
}
mkdirSync(join(root, 'dist'), { recursive: true })
copyFileSync(wasm, join(root, 'dist', 'termy.wasm'))
console.log(`termy.wasm: ${(statSync(wasm).size / 1024).toFixed(0)} KiB`)
