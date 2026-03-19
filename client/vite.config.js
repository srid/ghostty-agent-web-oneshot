import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
  },
  optimizeDeps: {
    exclude: ['ghostty-web'],
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:7681',
        changeOrigin: true,
      },
      '/ws': {
        target: 'ws://localhost:7681',
        ws: true,
      },
    },
  },
});
