<template>
  <div class="page">
    <div class="page-toolbar">
      <h2 class="page-title">账号管理</h2>
      <div>
        <el-button :loading="store.loading" @click="load">刷新</el-button>
        <el-button @click="openImport"
          ><el-icon><Upload /></el-icon>从 JSON 导入</el-button
        >
        <el-button type="primary" @click="open()">新增账号</el-button>
      </div>
    </div>

    <div class="account-filters">
      <el-input
        v-model="nameFilter"
        clearable
        placeholder="按名称搜索"
        class="account-name-filter"
        @keyup.enter="load"
      />
      <el-select v-model="typeFilter" clearable placeholder="全部类型" class="account-type-filter">
        <el-option label="OpenAI" value="openai" />
        <el-option label="Anthropic" value="anthropic" />
      </el-select>
      <el-button type="primary" :loading="store.loading" @click="load">查询</el-button>
    </div>

    <el-tabs v-model="statusFilter" class="account-status-tabs" @tab-change="() => load()">
      <el-tab-pane label="全部" name="all" />
      <el-tab-pane label="启用" name="active" />
      <el-tab-pane label="禁用" name="disabled" />
    </el-tabs>

    <el-table :data="store.items" v-loading="store.loading" class="compact-table" border stripe>
      <el-table-column prop="name" label="名称" min-width="170" />
      <el-table-column prop="multiplier" label="倍率" width="90">
        <template #default="{ row }">
          <span>{{ Number(row.multiplier).toFixed(2) }}</span>
        </template>
      </el-table-column>
      <el-table-column prop="type" label="类型" width="100" />
      <el-table-column prop="status" label="状态" width="100">
        <template #default="{ row }">
          <el-button link :type="row.status === 'active' ? 'success' : 'info'" @click="toggle(row)">
            {{ row.status === 'active' ? '启用' : '禁用' }}
          </el-button>
        </template>
      </el-table-column>
      <el-table-column prop="priority" label="优先级" width="130">
        <template #default="{ row }">
          <el-input-number
            v-model="row.priority"
            :min="0"
            :max="9"
            size="small"
            @change="priority(row)"
          />
        </template>
      </el-table-column>
      <el-table-column label="平均耗时" width="105">
        <template #default="{ row }">
          <span
            :class="(row.monitor_average_duration_ms ?? 0) > SLOW_DURATION_MS ? 'warning-text' : ''"
          >
            {{ formatDuration(row.monitor_average_duration_ms) }}
          </span>
        </template>
      </el-table-column>
      <el-table-column prop="test_default_model" label="测试默认模型" min-width="150" />
      <el-table-column label="操作" width="330" fixed="right">
        <template #default="{ row }">
          <el-button link type="primary" @click="open(row)">编辑</el-button>
          <el-button link type="primary" title="复制账号" @click="copy(row)">
            <el-icon><CopyDocument /></el-icon>复制
          </el-button>
          <el-button link type="primary" title="复制为 JSON" @click="copyJson(row)">
            <el-icon><CopyDocument /></el-icon>JSON
          </el-button>
          <el-button link type="warning" @click="test(row)">测试</el-button>
          <el-button link type="danger" @click="remove(row)">删除</el-button>
        </template>
      </el-table-column>
    </el-table>

    <AccountFormDialog
      v-model="dialog"
      :account="editingAccount"
      :models="models.items"
      @save="save"
    />
    <AccountTestDialog
      v-if="testAccount"
      v-model="testDialog"
      :account="testAccount"
      :models="testModels"
      @finished="load"
    />
    <el-dialog
      v-model="importDialog"
      title="从 JSON 导入账号"
      width="620px"
      destroy-on-close
      @closed="importJson = ''"
    >
      <el-input
        v-model="importJson"
        type="textarea"
        :rows="14"
        autofocus
        placeholder="请粘贴由 AIMux 导出的账号 JSON"
      />
      <template #footer>
        <el-button @click="importDialog = false">取消</el-button>
        <el-button type="primary" @click="importFromJson">导入到新增表单</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { CopyDocument, Upload } from '@element-plus/icons-vue';
