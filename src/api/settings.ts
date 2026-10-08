import { get, put } from './client';
export type Settings = {
  host: string;
  port: number;
  upstream_timeout_seconds: number;
  first_token_timeout_seconds: number;
  request_retry_attempts: number;
  upstream_proxy_enabled: boolean;
  upstream_proxy_url: string;
  monitoring_enabled: boolean;
  monitoring_interval_minutes: number;
  monitoring_recent_count: number;
  monitoring_start_time: string | null;
  monitoring_end_time: string | null;
  local_token: string;
  launch_at_login: boolean;
};
export type MonitoringSettings = {
  monitoring_enabled: boolean;
  monitoring_start_time: string | null;
  monitoring_end_time: string | null;
  monitoring_interval_minutes: number;
  monitoring_recent_count: number;
};
export const settingsApi = {
  get: () => get<Settings>('/api/settings'),
  update: (v: Settings) => put<Settings>('/api/settings', v),
  updateMonitoring: (v: MonitoringSettings) =>
    put<MonitoringSettings>('/api/settings/monitoring', v),
};
