<script lang="ts">
  import Icon from './Icon.svelte';
  import type { Session } from '../lib/types';
  let { session, onLogout }: { session: Session | null; onLogout: () => void } = $props();
</script>

<header class="site-header app-header">
  <a class="brand" href="/" aria-label="Rapidmaker — strona główna">
    <span class="brand-mark"><span></span><span></span><span></span></span>rapidmaker<span
      class="brand-dot">.</span
    >
  </a>
  <nav aria-label="Nawigacja główna">
    <a href="/wycena">Wycena online</a>
    <a href="/#jak-to-dziala">Jak to działa</a>
  </nav>
  <div class="account-actions">
    <a
      class="account-link"
      aria-label={session ? 'Moje konto' : 'Zaloguj się'}
      href={session?.role === 'admin'
        ? '/admin/zamowienia'
        : session
          ? '/konto/zamowienia'
          : '/logowanie'}
    >
      <Icon name="user" size={17} /><span class="account-label"
        >{session ? 'Moje konto' : 'Zaloguj się'}</span
      >
    </a>
    {#if session}<button class="nav-link" onclick={onLogout}>Wyloguj</button>{/if}
  </div>
</header>
