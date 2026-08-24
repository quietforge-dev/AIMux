<template>
  <el-dialog
    v-model="visible"
    :title="`测试账号 · ${account.name}`"
    width="860px"
    top="5vh"
    :close-on-click-modal="!testing"
    :close-on-press-escape="!testing"
    :before-close="handleClose"
    class="account-test-dialog"
  >
    <div class="test-toolbar">
      <el-form inline>
        <el-form-item label="选择测试模型">
          <el-select v-model="selectedModel" :disabled="testing" style="width: 240px">
            <el-option v-if="account.test_default_model" label="使用账号默认模型" value="" />
            <el-option v-for="model in models" :key="model" :label="model" :value="model" />
          </el-select>
        </el-form-item>
        <el-form-item label="测试请求形态">
          <el-tag>{{ requestType }}</el-tag>
        </el-form-item>
      </el-form>
      <el-button type="primary" :loading="testing" @click="runTest">开始测试</el-button>
    </div>

    <el-progress :percentage="progress" :status="progressStatus" :format="() => progressLabel" />

    <div class="test-log" aria-live="polite">
      <template v-if="logs.length">
        <div v-for="(entry, index) in logs" :key="index" :class="['log-entry', entry.kind]">
          <div class="log-label">{{ entry.label }}</div>
          <pre v-if="entry.preformatted">{{ entry.text }}</pre>
          <div v-else>{{ entry.text }}</div>
        </div>
      </template>
      <el-empty v-else description="等待测试" :image-size="70" />
    </div>

    <template #footer>
      <el-button @click="closeDialog">{{ testing ? '取消测试' : '关闭' }}</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { ElMessage } from 'element-plus';
import { accountsApi, type Account } from '../../api/accounts';

type LogEntry = {
  label: string;
  text: string;
  kind: 'info' | 'success' | 'error' | 'request' | 'response';
  preformatted?: boolean;
};

const props = defineProps<{
  account: Account;
  models: string[];
}>();
const emit = defineEmits<{
  finished: [];
}>();
const visible = defineModel<boolean>({ required: true });
const selectedModel = ref('');
const logs = ref<LogEntry[]>([]);
const testing = ref(false);
const activeController = ref<AbortController>();
const progress = ref(0);
const progressLabel = ref('等待测试');
const progressStatus = ref<'success' | 'exception' | undefined>();

const requestType = computed(() =>
  props.account.type === 'anthropic' ? 'Messages' : 'Chat Completions',
);
const endpoint = computed(() =>
  props.account.type === 'anthropic' ? '/v1/messages' : '/v1/chat/completions',
);
const requestBody = computed(() => {
  const body: Record<string, unknown> = {
    max_tokens: 1,
    messages: [{ role: 'user', content: 'ping' }],
  };
  if (selectedModel.value) body.model = selectedModel.value;
  return body;
});

const reset = () => {
  selectedModel.value = props.account.test_default_model ? '' : (props.models[0] ?? '');
  logs.value = [];
  progress.value = 0;
  progressLabel.value = '等待测试';
  progressStatus.value = undefined;
};

const addLog = (
  label: string,
  text: string,
  kind: LogEntry['kind'] = 'info',
  preformatted = false,
) => {
  logs.value.push({ label, text, kind, preformatted });
};

const pretty = (value: unknown) => {
  if (typeof value !== 'string') return JSON.stringify(value, null, 2);
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
};

const runTest = async () => {
  if (testing.value) return;
  const controller = new AbortController();
  activeController.value = controller;
  testing.value = true;
  progress.value = 10;
  progressLabel.value = '正在发送请求...';
  progressStatus.value = undefined;
  logs.value = [];
  addLog('开始测试账号', props.account.name, 'info');
  addLog('账号类型', props.account.type, 'info');
  addLog('端点', `POST ${endpoint.value}`, 'info');
  addLog('请求体', JSON.stringify(requestBody.value, null, 2), 'request', true);
  try {
    const result = await accountsApi.test(
      props.account.id,
      selectedModel.value || undefined,
      controller.signal,
    );
    if (controller.signal.aborted) return;
    progress.value = 100;
    progressLabel.value = '测试完成';
    const success = Boolean(result.success);
    progressStatus.value = success ? 'success' : 'exception';
    if (result.status_code != null) {
      addLog('响应状态', String(result.status_code), success ? 'success' : 'error');
    }
    if (result.response_body || result.error_message) {
      addLog(
        '响应内容',
        pretty(result.response_body || result.error_message),
        success ? 'response' : 'error',
        true,
      );
    }
    addLog(
      success ? '测试通过' : '测试失败',
      result.error_code ?? '',
      success ? 'success' : 'error',
    );
    emit('finished');
  } catch (error) {
    if (controller.signal.aborted) return;
    progress.value = 100;
    progressLabel.value = '测试异常';
    progressStatus.value = 'exception';
    addLog('请求异常', String(error), 'error');
    ElMessage.error(String(error));
    emit('finished');
  } finally {
    if (activeController.value === controller) {
      activeController.value = undefined;
      testing.value = false;
    }
  }
};

const cancelTest = () => {
  const controller = activeController.value;
  if (!controller) return;
  controller.abort();
  activeController.value = undefined;
  testing.value = false;
};

const closeDialog = () => {
  cancelTest();
  visible.value = false;
};

const handleClose = (done: () => void) => {
  cancelTest();
  done();
};

watch(() => props.account.id, reset);
onMounted(reset);
onBeforeUnmount(cancelTest);
</script>

<style scoped lang="scss">
.test-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.test-toolbar :deep(.el-form) {
  margin-bottom: 0;
}

.test-log {
  min-height: 360px;
  max-height: 520px;
  overflow: auto;
  margin-top: 14px;
  padding: 14px 16px;
  border: 1px solid #303747;
  border-radius: 6px;
  background: #1e1e1e;
  color: #d4d4d4;
  font-family: Consolas, 'Courier New', monospace;
  font-size: 13px;
}

.log-entry {
  margin-bottom: 12px;
  white-space: pre-wrap;
}

.log-label {
  color: #9cdcfe;
  margin-bottom: 4px;
}

.log-entry.success .log-label {
  color: #4ec9b0;
}

.log-entry.error .log-label {
  color: #f48771;
}

.log-entry.request .log-label,
.log-entry.response .log-label {
  color: #9cdcfe;
}

pre {
  margin: 0;
  white-space: pre-wrap;
}
</style>
