<template>
  <div class="page">
    <div class="page-toolbar">
      <h2 class="page-title">设置</h2>
      <el-button type="primary" :loading="saving" @click="save">保存设置</el-button>
    </div>
    <el-card class="settings-card">
      <el-form :model="form" label-width="120px" class="settings-form">
        <div class="settings-grid">
          <el-form-item label="监听地址">
            <el-input v-model="form.host" />
          </el-form-item>
          <el-form-item label="端口">
            <el-input-number v-model="form.port" :min="1" :max="65535" />
          </el-form-item>
          <el-form-item label="上游超时（秒）">
            <el-input-number v-model="form.upstream_timeout_seconds" :min="1" />
          </el-form-item>
          <el-form-item label="首字超时（秒）">
            <el-input-number v-model="form.first_token_timeout_seconds" :min="1" />
          </el-form-item>
          <el-form-item label="重试次数">
            <el-input-number v-model="form.request_retry_attempts" :min="1" :max="20" />
          </el-form-item>
          <el-form-item label="开机自启">
            <el-switch v-model="form.launch_at_login" />
          </el-form-item>
          <el-form-item label="启用上游代理">
            <el-switch v-model="form.upstream_proxy_enabled" />
          </el-form-item>
          <el-form-item label="上游代理地址">
            <el-input v-model="form.upstream_proxy_url" />
          </el-form-item>

          <el-divider content-position="left">API 请求地址</el-divider>
          <el-form-item label="OpenAI 请求地址" label-width="150px">
            <div class="address-field">
              <el-input :model-value="openaiAddress" readonly>
                <template #append>
                  <el-button
                    title="复制 OpenAI 请求地址"
                    @click="copyAddress('OpenAI', openaiAddress)"
                  >
                    <el-icon><CopyDocument /></el-icon>复制
                  </el-button>
                </template>
              </el-input>
            </div>
          </el-form-item>
          <el-form-item label="Anthropic 请求地址" label-width="150px">
            <div class="address-field">
              <el-input :model-value="anthropicAddress" readonly>
                <template #append>
                  <el-button
                    title="复制 Anthropic 请求地址"
                    @click="copyAddress('Anthropic', anthropicAddress)"
                  >
                    <el-icon><CopyDocument /></el-icon>复制
                  </el-button>
                </template>
              </el-input>
            </div>
          </el-form-item>
          <el-form-item label="本地令牌">
            <el-input v-model="form.local_token" show-password />
          </el-form-item>
          <el-form-item label="数据目录">
            <el-button @click="openDataDirectory">打开数据目录</el-button>
          </el-form-item>
        </div>
      </el-form>
    </el-card>
  </div>
</template>
<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { ElMessage } from 'element-plus';
import { CopyDocument } from '@element-plus/icons-vue';
import { settingsApi, type Settings } from '../../api/settings';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
const form = reactive<Settings>({
  host: '127.0.0.1',
  port: 7789,
  upstream_timeout_seconds: 300,
  first_token_timeout_seconds: 60,
  request_retry_attempts: 10,
  upstream_proxy_enabled: false,
  upstream_proxy_url: 'http://127.0.0.1:7890',
  monitoring_enabled: true,
  local_token: '',
  launch_at_login: false,
});
const saving = ref(false);
const gatewayHost = computed(() => {
  const host = form.host.trim();
  return !host || host === '0.0.0.0' ? '127.0.0.1' : host;
});
const gatewayOrigin = computed(() => `http://${gatewayHost.value}:${form.port}`);
const openaiAddress = computed(() => `${gatewayOrigin.value}/v1`);
const anthropicAddress = computed(() => gatewayOrigin.value);

onMounted(async () => {
  Object.assign(form, await settingsApi.get());
  if (isTauri()) {
    try {
      form.launch_at_login = await isEnabled();
    } catch (error) {
      ElMessage.warning(`读取开机自启状态失败：${String(error)}`);
    }
  }
});
const syncAutostart = async (desired: boolean) => {
  if (!isTauri()) return;
  const current = await isEnabled();
  if (current === desired) return;
  if (desired) await enable();
  else await disable();
};

const save = async () => {
  saving.value = true;
  try {
    await syncAutostart(form.launch_at_login);
    await settingsApi.update(form);
    ElMessage.success('设置已保存');
  } catch (error) {
    ElMessage.error(`保存设置失败：${String(error)}`);
  } finally {
    saving.value = false;
  }
};
const openDataDirectory = () =>
  invoke('open_data_directory').catch((e) => ElMessage.error(String(e)));

const copyAddress = async (name: string, address: string) => {
  try {
    await navigator.clipboard.writeText(address);
    ElMessage.success(`${name} 请求地址已复制`);
  } catch (error) {
    ElMessage.error(`复制失败：${String(error)}`);
  }
};
</script>

<style scoped lang="scss">
.settings-form {
  max-width: 1180px;
}

.settings-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  column-gap: 32px;
  row-gap: 2px;
}

.settings-grid :deep(.el-divider) {
  grid-column: 1 / -1;
  margin: 8px 0 18px;
}

.settings-grid :deep(.el-form-item) {
  min-width: 0;
}

.settings-grid :deep(.el-input-number),
.settings-grid :deep(.el-input) {
  max-width: 100%;
}

.address-field {
  display: flex;
  width: 100%;
}

.address-field :deep(.el-input) {
  flex: 1;
}

@media (max-width: 1050px) {
  .settings-grid {
    grid-template-columns: 1fr;
  }

  .settings-grid :deep(.el-divider) {
    grid-column: auto;
  }
}
</style>
