<template>
  <div class="page">
    <div class="page-toolbar">
      <h2 class="page-title">监控管理</h2>
      <div class="monitor-actions">
        <el-switch
          v-model="enabled"
          active-text="账号监控"
          :loading="savingEnabled"
          :before-change="changeEnabled"
        />
        <span class="range-label">监控时段</span>
        <el-time-picker
          v-model="timeRange"
          is-range
          format="HH:mm"
          range-separator="至"
          start-placeholder="开始时间"
          end-placeholder="结束时间"
          :disabled="savingRange"
          @change="changeRange"
        />
        <span v-if="enabled && !inWindow" class="window-hint">当前不在监控时段</span>
        <el-button :loading="loading" @click="load">刷新</el-button>
      </div>
    </div>

    <el-table :data="items" v-loading="loading" border class="compact-table">
      <el-table-column prop="account_name" label="账号" min-width="140" fixed="left" />
      <el-table-column prop="account_type" label="类型" width="90" />
      <el-table-column prop="multiplier" label="倍率" width="60">
        <template #default="{ row }">{{ Number(row.multiplier).toFixed(2) }}</template>
      </el-table-column>
      <el-table-column prop="priority" label="优先级" width="70" />
      <el-table-column label="测试模型" min-width="120">
        <template #default="{ row }">
          {{ row.model || latest(row.records)?.model || '-' }}
        </template>
      </el-table-column>
      <el-table-column label="最近检查" width="160">
        <template #default="{ row }">{{ formatTime(latest(row.records)?.checked_at) }}</template>
      </el-table-column>
      <el-table-column label="平均耗时" width="90">
        <template #default="{ row }">
          <span
            :class="(row.monitor_average_duration_ms ?? 0) > SLOW_THRESHOLD ? 'warning-text' : ''"
          >
            {{ avgText(row.monitor_average_duration_ms) }}
          </span>
        </template>
      </el-table-column>
      <el-table-column label="结果" width="60">
        <template #default="{ row }">
          <span
            v-if="latest(row.records)"
            :class="latest(row.records)?.success ? 'success-text' : 'failure-text'"
          >
            {{ latest(row.records)?.success ? '成功' : '失败' }}
          </span>
          <span v-else>-</span>
        </template>
      </el-table-column>
      <el-table-column label="最近30次检测记录" min-width="620">
        <template #default="{ row }">
          <div class="checks">
            <span
              v-for="(record, index) in normalized(row.records)"
              :key="index"
              class="check"
              :class="cellClass(record)"
              :title="cellTitle(record)"
            />
          </div>
        </template>
      </el-table-column>
    </el-table>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { monitorApi, type MonitorItem, type MonitorRecord } from '../../api/monitor';
import { settingsApi } from '../../api/settings';
import { ElMessage } from 'element-plus';

const STATUS_COUNT = 30;
const SLOW_THRESHOLD = 20_000;
const items = ref<MonitorItem[]>([]);
const loading = ref(false);
const enabled = ref(false);
const savingEnabled = ref(false);
const timeRange = ref<[Date, Date] | null>(null);
const savingRange = ref(false);
let timer: number | undefined;

const load = async () => {
  if (loading.value) return;
  loading.value = true;
  try {
    const result = await monitorApi.list();
    items.value = result.items;
    enabled.value = result.monitoring_enabled;
    timeRange.value = toRange(result.monitoring_start_time, result.monitoring_end_time);
  } finally {
    loading.value = false;
  }
};

const toTime = (value?: string | null) => {
  if (!value) return null;
  const [hours, minutes] = value.split(':').map(Number);
  if (Number.isNaN(hours) || Number.isNaN(minutes)) return null;
  const date = new Date();
  date.setHours(hours, minutes, 0, 0);
  return date;
};

const toRange = (start?: string | null, end?: string | null): [Date, Date] | null => {
  const startDate = toTime(start);
  const endDate = toTime(end);
  return startDate && endDate ? [startDate, endDate] : null;
};

