import { defineStore } from 'pinia';
import { accountsApi, type Account } from '../api/accounts';
export const useAccountsStore = defineStore('accounts', {
  state: () => ({ items: [] as Account[], total: 0, loading: false }),
  actions: {
    async load(status?: Account['status'], name?: string, accountType?: Account['type']) {
      this.loading = true;
      try {
        const params = new URLSearchParams({ limit: '200' });
        if (status) params.set('status', status);
        if (name?.trim()) params.set('name', name.trim());
        if (accountType) params.set('type', accountType);
        const data = await accountsApi.list(`?${params.toString()}`);
        this.items = data.items;
        this.total = data.total;
      } finally {
        this.loading = false;
      }
    },
  },
});
