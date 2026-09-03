<template>
  <el-dialog
    v-model="visible"
    :title="editing ? '编辑账号' : '新增账号'"
    width="820px"
    top="5vh"
    destroy-on-close
    class="account-dialog"
  >
    <el-form
      ref="formRef"
      :model="form"
      :rules="rules"
      label-width="100px"
      label-position="right"
      class="account-form"
    >
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="名称" prop="name" required>
            <el-input v-model="form.name" autofocus placeholder="例如: 生产主账号" />
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="类型" prop="type" required>
            <el-select v-model="form.type" style="width: 100%">
              <el-option label="OpenAI" value="openai" />
              <el-option label="Anthropic" value="anthropic" />
            </el-select>
          </el-form-item>
        </el-col>
      </el-row>

      <el-form-item label="上游地址" prop="base_url" required>
        <el-input v-model="form.base_url" placeholder="https://api.openai.com/v1" />
      </el-form-item>

      <el-form-item label="API密钥" prop="api_key" required>
        <el-input v-model="form.api_key" show-password placeholder="请输入 API Key" />
      </el-form-item>

      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="优先级" prop="priority" required>
            <el-input-number v-model="form.priority" :min="0" :max="9" style="width: 100%" />
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="倍率" prop="multiplier" required>
            <el-input-number
              v-model="form.multiplier"
              :min="0.01"
              :max="0.3"
              :step="0.01"
              :precision="2"
              style="width: 100%"
            />
          </el-form-item>
        </el-col>
      </el-row>

      <el-form-item label="支持模型" prop="supported_models" required>
        <div class="model-picker">
          <div class="model-picker-toolbar">
            <el-checkbox
              :model-value="allModelsSelected"
              :indeterminate="someModelsSelected"
              :disabled="!availableModels.length"
              @change="toggleAllModels"
            >
              全选
            </el-checkbox>
            <span class="model-count"
              >已选 {{ selectedModelCount }} / {{ availableModels.length }}</span
            >
          </div>
          <el-checkbox-group v-model="form.supported_models" class="model-checkbox-group">
            <el-checkbox v-for="model in availableModels" :key="model.id" :label="model.name">
              {{ model.name }}
            </el-checkbox>
          </el-checkbox-group>
          <span v-if="!availableModels.length" class="muted">该协议暂无模型目录</span>
        </div>
      </el-form-item>

      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="测试模型" prop="test_default_model" required>
            <el-select
              v-model="form.test_default_model"
              clearable
              style="width: 100%"
              :disabled="!form.supported_models.length"
              placeholder="使用模型维护默认值"
            >
              <el-option
                v-for="name in form.supported_models"
                :key="name"
                :label="name"
                :value="name"
              />
            </el-select>
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="标签">
            <el-input v-model="tagsText" placeholder="多个标签用逗号分隔" />
          </el-form-item>
        </el-col>
      </el-row>

      <el-form-item label="模型映射">
        <div class="mapping-section">
          <template v-if="mappingRows.length > 0">
            <el-table
              :data="mappingRows"
              border
              size="small"
              max-height="150"
              class="mapping-table"
            >
              <el-table-column label="客户端模型" min-width="240">
                <template #default="{ row }">
                  <el-select
                    v-model="row.client_model"
                    size="small"
                    placeholder="选择客户端模型"
                    style="width: 100%"
                  >
                    <el-option
                      v-for="name in form.supported_models"
                      :key="name"
                      :label="name"
                      :value="name"
                    />
                  </el-select>
                </template>
              </el-table-column>
              <el-table-column label="上游模型" min-width="240">
                <template #default="{ row }">
                  <el-select
                    v-model="row.upstream_model"
                    size="small"
                    placeholder="选择上游模型"
                    style="width: 100%"
                  >
                    <el-option
                      v-for="name in upstreamModelOptions"
                      :key="name"
                      :label="name"
                      :value="name"
                    />
                  </el-select>
                </template>
              </el-table-column>
              <el-table-column label="操作" width="70" align="center" fixed="right">
                <template #default="{ $index }">
                  <el-button link type="danger" size="small" @click="removeMapping($index)">
                    删除
                  </el-button>
                </template>
              </el-table-column>
            </el-table>
            <el-button class="mapping-add-btn" plain size="small" @click="addMapping">
              <el-icon><Plus /></el-icon>
              新增映射
            </el-button>
          </template>
          <template v-else>
            <el-button plain size="small" @click="addMapping">
              <el-icon><Plus /></el-icon>
              添加模型映射 (默认无需配置)
            </el-button>
          </template>
        </div>
      </el-form-item>

      <el-form-item label="备注">
        <el-input v-model="form.notes" type="textarea" :rows="2" placeholder="可选备注信息" />
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" @click="save">保存</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue';
import { ElMessage, type FormInstance, type FormRules } from 'element-plus';
import { Plus } from '@element-plus/icons-vue';
import type { Account } from '../../api/accounts';
import type { CatalogModel } from '../../api/models';

