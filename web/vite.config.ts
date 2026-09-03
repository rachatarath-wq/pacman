import { defineConfig } from 'vite';

// The Rust → WASM package lives in ./pkg and is imported as an ES module.
export default defineConfig({
  // Relative asset paths so the build works when hosted under any subpath
  // (e.g. GitHub Pages project site at /pacman/).
  base: './',
  server: {
    port: 5173,
    open: true,
  },
  build: {
    target: 'esnext',
  },
  // Don't let Vite try to pre-bundle the wasm glue module.
  optimizeDeps: {
    exclude: ['./pkg/pacman.js'],
  },
});
