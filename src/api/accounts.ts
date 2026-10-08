import { del, get, post, put } from './client';
export type Account = {
  id: string;
  name: string;
  type: 'openai' | 'anthropic';
  base_url: string;
  api_key: string;
  status: 'active' | 'disabled';
  disabled_until?: number | null;
  priority: number;
  multiplier: number;
  monitor_average_duration_ms?: number | null;
  test_default_model?: string;
  model_mappings?: Record<string, string>;
  supported_models?: string[];
  tags?: string[];
  notes?: string | null;
  last_error_message?: string | null;
  total_requests: number;
  total_tokens: number;
};
export type AccountTestResult = {
  account_id: string;
  success: boolean;
  status_code?: number;
  error_code?: string;
  error_message?: string;
  response_body?: string;
  model?: string;
};
export type DiscoveredModels = {
  models: string[];
};
export const accountsApi = {
  list: (params = '') => get<{ items: Account[]; total: number }>(`/api/accounts${params}`),
  create: (v: unknown) => post<Account>('/api/accounts', v),
  update: (id: string, v: unknown) => put<Account>(`/api/accounts/${id}`, v),
  remove: (id: string) => del<void>(`/api/accounts/${id}`),
  toggle: (id: string) => post<Account>(`/api/accounts/${id}/toggle-status`),
  setStatus: (id: string, status: 'active' | 'disabled', duration_hours?: 1 | 3 | 12) =>
    post<Account>(`/api/accounts/${id}/set-status`, {
      status,
      duration_hours: status === 'disabled' ? (duration_hours ?? null) : null,
    }),
  priority: (id: string, priority: number) =>
    post<Account>(`/api/accounts/${id}/adjust-priority?priority=${priority}`),
  test: (id: string, model?: string, signal?: AbortSignal) =>
    post<AccountTestResult>(`/api/accounts/${id}/test`, { model }, { signal }),
  discoverModels: (account: Pick<Account, 'type' | 'base_url' | 'api_key'>) =>
    post<DiscoveredModels>('/api/accounts/discover-models', account),
};
