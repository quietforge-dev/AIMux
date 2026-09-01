<template>
  <div class="page">
    <div class="page-toolbar">
      <h2 class="page-title">账号管理</h2>
      <div>
        <el-button :loading="store.loading" @click="load">刷新</el-button>
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
      <el-table-column label="操作" width="245" fixed="right">
        <template #default="{ row }">
          <el-button link type="primary" @click="open(row)">编辑</el-button>
          <el-button link type="primary" title="复制账号" @click="copy(row)">
            <el-icon><CopyDocument /></el-icon>复制
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
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { CopyDocument } from '@element-plus/icons-vue';
import { accountsApi, type Account } from '../../api/accounts';
import { useAccountsStore } from '../../stores/accounts';
import { useModelsStore } from '../../stores/models';
import AccountFormDialog from '../../components/accounts/AccountFormDialog.vue';
import AccountTestDialog from '../../components/accounts/AccountTestDialog.vue';

const store = useAccountsStore();
const models = useModelsStore();
const dialog = ref(false);
const editingAccount = ref<Account>();
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

const copy = async (row: Account) => {
  if (!models.items.length) await models.load();
  editingAccount.value = { ...row, id: '' };
  dialog.value = true;
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
