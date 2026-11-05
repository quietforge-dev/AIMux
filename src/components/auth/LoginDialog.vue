<template>
  <el-dialog
    v-model="visible"
    title="登录 AIMux"
    width="380px"
    :close-on-click-modal="false"
    :show-close="false"
  >
    <el-form @submit.prevent="submit">
      <el-form-item label="账号">
        <el-input v-model="username" autocomplete="username" autofocus />
      </el-form-item>
      <el-form-item label="密码">
        <el-input
          v-model="password"
          type="password"
          show-password
          autocomplete="current-password"
          @keyup.enter="submit"
        />
      </el-form-item>
      <el-button type="primary" native-type="submit" :loading="loading" style="width: 100%">
        登录
      </el-button>
    </el-form>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { ElMessage } from 'element-plus';
import { useAuthStore } from '../../stores/auth';
import { useRouter } from 'vue-router';
const auth = useAuthStore();
const router = useRouter();

const visible = defineModel<boolean>({ required: true });
const username = ref('');
const password = ref('');
const loading = ref(false);

const submit = async () => {
  if (!username.value.trim() || !password.value) {
    ElMessage.warning('请输入账号和密码');
    return;
  }
  loading.value = true;
  try {
    await auth.login(username.value.trim(), password.value);
    if (router.currentRoute.value.meta.requiresAdmin && !auth.isAdmin)
      await router.replace('/accounts');
    password.value = '';
    visible.value = false;
  } catch (error) {
    ElMessage.error(String(error));
  } finally {
    loading.value = false;
  }
};
</script>
