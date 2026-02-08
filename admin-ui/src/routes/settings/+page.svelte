<script lang="ts">
  import { api } from '$lib/api';
  import { onMount } from 'svelte';

  let config = $state<any>(null);
  let loading = $state(true);
  let error = $state('');

  async function loadConfig() {
    loading = true;
    error = '';
    const response = await api.getConfig();
    
    if (response.success) {
      config = response.data;
    } else {
      error = response.error || 'Failed to load configuration';
    }
    loading = false;
  }

  onMount(() => {
    loadConfig();
  });

  function formatValue(value: any): string {
    if (value === null || value === undefined) {
      return 'Not set';
    }
    if (typeof value === 'boolean') {
      return value ? 'Yes' : 'No';
    }
    if (typeof value === 'object') {
      return JSON.stringify(value, null, 2);
    }
    return String(value);
  }
</script>

<div>
  <h1 class="text-3xl font-bold mb-8">Settings</h1>

  {#if error}
    <div class="mb-6 p-4 bg-red-900/50 border border-red-700 rounded-lg text-red-200">
      {error}
    </div>
  {/if}

  {#if loading}
    <div class="text-gray-400">Loading configuration...</div>
  {:else if config}
    <div class="space-y-6">
      <!-- Server Configuration -->
      <div class="bg-gray-900 border border-gray-800 rounded-lg p-6">
        <h2 class="text-xl font-semibold mb-4">Server Configuration</h2>
        <dl class="space-y-4">
          <div class="flex justify-between border-b border-gray-800 pb-3">
            <dt class="text-gray-400">Hostname</dt>
            <dd class="text-white font-mono">{formatValue(config.hostname)}</dd>
          </div>
          <div class="flex justify-between border-b border-gray-800 pb-3">
            <dt class="text-gray-400">Service DID</dt>
            <dd class="text-white font-mono text-sm">{formatValue(config.service_did)}</dd>
          </div>
          <div class="flex justify-between border-b border-gray-800 pb-3">
            <dt class="text-gray-400">Port</dt>
            <dd class="text-white font-mono">{formatValue(config.port)}</dd>
          </div>
          <div class="flex justify-between">
            <dt class="text-gray-400">Multi-user Mode</dt>
            <dd class="text-white">{formatValue(config.multi_user_mode)}</dd>
          </div>
        </dl>
      </div>

      <!-- Invite Configuration -->
      <div class="bg-gray-900 border border-gray-800 rounded-lg p-6">
        <h2 class="text-xl font-semibold mb-4">Invite Configuration</h2>
        <dl class="space-y-4">
          <div class="flex justify-between">
            <dt class="text-gray-400">Invites Required</dt>
            <dd class="text-white">{formatValue(config.invites_required)}</dd>
          </div>
        </dl>
      </div>

      <!-- AppView Configuration -->
      {#if config.appview_url || config.appview_did}
        <div class="bg-gray-900 border border-gray-800 rounded-lg p-6">
          <h2 class="text-xl font-semibold mb-4">AppView Configuration</h2>
          <dl class="space-y-4">
            {#if config.appview_url}
              <div class="flex justify-between border-b border-gray-800 pb-3">
                <dt class="text-gray-400">AppView URL</dt>
                <dd class="text-white font-mono">{formatValue(config.appview_url)}</dd>
              </div>
            {/if}
            {#if config.appview_did}
              <div class="flex justify-between">
                <dt class="text-gray-400">AppView DID</dt>
                <dd class="text-white font-mono text-sm">{formatValue(config.appview_did)}</dd>
              </div>
            {/if}
          </dl>
        </div>
      {/if}

      <!-- Relay Configuration -->
      {#if config.relay_url}
        <div class="bg-gray-900 border border-gray-800 rounded-lg p-6">
          <h2 class="text-xl font-semibold mb-4">Relay Configuration</h2>
          <dl class="space-y-4">
            <div class="flex justify-between">
              <dt class="text-gray-400">Relay URL</dt>
              <dd class="text-white font-mono">{formatValue(config.relay_url)}</dd>
            </div>
          </dl>
        </div>
      {/if}

      <!-- Note -->
      <div class="bg-blue-900/20 border border-blue-800 rounded-lg p-4">
        <p class="text-blue-200 text-sm">
          Note: Configuration is read-only. To modify settings, edit the PDS configuration file and restart the server.
        </p>
      </div>
    </div>
  {/if}
</div>
