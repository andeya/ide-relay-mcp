<script setup lang="ts">
import { computed, ref } from "vue";
import type { IdeKind } from "../types/relay-app";

const props = defineProps<{
  strings: Record<string, string>;
  error?: string;
  busy?: boolean;
}>();

const emit = defineEmits<{
  (_e: "select", _ide: IdeKind): void;
}>();

const S = computed(() => props.strings);
const selectedIde = ref<IdeKind | null>(null);

function onSelect(ide: IdeKind) {
  if (props.busy) return;
  selectedIde.value = ide;
  emit("select", ide);
}

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
          :class="{ 'ideCard--busy': props.busy && selectedIde === opt.key }"
          :disabled="props.busy"
          @click="onSelect(opt.key)"
        >
          <span v-if="props.busy && selectedIde === opt.key" class="ideCardSpinner" />
          <span v-else class="ideCardIcon">{{ opt.icon }}</span>
          <span class="ideCardName">{{ S[opt.nameKey] }}</span>
          <span class="ideCardDesc">{{ S[opt.descKey] }}</span>
        </button>
      </div>

      <p v-if="props.error" class="ideSelectionError" role="alert">{{ props.error }}</p>
    </div>
  </div>
</template>
