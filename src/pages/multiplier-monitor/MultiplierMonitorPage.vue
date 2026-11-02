<template>
  <div class="page">
    <div class="page-toolbar">
      <div>
        <h2 class="page-title">倍率监控</h2>
        <div class="page-description">
          按账号检查 Sub2API 或 New API 倍率，同步到本地倍率默认关闭。
        </div>
      </div>
      <div class="page-actions">
        <el-button :loading="loading" @click="load">刷新</el-button>
        <el-button :disabled="!hasCustomWidths" @click="resetColumnWidths">恢复默认列宽</el-button>
      </div>
    </div>
    <el-tabs v-model="statusFilter" class="account-status-tabs">
      <el-tab-pane label="全部" name="all" />
      <el-tab-pane label="启用" name="active" />
      <el-tab-pane label="禁用" name="disabled" />
    </el-tabs>
    <el-table
      :key="tableKey"
      :data="filteredItems"
      v-loading="loading"
      border
      class="compact-table"
      @header-dragend="handleColumnResize"
    >
      <el-table-column
        column-key="account"
        prop="account_name"
        label="账号"
        :width="columnWidth('account', 190)"
        fixed="left"
        show-overflow-tooltip
      />
      <el-table-column column-key="protocol" label="协议" :width="columnWidth('protocol', 105)">
        <template #default="{ row }">{{ row.account_type }}</template>
      </el-table-column>
      <el-table-column column-key="platform" label="上游平台" :width="columnWidth('platform', 120)">
        <template #default="{ row }">{{
          row.configured ? (row.platform_type === 'newapi' ? 'New API' : 'Sub2API') : '未配置'
        }}</template>
      </el-table-column>
      <el-table-column
        column-key="group"
        label="New API 分组"
        :width="columnWidth('group', 150)"
        show-overflow-tooltip
      >
        <template #default="{ row }">{{
          row.platform_type === 'newapi' ? row.new_api_group || '-' : '-'
        }}</template>
      </el-table-column>
      <el-table-column column-key="current" label="本地倍率" :width="columnWidth('current', 100)">
        <template #default="{ row }">{{ row.account_multiplier.toFixed(2) }}</template>
      </el-table-column>
      <el-table-column column-key="remote" label="上游倍率" :width="columnWidth('remote', 100)">
        <template #default="{ row }">
          {{
            row.last_remote_multiplier == null ? '-' : Number(row.last_remote_multiplier).toFixed(2)
          }}
        </template>
      </el-table-column>
      <el-table-column column-key="sync" label="同步倍率" :width="columnWidth('sync', 105)">
        <template #default="{ row }"
          ><el-tag
            v-if="row.configured"
            :type="row.sync_upstream_multiplier ? 'warning' : 'info'"
            size="small"
            >{{ row.sync_upstream_multiplier ? '开启' : '关闭' }}</el-tag
          ><span v-else class="muted">-</span></template
        >
      </el-table-column>
      <el-table-column column-key="enabled" label="监控" :width="columnWidth('enabled', 100)">
        <template #default="{ row }"
          ><el-switch
            :model-value="row.enabled"
            :loading="busyIds.has(row.account_id)"
            @change="toggle(row)"
        /></template>
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
        min-width="150"
        show-overflow-tooltip
      >
        <template #default="{ row }">
          <el-tag v-if="row.running || busyIds.has(row.account_id)" type="warning">正在检查</el-tag>
          <el-tag v-else-if="row.last_error" type="danger">{{ row.last_error }}</el-tag>
          <el-tag v-else-if="row.last_result === 'observed'" type="warning"
            >发现差异（未同步）</el-tag
          >
          <el-tag v-else-if="row.last_success_at" type="success">检查成功</el-tag>
          <span v-else class="muted">尚未检查</span>
        </template>
      </el-table-column>
      <el-table-column
        column-key="actions"
        label="操作"
        :width="columnWidth('actions', 210)"
        fixed="right"
      >
        <template #default="{ row }">
          <el-button link type="primary" @click="edit(row)">配置</el-button>
          <el-button
            link
            type="primary"
            :disabled="!row.enabled || busyIds.has(row.account_id)"
            :loading="checkingIds.has(row.account_id)"
            @click="check(row)"
            >立即检查</el-button
          >
          <el-button link type="primary" @click="openLogs(row)">日志</el-button>
        </template>
      </el-table-column>
    </el-table>
    <el-dialog
      v-model="formVisible"
      title="倍率监控配置"
      width="560px"
      :close-on-click-modal="false"
    >
      <el-form v-if="editing" label-width="130px">
        <el-form-item label="账号"
          ><span>{{ editing.account_name }}</span></el-form-item
        >
        <el-form-item label="账号协议"
          ><span>{{ editing.account_type }}</span></el-form-item
        >
        <el-form-item label="上游平台">
          <el-select v-model="form.platform_type" @change="loadGroups">
            <el-option label="Sub2API" value="sub2api" />
            <el-option label="New API" value="newapi" />
          </el-select>
        </el-form-item>
        <el-form-item v-if="form.platform_type === 'newapi'" label="New API 分组">
          <el-select
            v-model="form.new_api_group"
            :loading="groupsLoading"
            placeholder="选择上游分组"
            filterable
          >
            <el-option
              v-for="group in groups"
              :key="group.name"
              :label="`${group.name} (${group.rate})`"
              :value="group.name"
            />
          </el-select>
        </el-form-item>
        <el-form-item label="倍率除数">
          <el-input-number v-model="form.multiplier_divisor" :min="1" :max="10000" :precision="0" />
        </el-form-item>
        <el-form-item label="同步上游倍率"
          ><el-switch v-model="form.sync_upstream_multiplier"
        /></el-form-item>
        <el-form-item label="说明"
          ><span class="hint">关闭时只记录上游倍率，不修改账号本地倍率。</span></el-form-item
        >
      </el-form>
      <template #footer>
        <el-button @click="formVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="save">保存</el-button>
      </template>
    </el-dialog>
    <MultiplierMonitorLogDialog v-model:visible="logVisible" :config="logConfig" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { ElMessage } from 'element-plus';
