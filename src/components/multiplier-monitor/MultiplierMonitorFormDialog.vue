<template>
  <el-dialog
    v-model="visible"
    :title="editing ? '编辑倍率监控' : '新增倍率监控'"
    width="860px"
    destroy-on-close
    :close-on-click-modal="false"
  >
    <el-form ref="formRef" :model="form" :rules="rules" label-width="92px">
      <el-form-item label="配置名称" prop="name">
        <el-input v-model="form.name" maxlength="100" placeholder="例如：主站倍率同步" />
      </el-form-item>
      <el-form-item label="查询 URL" prop="url">
        <el-input
          v-model="form.url"
          placeholder="https://example.com/api/keys"
          autocomplete="off"
        />
      </el-form-item>
      <el-form-item label="查询 Token" prop="token">
        <el-input
          v-model="form.token"
          type="password"
          show-password
          autocomplete="new-password"
          :placeholder="editing ? '留空表示保留原 Token' : '请输入接口访问 Token'"
        />
        <div class="field-hint">可以填写裸 Token 或 Bearer Token，保存后不会回显明文。</div>
      </el-form-item>
      <el-form-item label="启用监控">
        <el-switch v-model="form.enabled" />
        <span class="field-hint inline">启用后每小时检查一次，新配置会尽快执行首次检查。</span>
      </el-form-item>
      <el-form-item label="监控账号" prop="account_ids">
        <div class="account-transfer-wrap">
          <el-transfer
            v-model="form.account_ids"
            filterable
            :filter-method="filterAccount"
            :data="transferOptions"
            :titles="['待选账号', '已选账号']"
            :button-texts="['移除', '选择']"
            filter-placeholder="搜索账号"
            class="account-transfer"
          >
            <template #default="{ option }">
              <div class="account-option">
                <span class="account-name">{{ option.name }}</span>
                <span class="account-meta">
                  {{ option.type }} · {{ Number(option.multiplier).toFixed(2) }}
                  <span v-if="option.status !== 'active'" class="disabled-label">· 已停用</span>
                </span>
              </div>
            </template>
          </el-transfer>
          <div class="transfer-summary">
            可选 {{ transferOptions.length }} 个账号，已选择 {{ form.account_ids.length }} 个
            <span v-if="excludedAccountCount"
              >，已排除其他配置中的 {{ excludedAccountCount }} 个</span
            >
          </div>
        </div>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button :disabled="saving" @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="saving" @click="submit">保存</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue';
import type { FormInstance, FormRules } from 'element-plus';
import type {
  MultiplierMonitorAccountOption,
  MultiplierMonitorConfig,
  MultiplierMonitorCreate,
  MultiplierMonitorUpdate,
} from '../../api/multiplierMonitor';

type TransferOption = MultiplierMonitorAccountOption & {
  key: string;
  label: string;
};

type FormModel = {
  name: string;
  url: string;
  token: string;
  account_ids: string[];
  enabled: boolean;
};

const visible = defineModel<boolean>('visible', { required: true });
const props = defineProps<{
  config?: MultiplierMonitorConfig;
  options: MultiplierMonitorAccountOption[];
  usedAccountIds: string[];
  saving: boolean;
}>();
const emit = defineEmits<{
  save: [payload: MultiplierMonitorCreate | MultiplierMonitorUpdate];
}>();

const formRef = ref<FormInstance>();
const editing = computed(() => Boolean(props.config));
const createForm = (): FormModel => ({
  name: '',
  url: '',
  token: '',
  account_ids: [],
  enabled: true,
});
const form = reactive<FormModel>(createForm());
const usedAccountIdSet = computed(() => new Set(props.usedAccountIds));
const currentAccountIdSet = computed(() => new Set(props.config?.account_ids ?? []));
const availableOptions = computed(() =>
  props.options.filter(
    (account) =>
      !usedAccountIdSet.value.has(account.id) || currentAccountIdSet.value.has(account.id),
  ),
);
const excludedAccountCount = computed(() => props.options.length - availableOptions.value.length);

