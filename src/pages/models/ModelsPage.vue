<template>
  <div class="page">
    <div class="page-toolbar">
      <h2 class="page-title">模型维护</h2>
      <div class="model-actions">
        <el-select
          v-model="typeFilter"
          clearable
          placeholder="协议类型"
          class="type-filter"
          @change="load"
        >
          <el-option label="OpenAI" value="openai" />
          <el-option label="Anthropic" value="anthropic" />
        </el-select>
        <ProviderSelect
          v-model="providerFilter"
          clearable
          placeholder="供应商"
          class="provider-filter"
          @change="load"
        />
        <el-button :loading="loading" @click="load">刷新</el-button>
        <el-button :disabled="!hasCustomWidths" @click="resetColumnWidths">恢复默认列宽</el-button>
        <el-button type="primary" @click="open()">新增模型</el-button>
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
        label="模型名称"
        :width="columnWidth('name')"
        min-width="260"
      />
      <el-table-column
        column-key="provider"
        prop="provider"
        label="供应商"
        :width="columnWidth('provider', 180)"
      >
        <template #default="{ row }">
          <ProviderLabel :provider="row.provider" />
        </template>
      </el-table-column>
      <el-table-column
        column-key="type"
        prop="type"
        label="协议类型"
        :width="columnWidth('type', 140)"
      />
      <el-table-column
        column-key="isDefault"
        label="测试默认"
        :width="columnWidth('isDefault', 130)"
      >
        <template #default="{ row }">
          <el-tag v-if="row.is_default" type="success">默认</el-tag>
          <el-button v-else link type="primary" @click="setDefault(row)">设为默认</el-button>
        </template>
      </el-table-column>
      <el-table-column column-key="actions" label="操作" :width="columnWidth('actions', 150)">
        <template #default="{ row }">
          <el-button link type="primary" @click="open(row)">编辑</el-button>
          <el-button link type="danger" @click="remove(row)">删除</el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog v-model="dialog" :title="editing ? '编辑模型' : '新增模型'" width="460px">
      <el-form ref="formRef" :model="form" :rules="rules" label-width="90px">
        <el-form-item label="名称" prop="name">
          <el-input v-model="form.name" placeholder="请输入模型名称" />
        </el-form-item>
        <el-form-item label="协议类型" prop="type">
          <el-select v-model="form.type" style="width: 100%" @change="changeType">
            <el-option label="OpenAI" value="openai" />
            <el-option label="Anthropic" value="anthropic" />
          </el-select>
        </el-form-item>
        <el-form-item label="供应商" prop="provider">
          <ProviderSelect
            v-model="form.provider"
            style="width: 100%"
            @change="providerTouched = true"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialog = false">取消</el-button>
        <el-button type="primary" @click="save">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref } from 'vue';
import { ElMessage, ElMessageBox, type FormInstance, type FormRules } from 'element-plus';
import { modelsApi, type CatalogModel, type ModelPayload, type ModelType } from '../../api/models';
import ProviderLabel from '../../components/models/ProviderLabel.vue';
import ProviderSelect from '../../components/models/ProviderSelect.vue';
import type { ModelProvider } from '../../constants/providers';
import { useTableColumnWidths } from '../../composables/useTableColumnWidths';

const { tableKey, hasCustomWidths, columnWidth, handleColumnResize, resetColumnWidths } =
  useTableColumnWidths('models');
type ModelForm = ModelPayload & {
  id?: string;
};

const createForm = (): ModelForm => ({
  id: undefined,
  name: '',
  type: 'openai',
  provider: 'openai',
});

const items = ref<CatalogModel[]>([]);
const loading = ref(false);
const typeFilter = ref<ModelType | ''>('');
const providerFilter = ref<ModelProvider | ''>('');
const dialog = ref(false);
const formRef = ref<FormInstance>();
const form = reactive<ModelForm>(createForm());
const providerTouched = ref(false);
const editing = computed(() => Boolean(form.id));

const rules: FormRules<ModelForm> = {
  name: [{ required: true, message: '请输入模型名称', trigger: 'blur' }],
  type: [{ required: true, message: '请选择协议类型', trigger: 'change' }],
  provider: [{ required: true, message: '请选择供应商', trigger: 'change' }],
};

const load = async () => {
  loading.value = true;
  try {
    items.value = (
      await modelsApi.list({
        type: typeFilter.value || undefined,
        provider: providerFilter.value || undefined,
      })
    ).items;
  } finally {
    loading.value = false;
  }
};

const open = (row?: CatalogModel) => {
  providerTouched.value = Boolean(row);
  Object.assign(
    form,
    row ? { id: row.id, name: row.name, type: row.type, provider: row.provider } : createForm(),
  );
  dialog.value = true;
  nextTick(() => formRef.value?.clearValidate());
};

const changeType = (type: ModelType) => {
  if (!providerTouched.value) form.provider = type;
};

const save = async () => {
  const valid = await formRef.value?.validate().catch(() => false);
  if (!valid) return;
  const payload: ModelPayload = {
    name: form.name.trim(),
    type: form.type,
    provider: form.provider,
  };
  try {
    if (form.id) await modelsApi.update(form.id, payload);
    else await modelsApi.create(payload);
    dialog.value = false;
    await load();
    ElMessage.success('保存成功');
  } catch (error) {
    ElMessage.error(String(error));
  }
};

const setDefault = async (row: CatalogModel) => {
  await modelsApi.setDefault(row.id);
  await load();
};

const remove = async (row: CatalogModel) => {
  try {
    await ElMessageBox.confirm(`确认删除 ${row.name}？`, '提示');
    await modelsApi.remove(row.id);
    await load();
    ElMessage.success('删除成功');
  } catch (error) {
    if (error !== 'cancel' && error !== 'close') ElMessage.error(String(error));
  }
};

onMounted(load);
</script>

<style scoped>
.model-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.type-filter {
  width: 150px;
}

.provider-filter {
  width: 180px;
}
</style>
