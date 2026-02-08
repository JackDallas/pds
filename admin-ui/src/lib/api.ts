import { auth } from './auth.svelte';

const API_BASE = typeof window !== 'undefined' 
  ? window.location.origin 
  : '';

interface XrpcResponse<T = any> {
  success: boolean;
  data?: T;
  error?: string;
}

export class ApiClient {
  private async request<T>(method: string, path: string, body?: any): Promise<XrpcResponse<T>> {
    const headers: HeadersInit = {
      'Content-Type': 'application/json'
    };

    if (auth.accessJwt) {
      headers['Authorization'] = `Bearer ${auth.accessJwt}`;
    }

    try {
      const response = await fetch(`${API_BASE}${path}`, {
        method,
        headers,
        body: body ? JSON.stringify(body) : undefined
      });

      if (!response.ok) {
        const errorText = await response.text();
        return {
          success: false,
          error: errorText || `HTTP ${response.status}`
        };
      }

      const data = await response.json();
      return {
        success: true,
        data
      };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : 'Network error'
      };
    }
  }

  // Auth
  async login(identifier: string, password: string) {
    return this.request<{ accessJwt: string; did: string; handle: string }>('POST', '/xrpc/com.atproto.server.createSession', {
      identifier,
      password
    });
  }

  // Admin - Accounts
  async listAccounts(params?: { limit?: number; cursor?: string; query?: string }) {
    const query = new URLSearchParams();
    if (params?.limit) query.set('limit', params.limit.toString());
    if (params?.cursor) query.set('cursor', params.cursor);
    if (params?.query) query.set('query', params.query);
    
    const queryStr = query.toString();
    return this.request<{ accounts: any[]; cursor?: string }>('GET', `/xrpc/tools.ozone.moderation.queryStatuses${queryStr ? '?' + queryStr : ''}`);
  }

  async getAccountInfo(did: string) {
    return this.request('GET', `/xrpc/com.atproto.admin.getAccountInfo?did=${encodeURIComponent(did)}`);
  }

  async takedownAccount(did: string) {
    return this.request('POST', '/xrpc/com.atproto.admin.updateSubjectStatus', {
      subject: { $type: 'com.atproto.admin.defs#repoRef', did },
      takedown: { applied: true }
    });
  }

  async restoreAccount(did: string) {
    return this.request('POST', '/xrpc/com.atproto.admin.updateSubjectStatus', {
      subject: { $type: 'com.atproto.admin.defs#repoRef', did },
      takedown: { applied: false }
    });
  }

  // Admin - Invites
  async listInviteCodes(params?: { limit?: number; cursor?: string }) {
    const query = new URLSearchParams();
    if (params?.limit) query.set('limit', params.limit.toString());
    if (params?.cursor) query.set('cursor', params.cursor);
    
    const queryStr = query.toString();
    return this.request<{ codes: any[]; cursor?: string }>('GET', `/xrpc/com.atproto.server.getAccountInviteCodes${queryStr ? '?' + queryStr : ''}`);
  }

  async createInviteCode(useCount: number) {
    return this.request<{ code: string }>('POST', '/xrpc/com.atproto.server.createInviteCode', {
      useCount
    });
  }

  async disableInviteCode(code: string) {
    return this.request('POST', '/xrpc/com.atproto.admin.disableInviteCodes', {
      codes: [code]
    });
  }

  // Stats
  async getStats() {
    return this.request<{ accountCount: number; repoCount: number; inviteCount: number }>('GET', '/xrpc/com.dallaspds.admin.getStats');
  }

  // Config
  async getConfig() {
    return this.request('GET', '/xrpc/com.dallaspds.admin.getConfig');
  }
}

export const api = new ApiClient();
