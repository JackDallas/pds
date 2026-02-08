<script lang="ts">
  import { goto } from '$app/navigation';
  import { auth } from '$lib/auth.svelte';
  import { api } from '$lib/api';
  import { onMount } from 'svelte';

  let identifier = $state('');
  let password = $state('');
  let error = $state('');
  let loading = $state(false);

  onMount(() => {
    // Redirect to dashboard if already logged in
    if (auth.isAuthenticated) {
      goto('/admin/dashboard');
    }
  });

  async function handleLogin() {
    if (!identifier || !password) {
      error = 'Please enter both identifier and password';
      return;
    }

    loading = true;
    error = '';

    const response = await api.login(identifier, password);

    if (response.success && response.data) {
      // Check if user is admin by calling getAccountInfo
      const infoResponse = await api.getAccountInfo(response.data.did);
      const isAdmin = infoResponse.success && infoResponse.data?.isAdmin;

      if (!isAdmin) {
        error = 'Access denied: Admin privileges required';
        loading = false;
        return;
      }

      auth.login(response.data.accessJwt, response.data.did, response.data.handle, true);
      goto('/admin/dashboard');
    } else {
      error = response.error || 'Login failed';
      loading = false;
    }
  }
</script>

<div class="w-full max-w-md p-8">
  <div class="bg-gray-900 rounded-lg shadow-xl p-8 border border-gray-800">
    <h1 class="text-3xl font-bold text-center mb-2">PDS Admin</h1>
    <p class="text-gray-400 text-center mb-8">Sign in to continue</p>

    {#if error}
      <div class="mb-6 p-4 bg-red-900/50 border border-red-700 rounded-lg text-red-200 text-sm">
        {error}
      </div>
    {/if}

    <form onsubmit={(e) => { e.preventDefault(); handleLogin(); }}>
      <div class="mb-4">
        <label for="identifier" class="block text-sm font-medium text-gray-300 mb-2">
          Handle or Email
        </label>
        <input
          id="identifier"
          type="text"
          bind:value={identifier}
          class="w-full px-4 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-white"
          placeholder="admin.example.com"
          disabled={loading}
        />
      </div>

      <div class="mb-6">
        <label for="password" class="block text-sm font-medium text-gray-300 mb-2">
          Password
        </label>
        <input
          id="password"
          type="password"
          bind:value={password}
          class="w-full px-4 py-2 bg-gray-800 border border-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 text-white"
          placeholder="••••••••"
          disabled={loading}
        />
      </div>

      <button
        type="submit"
        disabled={loading}
        class="w-full py-3 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-700 text-white font-medium rounded-lg transition-colors"
      >
        {loading ? 'Signing in...' : 'Sign In'}
      </button>
    </form>
  </div>
</div>