const formatHHmm = (value: Date) =>
  `${String(value.getHours()).padStart(2, '0')}:${String(value.getMinutes()).padStart(2, '0')}`;

const currentWindow = () => {
  if (!timeRange.value) {
    return { monitoring_start_time: null, monitoring_end_time: null };
  }
  return {
    monitoring_start_time: formatHHmm(timeRange.value[0]),
    monitoring_end_time: formatHHmm(timeRange.value[1]),
  };
};

const inWindow = computed(() => {
  if (!timeRange.value) return true;
  const now = new Date();
  const minutes = now.getHours() * 60 + now.getMinutes();
  const toMinutes = (value: Date) => value.getHours() * 60 + value.getMinutes();
  const start = toMinutes(timeRange.value[0]);
  const end = toMinutes(timeRange.value[1]);
  return start < end ? minutes >= start && minutes <= end : minutes >= start || minutes <= end;
});

const changeEnabled = async () => {
  const next = !enabled.value;
  savingEnabled.value = true;
  try {
    await settingsApi.updateMonitoring({
      monitoring_enabled: next,
      ...currentWindow(),
    });
    ElMessage.success(next ? '账号监控已开启' : '账号监控已关闭');
    return true;
  } catch (error) {
    ElMessage.error(`更新账号监控失败：${String(error)}`);
    return false;
  } finally {
    savingEnabled.value = false;
  }
};

const changeRange = async () => {
  savingRange.value = true;
  try {
    await settingsApi.updateMonitoring({
      monitoring_enabled: enabled.value,
      ...currentWindow(),
    });
    ElMessage.success('监控时间范围已保存');
  } catch (error) {
    ElMessage.error(`更新监控时间范围失败：${String(error)}`);
    await load();
  } finally {
    savingRange.value = false;
  }
};

const latest = (records: MonitorRecord[]) => records.at(-1);

const normalized = (records: MonitorRecord[]) => {
  const recent = records.slice(-STATUS_COUNT);
  const emptyCount = STATUS_COUNT - recent.length;
  return Array.from({ length: STATUS_COUNT }, (_, index) => recent[index - emptyCount] ?? null);
};

const avgText = (duration?: number) =>
  duration == null ? '-' : `${(duration / 1000).toFixed(2)} 秒`;

const formatTime = (value?: string) => {
  if (!value) return '-';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { hour12: false });
};

const cellClass = (record: MonitorRecord | null) => {
  if (!record) return 'empty';
  if (!record.success) return 'fail';
  return (record.duration_ms ?? 0) > SLOW_THRESHOLD ? 'slow' : 'ok';
};

const cellTitle = (record: MonitorRecord | null) => {
  if (!record) return '暂无监控记录';
  const result = record.success ? '成功' : '失败';
  const duration =
    record.duration_ms == null ? '-' : `${(record.duration_ms / 1000).toFixed(2)} 秒`;
  const error = record.success ? '' : `，错误：${record.error_message || record.error_code || '-'}`;
  return `${formatTime(record.checked_at)}，${result}，耗时：${duration}，状态码：${record.status_code ?? '-'}${error}`;
};

onMounted(() => {
  load();
  timer = window.setInterval(load, 30_000);
});
onUnmounted(() => {
  if (timer !== undefined) window.clearInterval(timer);
});
</script>

<style scoped>
.monitor-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.range-label {
  color: #606266;
  font-size: 14px;
}

.window-hint {
  color: #e0a800;
  font-size: 12px;
}

.checks {
  display: flex;
  gap: 0;
  height: 26px;
  align-items: center;
}

.check {
  display: block;
  width: 20px;
  height: 20px;
  border-right: 1px solid white;
}

.empty {
  background: #9ca3af;
}

.ok {
  background: #2e9f63;
}

.fail {
  background: #c43d4b;
}

.slow {
  background: #e0a800;
}

.check:first-child {
  border-radius: 3px 0 0 3px;
}

.check:last-child {
  border-radius: 0 3px 3px 0;
}

.success-text {
  color: #2e9f63;
}

.failure-text {
  color: #c43d4b;
  font-weight: 600;
}
</style>
