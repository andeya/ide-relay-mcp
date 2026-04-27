<script setup lang="ts">
/**
 * One submitted user Answer: markdown body + attachment strip.
 * Includes Md/Raw toggle and Copy button for the text portion.
 */
import { computed, ref, onBeforeUnmount } from "vue";
import type { QaRound } from "../types/relay-app";
import { parsedUserReplyFromRound } from "../utils/parseRelayFeedbackReply";
import { safeMarkdownToHtml } from "../utils/safeMarkdown";
import { t } from "../i18n";
import QaReplyAttachments from "./QaReplyAttachments.vue";

const props = defineProps<{
  round: QaRound;
  zoomTitle: string;
}>();

const emit = defineEmits<{
  preview: [src: string];
}>();

const parsed = computed(() => parsedUserReplyFromRound(props.round));
const bodyHtml = computed(() => safeMarkdownToHtml(parsed.value.text));

const showRaw = ref(false);
const copied = ref(false);
let copiedTimer: ReturnType<typeof setTimeout> | null = null;

async function copyText() {
  try {
    await navigator.clipboard.writeText(parsed.value.text ?? "");
    copied.value = true;
    if (copiedTimer) clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => {
      copied.value = false;
      copiedTimer = null;
    }, 1500);
  } catch {
    /* clipboard API may be unavailable */
  }
}

onBeforeUnmount(() => {
  if (copiedTimer) clearTimeout(copiedTimer);
});
</script>

<template>
  <div>
    <div v-if="parsed.text" class="retellContainer retellContainer--user">
      <div class="retellToolbar">
        <button
          class="retellToolBtn"
          :class="{ 'retellToolBtn--active retellToolBtn--activeUser': !showRaw }"
          :aria-pressed="!showRaw"
          :title="t('retellViewMd')"
          @click="showRaw = false"
        >
          <svg width="14" height="12" viewBox="0 0 208 128" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
            <rect x="5" y="5" width="198" height="118" rx="15" stroke="currentColor" stroke-width="12" fill="none" />
            <path d="M30 98V30l40 40 40-40v68" stroke="currentColor" stroke-width="12" stroke-linecap="round" stroke-linejoin="round" fill="none" />
            <path d="M155 68l23 24 23-24" stroke="currentColor" stroke-width="12" stroke-linecap="round" stroke-linejoin="round" fill="none" />
            <line x1="178" y1="36" x2="178" y2="92" stroke="currentColor" stroke-width="12" stroke-linecap="round" />
          </svg>
        </button>
        <button
          class="retellToolBtn"
          :class="{ 'retellToolBtn--active retellToolBtn--activeUser': showRaw }"
          :aria-pressed="showRaw"
          :title="t('retellViewRaw')"
          @click="showRaw = true"
        >
          <svg width="14" height="12" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
            <path d="M8 18l-6-6 6-6M16 6l6 6-6 6" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
        <span class="retellToolDivider" />
        <button
          class="retellToolBtn retellCopyBtn"
          :class="{ 'retellCopyBtn--ok': copied }"
          :title="copied ? t('retellCopied') : t('retellCopy')"
          @click="copyText"
        >
          <svg v-if="copied" width="13" height="12" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
            <path d="M5 13l4 4L19 7" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <svg v-else width="13" height="12" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
            <rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="2" />
            <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" stroke="currentColor" stroke-width="2" />
          </svg>
        </button>
      </div>
      <pre v-if="showRaw" class="qaRoundMd qaRoundMd--user retellRawPre">{{ parsed.text.trim() }}</pre>
      <div
        v-else
        class="qaRoundMd qaRoundMd--user qaRoundMd--bubble"
        v-html="bodyHtml"
      />
    </div>
    <QaReplyAttachments
      :paths="parsed.imagePaths"
      :file-paths="parsed.filePaths"
      :zoom-title="zoomTitle"
      @preview="emit('preview', $event)"
    />
  </div>
</template>