type AccountForm = {
  id?: string;
  name: string;
  type: Account['type'];
  base_url: string;
  api_key: string;
  priority: number;
  multiplier: number;
  supported_models: string[];
  test_default_model: string;
  tags: string[];
  notes: string;
};

type MappingRow = {
  client_model: string;
  upstream_model: string;
};

type AccountPayload = AccountForm & {
  tags: string[];
  model_mappings: Record<string, string>;
};

const props = defineProps<{
  account?: Account;
  models: CatalogModel[];
}>();
const emit = defineEmits<{
  save: [payload: AccountPayload];
}>();
const visible = defineModel<boolean>({ required: true });

const formRef = ref<FormInstance>();
const tagsText = ref('');
const mappingRows = ref<MappingRow[]>([]);

const createForm = (): AccountForm => ({
  name: '',
  type: 'openai',
  base_url: '',
  api_key: '',
  priority: 5,
  multiplier: 0.1,
  supported_models: [],
  test_default_model: '',
  tags: [],
  notes: '',
});

const form = reactive<AccountForm>(createForm());
const editing = computed(() => Boolean(props.account?.id));
const availableModels = computed(() => {
  const models = props.models.filter((model) => model.type === form.type);
  const known = new Set(models.map((model) => model.name));
  for (const name of form.supported_models) {
    if (!known.has(name)) {
      models.push({
        id: `imported-${form.type}-${name}`,
        name,
        type: form.type,
        is_default: 0,
      });
    }
  }
  return models;
});
const selectedModelCount = computed(
  () => availableModels.value.filter((model) => form.supported_models.includes(model.name)).length,
);
const allModelsSelected = computed(
  () =>
    availableModels.value.length > 0 && selectedModelCount.value === availableModels.value.length,
);
const someModelsSelected = computed(() => selectedModelCount.value > 0 && !allModelsSelected.value);
const upstreamModelOptions = computed(() => {
  const names = availableModels.value.map((model) => model.name);
  for (const row of mappingRows.value) {
    if (row.upstream_model && !names.includes(row.upstream_model)) names.push(row.upstream_model);
  }
  return names;
});

const toggleAllModels = (checked: boolean) => {
  form.supported_models = checked ? availableModels.value.map((model) => model.name) : [];
};

const rules: FormRules = {
  name: [{ required: true, message: '请输入名称', trigger: 'blur' }],
  type: [{ required: true, message: '请选择类型', trigger: 'change' }],
  base_url: [{ required: true, message: '请输入上游地址', trigger: 'blur' }],
  api_key: [{ required: true, message: '请输入 API 密钥', trigger: 'blur' }],
  priority: [{ required: true, type: 'number', message: '请输入优先级', trigger: 'change' }],
  multiplier: [{ required: true, type: 'number', message: '请输入倍率', trigger: 'change' }],
  supported_models: [
    {
      validator: (_rule, value, callback) =>
        value?.length ? callback() : callback(new Error('请至少选择一个支持模型')),
      trigger: 'change',
    },
  ],
  test_default_model: [{ required: true, message: '请选择测试默认模型', trigger: 'change' }],
};

