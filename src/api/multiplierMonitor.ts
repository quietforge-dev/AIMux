import { get, post, put } from './client';

export type MultiplierMonitorRunSummary = {
  run_id: string;
  started_at: string;
  finished_at: string;
  total: number;
  updated: number;
  unchanged: number;
  issues: number;
  failed: number;
};

export type MultiplierMonitorConfig = {
  id: string;
  account_id: string;
  account_name: string;
  account_type: 'openai' | 'anthropic';
  account_status: 'active' | 'disabled';
  account_multiplier: number;
  configured: boolean;
  platform_type: 'sub2api' | 'newapi';
  new_api_group?: string | null;
  sync_upstream_multiplier: boolean;
  multiplier_divisor: number;
  enabled: boolean;
  last_started_at?: string | null;
  last_finished_at?: string | null;
  last_success_at?: string | null;
  last_error?: string | null;
  last_remote_multiplier?: number | null;
  last_result?: string | null;
  running: boolean;
  last_run?: MultiplierMonitorRunSummary | null;
  created_at: string;
  updated_at: string;
};

export type MultiplierMonitorUpdate = {
  platform_type: 'sub2api' | 'newapi';
  new_api_group?: string | null;
  sync_upstream_multiplier: boolean;
  multiplier_divisor: number;
  enabled: boolean;
};

export type MultiplierMonitorCheckResult = {
  run_id: string;
  total: number;
  updated: number;
  unchanged: number;
  issues: number;
  failed: number;
};

export type MultiplierMonitorLog = {
  id: string;
  run_id: string;
  config_id: string;
  config_name: string;
  account_id?: string | null;
  account_name?: string | null;
  started_at: string;
  finished_at: string;
  duration_ms?: number | null;
  result: string;
  old_multiplier?: number | null;
  remote_multiplier?: number | null;
  error_code?: string | null;
  error_message?: string | null;
  http_status?: number | null;
};

export type NewApiGroup = { name: string; rate: string };

export const multiplierMonitorApi = {
  list: () => get<{ items: MultiplierMonitorConfig[] }>('/api/multiplier-monitors'),
  update: (accountId: string, payload: MultiplierMonitorUpdate) =>
    put<MultiplierMonitorConfig>(`/api/multiplier-monitors/${accountId}`, payload),
  toggle: (accountId: string) =>
    post<MultiplierMonitorConfig>(`/api/multiplier-monitors/${accountId}/toggle`),
  check: (accountId: string) =>
    post<MultiplierMonitorCheckResult>(`/api/multiplier-monitors/${accountId}/check`),
  newApiGroups: (accountId: string) =>
    get<{ items: NewApiGroup[] }>(`/api/multiplier-monitors/${accountId}/newapi-groups`),
  logs: (accountId: string, offset: number, limit: number, result?: string) => {
    const params = new URLSearchParams({ offset: String(offset), limit: String(limit) });
    if (result) params.set('result', result);
    return get<{ items: MultiplierMonitorLog[]; total: number }>(
      `/api/multiplier-monitors/${accountId}/logs?${params}`,
    );
  },
};
