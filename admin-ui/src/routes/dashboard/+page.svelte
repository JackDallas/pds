<script lang="ts">
  import { api } from '$lib/api';
  import { onMount } from 'svelte';

  let stats = $state<any>(null);
  let loading = $state(true);
  let error = $state('');

  async function loadStats() {
    loading = true;
    error = '';
    const response = await api.getStats();
    
    if (response.success) {
      stats = response.data;
    } else {
      error = response.error || 'Failed to load stats';
    }
    loading = false;
  }

  onMount(() => {
    loadStats();
  });
</script>

<div>
  <h1 class="text-3xl font-bold mb-8">Dashboard</h1>

  {#if error}
    <div class="mb-6 p-4 bg-red-900/50 border border-red-700 rounded-lg text-red-200">
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="text-gray-400">Loading stats...</div>
  {:else if stats}
    <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
      <!-- Accounts Card -->
      <div class="bg-gray-900 border border-gray-800 rounded-lg p-6">
        <div class="flex items-center justify-between mb-2">
          <h2 class="text-lg font-medium text-gray-300">Total Accounts</h2>
          <span class="text-2xl">👥</span>
        </div>
        <p class="text-3xl font-bold text-blue-400">{stats.accountCount}</p>
      </div>

      <!-- Repos Card -->
      <div class="bg-gray-900 border border-gray-800 rounded-lg p-6">
        <div class="flex items-center justify-between mb-2">
          <h2 class="text-lg font-medium text-gray-300">Total Repos</h2>
          <span class="text-2xl">📦</span>
        </div>
        <p class="text-3xl font-bold text-green-400">{stats.repoCount}</p>
      </div>

      <!-- Invites Card -->
      <div class="bg-gray-900 border border-gray-800 rounded-lg p-6">
        <div class="flex items-center justify-between mb-2">
          <h2 class="text-lg font-medium text-gray-300">Invite Codes</h2>
          <span class="text-2xl">🎫</span>
        </div>
        <p class="text-3xl font-bold text-purple-400">{stats.inviteCount}</p>
      </div>
    </div>

    <!-- Quick Actions -->
    <div class="mt-8">
      <h2 class="text-xl font-semibold mb-4">Quick Actions</h2>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <a
          href="/admin/accounts"
          class="block bg-gray-900 border border-gray-800 rounded-lg p-6 hover:border-blue-600 transition-colors"
        >
          <h3 class="font-medium text-lg mb-2">Manage Accounts</h3>
          <p class="text-gray-400 text-sm">View, search, and moderate user accounts</p>
        </a>
        
        <a
          href="/admin/invites"
          class="block bg-gray-900 border border-gray-800 rounded-lg p-6 hover:border-blue-600 transition-colors"
        >
          <h3 class="font-medium text-lg mb-2">Invite Codes</h3>
          <p class="text-gray-400 text-sm">Create and manage invite codes</p>
        </a>
      </div>
    </div>
  {/if}
</div>
