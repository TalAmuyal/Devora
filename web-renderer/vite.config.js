import { defineConfig } from 'vite';
export default defineConfig({ base: './', build: { chunkSizeWarningLimit: Infinity, manifest: true, emptyOutDir: true } });
