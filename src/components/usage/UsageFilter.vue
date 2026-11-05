<template>
  <el-form inline class="usage-filter" @submit.prevent="emit('query')">
    <el-form-item label="筛选" class="filter-label">
      <el-input
        v-model="filters.account_id"
        class="account-filter"
        placeholder="账号 ID"
        clearable
      />
    </el-form-item>
    <el-form-item>
      <el-input v-model="filters.model" class="model-filter" placeholder="模型" clearable />
    </el-form-item>
    <el-form-item>
      <el-select v-model="filters.kind" class="select-filter" placeholder="全部类型" clearable>
        <el-option label="OpenAI" value="openai" />
        <el-option label="Anthropic" value="anthropic" />
      </el-select>
    </el-form-item>
    <el-form-item label="推理强度">
      <el-select v-model="filters.reasoning_effort" class="select-filter" aria-label="推理强度筛选">
        <el-option label="非空（有推理强度）" value="__nonempty__" />
        <el-option label="全部" value="" />
        <el-option
          v-for="effort in reasoningEfforts"
          :key="effort"
          :label="effort"
          :value="effort"
        />
      </el-select>
    </el-form-item>
    <el-form-item>
      <el-select v-model="filters.success" class="select-filter" placeholder="全部结果" clearable>
        <el-option label="成功" :value="true" />
        <el-option label="失败" :value="false" />
      </el-select>
    </el-form-item>
    <el-form-item>
      <el-date-picker
        v-model="filters.started_after"
        class="date-filter"
        type="date"
        value-format="YYYY-MM-DD"
        placeholder="开始日期"
        clearable
      />
    </el-form-item>
    <el-form-item>
      <el-date-picker
        v-model="filters.started_before"
        class="date-filter"
        type="date"
        value-format="YYYY-MM-DD"
        placeholder="结束日期"
        clearable
      />
    </el-form-item>
    <el-form-item class="filter-submit">
      <el-button type="primary" native-type="submit">查询</el-button>
    </el-form-item>
  </el-form>
</template>

<script setup lang="ts">
import type { UsageFilterState } from '../../api/usage';

const reasoningEfforts = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'];

const filters = defineModel<UsageFilterState>({ required: true });
const emit = defineEmits<{ query: [] }>();
</script>

<style scoped>
.usage-filter {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0 8px;
  margin-bottom: 10px;
}

.usage-filter :deep(.el-form-item) {
  margin-right: 0;
  margin-bottom: 10px;
}

.usage-filter :deep(.el-input),
.usage-filter :deep(.el-select),
.usage-filter :deep(.el-date-editor) {
  flex: none;
}

.account-filter,
.model-filter {
  width: 220px;
}

.select-filter {
  width: 150px;
}

.date-filter {
  width: 180px;
}

.filter-submit {
  margin-left: 2px;
}

@media (max-width: 1280px) {
  .account-filter,
  .model-filter {
    width: 190px;
  }

  .select-filter {
    width: 135px;
  }

  .date-filter {
    width: 165px;
  }
}
</style>
