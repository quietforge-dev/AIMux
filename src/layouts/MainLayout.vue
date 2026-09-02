<template>
  <el-container class="shell">
    <el-aside width="214px" class="sidebar">
      <div class="brand"><span class="brand-mark">A</span><span>AIMux</span></div>
      <el-menu :default-active="route.path" router class="nav">
        <el-menu-item index="/accounts"
          ><el-icon><User /></el-icon>账号管理</el-menu-item
        >
        <el-menu-item index="/models"
          ><el-icon><Grid /></el-icon>模型维护</el-menu-item
        >
        <el-menu-item index="/usage"
          ><el-icon><Tickets /></el-icon>使用记录</el-menu-item
        >
        <el-menu-item index="/statistics"
          ><el-icon><DataAnalysis /></el-icon>数据统计</el-menu-item
        >
        <el-menu-item index="/monitor"
          ><el-icon><Monitor /></el-icon>监控管理</el-menu-item
        >
        <el-menu-item index="/settings"
          ><el-icon><Setting /></el-icon>设置</el-menu-item
        >
      </el-menu>
      <div class="sidebar-foot">
        <div>{{ clock }}</div>
        <div class="sidebar-actions">
          <el-button text class="sidebar-action" @click="openGithub">
            <Github class="sidebar-action-icon" :size="16" :stroke-width="2" />
            <span>GitHub</span>
          </el-button>
          <el-button
            text
            class="sidebar-action"
            :loading="updateChecking"
            :loading-icon="RefreshCw"
            :disabled="updateChecking || updateInstalling"
            @click="checkForUpdates"
          >
            <RefreshCw
              v-if="!updateChecking"
              class="sidebar-action-icon"
              :size="16"
              :stroke-width="2"
            />
            <span>{{ updateChecking ? '正在检查更新...' : '检查更新' }}</span>
          </el-button>
        </div>
        <div class="version">v{{ app.version }}</div>
      </div>
    </el-aside>
    <el-main class="main"><router-view /></el-main>
  </el-container>
  <AppUpdateDialog
    v-model:visible="updateVisible"
    :current-version="app.version"
    :version="updateVersion"
    :notes="updateNotes"
    :installing="updateInstalling"
    :progress-percentage="updateProgressPercentage"
    :progress-label="updateProgressLabel"
    @install="installUpdate"
    @manual-download="openReleasePage"
  />
</template>
<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue';
import { useRoute } from 'vue-router';
import { invoke } from '@tauri-apps/api/core';
import { ElMessage, ElMessageBox } from 'element-plus';
import { DataAnalysis, Grid, Monitor, Setting, Tickets, User } from '@element-plus/icons-vue';
import { Github, RefreshCw } from 'lucide-vue-next';
import { useAppStore } from '../stores/app';
import AppUpdateDialog from '../components/app/AppUpdateDialog.vue';
import { useAppUpdater } from '../composables/useAppUpdater';
const route = useRoute();
const app = useAppStore();
const clock = computed(() => app.now.toLocaleTimeString('zh-CN', { hour12: false }));

const GITHUB_URL = 'https://github.com/quietforge-dev/AIMux';
const AUTO_UPDATE_INITIAL_DELAY_MS = 5_000;
const AUTO_UPDATE_INTERVAL_MS = 6 * 60 * 60 * 1_000;
const updater = useAppUpdater();
const {
  checking: updateChecking,
  installing: updateInstalling,
  notes: updateNotes,
  progressLabel: updateProgressLabel,
  progressPercentage: updateProgressPercentage,
  version: updateVersion,
  visible: updateVisible,
} = updater;

const openExternal = async (url: string) => {
  try {
    await invoke('open_external_url', { url });
  } catch {
    window.open(url, '_blank', 'noopener,noreferrer');
  }
};

const openGithub = async () => {
  await ElMessageBox.alert(`将使用默认浏览器打开：\n${GITHUB_URL}`, '打开 GitHub', {
    confirmButtonText: '打开',
  });
  await openExternal(GITHUB_URL);
};

const checkForUpdates = async () => {
  if (updateChecking.value) {
    ElMessage.info('正在检查更新，请稍候');
    return;
  }
  if (updateInstalling.value) {
    ElMessage.info('正在安装更新，请稍候');
    return;
  }
  try {
    const result = await updater.checkForUpdates();
    if (result && !result.available) {
      ElMessage.success(`当前已是最新版本：v${app.version}`);
    }
  } catch (error) {
    ElMessage.error(`检查更新失败：${error instanceof Error ? error.message : String(error)}`);
  }
};

const checkForUpdatesSilently = async () => {
  try {
    await updater.checkForUpdates();
  } catch {
    // 自动检查失败时静默处理，避免网络波动打扰正常使用。
  }
};

let initialUpdateTimer: number | undefined;
let periodicUpdateTimer: number | undefined;

onMounted(() => {
  app.startClock();
  void app.loadVersion();
  initialUpdateTimer = window.setTimeout(
    () => void checkForUpdatesSilently(),
    AUTO_UPDATE_INITIAL_DELAY_MS,
  );
  periodicUpdateTimer = window.setInterval(
    () => void checkForUpdatesSilently(),
    AUTO_UPDATE_INTERVAL_MS,
  );
});

onUnmounted(() => {
  if (initialUpdateTimer !== undefined) window.clearTimeout(initialUpdateTimer);
  if (periodicUpdateTimer !== undefined) window.clearInterval(periodicUpdateTimer);
});

const installUpdate = async () => {
  try {
    await updater.installAndRelaunch();
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
};

const openReleasePage = async () => {
  await openExternal(updater.releasePageUrl());
};
</script>
<style scoped lang="scss">
.shell {
  height: 100vh;
}
.sidebar {
  background: #172033;
  color: #d9e1ee;
  display: flex;
  flex-direction: column;
}
.brand {
  height: 64px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 22px;
  font-size: 20px;
  font-weight: 700;
}
.brand-mark {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: #3b82f6;
  color: white;
  display: grid;
  place-items: center;
}
.nav {
  border: 0;
  background: transparent;
  flex: 1;
}
.nav :deep(.el-menu-item) {
  color: #c2cada;
  height: 46px;
}
.nav :deep(.el-menu-item.is-active) {
  color: #fff;
  background: #263655;
}
.nav :deep(.el-menu-item:hover) {
  background: #202e49;
}
.sidebar-foot {
  padding: 18px 22px;
  color: #aab7cb;
  font-size: 13px;
  line-height: 1.9;
}
.sidebar-actions {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  margin: 8px 0 4px;
}
.sidebar-action {
  justify-content: flex-start;
  color: #aab7cb;
  min-height: 30px;
  width: 100%;
  margin: 0;
  padding: 0;
}
.sidebar-action-icon {
  flex: 0 0 auto;
  margin-right: 6px;
}
.sidebar-action:hover {
  color: #fff;
  background: #202e49;
}
.version {
  color: #72809a;
}
.main {
  padding: 0;
  background: #f5f7fa;
  overflow: auto;
}
</style>
