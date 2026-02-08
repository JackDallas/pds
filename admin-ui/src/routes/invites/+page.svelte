<script lang="ts">
  import { api } from '$lib/api';
  import { onMount } from 'svelte';

  let codes = $state<any[]>([]);
  let loading = $state(true);
  let error = $state('');
  let createLoading = $state(false);
  let useCount = $state(1);
  let actionLoading = $state<string | null>(null);

  async function loadInviteCodes() {
    loading = true;
    error = '';
    const response = await api.listInviteCodes({ limit: 100 });
    
    if (response.success) {
      codes = response.data?.codes || [];
    } else {
      error = response.error || 'Failed to load invite codes';
    }
    loading = false;
  }

  async function handleCreateCode() {
    if (useCount < 1) {
      error = 'Use count must be at least 1';
      return;
    }

    createLoading = true;
    error = '';
    const response = await api.createInviteCode(useCount);
    
    if (response.success) {
      await loadInviteCodes();
      useCount = 1;
    } else {
      error = response.error || 'Failed to create invite code';
    }
    createLoading = false;
  }

  async function handleDisable(code: string) {
    if (!confirm('Are you sure you want to disable this invite code?')) {
      return;
    }

    actionLoading = code;
    const response = await api.disableInviteCode(code);
    
    if (response.success) {
      await loadInviteCodes();
    } else {
      error = response.error || 'Failed to disable invite code';
    }
    actionLoading = null;
  }

  onMount(() => {
    loadInviteCodes();
  });
</script>

<div>
  <h1 class="text-3xl font-bold mb-8">Invite Codes</h1>

  {#if error}
    <div class="mb-6 p-4 bg-red-900/50 border border-red-700 rounded-lg text-red-200">
      {error}
    </div>
  {/if}

  <!-- Create Invite Code Form -->
  <div class="bg-gray-900 border border-gray-800 rounded-lg p-6 mb-8">
    <h2 class="text-xl font-semibold mb-4">Create New Invite Code</h2>
    <form onsubmit={(e) => { e.preventDefault(); handleCreateCode(); }} class="flex gap-4 items-end">
      <div class="flex-1">
        <label for="useCount" class="block text-sm font-medium text-gray-300 mb-2">
          Number of Uses
        </label>
        <input
          id="useCount"
          type="number"
          min="1"
          bind:value={useCount}
          class="w-full px-4 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-white"
          disabled={createLoading}
        />
      </div>
      <button
        type="submit"
        disabled={createLoading}
        class="px-6 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-700 text-white font-medium rounded-lg transition-colors"
      >
        {createLoading ? 'Creating...' : 'Create Code'}
      </button>
    </form>
  </div>

  <!-- Invite Codes List -->
  {#if loading}
    <div class="text-gray-400">Loading invite codes...</div>
  {:else if codes.length === 0}
    <div class="text-gray-400">No invite codes found</div>
  {:else}
    <div class="bg-gray-900 border border-gray-800 rounded-lg overflow-hidden">
      <table class="w-full">
        <thead class="bg-gray-800">
          <tr>
            <th class="px-6 py-3 text-left text-sm font-medium text-gray-300">Code</th>
            <th class="px-6 py-3 text-left text-sm font-medium text-gray-300">Uses</th>
            <th class="px-6 py-3 text-left text-sm font-medium text-gray-300">Status</th>
            <th class="px-6 py-3 text-right text-sm font-medium text-gray-300">Actions</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-gray-800">
          {#each codes as code}
            <tr class="hover:bg-gray-800/50">
              <td class="px-6 py-4 text-sm text-white font-mono">
                {code.code}
              </td>
              <td class="px-6 py-4 text-sm text-gray-400">
                {code.used} / {code.available}
              </td>
              <td class="px-6 py-4 text-sm">
                {#if code.disabled}
                  <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-900/50 text-red-300">
                    Disabled
                  </span>
                {:else if code.used >= code.available}
                  <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-700 text-gray-300">
                    Exhausted
                  </span>
                {:else}
                  <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-900/50 text-green-300">
                    Active
                  </span>
                {/if}
              </td>
              <td class="px-6 py-4 text-sm text-right">
                {#if !code.disabled}
                  <button
                    onclick={() => handleDisable(code.code)}
                    disabled={actionLoading === code.code}
                    class="px-4 py-2 text-sm bg-red-600 hover:bg-red-700 disabled:bg-gray-700 text-white rounded-lg transition-colors"
                  >
                    {actionLoading === code.code ? 'Disabling...' : 'Disable'}
                  </button>
                {:else}
                  <span class="text-gray-500">-</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