import { accountsApi, type Account } from '../../api/accounts';
import { useAccountsStore } from '../../stores/accounts';
import { useModelsStore } from '../../stores/models';
import AccountFormDialog from '../../components/accounts/AccountFormDialog.vue';
import AccountTestDialog from '../../components/accounts/AccountTestDialog.vue';

const store = useAccountsStore();
const models = useModelsStore();
const dialog = ref(false);
const editingAccount = ref<Account>();
const importDialog = ref(false);
const importJson = ref('');
const testDialog = ref(false);
const testAccount = ref<Account>();
const statusFilter = ref<'all' | Account['status']>('active');
const nameFilter = ref('');
const typeFilter = ref<Account['type']>();
const testModels = computed(() => {
  const account = testAccount.value;
  if (!account) return [];
  const supported = [...new Set(account.supported_models?.filter(Boolean) ?? [])];
  return supported.length ? supported : models.byType(account.type).map((model) => model.name);
});
const SLOW_DURATION_MS = 20_000;

const load = async () => {
  const status = statusFilter.value === 'all' ? undefined : statusFilter.value;
  await Promise.all([store.load(status, nameFilter.value, typeFilter.value), models.load()]);
};

const open = async (row?: Account) => {
  if (!models.items.length) await models.load();
  editingAccount.value = row;
  dialog.value = true;
};

type AccountTransfer = {
  format: 'aimux-account';
  version: 1;
  name: string;
  type: Account['type'];
  base_url: string;
  api_key: string;
  priority: number;
  multiplier: number;
  supported_models: string[];
  test_default_model: string;
  model_mappings: Record<string, string>;
  tags: string[];
  notes: string;
};

const toTransfer = (row: Account): AccountTransfer => ({
  format: 'aimux-account',
  version: 1,
  name: row.name,
  type: row.type,
  base_url: row.base_url,
  api_key: row.api_key,
  priority: row.priority,
  multiplier: row.multiplier,
  supported_models: row.supported_models ?? [],
  test_default_model: row.test_default_model ?? '',
  model_mappings: row.model_mappings ?? {},
  tags: row.tags ?? [],
  notes: row.notes ?? '',
});

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const strings = (value: unknown, field: string): string[] => {
  if (!Array.isArray(value) || value.some((item) => typeof item !== 'string')) {
    throw new Error(`${field} 必须是字符串数组`);
  }
  return value.map((item) => item.trim()).filter(Boolean);
};

const requiredString = (value: unknown, field: string): string => {
  if (typeof value !== 'string' || !value.trim()) throw new Error(`${field} 不能为空`);
  return value.trim();
};

