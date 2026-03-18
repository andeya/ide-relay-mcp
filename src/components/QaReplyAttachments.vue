<script setup lang="ts">
/**
 * Thumbnails for submitted Answer images; click opens parent lightbox.
 */
import { invoke } from "@tauri-apps/api/core";
import { onBeforeUnmount, ref, watch } from "vue";

const props = defineProps<{
  paths: string[];
  zoomTitle: string;
}>();

const emit = defineEmits<{
  preview: [src: string];
}>();

const thumbs = ref<Record<string, string>>({});
const failed = ref<Set<string>>(new Set());
let cancelled = false;

async function loadPath(p: string) {
  if (thumbs.value[p] || failed.value.has(p)) return;
  try {
    const u = await invoke<string>("read_feedback_attachment_data_url", {
      path: p,
    });
    if (!cancelled) {
      thumbs.value = { ...thumbs.value, [p]: u };
    }
  } catch {
    if (!cancelled) {
      failed.value = new Set([...failed.value, p]);
    }
  }
}

watch(
  () => props.paths,
  (ps) => {
    for (const p of ps) void loadPath(p);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  cancelled = true;
});

function onThumbClick(p: string) {
  const u = thumbs.value[p];
  if (u) emit("preview", u);
}
</script>

<template>
  <div
    v-if="paths.length"
    class="qaReplyAttachRow"
    role="group"
    :aria-label="zoomTitle"
  >
    <button
      v-for="p in paths"
      :key="p"
      type="button"
      class="qaReplyAttachBtn"
      :title="zoomTitle"
      :disabled="!thumbs[p] && !failed.has(p)"
      @click="onThumbClick(p)"
    >
      <img
        v-if="thumbs[p]"
        class="qaReplyAttachImg"
        :src="thumbs[p]"
        alt=""
      />
      <span v-else-if="failed.has(p)" class="qaReplyAttachFail">?</span>
      <span v-else class="qaReplyAttachLoad" aria-hidden="true" />
    </button>
  </div>
</template>

<style scoped>
.qaReplyAttachRow {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid rgba(148, 163, 184, 0.12);
}

.qaReplyAttachBtn {
  padding: 0;
  border: 1px solid rgba(148, 163, 184, 0.25);
  border-radius: 10px;
  overflow: hidden;
  width: 72px;
  height: 72px;
  cursor: pointer;
  background: rgba(7, 13, 25, 0.6);
  display: grid;
  place-items: center;
}

.qaReplyAttachBtn:disabled {
  cursor: wait;
  opacity: 0.7;
}

.qaReplyAttachImg {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.qaReplyAttachFail {
  font-size: 1.25rem;
  color: #94a3b8;
}

.qaReplyAttachLoad {
  width: 22px;
  height: 22px;
  border: 2px solid rgba(148, 163, 184, 0.3);
  border-top-color: #a5b4fc;
  border-radius: 50%;
  animation: qaSpin 0.7s linear infinite;
}

@keyframes qaSpin {
  to {
    transform: rotate(360deg);
  }
}
</style>
