import { del, get } from './client';
export type UsageRecord = {
  id: string;
  trace_id?: string;
  started_at: string;
  ended_at?: string;
  account_id?: string;
  account_name?: string;
  account_type?: string;
  model?: string;
  endpoint?: string;
  reasoning_effort?: string;
  success: boolean;
  stream?: boolean;
  status_code?: number;
  error_code?: string;
  duration_ms?: number;
  first_token_ms?: number;
  input_tokens?: number;
  output_tokens?: number;
  cached_tokens?: number;
  total_tokens?: number;
  error_message?: string;
  client_ip?: string;
  attempts?: number;
};
export type UsageFilterState = {
  account_id: string;
  model: string;
  kind: string;
  reasoning_effort: string;
  success: boolean | undefined;
  started_after: string;
  started_before: string;
};
export const usageApi = {
  list: (query: string) =>
    get<{
      items: UsageRecord[];
      total: number;
      summary: {
        request_count: number;
        success_rate: number;
        average_duration_ms: number;
        total_tokens: number;
      };
    }>(`/api/usage/records${query}`),
  detail: (id: string) => get<UsageRecord>(`/api/usage/records/${id}`),
  cleanup: (days: number) => del<{ deleted: number }>(`/api/usage/records/expired?days=${days}`),
  statistics: () => get<Statistics>('/api/usage/statistics'),
};
export type TokenSummary = {
  input_tokens: number;
  output_tokens: number;
  cached_tokens: number;
  total_tokens: number;
  cache_rate: number | null;
};
export type Statistics = {
  total: TokenSummary;
  yesterday: TokenSummary;
  today: TokenSummary;
  accounts_today: Array<
    TokenSummary & {
      account_id: string;
      account_name: string;
      account_type: string;
      multiplier: number;
      priority: number;
      recent_cache_rate: number | null;
      recent_cache_count: number;
    }
  >;
};
