import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  base: '/assets/graph-app/',
  build: {
    outDir: '../src/assets/graph-app',
    emptyOutDir: true,
  },
});
