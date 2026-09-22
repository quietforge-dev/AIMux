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
  name: string;
  url: string;
  account_ids: string[];
  multiplier_divisor: number;
  enabled: boolean;
  has_token: boolean;
  has_refresh_token: boolean;
  last_started_at?: string | null;
  last_finished_at?: string | null;
  running: boolean;
  last_run?: MultiplierMonitorRunSummary | null;
  created_at: string;
  updated_at: string;
};

export type MultiplierMonitorAccountOption = {
  id: string;
  name: string;
  type: 'openai' | 'anthropic';
  status: 'active' | 'disabled';
  multiplier: number;
};

export type MultiplierMonitorCreate = {
  name: string;
  url: string;
  token: string;
  refresh_token: string;
  account_ids: string[];
  multiplier_divisor: number;
  enabled: boolean;
};

export type MultiplierMonitorUpdate = Omit<MultiplierMonitorCreate, 'token' | 'refresh_token'> & {
  token?: string;
  refresh_token?: string;
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

export const multiplierMonitorApi = {
  list: () => get<{ items: MultiplierMonitorConfig[] }>('/api/multiplier-monitors'),
  accountOptions: () =>
    get<{ items: MultiplierMonitorAccountOption[] }>('/api/multiplier-monitors/account-options'),
  create: (payload: MultiplierMonitorCreate) =>
    post<MultiplierMonitorConfig>('/api/multiplier-monitors', payload),
  update: (id: string, payload: MultiplierMonitorUpdate) =>
    put<MultiplierMonitorConfig>(`/api/multiplier-monitors/${id}`, payload),
  toggle: (id: string) => post<MultiplierMonitorConfig>(`/api/multiplier-monitors/${id}/toggle`),
  check: (id: string) => post<MultiplierMonitorCheckResult>(`/api/multiplier-monitors/${id}/check`),
  logs: (id: string, offset: number, limit: number, result?: string) => {
    const params = new URLSearchParams({ offset: String(offset), limit: String(limit) });
    if (result) params.set('result', result);
    return get<{ items: MultiplierMonitorLog[]; total: number }>(
      `/api/multiplier-monitors/${id}/logs?${params}`,
    );
  },
};
