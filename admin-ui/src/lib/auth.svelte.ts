interface AuthState {
  accessJwt: string | null;
  did: string | null;
  handle: string | null;
  isAdmin: boolean;
}

class Auth {
  private state = $state<AuthState>({
    accessJwt: null,
    did: null,
    handle: null,
    isAdmin: false
  });

  constructor() {
    if (typeof window !== 'undefined') {
      this.loadFromStorage();
    }
  }

  get accessJwt() {
    return this.state.accessJwt;
  }

  get did() {
    return this.state.did;
  }

  get handle() {
    return this.state.handle;
  }

  get isAdmin() {
    return this.state.isAdmin;
  }

  get isAuthenticated() {
    return this.state.accessJwt !== null;
  }

  login(accessJwt: string, did: string, handle: string, isAdmin: boolean) {
    this.state.accessJwt = accessJwt;
    this.state.did = did;
    this.state.handle = handle;
    this.state.isAdmin = isAdmin;
    this.saveToStorage();
  }

  logout() {
    this.state.accessJwt = null;
    this.state.did = null;
    this.state.handle = null;
    this.state.isAdmin = false;
    if (typeof window !== 'undefined') {
      localStorage.removeItem('auth');
    }
  }

  private loadFromStorage() {
    try {
      const stored = localStorage.getItem('auth');
      if (stored) {
        const parsed = JSON.parse(stored);
        this.state.accessJwt = parsed.accessJwt;
        this.state.did = parsed.did;
        this.state.handle = parsed.handle;
        this.state.isAdmin = parsed.isAdmin;
      }
    } catch (e) {
      console.error('Failed to load auth from storage:', e);
    }
  }

  private saveToStorage() {
    if (typeof window !== 'undefined') {
      localStorage.setItem('auth', JSON.stringify(this.state));
    }
  }
}

export const auth = new Auth();