const reset = async () => {
  const row = props.account;
  const supportedModels = [...(row?.supported_models ?? [])];
  if (row?.test_default_model && !supportedModels.includes(row.test_default_model)) {
    supportedModels.push(row.test_default_model);
  }
  Object.assign(
    form,
    row
      ? {
          id: row.id,
          name: row.name,
          type: row.type,
          base_url: row.base_url,
          api_key: row.api_key,
          priority: row.priority,
          multiplier: row.multiplier,
          supported_models: supportedModels,
          test_default_model: row.test_default_model ?? '',
          tags: row.tags ?? [],
          notes: row.notes ?? '',
        }
      : createForm(),
  );
  tagsText.value = row?.tags?.join(', ') ?? '';
  mappingRows.value = Object.entries(row?.model_mappings ?? {}).map(
    ([client_model, upstream_model]) => ({ client_model, upstream_model }),
  );
  await nextTick();
  formRef.value?.clearValidate();
};

const addMapping = () => {
  mappingRows.value.push({ client_model: '', upstream_model: '' });
};

const removeMapping = (index: number) => {
  mappingRows.value.splice(index, 1);
};

const serializeMappings = (): Record<string, string> => {
  const mappings: Record<string, string> = {};
  for (const [index, row] of mappingRows.value.entries()) {
    const client = row.client_model.trim();
    const upstream = row.upstream_model.trim();
    if (!client || !upstream) throw new Error(`模型映射第 ${index + 1} 行不能为空`);
    if (mappings[client]) throw new Error(`模型映射中客户端模型重复：${client}`);
    if (client === upstream) throw new Error(`模型映射第 ${index + 1} 行的两个模型不能相同`);
    mappings[client] = upstream;
  }
  return mappings;
};

const save = async () => {
  const valid = await formRef.value?.validate().catch(() => false);
  if (!valid) return;
  try {
    emit('save', {
      ...form,
      tags: tagsText.value
        .split(',')
        .map((tag) => tag.trim())
        .filter(Boolean),
      notes: form.notes.trim(),
      model_mappings: serializeMappings(),
    });
  } catch (error) {
    ElMessage.error(String(error));
  }
};

watch(
  () => visible.value,
  (isVisible) => {
    if (isVisible) reset();
  },
);

watch(
  () => props.account?.id,
  () => {
    if (visible.value) reset();
  },
);

watch(
  () => form.type,
  () => {
    const available = new Set(availableModels.value.map((model) => model.name));
    form.supported_models = form.supported_models.filter((name) => available.has(name));
    if (!form.supported_models.includes(form.test_default_model)) form.test_default_model = '';
  },
);

watch(
  () => [...form.supported_models],
  (selected) => {
    if (!selected.includes(form.test_default_model)) form.test_default_model = '';
    for (const row of mappingRows.value) {
      if (row.client_model && !selected.includes(row.client_model)) row.client_model = '';
    }
  },
);
</script>

<style scoped lang="scss">
.account-dialog :deep(.el-dialog__body) {
  max-height: calc(100vh - 140px);
  overflow-y: auto;
  padding: 16px 20px 8px;
}

.account-form :deep(.el-form-item) {
  margin-bottom: 14px;
}

.model-picker {
  width: 100%;
  max-height: 105px;
  overflow-y: auto;
  padding: 6px 12px;
  border: 1px solid var(--el-border-color);
  border-radius: 4px;
  background-color: var(--el-fill-color-blank);
}

.model-picker-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.model-picker-toolbar :deep(.el-checkbox) {
  margin-bottom: 0;
  height: 24px;
}

.model-checkbox-group {
  display: flex;
  flex-wrap: wrap;
}

.model-checkbox-group :deep(.el-checkbox) {
  margin-right: 14px;
  margin-bottom: 2px;
  height: 24px;
}

.model-count {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.mapping-section {
  width: 100%;
}

.mapping-table {
  width: 100%;
}

.mapping-add-btn {
  margin-top: 6px;
}
</style>
