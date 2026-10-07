import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vitest/config'

const src = (pkg: string): string => fileURLToPath(new URL(`./${pkg}/src/index.ts`, import.meta.url))

export default defineConfig({
  resolve: {
    alias: { '@termysh/core': src('core'), '@termysh/web': src('web') },
  },
  test: {
    include: ['*/test/**/*.test.ts'],
  },
})
