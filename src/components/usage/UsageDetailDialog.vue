<template>
  <el-dialog v-model="visible" title="使用记录详情" width="920px" top="5vh">
    <template v-if="record">
      <el-descriptions :column="2" border size="small">
        <el-descriptions-item label="记录 ID">{{ text(record.id) }}</el-descriptions-item>
        <el-descriptions-item label="Trace ID">{{ text(record.trace_id) }}</el-descriptions-item>
        <el-descriptions-item label="开始时间">{{
          formatTime(record.started_at)
        }}</el-descriptions-item>
        <el-descriptions-item label="结束时间">{{
          formatTime(record.ended_at)
        }}</el-descriptions-item>
        <el-descriptions-item label="总耗时">{{
          formatMsRaw(record.duration_ms)
        }}</el-descriptions-item>
        <el-descriptions-item label="首 Token 用时">{{
          formatMsRaw(record.first_token_ms)
        }}</el-descriptions-item>
        <el-descriptions-item label="吞吐量">{{ formatThroughput(record) }}</el-descriptions-item>
        <el-descriptions-item label="重试次数">{{
          displayRetryCount(record.attempts)
        }}</el-descriptions-item>
        <el-descriptions-item label="账号名称">{{
          text(record.account_name)
        }}</el-descriptions-item>
        <el-descriptions-item label="账号 ID">{{ text(record.account_id) }}</el-descriptions-item>
        <el-descriptions-item label="账号类型">{{
          text(record.account_type)
        }}</el-descriptions-item>
        <el-descriptions-item label="模型">{{ text(record.model) }}</el-descriptions-item>
        <el-descriptions-item label="推理强度">{{
          text(record.reasoning_effort)
        }}</el-descriptions-item>
        <el-descriptions-item label="接口">{{ text(record.endpoint) }}</el-descriptions-item>
        <el-descriptions-item label="流式">{{ record.stream ? '是' : '否' }}</el-descriptions-item>
        <el-descriptions-item label="客户端 IP">{{ text(record.client_ip) }}</el-descriptions-item>
        <el-descriptions-item label="结果">
          <span :class="resultClass(record)">{{ resultText(record) }}</span>
        </el-descriptions-item>
        <el-descriptions-item label="状态码">{{ text(record.status_code) }}</el-descriptions-item>
        <el-descriptions-item label="错误码">{{ text(record.error_code) }}</el-descriptions-item>
      </el-descriptions>

      <div class="token-line">
        Token 用量：输入 {{ formatToken(record.input_tokens) }} / 输出
        {{ formatToken(record.output_tokens) }} / 缓存 {{ formatToken(record.cached_tokens) }} /
        合计
        {{ formatToken(record.total_tokens) }}
      </div>
      <div class="error-title">错误信息：</div>
      <el-input
        :model-value="text(record.error_message)"
        type="textarea"
        readonly
        :rows="5"
        class="error-view"
      />
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import type { UsageRecord } from '../../api/usage';
import { formatThroughput, formatToken } from '../../utils/token';

const visible = defineModel<boolean>({ required: true });
defineProps<{ record?: UsageRecord }>();

const formatTime = (value?: string) => {
  if (!value) return '-';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { hour12: false });
};

const formatMsRaw = (value?: number) => (value == null ? '-' : `${value} ms`);
const displayRetryCount = (attempts?: number) => Math.max((attempts ?? 1) - 1, 0);
const text = (value: unknown) => (value == null || value === '' ? '-' : String(value));
const resultText = (usage: UsageRecord) =>
  usage.ended_at ? (usage.success ? '成功' : '失败') : '进行中';
const resultClass = (usage: UsageRecord) =>
  usage.ended_at ? (usage.success ? 'success-text' : 'failure-text') : 'pending-text';
</script>

<style scoped>
.success-text {
  color: #2e9f63;
}

.failure-text {
  color: #c43d4b;
  font-weight: 600;
}

.pending-text {
  color: #b7791f;
  font-weight: 600;
}

.token-line {
  padding: 14px 0;
  color: #344054;
}

.error-title {
  margin-bottom: 6px;
}

.error-view :deep(textarea) {
  font-family: Consolas, monospace;
}
</style>
