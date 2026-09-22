<template>
  <div class="page">
    <div class="page-toolbar">
      <div>
        <h2 class="page-title">倍率监控</h2>
        <div class="page-description">
          每小时查询远端密钥倍率，仅在倍率变化且数据有效时更新账号。
        </div>
      </div>
      <div class="page-actions">
        <el-button :loading="loading" @click="load">刷新</el-button>
        <el-button :disabled="!hasCustomWidths" @click="resetColumnWidths">
          恢复默认列宽
        </el-button>
        <el-button type="primary" @click="open()">新增配置</el-button>
      </div>
    </div>

    <el-table
      :key="tableKey"
      :data="items"
      v-loading="loading"
      border
      class="compact-table"
      @header-dragend="handleColumnResize"
    >
      <el-table-column
        column-key="name"
        prop="name"
        label="配置名称"
        :width="columnWidth('name', 180)"
        fixed="left"
      />
      <el-table-column
        column-key="url"
        label="查询 URL"
        :width="columnWidth('url')"
        min-width="260"
        show-overflow-tooltip
      >
        <template #default="{ row }">{{ displayUrl(row.url) }}</template>
      </el-table-column>
      <el-table-column
        column-key="accounts"
        label="监控账号"
        :width="columnWidth('accounts', 220)"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <span>{{ accountSummary(row.account_ids) }}</span>
        </template>
      </el-table-column>
      <el-table-column
        column-key="multiplierDivisor"
        label="倍率换算"
        :width="columnWidth('multiplierDivisor', 105)"
      >
        <template #default="{ row }">返回值 ÷{{ row.multiplier_divisor }}</template>
      </el-table-column>
      <el-table-column
        column-key="refreshToken"
        label="自动刷新"
        :width="columnWidth('refreshToken', 105)"
      >
        <template #default="{ row }">
          <el-tag :type="row.has_refresh_token ? 'success' : 'info'" size="small">
            {{ row.has_refresh_token ? '已配置' : '未配置' }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column column-key="enabled" label="状态" :width="columnWidth('enabled', 90)">
        <template #default="{ row }">
          <el-switch
            :model-value="row.enabled"
            :loading="togglingIds.has(row.id)"
            @change="toggle(row)"
          />
        </template>
      </el-table-column>
      <el-table-column
        column-key="lastCheck"
        label="最近检查"
        :width="columnWidth('lastCheck', 175)"
      >
        <template #default="{ row }">{{ formatTime(row.last_finished_at) }}</template>
      </el-table-column>
      <el-table-column
        column-key="lastResult"
        label="最近结果"
        :width="columnWidth('lastResult', 180)"
      >
        <template #default="{ row }">
          <el-tag v-if="isChecking(row)" type="warning">正在检查</el-tag>
          <template v-else-if="row.last_run">
            <el-tag :type="summaryType(row.last_run)">{{ summaryLabel(row.last_run) }}</el-tag>
          </template>
          <span v-else class="muted">尚未检查</span>
        </template>
      </el-table-column>
      <el-table-column
        column-key="actions"
        label="操作"
        :width="columnWidth('actions', 220)"
        fixed="right"
      >
        <template #default="{ row }">
          <el-button link type="primary" @click="open(row)">编辑</el-button>
          <el-button
            link
            type="primary"
            :loading="checkingIds.has(row.id)"
            :disabled="!row.enabled || isChecking(row)"
            @click="check(row)"
          >
            立即检查
          </el-button>
          <el-button link type="primary" @click="openLogs(row)">日志</el-button>
        </template>
      </el-table-column>
    </el-table>

    <MultiplierMonitorFormDialog
      v-model:visible="formVisible"
      :config="editingConfig"
      :options="accountOptions"
      :used-account-ids="usedAccountIds"
      :saving="saving"
      @save="save"
    />
    <MultiplierMonitorLogDialog v-model:visible="logVisible" :config="logConfig" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { ElMessage } from 'element-plus';
import {
  multiplierMonitorApi,
  type MultiplierMonitorAccountOption,
  type MultiplierMonitorConfig,
  type MultiplierMonitorCreate,
  type MultiplierMonitorRunSummary,
  type MultiplierMonitorUpdate,
} from '../../api/multiplierMonitor';
import MultiplierMonitorFormDialog from '../../components/multiplier-monitor/MultiplierMonitorFormDialog.vue';
import MultiplierMonitorLogDialog from '../../components/multiplier-monitor/MultiplierMonitorLogDialog.vue';
import { useTableColumnWidths } from '../../composables/useTableColumnWidths';

const { tableKey, hasCustomWidths, columnWidth, handleColumnResize, resetColumnWidths } =
  useTableColumnWidths('multiplier-monitor');
const items = ref<MultiplierMonitorConfig[]>([]);
const accountOptions = ref<MultiplierMonitorAccountOption[]>([]);
const loading = ref(false);
const saving = ref(false);
const formVisible = ref(false);
const editingConfig = ref<MultiplierMonitorConfig>();
const logVisible = ref(false);
const logConfig = ref<MultiplierMonitorConfig>();
const checkingIds = ref(new Set<string>());
const togglingIds = ref(new Set<string>());
const usedAccountIds = computed(() => [
  ...new Set(
    items.value
      .filter((config) => config.id !== editingConfig.value?.id)
      .flatMap((config) => config.account_ids),
  ),
]);

const load = async () => {
  loading.value = true;
  try {
    items.value = (await multiplierMonitorApi.list()).items;
  } catch (error) {
    ElMessage.error(String(error));
  } finally {
    loading.value = false;
  }
};

const loadAccountOptions = async () => {
  try {
    accountOptions.value = (await multiplierMonitorApi.accountOptions()).items;
  } catch (error) {
    ElMessage.error(String(error));
  }
};

const open = async (config?: MultiplierMonitorConfig) => {
  if (!accountOptions.value.length) await loadAccountOptions();
  editingConfig.value = config;
  formVisible.value = true;
};

const save = async (payload: MultiplierMonitorCreate | MultiplierMonitorUpdate) => {
  saving.value = true;
  try {
    if (editingConfig.value) {
      await multiplierMonitorApi.update(editingConfig.value.id, payload as MultiplierMonitorUpdate);
    } else {
      await multiplierMonitorApi.create(payload as MultiplierMonitorCreate);
    }
    formVisible.value = false;
    await Promise.all([load(), loadAccountOptions()]);
    ElMessage.success('倍率监控配置已保存');
  } catch (error) {
    ElMessage.error(String(error));
  } finally {
    saving.value = false;
  }
};

const toggle = async (config: MultiplierMonitorConfig) => {
  if (togglingIds.value.has(config.id)) return;
  togglingIds.value = new Set(togglingIds.value).add(config.id);
  try {
    await multiplierMonitorApi.toggle(config.id);
    await load();
  } catch (error) {
    ElMessage.error(String(error));
  } finally {
    const next = new Set(togglingIds.value);
    next.delete(config.id);
    togglingIds.value = next;
  }
};

const check = async (config: MultiplierMonitorConfig) => {
  if (checkingIds.value.has(config.id)) {
    ElMessage.info('该配置正在检查，请稍候');
    return;
  }
  checkingIds.value = new Set(checkingIds.value).add(config.id);
  try {
    const result = await multiplierMonitorApi.check(config.id);
    if (result.failed) {
      ElMessage.warning('检查失败，未更新账号倍率，请查看日志');
    } else if (result.issues) {
      ElMessage.warning(
        `检查完成：更新 ${result.updated} 个，未变化 ${result.unchanged} 个，异常 ${result.issues} 个`,
      );
    } else {
      ElMessage.success(`检查完成：更新 ${result.updated} 个，未变化 ${result.unchanged} 个`);
    }
    await load();
  } catch (error) {
    ElMessage.error(String(error));
  } finally {
    const next = new Set(checkingIds.value);
    next.delete(config.id);
    checkingIds.value = next;
  }
};

const openLogs = (config: MultiplierMonitorConfig) => {
  logConfig.value = config;
  logVisible.value = true;
};

const isChecking = (config: MultiplierMonitorConfig) =>
  config.running || checkingIds.value.has(config.id);

const accountSummary = (ids: string[]) => {
  const names = ids
    .map((id) => accountOptions.value.find((account) => account.id === id)?.name ?? '账号已删除')
    .slice(0, 3);
  const suffix = ids.length > 3 ? ` 等 ${ids.length} 个` : `（${ids.length} 个）`;
  return names.length ? `${names.join('、')}${suffix}` : '-';
};

const displayUrl = (value: string) => {
  try {
    const url = new URL(value);
    return `${url.origin}${url.pathname}${url.search ? '?***' : ''}`;
  } catch {
    return value;
  }
};

const formatTime = (value?: string | null) =>
  value ? new Date(value).toLocaleString('zh-CN', { hour12: false }) : '-';

const summaryType = (summary: MultiplierMonitorRunSummary) => {
  if (summary.failed) return 'danger';
  if (summary.issues) return 'warning';
  if (summary.updated) return 'success';
  return 'info';
};

const summaryLabel = (summary: MultiplierMonitorRunSummary) => {
  if (summary.failed) return '检查失败';
  if (summary.issues) return `异常 ${summary.issues} 个`;
  if (summary.updated) return `已更新 ${summary.updated} 个`;
  return '倍率无变化';
};

onMounted(async () => {
  await Promise.all([load(), loadAccountOptions()]);
});
</script>

<style scoped>
.page-description {
  margin-top: 5px;
  color: #7b8494;
  font-size: 13px;
}

.page-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}
</style>
