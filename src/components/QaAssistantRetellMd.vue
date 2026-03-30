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
        <!-- Markdown "M↓" logo -->
        <svg width="14" height="12" viewBox="0 0 208 128" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <rect x="5" y="5" width="198" height="118" rx="15" stroke="currentColor" stroke-width="12" fill="none"/>
          <path d="M30 98V30l40 40 40-40v68" stroke="currentColor" stroke-width="12" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
          <path d="M155 68l23 24 23-24" stroke="currentColor" stroke-width="12" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
          <line x1="178" y1="36" x2="178" y2="92" stroke="currentColor" stroke-width="12" stroke-linecap="round"/>
        </svg>
      </button>
      <button
        class="retellToolBtn"
        :class="{ 'retellToolBtn--active': showRaw }"
        :aria-pressed="showRaw"
        title="Raw"
        @click="showRaw = true"
      >
        <!-- Code brackets </> -->
        <svg width="14" height="12" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <path d="M8 18l-6-6 6-6M16 6l6 6-6 6" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>
      <span class="retellToolDivider" />
      <button
        class="retellToolBtn retellCopyBtn"
        :class="{ 'retellCopyBtn--ok': copied }"
        :title="copied ? 'Copied!' : 'Copy'"
        @click="copyRetell"
      >
        <!-- Check mark when copied, clipboard otherwise -->
        <svg v-if="copied" width="13" height="12" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <path d="M5 13l4 4L19 7" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <svg v-else width="13" height="12" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="2"/>
          <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" stroke="currentColor" stroke-width="2"/>
        </svg>
      </button>
    </div>
    <pre v-if="showRaw" class="qaRoundMd qaRoundMd--agent retellRawPre">{{ retell }}</pre>
    <div v-else class="qaRoundMd qaRoundMd--agent" v-html="html" />
  </div>
</template>
