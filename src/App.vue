<template>
  <router-view v-if="!loginVisible" />
  <LoginDialog v-model="loginVisible" />
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import LoginDialog from './components/auth/LoginDialog.vue';

const loginVisible = ref(false);
const requireLogin = () => {
  loginVisible.value = true;
};
onMounted(() => window.addEventListener('aimux-auth-required', requireLogin));
onUnmounted(() => window.removeEventListener('aimux-auth-required', requireLogin));
</script>
