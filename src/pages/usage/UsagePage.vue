<template>
  <div class="page">
    <div class="page-toolbar">
      <h2 class="page-title">使用记录</h2>
      <div>
        <el-button @click="reset">重置</el-button>
        <el-button @click="cleanupVisible = true">清除历史记录</el-button>
      </div>
    </div>

    <UsageFilter v-model="filters" @query="queryFromFirstPage" />

    <div class="summary-grid">
      <div class="metric">
        <div class="metric-label">请求数</div>
        <div class="metric-value">{{ summary.request_count }}</div>
      </div>
      <div class="metric">
        <div class="metric-label">最近10次成功率</div>
        <div class="metric-value">{{ (pageSuccessRate * 100).toFixed(2) }}%</div>
      </div>
      <div class="metric">
        <div class="metric-label">最近10次平均耗时</div>
        <div class="metric-value">{{ formatMs(pageAverageDurationMs) }}</div>
      </div>
      <div class="metric">
        <div class="metric-label">最近10次平均缓存率</div>
        <div class="metric-value">{{ formatPercentage(pageAverageCacheRate) }}</div>
      </div>
    </div>

    <el-table :data="items" v-loading="loading" border stripe class="compact-table">
      <el-table-column label="时间" min-width="110">
        <template #default="{ row }">{{ formatTime(row.started_at) }}</template>
      </el-table-column>
      <el-table-column prop="account_name" label="账号" min-width="120" />
      <el-table-column prop="account_type" label="类型" width="90" />
      <el-table-column prop="model" label="模型" min-width="130" />
      <el-table-column prop="reasoning_effort" label="推理强度" width="90" />
      <el-table-column prop="endpoint" label="接口" min-width="140" />
      <el-table-column label="结果" width="80">
        <template #default="{ row }">
          <el-tooltip
            v-if="isFailure(row)"
            :content="failureReason(row)"
            placement="top"
            :show-after="200"
            effect="dark"
          >
            <span :class="resultClass(row)">{{ resultText(row) }}</span>
          </el-tooltip>
          <span v-else :class="resultClass(row)">{{ resultText(row) }}</span>
        </template>
      </el-table-column>
      <el-table-column label="延迟" width="110">
        <template #default="{ row }">
          <div class="latency-cell">
            <div :class="(row.first_token_ms ?? 0) > 10_000 ? 'warning-text' : ''">
              首字:{{ formatSeconds(row.first_token_ms) }}
            </div>
            <div :class="(row.duration_ms ?? 0) > 20_000 ? 'warning-text' : ''">
              总耗时:{{ formatSeconds(row.duration_ms) }}
            </div>
          </div>
        </template>
      </el-table-column>
      <el-table-column label="吞吐量" width="120">
        <template #default="{ row }">{{ formatThroughput(row) }}</template>
      </el-table-column>
      <el-table-column label="重试次数" width="90">
        <template #default="{ row }">{{ displayRetryCount(row.attempts) }}</template>
      </el-table-column>
      <el-table-column label="Token用量" min-width="180">
        <template #default="{ row }">
          <div class="token-usage-cell">
            <div>
              输入: {{ formatToken(row.input_tokens) }} / 缓存:
              {{ formatToken(row.cached_tokens) }}
            </div>
            <div>
              输出: {{ formatToken(row.output_tokens) }} / 合计:
              {{ formatToken(row.total_tokens) }}
            </div>
            <div>缓存率: {{ formatCacheRate(row) }}</div>
          </div>
        </template>
      </el-table-column>
      <el-table-column label="操作" width="80" fixed="right">
        <template #default="{ row }">
          <el-button link type="primary" @click="showDetail(row.id)">详情</el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-pagination
      v-model:current-page="page"
      :page-size="PAGE_SIZE"
      :total="total"
      layout="total, prev, pager, next"
      class="pagination"
      @current-change="load"
    />

    <UsageDetailDialog v-model="detailVisible" :record="selected" />
    <UsageCleanupDialog v-model="cleanupVisible" :loading="cleanupLoading" @confirm="cleanup" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue';
import { ElMessage } from 'element-plus';
import { usageApi, type UsageFilterState, type UsageRecord } from '../../api/usage';
import UsageCleanupDialog from '../../components/usage/UsageCleanupDialog.vue';
import UsageDetailDialog from '../../components/usage/UsageDetailDialog.vue';
import UsageFilter from '../../components/usage/UsageFilter.vue';
import { formatThroughput, formatToken } from '../../utils/token';

const PAGE_SIZE = 10;
const items = ref<UsageRecord[]>([]);
const total = ref(0);
const page = ref(1);
const loading = ref(false);
const filters = reactive<UsageFilterState>({
  account_id: '',
  model: '',
  kind: '',
  success: undefined as boolean | undefined,
  started_after: '',
  started_before: '',
});
const summary = reactive({
  request_count: 0,
  success_rate: 0,
  average_duration_ms: 0,
  total_tokens: 0,
});
const detailVisible = ref(false);
const selected = ref<UsageRecord>();
const cleanupVisible = ref(false);
const cleanupLoading = ref(false);

