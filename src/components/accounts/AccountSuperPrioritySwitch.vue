<template>
  <el-tooltip :content="hint">
    <span>
      <el-switch
        :model-value="enabled"
        :disabled="!provider || busy"
        :loading="savingId === accountId"
        :aria-label="`${accountName} 超级优先`"
        @change="(value: string | number | boolean) => change(Boolean(value))"
      />
    </span>
  </el-tooltip>
  <el-tag v-if="enabled && (priority < 5 || status !== 'active')" type="info" size="small">
    暂未生效
  </el-tag>
</template>

<script lang="ts">
import { ref } from 'vue';

// Serialize replacement and refresh across every instance of this shared control.
const busy = ref(false);
const savingId = ref<string>();
</script>

<script setup lang="ts">
import { computed } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { accountsApi } from '../../api/accounts';

const props = defineProps<{
  accountId: string;
  accountName: string;
  provider?: string | null;
  enabled: boolean;
  priority: number;
  status: string;
  refresh: () => Promise<void>;
}>();

const hint = computed(() => {
  if (!props.provider) return '请先编辑账号并指定供应商';
  if (props.enabled && props.status !== 'active') return '账号已禁用，超级优先暂未生效';
  if (props.enabled && props.priority < 5) return '优先级低于 5，恢复到 5 后自动生效';
  return '同协议、同供应商仅保留一个；账号启用且优先级 ≥ 5 时优先调度匹配模型';
});

const change = async (enabled: boolean) => {
  if (busy.value || !props.provider) return;
  const accountId = props.accountId;
  const accountName = props.accountName;
  busy.value = true;
  try {
    if (enabled) {
      await ElMessageBox.confirm(
        `将 ${accountName} 设为超级优先，会替换同协议、同供应商的原选择。优先级至少为 5 时生效。`,
        '超级优先',
        { type: 'warning' },
      );
    }
    savingId.value = accountId;
    await accountsApi.setSuperPriority(accountId, enabled);
    await props.refresh();
    ElMessage.success(enabled ? '超级优先已开启' : '超级优先已关闭');
  } catch (error) {
    if (error !== 'cancel' && error !== 'close') ElMessage.error(String(error));
  } finally {
    savingId.value = undefined;
    busy.value = false;
  }
};
</script>
