<script lang="ts">
  import type { Order, Role } from '../lib/types';
  import { sendMessage, markMessagesRead } from '../lib/api';
  import { date } from '../lib/format';
  import Button from './ui/Button.svelte';
  let { order, role, onchange }: { order: Order; role: Role; onchange: (value: Order) => void } =
    $props();
  let text = $state('');
  let busy = $state(false);
  let error = $state('');
  let unread = $derived(
    order.messages.filter((message) => message.sender !== role && !message.read).length,
  );
  async function send(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = '';
    try {
      onchange(await sendMessage(order.id, text));
      text = '';
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
  async function read() {
    error = '';
    try {
      onchange(await markMessagesRead(order.id));
    } catch (e) {
      error = (e as Error).message;
    }
  }
</script>

<section id="rozmowa" class="panel stack" aria-labelledby="chat-title">
  <div class="toolbar">
    <h2 id="chat-title">Rozmowa o zamówieniu</h2>
    {#if unread}<Button variant="ghost" onclick={read}>Oznacz {unread} jako przeczytane</Button
      >{/if}
  </div>

  <div class="messages" aria-live="polite">
    {#each order.messages as message (message.id)}<article class:mine={message.sender === role}>
        <small
          >{message.sender === 'admin' ? 'Rapidmaker' : 'Klient'} · {date(
            message.createdAt,
          )}{!message.read && message.sender !== role ? ' · Nowa' : ''}</small
        >
        <p>{message.text}</p>
      </article>{:else}<p class="empty-state">
        Tu ustalisz szczegóły z {role === 'admin' ? 'klientem' : 'obsługą'}.
      </p>{/each}
  </div>
  <form onsubmit={send} class="stack">
    <label for="message">Wiadomość</label><textarea
      id="message"
      bind:value={text}
      rows="3"
      maxlength="4000"
      placeholder="Napisz, w czym możemy pomóc…"
      required></textarea>{#if error}<p role="alert" class="alert error">{error}</p>{/if}
    <div>
      <Button type="submit" disabled={busy || !text.trim()}
        >{busy ? 'Wysyłanie…' : 'Wyślij wiadomość'}</Button
      >
    </div>
  </form>
</section>

<style>
  .messages {
    display: grid;
    gap: 12px;
    max-height: 380px;
    overflow-y: auto;
  }
  article {
    padding: 16px;
    background: var(--color-control);
    border-radius: var(--radius-md);
    max-width: 88%;
    border: 0;
  }
  article.mine {
    justify-self: end;
    background: var(--glow);
  }
  article p {
    margin: 6px 0 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  small {
    color: var(--color-muted);
  }
</style>
