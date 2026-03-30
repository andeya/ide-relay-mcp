<script setup lang="ts">
/**
 * Assistant retell: Markdown (default) / raw toggle + copy button.
 * Computed once per retell change to avoid re-parsing on unrelated parent updates.
 */
import { computed, ref } from "vue";
import { safeMarkdownToHtml } from "../utils/safeMarkdown";

const props = defineProps<{
  retell: string;
}>();

const html = computed(() => safeMarkdownToHtml(props.retell ?? ""));
const showRaw = ref(false);
const copied = ref(false);

async function copyRetell() {
  try {
    await navigator.clipboard.writeText(props.retell ?? "");
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 1500);
  } catch {
    /* clipboard API may be unavailable in some environments */
  }
}
</script>

<template>
  <div class="retellContainer">
    <div class="retellToolbar">
      <button
        class="retellToolBtn"
        :class="{ 'retellToolBtn--active': !showRaw }"
        :aria-pressed="!showRaw"
        title="Markdown"
        @click="showRaw = false"
      >
        Md
      </button>
      <button
        class="retellToolBtn"
        :class="{ 'retellToolBtn--active': showRaw }"
        :aria-pressed="showRaw"
        title="Raw"
        @click="showRaw = true"
      >
        Raw
      </button>
      <span class="retellToolDivider" />
      <button
        class="retellToolBtn retellCopyBtn"
        :class="{ 'retellCopyBtn--ok': copied }"
        :title="copied ? 'Copied!' : 'Copy'"
        @click="copyRetell"
      >
        {{ copied ? "✓" : "⧉" }}
      </button>
    </div>
    <pre v-if="showRaw" class="qaRoundMd qaRoundMd--agent retellRawPre">{{ retell }}</pre>
    <div v-else class="qaRoundMd qaRoundMd--agent" v-html="html" />
  </div>
</template>
