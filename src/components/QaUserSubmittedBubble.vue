<script setup lang="ts">
/**
 * One submitted user Answer: markdown body + attachment strip. Parses once per round update.
 */
import { computed } from "vue";
import type { QaRound } from "../types/relay-app";
import { parsedUserReplyFromRound } from "../utils/parseRelayFeedbackReply";
import { safeMarkdownToHtml } from "../utils/safeMarkdown";
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
</script>

<template>
  <div>
    <div
      v-if="parsed.text"
      class="qaRoundMd qaRoundMd--user qaRoundMd--bubble"
      v-html="bodyHtml"
    />
    <QaReplyAttachments
      :paths="parsed.imagePaths"
      :file-paths="parsed.filePaths"
      :zoom-title="zoomTitle"
      @preview="emit('preview', $event)"
    />
  </div>
</template>
