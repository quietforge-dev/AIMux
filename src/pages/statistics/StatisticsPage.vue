<template>
  <div class="page">
    <div class="page-toolbar">
      <h2 class="page-title">数据统计</h2>
      <el-button :loading="loading" @click="load">刷新</el-button>
    </div>

    <div class="summary-cards">
      <section v-for="group in groups" :key="group.key" class="summary-line">
        <h3 class="summary-card-title">{{ group.label }}</h3>
        <div v-for="metric in metrics" :key="metric.key" class="summary-card">
          <span class="summary-label">{{ metric.label }}</span>
          <strong class="summary-value">
            {{
              metric.key === 'cache_rate'
                ? rate(summary(group.key).cache_rate)
                : formatToken(summary(group.key)[metric.key])
            }}
          </strong>
        </div>
      </section>
    </div>

    <el-divider />
    <h3>启用账号今日统计</h3>
    <el-table :data="accounts" border class="compact-table">
      <el-table-column prop="account_name" label="账号" min-width="180" />
      <el-table-column prop="account_type" label="类型" width="90" />
      <el-table-column prop="multiplier" label="倍率" width="80">
        <template #default="{ row }">{{ Number(row.multiplier).toFixed(2) }}</template>
      </el-table-column>
      <el-table-column prop="priority" label="优先级" width="80" />
      <el-table-column label="总Token" min-width="110">
        <template #default="{ row }">{{ formatToken(row.total_tokens) }}</template>
      </el-table-column>
      <el-table-column label="总输入" min-width="110">
        <template #default="{ row }">{{ formatToken(row.input_tokens) }}</template>
      </el-table-column>
      <el-table-column label="总输出" min-width="110">
        <template #default="{ row }">{{ formatToken(row.output_tokens) }}</template>
      </el-table-column>
      <el-table-column label="总缓存" min-width="110">
        <template #default="{ row }">{{ formatToken(row.cached_tokens) }}</template>
      </el-table-column>
      <el-table-column label="缓存率" min-width="100">
        <template #default="{ row }">{{ rate(row.cache_rate) }}</template>
      </el-table-column>
      <el-table-column label="最近20次缓存率" min-width="150">
        <template #default="{ row }">
          <el-tooltip
            :content="`今日最近 ${row.recent_cache_count} 条有效使用记录，按输入Token加权计算`"
            placement="top"
          >
            <span>{{ rate(row.recent_cache_rate) }}</span>
          </el-tooltip>
        </template>
      </el-table-column>
    </el-table>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { usageApi, type Statistics, type TokenSummary } from '../../api/usage';
import { formatToken } from '../../utils/token';

type SummaryKey = 'total' | 'yesterday' | 'today';
type MetricKey = keyof TokenSummary;

const groups: Array<{ key: SummaryKey; label: string }> = [
  { key: 'total', label: '总计' },
  { key: 'yesterday', label: '昨日' },
  { key: 'today', label: '今日' },
];
const metrics: Array<{ key: MetricKey; label: string }> = [
  { key: 'total_tokens', label: '总Token' },
  { key: 'input_tokens', label: '总输入' },
  { key: 'output_tokens', label: '总输出' },
  { key: 'cached_tokens', label: '总缓存' },
  { key: 'cache_rate', label: '缓存率' },
];
const emptySummary: TokenSummary = {
  input_tokens: 0,
  output_tokens: 0,
  cached_tokens: 0,
  total_tokens: 0,
  cache_rate: null,
};
const data = ref<Statistics>();
const loading = ref(false);
const accounts = computed(() => data.value?.accounts_today ?? []);
const summary = (key: SummaryKey) => data.value?.[key] ?? emptySummary;

const load = async () => {
  loading.value = true;
  try {
    data.value = await usageApi.statistics();
  } finally {
    loading.value = false;
  }
};

const rate = (value: number | null | undefined) =>
  value == null ? '-' : `${(value * 100).toFixed(2)}%`;

onMounted(load);
</script>

<style scoped>
.summary-cards {
  display: grid;
  gap: 10px;
  margin-bottom: 18px;
}

.summary-line {
  display: grid;
  grid-template-columns: 64px repeat(5, minmax(0, 1fr));
  gap: 10px;
  align-items: stretch;
}

.summary-card {
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 5px;
  min-width: 0;
}

.summary-card-title {
  margin: 0;
  display: flex;
  align-items: center;
  font-size: 16px;
}

.summary-label,
.summary-value {
  white-space: nowrap;
}

.summary-label {
  color: #667085;
}

.summary-value {
  color: #1f2937;
  font-size: 17px;
}

.page > h3 {
  margin: 0 0 10px;
  font-size: 16px;
}

@media (max-width: 1280px) {
  .summary-line {
    grid-template-columns: 56px repeat(5, minmax(130px, 1fr));
    overflow-x: auto;
  }
}
</style>
