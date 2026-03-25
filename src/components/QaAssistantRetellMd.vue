<script setup lang="ts">
/**
 * Assistant retell markdown: computed once per retell change — avoids re-parsing all
 * history when unrelated parent state updates (e.g. composer input).
 */
import { computed } from "vue";
import { safeMarkdownToHtml } from "../utils/safeMarkdown";

const props = defineProps<{
  retell: string;
}>();

const html = computed(() => safeMarkdownToHtml(props.retell ?? ""));
</script>

<template>
  <div class="qaRoundMd qaRoundMd--agent" v-html="html" />
</template>
