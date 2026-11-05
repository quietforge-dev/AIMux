import { post } from './client';

export const authApi = {
  login: (username: string, password: string) =>
    post<{ token: string }>('/api/auth/login', { username, password }),
};