import {
  multiplierMonitorApi,
  type MultiplierMonitorConfig,
  type NewApiGroup,
} from '../../api/multiplierMonitor';
import MultiplierMonitorLogDialog from '../../components/multiplier-monitor/MultiplierMonitorLogDialog.vue';
import { useTableColumnWidths } from '../../composables/useTableColumnWidths';

const { tableKey, hasCustomWidths, columnWidth, handleColumnResize, resetColumnWidths } =
  useTableColumnWidths('multiplier-monitor');
const items = ref<MultiplierMonitorConfig[]>([]);
const statusFilter = ref<'all' | 'active' | 'disabled'>('active');
const filteredItems = computed(() =>
  statusFilter.value === 'all'
    ? items.value
    : items.value.filter((item) => item.account_status === statusFilter.value),
);
const loading = ref(false);
const saving = ref(false);
const formVisible = ref(false);
const editing = ref<MultiplierMonitorConfig>();
const form = reactive({
  platform_type: 'sub2api' as 'sub2api' | 'newapi',
  new_api_group: '',
  multiplier_divisor: 1,
  sync_upstream_multiplier: false,
});
const groups = ref<NewApiGroup[]>([]);
const groupsLoading = ref(false);
const busyIds = ref(new Set<string>());
const checkingIds = ref(new Set<string>());
const logVisible = ref(false);
const logConfig = ref<MultiplierMonitorConfig>();

const load = async () => {
  loading.value = true;
  try {
    items.value = (await multiplierMonitorApi.list()).items;
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    loading.value = false;
  }
};
const loadGroups = async () => {
  if (!editing.value || form.platform_type !== 'newapi') {
    groups.value = [];
    return;
  }
  groupsLoading.value = true;
  try {
    groups.value = (await multiplierMonitorApi.newApiGroups(editing.value.account_id)).items;
    if (!groups.value.some((g) => g.name === form.new_api_group)) form.new_api_group = '';
  } catch (e) {
    groups.value = [];
    ElMessage.error(`读取 New API 分组失败：${String(e)}`);
  } finally {
    groupsLoading.value = false;
  }
};
const edit = async (row: MultiplierMonitorConfig) => {
  editing.value = row;
  Object.assign(form, {
    platform_type: row.platform_type,
    new_api_group: row.new_api_group ?? '',
    multiplier_divisor: row.multiplier_divisor,
    sync_upstream_multiplier: row.sync_upstream_multiplier,
  });
  formVisible.value = true;
  if (form.platform_type === 'newapi') await loadGroups();
};
const save = async () => {
  if (!editing.value) return;
  if (form.platform_type === 'newapi' && !form.new_api_group) {
    ElMessage.warning('请选择 New API 分组');
    return;
  }
  saving.value = true;
  try {
    await multiplierMonitorApi.update(editing.value.account_id, {
      ...form,
      new_api_group: form.platform_type === 'newapi' ? form.new_api_group : null,
      enabled: editing.value.enabled,
    });
    formVisible.value = false;
    await load();
    ElMessage.success('倍率监控配置已保存');
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    saving.value = false;
  }
};
const toggle = async (row: MultiplierMonitorConfig) => {
  const id = row.account_id;
  if (busyIds.value.has(id)) return;
  if (!row.configured) {
    ElMessage.info('请先配置上游平台');
    await edit(row);
    return;
  }
  busyIds.value = new Set(busyIds.value).add(id);
  try {
    await multiplierMonitorApi.toggle(id);
    await load();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    const next = new Set(busyIds.value);
    next.delete(id);
    busyIds.value = next;
  }
};
const check = async (row: MultiplierMonitorConfig) => {
  const id = row.account_id;
  if (checkingIds.value.has(id)) return;
  checkingIds.value = new Set(checkingIds.value).add(id);
  try {
    const r = await multiplierMonitorApi.check(id);
    if (r.failed) ElMessage.warning('检查失败，未更新账号倍率');
    else if (r.updated) ElMessage.success('已同步上游倍率');
    else ElMessage.success('检查完成，未修改账号倍率');
    await load();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    const next = new Set(checkingIds.value);
    next.delete(id);
    checkingIds.value = next;
  }
};
const openLogs = (row: MultiplierMonitorConfig) => {
  logConfig.value = row;
  logVisible.value = true;
};
const formatTime = (value?: string | null) =>
  value ? new Date(value).toLocaleString('zh-CN', { hour12: false }) : '-';
onMounted(load);
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
.account-status-tabs {
  margin-bottom: 10px;
}
.hint {
  color: #7b8494;
  font-size: 12px;
}
</style>
