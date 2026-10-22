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
    <el-card class="priority-card">
      <template #header>优先级扣减策略</template>
      <el-form
        ref="priorityFormRef"
        :model="form"
        :rules="priorityRules"
        label-width="155px"
        class="settings-form"
      >
        <div class="priority-row">
          <el-form-item label="首字超时（秒）">
            <el-input-number v-model="form.first_token_timeout_seconds" :min="1" />
          </el-form-item>
          <el-form-item label="优先级扣减数" prop="first_token_timeout_priority_penalty">
            <el-input-number
              v-model="form.first_token_timeout_priority_penalty"
              :min="1"
              :max="3"
              :step="1"
              step-strictly
              :precision="0"
            />
          </el-form-item>
        </div>
        <div class="priority-row">
          <el-form-item label="上游超时（秒）">
            <el-input-number v-model="form.upstream_timeout_seconds" :min="1" />
          </el-form-item>
          <el-form-item label="优先级扣减数" prop="upstream_timeout_priority_penalty">
            <el-input-number
              v-model="form.upstream_timeout_priority_penalty"
              :min="1"
              :max="3"
              :step="1"
              step-strictly
              :precision="0"
            />
          </el-form-item>
        </div>
        <el-form-item label="请求失败扣减数" prop="request_failure_priority_penalty">
          <el-input-number
            v-model="form.request_failure_priority_penalty"
            :min="1"
            :max="3"
            :step="1"
            step-strictly
            :precision="0"
          />
        </el-form-item>
        <p class="priority-help">
          首字超时、上游超时分别按对应策略扣减，其他失败使用请求失败扣减数。每次失败只扣一次，优先级最低为
          0。保存后对后续请求生效。
        </p>
        <el-divider content-position="left">连续低缓存率扣减</el-divider>
        <el-form-item label="启用缓存率策略">
          <el-switch v-model="form.low_cache_priority_enabled" />
        </el-form-item>
        <div class="priority-row">
          <el-form-item label="连续低缓存次数" prop="low_cache_consecutive">
            <el-input-number
              v-model="form.low_cache_consecutive"
              :min="1"
              :max="1000"
              :precision="0"
              step-strictly
              :disabled="!form.low_cache_priority_enabled"
            />
          </el-form-item>
          <el-form-item label="缓存率低于（%）" prop="low_cache_threshold_percent">
            <el-input-number
              v-model="form.low_cache_threshold_percent"
              :min="1"
              :max="100"
              :precision="0"
              step-strictly
              :disabled="!form.low_cache_priority_enabled"
            />
          </el-form-item>
          <el-form-item label="优先级扣减数" prop="low_cache_priority_penalty">
            <el-input-number
              v-model="form.low_cache_priority_penalty"
              :min="1"
              :max="3"
              :precision="0"
              step-strictly
              :disabled="!form.low_cache_priority_enabled"
            />
          </el-form-item>
        </div>
        <p class="priority-help">
          仅统计实际网关请求，按账号累计，流式请求结束后计算一次。有效低缓存请求不增加优先级，连续达到次数后扣减并清零。
          缓存达标、请求失败或缓存数据缺失/异常会清零；成功且缓存达标或无法评估时仍按原规则加分。
          计数仅保存在当前进程内存中，重启或修改缓存策略后清零；账号测试和定时监控不参与统计。
        </p>
        <el-divider content-position="left">连续首字慢扣减</el-divider>
        <el-form-item label="启用首字慢策略">
          <el-switch v-model="form.slow_first_token_enabled" />
        </el-form-item>
        <div class="priority-row">
          <el-form-item label="连续首字慢次数" prop="slow_first_token_consecutive">
            <el-input-number
              v-model="form.slow_first_token_consecutive"
              :min="1"
              :max="1000"
              :precision="0"
              step-strictly
              :disabled="!form.slow_first_token_enabled"
            />
          </el-form-item>
          <el-form-item label="首字高于（秒）" prop="slow_first_token_threshold_seconds">
            <el-input-number
              v-model="form.slow_first_token_threshold_seconds"
              :min="1"
              :max="86400"
              :precision="0"
              step-strictly
              :disabled="!form.slow_first_token_enabled"
            />
          </el-form-item>
          <el-form-item label="优先级扣减数" prop="slow_first_token_priority_penalty">
            <el-input-number
              v-model="form.slow_first_token_priority_penalty"
              :min="1"
              :max="3"
              :precision="0"
              step-strictly
              :disabled="!form.slow_first_token_enabled"
            />
          </el-form-item>
        </div>
        <p class="priority-help">
          仅统计实际网关的成功流式请求，按账号累计，流结束后计算一次。首字耗时严格超过阈值时累计，达到次数后扣减并清零，不中断请求、不触发重试。
          首字达标、请求失败或缺少首字数据会清零。与缓存率策略独立计数；任一策略判定表现不佳，本次不加分，两项同时触发时合并扣减，最低为
          0。 计数仅保存在当前进程内存中，重启或修改本策略后清零；不影响上方首字超时配置。
        </p>
      </el-form>
    </el-card>
  </div>
</template>
<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import { ElMessage } from 'element-plus';
import type { FormInstance, FormRules } from 'element-plus';
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
  first_token_timeout_priority_penalty: 1,
  upstream_timeout_priority_penalty: 1,
  request_failure_priority_penalty: 2,
  low_cache_priority_enabled: false,
  low_cache_consecutive: 5,
  low_cache_threshold_percent: 85,
  low_cache_priority_penalty: 3,
  slow_first_token_enabled: false,
  slow_first_token_consecutive: 5,
  slow_first_token_threshold_seconds: 10,
  slow_first_token_priority_penalty: 2,
  upstream_proxy_enabled: false,
  upstream_proxy_url: 'http://127.0.0.1:7890',
  monitoring_enabled: true,
  monitoring_interval_minutes: 2,
  monitoring_recent_count: 30,
  monitoring_start_time: null,
  monitoring_end_time: null,
  local_token: '',
  launch_at_login: false,
});
const saving = ref(false);
const priorityFormRef = ref<FormInstance>();
const penaltyRule = {
  type: 'integer' as const,
  required: true,
  min: 1,
  max: 3,
  message: '请输入 1 到 3 的整数',
  trigger: 'change',
};
const priorityRules: FormRules<Settings> = {
  first_token_timeout_priority_penalty: [penaltyRule],
  upstream_timeout_priority_penalty: [penaltyRule],
  request_failure_priority_penalty: [penaltyRule],
  low_cache_priority_penalty: [penaltyRule],
  slow_first_token_priority_penalty: [penaltyRule],
  slow_first_token_consecutive: [{ ...penaltyRule, max: 1000, message: '请输入 1 到 1000 的整数' }],
  slow_first_token_threshold_seconds: [
    { ...penaltyRule, max: 86400, message: '请输入 1 到 86400 的整数' },
  ],
  low_cache_consecutive: [{ ...penaltyRule, max: 1000, message: '请输入 1 到 1000 的整数' }],
  low_cache_threshold_percent: [{ ...penaltyRule, max: 100, message: '请输入 1 到 100 的整数' }],
};
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
  if (!(await priorityFormRef.value?.validate().catch(() => false))) return;
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

.priority-card {
  margin-top: 16px;
}

.priority-card :deep(.el-form-item__label) {
  white-space: nowrap;
}

.priority-row {
  display: flex;
  flex-wrap: wrap;
  column-gap: 32px;
}

.priority-help {
  margin: 0 0 0 155px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 1.6;
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
