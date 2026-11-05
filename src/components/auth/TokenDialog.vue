<template>
  <el-dialog
    v-model="visible"
    title="保存 API Token"
    width="560px"
    :close-on-click-modal="false"
    @closed="emit('closed')"
  >
    <el-alert title="完整 Token 仅显示这一次，请立即复制保存。" type="warning" :closable="false" />
    <el-input :model-value="token" readonly type="textarea" :rows="3" class="token-value" />
    <template #footer>
      <el-button @click="copy">复制 Token</el-button>
      <el-button type="primary" @click="visible = false">已保存</el-button>
    </template>
  </el-dialog>
</template>
<script setup lang="ts">
import { ElMessage } from 'element-plus';
const visible = defineModel<boolean>({ required: true });
const props = defineProps<{ token: string }>();
const emit = defineEmits<{ closed: [] }>();
const copy = async () => {
  try {
    await navigator.clipboard.writeText(props.token);
    ElMessage.success('Token 已复制');
  } catch {
    ElMessage.error('复制失败，请手动选中复制');
  }
};
</script>
<style scoped>
.token-value {
  margin-top: 16px;
}
</style>
