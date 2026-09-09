export const formatToken = (value?: number | null): string => {
  if (value == null || !Number.isFinite(value)) return '-';
  if (value >= 1e9) return `${(value / 1e9).toFixed(2)}B`;
  if (value >= 1e6) return `${(value / 1e6).toFixed(2)}M`;
  if (value >= 1e3) return `${(value / 1e3).toFixed(2)}K`;
  return String(value);
};

export interface ThroughputSource {
  ended_at?: string;
  success: boolean;
  output_tokens?: number | null;
  duration_ms?: number | null;
  first_token_ms?: number | null;
  stream?: boolean;
}

export const calculateThroughput = (record?: ThroughputSource | null): number | null => {
  if (!record || !record.ended_at || !record.success) return null;
  if (record.output_tokens == null || record.output_tokens <= 0) return null;

  let activeDurationMs = record.duration_ms ?? 0;
  if (record.stream && record.first_token_ms != null && record.duration_ms != null) {
    const netMs = record.duration_ms - record.first_token_ms;
    if (netMs > 0) {
      activeDurationMs = netMs;
    }
  }

  if (activeDurationMs <= 0) return null;
  return record.output_tokens / (activeDurationMs / 1000);
};

export const formatThroughput = (record?: ThroughputSource | null): string => {
  const tps = calculateThroughput(record);
  if (tps == null || !Number.isFinite(tps)) return '-';
  return `${tps.toFixed(1)} token/s`;
};
