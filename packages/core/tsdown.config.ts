import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['esm'],
  platform: 'neutral',
  dts: true,
  // build-wasm.mjs writes dist/termy.wasm before this runs.
  clean: false,
})
