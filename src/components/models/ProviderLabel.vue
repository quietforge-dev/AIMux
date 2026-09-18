<template>
  <span v-if="meta" class="provider-label">
    <ProviderLogo :provider="props.provider" decorative />
    <span>{{ meta.label }}</span>
  </span>
  <span v-else>{{ props.provider }}</span>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { providerMeta } from '../../constants/providers';
import ProviderLogo from './ProviderLogo.vue';

const props = defineProps<{
  provider: string;
}>();

const meta = computed(() => providerMeta(props.provider));
</script>

<style scoped>
.provider-label {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
</style>
