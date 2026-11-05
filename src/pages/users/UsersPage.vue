<template>
  <div class="page" v-if="auth.isAdmin">
    <div class="page-toolbar">
      <h2 class="page-title">用户管理</h2>
      <el-button type="primary" @click="openCreate"
        ><el-icon><Plus /></el-icon>新增用户</el-button
      >
    </div>
    <el-alert
      title="管理员管理用户账号；每个用户的渠道、设置和使用记录独立。禁用用户会使登录会话及 API Token 停止访问，并停止后台监控。"
      type="info"
      :closable="false"
      class="users-help"
    />
    <el-table :data="items" v-loading="loading" row-key="id">
      <el-table-column prop="username" label="用户名" min-width="150" />
      <el-table-column label="角色" width="110"
        ><template #default="{ row }">{{
          row.role === 'admin' ? '管理员' : '普通用户'
        }}</template></el-table-column
      >
      <el-table-column label="状态" width="100"
        ><template #default="{ row }"
          ><el-tag :type="row.enabled ? 'success' : 'info'">{{
            row.enabled ? '启用' : '禁用'
          }}</el-tag></template
        ></el-table-column
      >
      <el-table-column label="API Token" min-width="170"
        ><template #default="{ row }">{{
          row.api_token_prefix ? `${row.api_token_prefix}…` : '未设置'
        }}</template></el-table-column
      >
      <el-table-column prop="created_at" label="创建时间" min-width="190" />
      <el-table-column label="操作" width="210"
        ><template #default="{ row }">
          <el-button link type="primary" @click="openPassword(row)">重置密码</el-button>
          <el-button
            v-if="row.role !== 'admin'"
            link
            :type="row.enabled ? 'danger' : 'success'"
            :loading="changing === row.id"
            @click="toggle(row)"
            >{{ row.enabled ? '禁用' : '启用' }}</el-button
          >
        </template></el-table-column
      >
    </el-table>
    <el-pagination
      v-model:current-page="page"
      :page-size="20"
      :total="total"
      layout="total, prev, pager, next"
      @current-change="load"
      class="users-pagination"
    />
    <el-dialog
      v-model="dialog"
      :title="editing ? `重置 ${editing.username} 的密码` : '新增用户'"
      width="440px"
      :close-on-click-modal="false"
    >
      <el-form @submit.prevent="save" label-width="90px">
        <el-alert
          v-if="formError"
          :title="formError"
          type="error"
          :closable="false"
          class="users-help"
        />
        <el-form-item v-if="!editing" label="用户名"
          ><el-input v-model="username" maxlength="64" autocomplete="off"
        /></el-form-item>
        <el-form-item label="新密码"
          ><el-input v-model="password" type="password" show-password autocomplete="new-password"
        /></el-form-item>
        <el-form-item label="确认密码"
          ><el-input
            v-model="confirmation"
            type="password"
            show-password
            autocomplete="new-password"
        /></el-form-item>
        <div v-if="editing" class="password-help">
          保存后，该用户已有网页登录会话会失效，API Token 保持有效。
        </div>
      </el-form>
      <template #footer
        ><el-button @click="dialog = false">取消</el-button
        ><el-button type="primary" :loading="saving" @click="save">保存</el-button></template
      >
    </el-dialog>
    <TokenDialog v-model="tokenVisible" :token="token" @closed="token = ''" />
  </div>
</template>
<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { Plus } from '@element-plus/icons-vue';
import { useAuthStore } from '../../stores/auth';
import { usersApi } from '../../api/users';
import type { User } from '../../api/auth';
import TokenDialog from '../../components/auth/TokenDialog.vue';
const auth = useAuthStore();
const items = ref<User[]>([]);
const total = ref(0);
const page = ref(1);
const loading = ref(false);
let loadId = 0;
const load = async () => {
  if (!auth.isAdmin) return;
  const id = ++loadId;
  loading.value = true;
  try {
    const result = await usersApi.list((page.value - 1) * 20, 20);
    if (id === loadId) {
      items.value = result.items;
      total.value = result.total;
    }
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    if (id === loadId) loading.value = false;
  }
};
const dialog = ref(false);
const editing = ref<User | null>(null);
const username = ref('');
const password = ref('');
const confirmation = ref('');
const formError = ref('');
const saving = ref(false);
const token = ref('');
const tokenVisible = ref(false);
const changing = ref('');
const openCreate = () => {
  editing.value = null;
  username.value = '';
  password.value = '';
  confirmation.value = '';
  formError.value = '';
  dialog.value = true;
};
const openPassword = (user: User) => {
  editing.value = user;
  password.value = '';
  confirmation.value = '';
  formError.value = '';
  dialog.value = true;
};
const save = async () => {
  formError.value = '';
  if ((!editing.value && !username.value.trim()) || password.value.length < 8) {
    formError.value = '请填写用户名和至少 8 位密码';
    return;
  }
  if (password.value !== confirmation.value) {
    formError.value = '两次密码不一致';
    return;
  }
  saving.value = true;
  try {
    if (editing.value) {
      await usersApi.setPassword(editing.value.id, password.value);
    } else {
      const created = await usersApi.create(username.value.trim(), password.value);
      token.value = created.token;
      tokenVisible.value = true;
    }
    const selfReset = editing.value?.id === auth.user?.id;
    dialog.value = false;
    password.value = '';
    confirmation.value = '';
    ElMessage.success('保存成功');
    if (selfReset) window.dispatchEvent(new CustomEvent('aimux-auth-required'));
    else await load();
  } catch (e) {
    formError.value = String(e);
  } finally {
    saving.value = false;
  }
};
const toggle = async (user: User) => {
  try {
    await ElMessageBox.confirm(
      `确定${user.enabled ? '禁用' : '启用'}用户 ${user.username}？`,
      '用户状态',
      { type: 'warning' },
    );
  } catch {
    return;
  }
  changing.value = user.id;
  try {
    await usersApi.setEnabled(user.id, !user.enabled);
    await load();
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    changing.value = '';
  }
};
onMounted(load);
</script>
<style scoped>
.users-help {
  margin-bottom: 16px;
}
.users-pagination {
  margin-top: 16px;
  justify-content: flex-end;
}
.password-help {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
</style>
