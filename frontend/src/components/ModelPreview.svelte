<script lang="ts">
  import { onMount } from 'svelte';
  import { apiFetch } from '../lib/http';
  import { createPartViewer, type ViewerState } from '../storefront/lib/partViewer';
  import type { StlMesh } from '../storefront/lib/stlParser';
  let {
    url,
    name,
    openInitially = false,
  }: { url: string; name: string; openInitially?: boolean } = $props();
  let expanded = $state(false);
  onMount(() => {
    expanded = openInitially;
  });
  let canvas = $state<HTMLCanvasElement>();
  let error = $state('');
  let loading = $state(true);
  let turn = (_angle: number) => {};
  $effect(() => {
    const element = canvas,
      source = url;
    if (!element) return;
    let active = true;
    const abort = new AbortController();
    const worker = new Worker(new URL('../storefront/lib/stl.worker.ts', import.meta.url), {
      type: 'module',
    });
    let renderer: ReturnType<typeof createPartViewer> | undefined;
    let observer: ResizeObserver | undefined;
    const timer = setTimeout(() => {
      abort.abort();
      worker.terminate();
      if (active) {
        error = 'Nie udało się wczytać podglądu. Odśwież stronę.';
        loading = false;
      }
    }, 30_000);
    loading = true;
    error = '';
    const fail = () => {
      if (active) {
        loading = false;
        error = 'Podgląd jest niedostępny. Oryginał możesz pobrać poniżej.';
        clearTimeout(timer);
      }
    };
    worker.onmessage = (event: MessageEvent<{ ok: boolean; mesh?: StlMesh }>) => {
      if (!active) return;
      clearTimeout(timer);
      if (!event.data.ok || !event.data.mesh) return fail();
      try {
        renderer = createPartViewer(element);
        if (!renderer) return fail();
        renderer.setPart('order', event.data.mesh);
        const state: ViewerState = {
          yaw: -0.45,
          pitch: 0.62,
          build: 1,
          flex: 0,
          layer: 0.13,
          sweep: 1,
          r: 0.94,
          g: 0.94,
          b: 0.94,
          oldR: 0.94,
          oldG: 0.94,
          oldB: 0.94,
          explode: 0,
          zoom: 1,
          upperOpacity: 1,
          density: 0.065,
          wall: 4,
        };
        const draw = () => renderer?.draw(state);
        turn = (angle) => {
          state.yaw += angle;
          draw();
        };
        observer = new ResizeObserver(() => {
          const rect = element.getBoundingClientRect(),
            ratio = Math.min(devicePixelRatio, 2);
          element.width = Math.max(1, Math.round(rect.width * ratio));
          element.height = Math.max(1, Math.round(rect.height * ratio));
          draw();
        });
        observer.observe(element);
        loading = false;
      } catch {
        fail();
      }
      worker.terminate();
    };
    worker.onerror = fail;
    void (async () => {
      try {
        if (
          !/^\/api\/(?:models\/[^/]+|(?:account|admin)\/orders\/[^/]+\/files\/[^/]+)\/preview\.glb$/.test(
            source,
          )
        )
          throw new Error('url');
        const response = await apiFetch(source, { signal: abort.signal });
        if (!response.ok || !response.body) throw new Error('response');
        const reader = response.body.getReader(),
          chunks: Uint8Array[] = [];
        let length = 0;
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          length += value.byteLength;
          if (length > 25 * 1024 * 1024) {
            await reader.cancel();
            throw new Error('size');
          }
          chunks.push(value);
        }
        if (!active) return;
        const buffer = new Uint8Array(length);
        let offset = 0;
        for (const chunk of chunks) {
          buffer.set(chunk, offset);
          offset += chunk.byteLength;
        }
        worker.postMessage({ buffer: buffer.buffer, format: 'glb' }, [buffer.buffer]);
      } catch {
        fail();
      }
    })();
    return () => {
      active = false;
      abort.abort();
      worker.terminate();
      observer?.disconnect();
      renderer?.dispose();
      clearTimeout(timer);
      turn = () => {};
    };
  });
</script>

<button
  class="preview-toggle"
  type="button"
  aria-expanded={expanded}
  onclick={() => (expanded = !expanded)}
>
  {expanded ? 'Ukryj podgląd' : 'Pokaż podgląd 3D'} · {name}
</button>
{#if expanded}
  <div class="model-preview" aria-busy={loading}>
    <canvas bind:this={canvas} aria-label={`Podgląd modelu ${name}`}></canvas>
    {#if loading}<p role="status">Wczytywanie podglądu…</p>{:else if error}<p role="status">
        {error}
      </p>{:else}<div class="preview-tools">
        <span>{name}</span><button
          type="button"
          aria-label="Obróć model w lewo"
          onclick={() => turn(-0.4)}>←</button
        ><button type="button" aria-label="Obróć model w prawo" onclick={() => turn(0.4)}>→</button>
      </div>{/if}
  </div>
{/if}

<style>
  .preview-toggle {
    padding: var(--space) calc(var(--space) * 2);
    margin-block: var(--space);
    text-align: left;
  }
  .model-preview {
    position: relative;
    overflow: hidden;
    border-radius: var(--card-radius);
    background: var(--surface);
  }
  canvas {
    display: block;
    width: 100%;
    height: 240px;
  }
  p {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    margin: 0;
    padding: calc(var(--space) * 2);
    text-align: center;
  }
  .preview-tools {
    display: flex;
    gap: var(--space);
    align-items: center;
    padding: var(--space) calc(var(--space) * 2);
  }
  .preview-tools span {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button {
    min-width: 36px;
    min-height: 36px;
    border: 1px solid var(--line);
    border-radius: var(--control-radius);
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
</style>
