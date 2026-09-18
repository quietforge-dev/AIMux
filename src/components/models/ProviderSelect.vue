<template>
  <el-select
    v-model="model"
    :clearable="props.clearable"
    :filterable="props.filterable"
    :placeholder="props.placeholder"
    @change="handleChange"
  >
    <template #label="{ value }">
      <ProviderLabel v-if="value" :provider="String(value)" />
    </template>
    <el-option
      v-for="provider in PROVIDERS"
      :key="provider.value"
      :label="provider.label"
      :value="provider.value"
    >
      <ProviderLabel :provider="provider.value" />
    </el-option>
  </el-select>
</template>

<script setup lang="ts">
import { PROVIDERS, type ModelProvider } from '../../constants/providers';
import ProviderLabel from './ProviderLabel.vue';

const model = defineModel<ModelProvider | ''>({ required: true });
const props = withDefaults(
  defineProps<{
    clearable?: boolean;
    filterable?: boolean;
    placeholder?: string;
  }>(),
  {
    clearable: false,
    filterable: true,
    placeholder: '请选择供应商',
  },
);
const emit = defineEmits<{
  change: [value: ModelProvider | ''];
}>();

const handleChange = (value: unknown) => {
  emit('change', value as ModelProvider | '');
};
</script>