const parseTransfer = (raw: string): AccountTransfer => {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    throw new Error('JSON 格式不正确');
  }
  if (!isRecord(value) || value.format !== 'aimux-account' || value.version !== 1) {
    throw new Error('不是有效的 AIMux 账号 JSON');
  }
  const type = value.type;
  if (type !== 'openai' && type !== 'anthropic')
    throw new Error('账号类型必须是 openai 或 anthropic');
  const name = requiredString(value.name, 'name');
  const baseUrl = requiredString(value.base_url, 'base_url');
  const apiKey = requiredString(value.api_key, 'api_key');
  if (
    typeof value.priority !== 'number' ||
    !Number.isInteger(value.priority) ||
    value.priority < 0 ||
    value.priority > 9
  ) {
    throw new Error('优先级必须是 0 到 9 的整数');
  }
  if (
    typeof value.multiplier !== 'number' ||
    !Number.isFinite(value.multiplier) ||
    value.multiplier < 0.01 ||
    value.multiplier > 0.3
  ) {
    throw new Error('倍率必须在 0.01 到 0.30 之间');
  }
  const supportedModels = strings(value.supported_models, 'supported_models');
  if (!supportedModels.length) throw new Error('supported_models 不能为空');
  if (typeof value.test_default_model !== 'string' || !value.test_default_model.trim()) {
    throw new Error('test_default_model 不能为空');
  }
  if (!supportedModels.includes(value.test_default_model.trim())) {
    throw new Error('test_default_model 必须包含在 supported_models 中');
  }
  if (!isRecord(value.model_mappings)) throw new Error('model_mappings 必须是对象');
  const modelMappings: Record<string, string> = {};
  for (const [client, upstream] of Object.entries(value.model_mappings)) {
    if (!client.trim() || typeof upstream !== 'string' || !upstream.trim()) {
      throw new Error('model_mappings 中的模型名称不能为空');
    }
    if (!supportedModels.includes(client.trim())) {
      throw new Error('模型映射的客户端模型必须在 supported_models 中');
    }
    if (client.trim() === upstream.trim()) throw new Error('模型映射的两端模型不能相同');
    modelMappings[client.trim()] = upstream.trim();
  }
  return {
    format: 'aimux-account',
    version: 1,
    name,
    type,
    base_url: baseUrl,
    api_key: apiKey,
    priority: value.priority,
    multiplier: value.multiplier,
    supported_models: supportedModels,
    test_default_model: value.test_default_model.trim(),
    model_mappings: modelMappings,
    tags: strings(value.tags, 'tags'),
    notes: typeof value.notes === 'string' ? value.notes : '',
  };
};

const openImport = () => {
  importJson.value = '';
  importDialog.value = true;
};

const importFromJson = async () => {
  try {
    const transfer = parseTransfer(importJson.value);
    if (!models.items.length) await models.load();
    editingAccount.value = {
      id: '',
      name: transfer.name,
      type: transfer.type,
      base_url: transfer.base_url,
      api_key: transfer.api_key,
      status: 'active',
      priority: transfer.priority,
      multiplier: transfer.multiplier,
      supported_models: transfer.supported_models,
      test_default_model: transfer.test_default_model,
      model_mappings: transfer.model_mappings,
      tags: transfer.tags,
      notes: transfer.notes,
      total_requests: 0,
      total_tokens: 0,
    };
    importDialog.value = false;
    dialog.value = true;
  } catch (error) {
    ElMessage.error(`导入失败：${String(error)}`);
  }
};

const copy = async (row: Account) => {
  if (!models.items.length) await models.load();
  editingAccount.value = { ...row, id: '' };
  dialog.value = true;
};

const copyJson = async (row: Account) => {
  try {
    await navigator.clipboard.writeText(JSON.stringify(toTransfer(row), null, 2));
    ElMessage.success('账号 JSON 已复制');
  } catch (error) {
    ElMessage.error(`复制失败：${String(error)}`);
  }
};

const save = async (payload: Record<string, unknown>) => {
  try {
    if (editingAccount.value?.id) await accountsApi.update(editingAccount.value.id, payload);
    else await accountsApi.create(payload);
    dialog.value = false;
    await load();
    ElMessage.success('保存成功');
  } catch (error) {
    ElMessage.error(String(error));
  }
};

const toggle = async (row: Account) => {
  await accountsApi.toggle(row.id);
  await load();
};

const priority = async (row: Account) => {
  await accountsApi.priority(row.id, row.priority);
  await load();
};

const remove = async (row: Account) => {
  await ElMessageBox.confirm(`确认删除 ${row.name}？`, '提示');
  await accountsApi.remove(row.id);
  await load();
};

const test = async (row: Account) => {
  if (!models.items.length) await models.load();
  testAccount.value = row;
  testDialog.value = true;
};

const formatDuration = (duration?: number | null) =>
  duration == null ? '-' : `${(duration / 1000).toFixed(2)} 秒`;

onMounted(load);
</script>

<style scoped>
.account-filters {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
}

.account-name-filter {
  width: 240px;
}

.account-type-filter {
  width: 150px;
}

.account-status-tabs {
  margin-bottom: 12px;
}
</style>
