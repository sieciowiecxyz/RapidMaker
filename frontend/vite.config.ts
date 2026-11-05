import { defineConfig, loadEnv, type ProxyOptions } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', 'RUST_API_PORT');
  const apiPort = Number(env.RUST_API_PORT ?? '18432');
  if (!Number.isInteger(apiPort) || apiPort < 1 || apiPort > 65535)
    throw new Error('Invalid RUST_API_PORT');
  const proxy: Record<string, ProxyOptions> = {
    '/api': {
      target: `http://127.0.0.1:${apiPort}`,
      changeOrigin: false,
      configure(proxy) {
        // Axum closes oversized uploads; preserve the 32 MiB contract if that
        // produces EPIPE before Vite has forwarded the upstream 413 response.
        proxy.on('error', (error, request, response) => {
          if (
            (error as NodeJS.ErrnoException).code !== 'EPIPE' ||
            request.method !== 'POST' ||
            request.url?.split('?')[0] !== '/api/models' ||
            Number(request.headers['content-length']) <= 32 * 1024 * 1024 ||
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
              message: 'Plik modelu może mieć maksymalnie 32 MiB.',
            }),
          );
        });
      },
    },
  };
  return {
    plugins: [tailwindcss(), svelte()],
    server: { proxy },
    preview: { proxy },
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
