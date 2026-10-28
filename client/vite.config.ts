import { defineConfig } from 'vite';

export default defineConfig({
  server: {proxy: {'/api': {target: process.env.NOMINATION_API_TARGET || 'http://127.0.0.1:8797', changeOrigin: false}}},
});
