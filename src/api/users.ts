import { get, post } from './client';
import type { User } from './auth';
export const usersApi = {
  list: (offset: number, limit: number) =>
    get<{ items: User[]; total: number }>(`/api/users?offset=${offset}&limit=${limit}`),
  create: (username: string, password: string) =>
    post<{ user: User; token: string }>('/api/users', { username, password }),
  setEnabled: (id: string, enabled: boolean) =>
    post<void>(`/api/users/${encodeURIComponent(id)}/enabled`, { enabled }),
  setPassword: (id: string, password: string) =>
    post<void>(`/api/users/${encodeURIComponent(id)}/password`, { password }),
};
