import { get, post } from './client';
export type User = {
  id: string;
  username: string;
  role: 'admin' | 'user';
  enabled: boolean;
  api_token_prefix: string;
  api_token_updated_at: string | null;
  created_at: string;
};
export type Identity = { user: User; desktop: boolean };
export const authApi = {
  login: (username: string, password: string) =>
    post<Identity & { token: string }>('/api/auth/login', { username, password }),
  me: () => get<Identity>('/api/auth/me'),
  logout: () => post<void>('/api/auth/logout'),
  changePassword: (current_password: string, new_password: string) =>
    post<void>('/api/auth/password', { current_password, new_password }),
  resetToken: () => post<{ token: string }>('/api/auth/token/reset'),
};