const transferOptions = computed<TransferOption[]>(() =>
  [
    ...availableOptions.value,
    ...(props.config?.account_ids ?? [])
      .filter((id) => !availableOptions.value.some((account) => account.id === id))
      .map((id): MultiplierMonitorAccountOption => ({
        id,
        name: '账号已删除',
        type: 'openai',
        status: 'disabled',
        multiplier: 0,
      })),
  ].map((account) => ({
    ...account,
    key: account.id,
    label: `${account.name} ${account.type} ${account.status} ${account.multiplier.toFixed(2)}`,
  })),
);

const rules: FormRules<FormModel> = {
  name: [{ required: true, message: '请输入配置名称', trigger: 'blur' }],
  url: [
    { required: true, message: '请输入查询 URL', trigger: 'blur' },
    {
      validator: (_rule, value: string, callback) => {
        try {
          const url = new URL(value);
          if (!['http:', 'https:'].includes(url.protocol)) throw new Error();
          callback();
        } catch {
          callback(new Error('请输入有效的 HTTP 或 HTTPS URL'));
        }
      },
      trigger: 'blur',
    },
  ],
  token: [
    {
      validator: (_rule, value: string, callback) => {
        if (!editing.value && !value.trim()) callback(new Error('请输入查询 Token'));
        else callback();
      },
      trigger: 'blur',
    },
  ],
  account_ids: [
    {
      validator: (_rule, value: string[], callback) => {
        if (!value.length) callback(new Error('请至少选择一个账号'));
        else callback();
      },
      trigger: 'change',
    },
  ],
};

const filterAccount = (query: string, item: TransferOption) =>
  item.label.toLowerCase().includes(query.trim().toLowerCase());

watch(visible, (open) => {
  if (!open) return;
  Object.assign(
    form,
    props.config
      ? {
          name: props.config.name,
          url: props.config.url,
          token: '',
          account_ids: [...props.config.account_ids],
          enabled: props.config.enabled,
        }
      : createForm(),
  );
  nextTick(() => formRef.value?.clearValidate());
});

const submit = async () => {
  const valid = await formRef.value?.validate().catch(() => false);
  if (!valid) return;
  const common = {
    name: form.name.trim(),
    url: form.url.trim(),
    account_ids: [...form.account_ids],
    enabled: form.enabled,
  };
  if (editing.value) {
    emit('save', { ...common, token: form.token.trim() || undefined });
  } else {
    emit('save', { ...common, token: form.token.trim() });
  }
};
</script>

<style scoped>
.field-hint {
  width: 100%;
  color: #7b8494;
  font-size: 12px;
  line-height: 20px;
}

.field-hint.inline {
  width: auto;
  margin-left: 10px;
}

.account-transfer-wrap {
  min-width: 0;
  width: 100%;
}

.account-transfer {
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  min-width: 0;
  width: 100%;
}

.account-transfer :deep(.el-transfer-panel) {
  flex: 1 1 0;
  min-width: 0;
  width: 0;
}

.account-transfer :deep(.el-transfer__buttons) {
  display: flex;
  flex: 0 0 104px;
  flex-direction: column;
  gap: 10px;
  align-items: stretch;
  padding: 0 12px;
}

.account-transfer :deep(.el-transfer__button) {
  width: 80px;
  margin: 0;
}

.account-transfer :deep(.el-transfer__button:nth-child(2)) {
  margin: 0;
}

.account-transfer :deep(.el-transfer-panel__body),
.account-transfer :deep(.el-transfer-panel__list.is-filterable) {
  height: 310px;
}

.account-option {
  display: flex;
  min-width: 0;
  flex-direction: column;
  padding: 3px 0;
  line-height: 18px;
}

.account-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.account-meta {
  color: #8a94a6;
  font-size: 11px;
}

.disabled-label {
  color: #b54708;
}

.transfer-summary {
  margin-top: 8px;
  color: #7b8494;
  font-size: 12px;
}
</style>
