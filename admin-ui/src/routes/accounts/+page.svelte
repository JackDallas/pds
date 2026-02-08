<script lang="ts">
  import { api } from '$lib/api';
  import { onMount } from 'svelte';

  let accounts = $state<any[]>([]);
  let loading = $state(true);
  let error = $state('');
  let searchQuery = $state('');
  let actionLoading = $state<string | null>(null);

  async function loadAccounts() {
    loading = true;
    error = '';
    const response = await api.listAccounts({ limit: 100, query: searchQuery });
    
    if (response.success) {
      accounts = response.data?.accounts || [];
    } else {
      error = response.error || 'Failed to load accounts';
    }
    loading = false;
  }

  async function handleTakedown(did: string) {
    if (!confirm('Are you sure you want to takedown this account?')) {
      return;
    }

    actionLoading = did;
    const response = await api.takedownAccount(did);
    
    if (response.success) {
      await loadAccounts();
    } else {
      error = response.error || 'Failed to takedown account';
    }
    actionLoading = null;
  }

  async function handleRestore(did: string) {
    actionLoading = did;
    const response = await api.restoreAccount(did);
    
    if (response.success) {
      await loadAccounts();
    } else {
      error = response.error || 'Failed to restore account';
    }
    actionLoading = null;
  }

  function handleSearch() {
    loadAccounts();
  }

  onMount(() => {
    loadAccounts();
  });
</script>

<div>
  <div class="flex items-center justify-between mb-8">
    <h1 class="text-3xl font-bold">Accounts</h1>
  </div>

  {#if error}
    <div class="mb-6 p-4 bg-red-900/50 border border-red-700 rounded-lg text-red-200">
      {error}
    </div>
  {/if}

  <!-- Search Bar -->
  <div class="mb-6">
    <form onsubmit={(e) => { e.preventDefault(); handleSearch(); }} class="flex gap-2">
      <input
        type="text"
        bind:value={searchQuery}
        placeholder="Search by handle or DID..."
        class="flex-1 px-4 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-white"
      />
      <button
        type="submit"
        class="px-6 py-2 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded-lg transition-colors"
      >
        Search
      </button>
    </form>
  </div>

  {#if loading}
    <div class="text-gray-400">Loading accounts...</div>
  {:else if accounts.length === 0}
    <div class="text-gray-400">No accounts found</div>
  {:else}
    <div class="bg-gray-900 border border-gray-800 rounded-lg overflow-hidden">
      <table class="w-full">
        <thead class="bg-gray-800">
          <tr>
            <th class="px-6 py-3 text-left text-sm font-medium text-gray-300">Handle</th>
            <th class="px-6 py-3 text-left text-sm font-medium text-gray-300">DID</th>
            <th class="px-6 py-3 text-left text-sm font-medium text-gray-300">Status</th>
            <th class="px-6 py-3 text-right text-sm font-medium text-gray-300">Actions</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-gray-800">
          {#each accounts as account}
            <tr class="hover:bg-gray-800/50">
              <td class="px-6 py-4 text-sm text-white">
                {account.subject?.handle || 'N/A'}
              </td>
              <td class="px-6 py-4 text-sm text-gray-400 font-mono">
                {account.subject?.did || 'N/A'}
              </td>
              <td class="px-6 py-4 text-sm">
                {#if account.takendown}
                  <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-900/50 text-red-300">
                    Taken Down
                  </span>
                {:else}
                  <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-900/50 text-green-300">
                    Active
                  </span>
                {/if}
              </td>
              <td class="px-6 py-4 text-sm text-right">
                {#if account.takendown}
                  <button
                    onclick={() => handleRestore(account.subject?.did)}
                    disabled={actionLoading === account.subject?.did}
                    class="px-4 py-2 text-sm bg-green-600 hover:bg-green-700 disabled:bg-gray-700 text-white rounded-lg transition-colors"
                  >
                    {actionLoading === account.subject?.did ? 'Restoring...' : 'Restore'}
                  </button>
                {:else}
                  <button
                    onclick={() => handleTakedown(account.subject?.did)}
                    disabled={actionLoading === account.subject?.did}
                    class="px-4 py-2 text-sm bg-red-600 hover:bg-red-700 disabled:bg-gray-700 text-white rounded-lg transition-colors"
                  >
                    {actionLoading === account.subject?.did ? 'Taking down...' : 'Takedown'}
                  </button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
