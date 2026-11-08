import assert from 'node:assert/strict';
import http from 'node:http';
import net from 'node:net';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';
import { createServer, loadConfigFromFile, normalizePath, resolveConfig } from 'vite';

const root = fileURLToPath(new URL('../', import.meta.url));
const mib = 1024 * 1024;

// Exercise the actual Vite proxy, including its default error response. No API,
// database, normal runtime ports, or large in-memory upload fixture is needed.
test(
  'upload proxy distinguishes EPIPE from a known size rejection',
  { timeout: 15000 },
  async (t) => {
    let upstreamResponse = false;
    const body = JSON.stringify({
      code: 'model_file_too_large',
      message: 'Upstream limit: 32 MiB.',
    });
    const upstream = net.createServer((socket) => {
      if (!upstreamResponse) {
        // Close immediately while the proxy is writing the incoming upload.
        socket.destroy();
        return;
      }
      socket.once('data', () => {
        socket.end(
          `HTTP/1.1 413 Payload Too Large\r\nContent-Type: application/json\r\nContent-Length: ${Buffer.byteLength(body)}\r\nConnection: close\r\n\r\n${body}`,
        );
      });
    });
    await new Promise((resolve) => upstream.listen(0, '127.0.0.1', resolve));
    t.after(() => new Promise((resolve) => upstream.close(resolve)));

    const loaded = await loadConfigFromFile(
      { command: 'serve', mode: 'test' },
      `${root}vite.config.ts`,
      root,
    );
    const config = loaded.config;
    const options = config.server.proxy['/api'];
    options.target = `http://127.0.0.1:${upstream.address().port}`;
    const configure = options.configure;
    const errors = [];
    options.configure = (proxy, ...args) => {
      configure(proxy, ...args);
      proxy.on('error', (error) => errors.push(error.code));
    };
    const vite = await createServer({
      ...config,
      root,
      configFile: false,
      logLevel: 'silent',
      server: { ...config.server, host: '127.0.0.1', port: 0, strictPort: true, watch: null },
    });
    t.after(() => vite.close());
    await vite.listen();
    const port = vite.httpServer.address().port;

    await t.test('private artifacts stay blocked without replacing Vite defaults', async () => {
      const defaults = await resolveConfig({ configFile: false, envFile: false }, 'serve');
      assert.deepEqual(vite.config.server.fs.deny, [
        ...defaults.server.fs.deny,
        '**/tests/artifacts/**',
        normalizePath(fileURLToPath(new URL('../.codex/**', import.meta.url))),
      ]);
      await mkdir(`${root}tests/artifacts`, { recursive: true });
      const directory = await mkdtemp(`${root}tests/artifacts/security-canary-`);
      t.after(() => rm(directory, { recursive: true, force: true }));
      const canary = 'harmless-private-artifact-regression-canary';
      const file = `${directory}/canary.txt`;
      await writeFile(file, canary, { mode: 0o600 });
      const direct = file.slice(root.length - 1);
      for (const url of [direct, `/@fs/${file}`]) {
        for (const suffix of ['', '?raw&import']) {
          const response = await fetch(`http://127.0.0.1:${port}${url}${suffix}`);
          assert.equal(response.status, 403, `${url}${suffix}`);
          assert.ok(!(await response.text()).includes(canary));
        }
      }
      for (const url of ['/', '/src/main.ts']) {
        const response = await fetch(`http://127.0.0.1:${port}${url}`);
        assert.equal(response.status, 200, url);
        assert.equal(response.headers.get('x-frame-options'), 'DENY');
        await response.text();
      }
    });

    async function upload(size) {
      return new Promise((resolve, reject) => {
        const request = http.request(
          {
            host: '127.0.0.1',
            port,
            path: '/api/models',
            method: 'POST',
            headers: { 'Content-Length': size },
          },
          (response) => {
            let result = '';
            response.setEncoding('utf8');
            response.on('data', (chunk) => {
              result += chunk;
            });
            response.on('end', () => {
              request.destroy();
              resolve({ status: response.statusCode, body: result });
            });
          },
        );
        request.on('error', reject);
        request.setTimeout(5000, () => request.destroy(new Error('Proxy upload timed out')));
        const chunk = Buffer.alloc(256 * 1024);
        let remaining = size;
        function write() {
          while (remaining && !request.destroyed) {
            const part = chunk.subarray(0, Math.min(chunk.length, remaining));
            remaining -= part.length;
            if (!request.write(part)) {
              request.once('drain', write);
              return;
            }
          }
          if (!remaining) request.end();
        }
        write();
      });
    }

    const medium = await upload(48 * mib);
    assert.equal(errors.pop(), 'EPIPE', 'fake upstream must cause a real proxy EPIPE');
    assert.equal(medium.status, 500, '48 MiB EPIPE retains the default Vite proxy error');
    assert.doesNotMatch(medium.body, /model_file_too_large|32 MiB/);

    const oversized = await upload(97 * mib);
    assert.equal(errors.pop(), 'EPIPE');
    assert.equal(oversized.status, 413);
    assert.deepEqual(JSON.parse(oversized.body), {
      code: 'model_file_too_large',
      message: 'Plik modelu może mieć maksymalnie 96 MiB.',
    });

    upstreamResponse = true;
    const rejected = await upload(48 * mib);
    assert.equal(rejected.status, 413);
    assert.equal(rejected.body, body, 'an actual upstream 413 must be forwarded unchanged');
  },
);
