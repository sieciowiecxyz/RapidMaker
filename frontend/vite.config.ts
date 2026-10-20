import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
const proxy = { '/api': 'http://127.0.0.1:18432' };
export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  server: { proxy },
  preview: { proxy },
});
