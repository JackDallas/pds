<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import '../app.css';
  import { auth } from '$lib/auth.svelte';
  import { onMount } from 'svelte';

  let { children } = $props();

  const navItems = [
    { href: '/admin/', label: 'Dashboard', icon: '📊' },
    { href: '/admin/accounts', label: 'Accounts', icon: '👥' },
    { href: '/admin/invites', label: 'Invites', icon: '🎫' },
    { href: '/admin/settings', label: 'Settings', icon: '⚙️' }
  ];

  function handleLogout() {
    auth.logout();
    goto('/admin/');
  }

  onMount(() => {
    // Redirect to login if not authenticated and not on login page
    if (!auth.isAuthenticated && $page.url.pathname !== '/admin/') {
      goto('/admin/');
    }
  });
</script>

<div class="min-h-screen bg-gray-950">
  {#if auth.isAuthenticated}
    <div class="flex h-screen">
      <!-- Sidebar -->
      <aside class="w-64 bg-gray-900 border-r border-gray-800">
        <div class="p-6">
          <h1 class="text-2xl font-bold text-blue-400">PDS Admin</h1>
          <p class="text-sm text-gray-400 mt-1">{auth.handle}</p>
        </div>

        <nav class="px-4 space-y-1">
          {#each navItems as item}
            <a
              href={item.href}
              class="flex items-center gap-3 px-4 py-3 rounded-lg transition-colors {$page.url.pathname === item.href ? 'bg-blue-600 text-white' : 'text-gray-300 hover:bg-gray-800'}"
            >
              <span class="text-xl">{item.icon}</span>
              <span>{item.label}</span>
            </a>
          {/each}
        </nav>

        <div class="absolute bottom-0 left-0 right-0 w-64 p-4 border-t border-gray-800">
          <button
            onclick={handleLogout}
            class="w-full px-4 py-2 text-sm text-gray-300 hover:text-white hover:bg-gray-800 rounded-lg transition-colors"
          >
            Logout
          </button>
        </div>
      </aside>

      <!-- Main content -->
      <main class="flex-1 overflow-auto">
        <div class="p-8">
          {@render children()}
        </div>
      </main>
    </div>
  {:else}
    <main class="flex items-center justify-center min-h-screen">
      {@render children()}
    </main>
  {/if}
</div>
