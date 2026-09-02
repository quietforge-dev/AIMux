<template>
  <div class="page">
    <div class="page-toolbar">
      <h2 class="page-title">模型维护</h2>
      <div>
        <el-select
          v-model="kind"
          clearable
          placeholder="类型筛选"
          @change="load"
          style="width: 150px; margin-right: 10px"
          ><el-option label="OpenAI" value="openai" /><el-option
            label="Anthropic"
            value="anthropic" /></el-select
        ><el-button :loading="loading" @click="load">刷新</el-button
        ><el-button type="primary" @click="open()">新增模型</el-button>
      </div>
    </div>
    <el-table :data="items" v-loading="loading" border class="compact-table"
      ><el-table-column prop="name" label="模型名称" min-width="260" /><el-table-column
        prop="type"
        label="类型"
        width="140"
      /><el-table-column label="测试默认" width="130"
        ><template #default="{ row }"
          ><el-tag v-if="row.is_default" type="success">默认</el-tag
          ><el-button v-else link type="primary" @click="setDefault(row)"
            >设为默认</el-button
          ></template
        ></el-table-column
      ><el-table-column label="操作" width="150"
        ><template #default="{ row }"
          ><el-button link type="primary" @click="open(row)">编辑</el-button
          ><el-button link type="danger" @click="remove(row)">删除</el-button></template
        ></el-table-column
      ></el-table
    ><el-dialog v-model="dialog" :title="editing ? '编辑模型' : '新增模型'" width="430px"
      ><el-form :model="form" label-width="90px"
        ><el-form-item label="名称"><el-input v-model="form.name" /></el-form-item
        ><el-form-item label="类型"
          ><el-select v-model="form.type"
            ><el-option label="OpenAI" value="openai" /><el-option
              label="Anthropic"
              value="anthropic" /></el-select></el-form-item></el-form
      ><template #footer
        ><el-button @click="dialog = false">取消</el-button
        ><el-button type="primary" @click="save">保存</el-button></template
      ></el-dialog
    >
  </div>
</template>
<script setup lang="ts">
import { onMounted, ref, reactive } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { modelsApi, type CatalogModel } from '../../api/models';
const items = ref<CatalogModel[]>([]),
  loading = ref(false),
  kind = ref(''),
  dialog = ref(false),
  editing = ref(false),
  form = reactive<any>({ name: '', type: 'openai' });
const load = async () => {
  loading.value = true;
  try {
    items.value = (await modelsApi.list(kind.value || undefined)).items;
  } finally {
    loading.value = false;
  }
};
const open = (row?: CatalogModel) => {
  editing.value = !!row;
  Object.assign(form, row ? { ...row } : { name: '', type: 'openai' });
  dialog.value = true;
};
const save = async () => {
  try {
    editing.value ? await modelsApi.update(form.id, form) : await modelsApi.create(form);
    dialog.value = false;
    await load();
    ElMessage.success('保存成功');
  } catch (e) {
    ElMessage.error(String(e));
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
