import { fileURLToPath } from 'node:url';
import { defineConfig, loadEnv, normalizePath, resolveConfig, type ProxyOptions } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
const headers = {
  'Content-Security-Policy': "frame-ancestors 'none'; object-src 'none'; base-uri 'self'",
  'X-Frame-Options': 'DENY',
  'X-Content-Type-Options': 'nosniff',
};
export default defineConfig(async ({ mode }) => {
  const defaults = await resolveConfig({ configFile: false, envFile: false }, 'serve', mode);
  const env = loadEnv(mode, '.', 'RUST_API_PORT');
  const apiPort = Number(env.RUST_API_PORT ?? '18432');
  if (!Number.isInteger(apiPort) || apiPort < 1 || apiPort > 65535)
    throw new Error('Invalid RUST_API_PORT');
  const proxy: Record<string, ProxyOptions> = {
    '/api': {
      target: `http://127.0.0.1:${apiPort}`,
      changeOrigin: false,
      configure(proxy) {
        // EPIPE alone does not prove a size rejection. Only infer 413 above
        // both current upload profiles (32 MiB standard, 96 MiB large-model).
        proxy.on('error', (error, request, response) => {
          if (
            (error as NodeJS.ErrnoException).code !== 'EPIPE' ||
            request.method !== 'POST' ||
            request.url?.split('?')[0] !== '/api/models' ||
            Number(request.headers['content-length']) <= 96 * 1024 * 1024 ||
            !Number.isFinite(Number(request.headers['content-length'])) ||
            !('writeHead' in response) ||
            response.headersSent ||
            response.writableEnded ||
            response.destroyed
          )
            return;
          response.writeHead(413, { 'Content-Type': 'application/json' }).end(
            JSON.stringify({
              code: 'model_file_too_large',
              message: 'Plik modelu może mieć maksymalnie 96 MiB.',
            }),
          );
        });
      },
    },
  };
  return {
    plugins: [tailwindcss(), svelte()],
    server: {
      proxy,
      headers,
      fs: {
        deny: [
          ...defaults.server.fs.deny,
          '**/tests/artifacts/**',
          normalizePath(fileURLToPath(new URL('./.codex/**', import.meta.url))),
        ],
      },
    },
    preview: { proxy, headers },
    build: {
      rollupOptions: {
        input: {
          main: 'index.html',
          admin: 'admin.html',
          adminDesign: 'admin-design.html',
          checkoutDesign: 'checkout-design.html',
        },
      },
    },
  };
});
