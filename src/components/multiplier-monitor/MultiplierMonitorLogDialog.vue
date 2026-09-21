<template>
  <el-dialog v-model="visible" :title="`倍率监控日志 · ${config?.name ?? ''}`" width="980px">
    <div class="log-toolbar">
      <el-select v-model="resultFilter" clearable placeholder="全部结果" @change="reload">
        <el-option v-for="option in resultOptions" :key="option.value" v-bind="option" />
      </el-select>
      <el-button :loading="loading" @click="load">刷新</el-button>
    </div>
    <el-table :data="items" v-loading="loading" border class="compact-table" height="430">
      <el-table-column label="检查时间" width="170">
        <template #default="{ row }">{{ formatTime(row.started_at) }}</template>
      </el-table-column>
      <el-table-column label="账号" min-width="150">
        <template #default="{ row }">{{ row.account_name || '配置级错误' }}</template>
      </el-table-column>
      <el-table-column label="结果" width="110">
        <template #default="{ row }">
          <el-tag :type="resultType(row.result)" size="small">
            {{ resultLabel(row.result) }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="倍率变化" width="130">
        <template #default="{ row }">{{ multiplierChange(row) }}</template>
      </el-table-column>
      <el-table-column label="耗时" width="90">
        <template #default="{ row }">{{ durationText(row.duration_ms) }}</template>
      </el-table-column>
      <el-table-column label="说明" min-width="230" show-overflow-tooltip>
        <template #default="{ row }">
          <span :class="row.error_message ? 'warning-text' : 'muted'">
            {{ row.error_message || '-' }}
          </span>
        </template>
      </el-table-column>
    </el-table>
    <div class="pagination-wrap">
      <el-pagination
        v-model:current-page="page"
        :page-size="pageSize"
        :total="total"
        layout="total, prev, pager, next"
        @current-change="load"
      />
    </div>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { ElMessage } from 'element-plus';
import {
  multiplierMonitorApi,
  type MultiplierMonitorConfig,
  type MultiplierMonitorLog,
} from '../../api/multiplierMonitor';

const visible = defineModel<boolean>('visible', { required: true });
const props = defineProps<{ config?: MultiplierMonitorConfig }>();
const items = ref<MultiplierMonitorLog[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 50;
const loading = ref(false);
const resultFilter = ref('');

const resultOptions = [
  { label: '已更新', value: 'updated' },
  { label: '无变化', value: 'unchanged' },
  { label: '未匹配密钥', value: 'missing_key' },
  { label: '倍率无效', value: 'invalid_rate' },
  { label: '账号已删除', value: 'account_missing' },
  { label: '账号已停用', value: 'skipped_disabled' },
  { label: '并发冲突', value: 'conflict' },
  { label: '检查失败', value: 'failed' },
  { label: '已取消', value: 'cancelled' },
];

const load = async () => {
  if (!props.config) return;
  loading.value = true;
  try {
    const response = await multiplierMonitorApi.logs(
      props.config.id,
      (page.value - 1) * pageSize,
      pageSize,
      resultFilter.value || undefined,
    );
    items.value = response.items;
    total.value = response.total;
  } catch (error) {
    ElMessage.error(String(error));
  } finally {
    loading.value = false;
  }
};

const reload = () => {
  page.value = 1;
  void load();
};

watch(visible, (open) => {
  if (!open) return;
  page.value = 1;
  resultFilter.value = '';
  void load();
});

const resultLabel = (result: string) =>
  resultOptions.find((option) => option.value === result)?.label ?? result;

const resultType = (result: string) => {
  if (result === 'updated') return 'success';
  if (result === 'unchanged') return 'info';
  if (result === 'failed') return 'danger';
  return 'warning';
};

const formatTime = (value?: string | null) =>
  value ? new Date(value).toLocaleString('zh-CN', { hour12: false }) : '-';

const durationText = (value?: number | null) =>
  value === null || value === undefined ? '-' : `${(value / 1000).toFixed(1)}s`;

const multiplierText = (value?: number | null) =>
  value === null || value === undefined ? '-' : Number(value).toFixed(2);

const multiplierChange = (row: MultiplierMonitorLog) => {
  if (row.old_multiplier === null || row.old_multiplier === undefined) return '-';
  if (row.remote_multiplier === null || row.remote_multiplier === undefined) {
    return multiplierText(row.old_multiplier);
  }
  return `${multiplierText(row.old_multiplier)} → ${multiplierText(row.remote_multiplier)}`;
};
</script>

<style scoped>
.log-toolbar {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-bottom: 12px;
}

.log-toolbar .el-select {
  width: 160px;
}

.pagination-wrap {
  display: flex;
  justify-content: flex-end;
  margin-top: 14px;
}
</style>
