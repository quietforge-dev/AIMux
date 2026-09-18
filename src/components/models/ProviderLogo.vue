<template>
  <span
    v-if="meta"
    class="provider-logo-frame"
    :title="props.decorative ? undefined : meta.label"
    :aria-label="props.decorative ? undefined : meta.label"
    :aria-hidden="props.decorative ? 'true' : undefined"
  >
    <img class="provider-logo" :src="meta.logo" alt="" />
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { providerMeta } from '../../constants/providers';

const props = defineProps<{
  provider?: string;
  decorative?: boolean;
}>();

const meta = computed(() => (props.provider ? providerMeta(props.provider) : undefined));
</script>

<style scoped>
.provider-logo-frame {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  flex: 0 0 22px;
  padding: 2px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 5px;
  background: #fff;
}

.provider-logo {
  display: block;
  width: 18px;
  height: 18px;
  object-fit: contain;
}
</style>
