<script lang="ts">
  import Icon from './Icon.svelte';
  import type { Session } from '../lib/types';
  let { session, onLogout }: { session: Session | null; onLogout: () => void } = $props();
</script>

<header class="site-header app-header">
  <a class="brand" href="/" aria-label="Rapidmaker — strona główna">
    <img src="/logo.svg" alt="" width="30" height="40" /><span>rapidmaker</span>
  </a>
  <nav aria-label="Nawigacja główna">
    <a href="/wycena?restore=cart">Wycena online</a>
    <a href="/#kontakt">Kontakt</a>
  </nav>
  <div class="account-actions">
    <a
      class="account-link"
      aria-label={session?.role === 'admin'
        ? 'Panel admina'
        : session
          ? 'Moje konto'
          : 'Zaloguj się'}
      href={session?.role === 'admin'
        ? session.adminVerified
          ? '/admin.html'
          : '/logowanie?operator=1'
        : session
          ? '/konto/zamowienia'
          : '/logowanie'}
    >
      <Icon name="user" size={17} /><span class="account-label"
        >{session?.role === 'admin' ? 'Panel admina' : session ? 'Moje konto' : 'Zaloguj się'}</span
      >
    </a>
    {#if session}<button class="nav-link" onclick={onLogout}>Wyloguj</button>{/if}
  </div>
</header>

<style>
  .brand {
    font-family: var(--font-display);
    letter-spacing: -0.035em;
  }
  .brand img {
    display: block;
    width: 30px;
    height: 40px;
    flex-shrink: 0;
  }
  @media (max-width: 640px) {
    .brand img {
      width: 23px;
      height: 32px;
    }
  }
</style>
