import { get } from './client';
export type MonitorRecord = {
  checked_at: string;
  model?: string;
  success: boolean;
  duration_ms?: number;
  status_code?: number;
  error_message?: string;
  error_code?: string;
};
export type MonitorItem = {
  account_id: string;
  account_name: string;
  account_type: string;
  multiplier: number;
  priority: number;
  monitor_average_duration_ms?: number;
  records: MonitorRecord[];
  model?: string;
};
export const monitorApi = {
  list: () =>
    get<{
      items: MonitorItem[];
      monitoring_enabled: boolean;
      monitoring_start_time: string | null;
      monitoring_end_time: string | null;
    }>('/api/monitor/records'),
};