const pageSuccessRate = computed(() => {
  if (!items.value.length) return 0;
  const successful = items.value.filter((record) => record.success).length;
  return successful / items.value.length;
});

const pageAverageDurationMs = computed(() => {
  const durations = items.value
    .map((record) => record.duration_ms)
    .filter((duration): duration is number => duration != null);
  if (!durations.length) return undefined;
  return Math.round(durations.reduce((sum, duration) => sum + duration, 0) / durations.length);
});

const cacheRate = (record: UsageRecord): number | undefined => {
  if (record.input_tokens == null || record.input_tokens <= 0 || record.cached_tokens == null) {
    return undefined;
  }
  return record.cached_tokens / record.input_tokens;
};

const pageAverageCacheRate = computed(() => {
  const rates = items.value.map(cacheRate).filter((rate): rate is number => rate != null);
  if (!rates.length) return undefined;
  return rates.reduce((sum, rate) => sum + rate, 0) / rates.length;
});

const query = () => {
  const params = new URLSearchParams({
    offset: String((page.value - 1) * PAGE_SIZE),
    limit: String(PAGE_SIZE),
  });
  if (filters.account_id.trim()) params.set('account_id', filters.account_id.trim());
  if (filters.model.trim()) params.set('model', filters.model.trim());
  if (filters.kind) params.set('type', filters.kind);
  if (filters.success !== undefined) params.set('success', String(filters.success));
  const startedAfter = dayBoundary(filters.started_after, false);
  const startedBefore = dayBoundary(filters.started_before, true);
  if (startedAfter) params.set('started_after', startedAfter);
  if (startedBefore) params.set('started_before', startedBefore);
  return `?${params.toString()}`;
};

const dayBoundary = (date: string, endOfDay: boolean) => {
  const value = date.trim();
  if (!value) return '';
  return `${value}T${endOfDay ? '23:59:59' : '00:00:00'}Z`;
};

const load = async () => {
  loading.value = true;
  try {
    const result = await usageApi.list(query());
    items.value = result.items;
    total.value = result.total;
    Object.assign(summary, result.summary);
  } finally {
    loading.value = false;
  }
};

const queryFromFirstPage = () => {
  page.value = 1;
  load();
};

const reset = () => {
  filters.account_id = '';
  filters.model = '';
  filters.kind = '';
  filters.success = undefined;
  filters.started_after = '';
  filters.started_before = '';
  queryFromFirstPage();
};

const showDetail = async (id: string) => {
  selected.value = await usageApi.detail(id);
  detailVisible.value = true;
};

const cleanup = async (days: number) => {
  cleanupLoading.value = true;
  try {
    const result = await usageApi.cleanup(days);
    cleanupVisible.value = false;
    ElMessage.success(`已清除 ${result.deleted} 条`);
    queryFromFirstPage();
  } catch (error) {
    ElMessage.error(`清除历史记录失败：${String(error)}`);
  } finally {
    cleanupLoading.value = false;
  }
};

const formatTime = (value?: string) => {
  if (!value) return '-';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { hour12: false });
};

const formatMs = (value?: number) => (value == null ? '-' : `${(value / 1000).toFixed(2)} 秒`);
const formatSeconds = (value?: number) => (value == null ? '-' : `${(value / 1000).toFixed(2)}s`);
const formatPercentage = (value?: number) => (value == null ? '-' : `${(value * 100).toFixed(2)}%`);
const displayRetryCount = (attempts?: number) => Math.max((attempts ?? 1) - 1, 0);
const resultText = (record: UsageRecord) =>
  record.ended_at ? (record.success ? '成功' : '失败') : '进行中';
const resultClass = (record: UsageRecord) =>
  record.ended_at ? (record.success ? 'success-text' : 'failure-text') : 'pending-text';
const isFailure = (record: UsageRecord) => Boolean(record.ended_at && !record.success);
const failureReason = (record: UsageRecord) => {
  const details = [
    record.error_message,
    record.error_code,
    record.status_code && `状态码 ${record.status_code}`,
  ]
    .filter(Boolean)
    .map(String);
  return details.join(' / ') || '未记录失败原因';
};
const formatCacheRate = (record: UsageRecord) => formatPercentage(cacheRate(record));

let refreshTimer: ReturnType<typeof setInterval> | undefined;

onMounted(() => {
  load();
  refreshTimer = setInterval(() => {
    if (!loading.value) load();
  }, 3000);
});

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer);
});
</script>

<style scoped>
.summary-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
  margin-bottom: 14px;
}

.pagination {
  justify-content: flex-end;
  margin-top: 14px;
}

.success-text {
  color: #2e9f63;
}

.failure-text {
  color: #c43d4b;
  font-weight: 600;
}

.pending-text {
  color: #b7791f;
  font-weight: 600;
}

.token-usage-cell {
  color: #475467;
  font-size: 12px;
  line-height: 1.45;
  white-space: nowrap;
}

.latency-cell {
  line-height: 1.45;
  white-space: nowrap;
}
</style>
