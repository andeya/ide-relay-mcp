<script setup lang="ts">
import { computed } from "vue";
import type { IdeKind } from "../types/relay-app";

const props = defineProps<{
  strings: Record<string, string>;
  error?: string;
}>();

const emit = defineEmits<{
  (_e: "select", _ide: IdeKind): void;
}>();

const S = computed(() => props.strings);

const options: { key: IdeKind; nameKey: string; descKey: string; icon: string }[] = [
  { key: "cursor", nameKey: "ideCursor", descKey: "ideCursorDesc", icon: "⌨" },
  { key: "claude_code", nameKey: "ideClaudeCode", descKey: "ideClaudeCodeDesc", icon: "🤖" },
  { key: "windsurf", nameKey: "ideWindsurf", descKey: "ideWindsurfDesc", icon: "🏄" },
  { key: "other", nameKey: "ideOther", descKey: "ideOtherDesc", icon: "⚙" },
];
</script>

<template>
  <div class="ideSelectionPage">
    <div class="ideSelectionContent">
      <h2 class="ideSelectionTitle">{{ S.ideSelectionTitle }}</h2>
      <p class="ideSelectionSubtitle">{{ S.ideSelectionSubtitle }}</p>

      <div class="ideSelectionGrid">
        <button
          v-for="opt in options"
          :key="opt.key"
          type="button"
          class="ideCard"
          @click="emit('select', opt.key)"
        >
          <span class="ideCardIcon">{{ opt.icon }}</span>
          <span class="ideCardName">{{ S[opt.nameKey] }}</span>
          <span class="ideCardDesc">{{ S[opt.descKey] }}</span>
        </button>
      </div>

      <p v-if="props.error" class="ideSelectionError" role="alert">{{ props.error }}</p>
    </div>
  </div>
</template>
