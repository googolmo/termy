import initWasm, { type InitInput, initSync as initWasmSync } from './wasm/termy_wasm.js'

/** Anything `WebAssembly` can instantiate the Termy module from. */
export type WasmSource =
  | string
  | URL
  | Request
  | Response
  | BufferSource
  | WebAssembly.Module
  | Promise<Response | BufferSource | WebAssembly.Module>

let ready: Promise<void> | undefined
let initialized = false

/**
 * Load the Termy WebAssembly module. Safe to call repeatedly; later calls
 * share the first load. Without `source`, `termy.wasm` is resolved next to
 * this module, which bundlers (Vite, webpack 5, Rspack, esbuild) and Node
 * understand.
 */
export function init(source?: WasmSource): Promise<void> {
  ready ??= load(source).then(
    () => {
      initialized = true
    },
    (error: unknown) => {
      ready = undefined
      throw error
    },
  )
  return ready
}

/** Synchronous init from bytes or a compiled module, e.g. in a worker. */
export function initSync(source: BufferSource | WebAssembly.Module): void {
  if (initialized) return
  initWasmSync({ module: source })
  initialized = true
  ready = Promise.resolve()
}

export function isInitialized(): boolean {
  return initialized
}

/** Throws a helpful error when the module is used before `init()`. */
export function assertInitialized(): void {
  if (!initialized) {
    throw new Error('@termysh/core: call `await init()` before creating a terminal')
  }
}

async function load(source: WasmSource | undefined): Promise<void> {
  const resolved = source ?? new URL('./termy.wasm', import.meta.url)
  if (resolved instanceof URL && resolved.protocol === 'file:') {
    // Node and Bun cannot fetch file: URLs.
    const { readFile } = await import(/* @vite-ignore */ 'node:fs/promises')
    await initWasm({ module_or_path: await readFile(resolved) })
    return
  }
  await initWasm({ module_or_path: resolved as InitInput | Promise<InitInput> })
}
