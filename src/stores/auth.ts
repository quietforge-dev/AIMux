import { defineStore } from 'pinia';
import { authApi, type User } from '../api/auth';
import { clearAuthToken, setAuthToken } from '../api/client';
import { useAccountsStore } from './accounts';
import { useModelsStore } from './models';
export const useAuthStore = defineStore('auth', {
  state: () => ({ user: null as User | null, desktop: false, ready: false }),
  getters: { isAdmin: (state) => !state.desktop && state.user?.role === 'admin' },
  actions: {
    clear() {
      this.user = null;
      useAccountsStore().$reset();
      useModelsStore().$reset();
    },
    async refresh() {
      const identity = await authApi.me();
      this.user = identity.user;
      this.desktop = identity.desktop;
      this.ready = true;
    },
    async login(username: string, password: string) {
      const identity = await authApi.login(username, password);
      this.clear();
      setAuthToken(identity.token);
      this.user = identity.user;
      this.desktop = identity.desktop;
      this.ready = true;
    },
    async logout() {
      await authApi.logout();
      clearAuthToken();
      this.clear();
    },
  },
});
