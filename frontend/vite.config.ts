import { defineConfig, loadEnv } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', 'RUST_API_PORT');
  const apiPort = Number(env.RUST_API_PORT ?? '18432');
  if (!Number.isInteger(apiPort) || apiPort < 1 || apiPort > 65535)
    throw new Error('Invalid RUST_API_PORT');
  const proxy = { '/api': { target: `http://127.0.0.1:${apiPort}`, changeOrigin: false } };
  return {
    plugins: [tailwindcss(), svelte()],
    server: { proxy },
    preview: { proxy },
    build: { rollupOptions: { input: { main: 'index.html', admin: 'admin.html' } } },
  };
});
