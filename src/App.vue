<template>
  <router-view v-if="auth.user && !loginVisible" />
  <div v-else-if="!loginVisible" class="auth-loading">
    <span>{{ error || '正在连接 AIMux…' }}</span>
    <el-button v-if="error" @click="initialize">重试连接</el-button>
  </div>
  <LoginDialog v-model="loginVisible" />
</template>
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import LoginDialog from './components/auth/LoginDialog.vue';
import { clearAuthToken } from './api/client';
import { useAuthStore } from './stores/auth';
import { useRouter } from 'vue-router';
const auth = useAuthStore();
const router = useRouter();
const loginVisible = ref(false);
const error = ref('');
const requireLogin = () => {
  clearAuthToken();
  auth.clear();
  loginVisible.value = true;
};
const initialize = async () => {
  error.value = '';
  try {
    await auth.refresh();
    if (router.currentRoute.value.meta.requiresAdmin && !auth.isAdmin)
      await router.replace('/accounts');
  } catch (e) {
    if (!loginVisible.value) error.value = String(e);
  }
};
onMounted(() => {
  window.addEventListener('aimux-auth-required', requireLogin);
  void initialize();
});
onUnmounted(() => window.removeEventListener('aimux-auth-required', requireLogin));
</script>
<style scoped>
.auth-loading {
  display: flex;
  min-height: 100vh;
  align-items: center;
  justify-content: center;
  gap: 16px;
}
</style>
