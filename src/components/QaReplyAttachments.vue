<script setup lang="ts">
/**
 * Answer attachments in composer-style row: image thumbs + file chips (same layout as input preview).
 */
import { invoke } from "@tauri-apps/api/core";
import { computed, onBeforeUnmount, ref, watch } from "vue";

const props = defineProps<{
  paths: string[];
  filePaths?: string[];
  zoomTitle: string;
}>();

const emit = defineEmits<{
  preview: [src: string];
}>();

const fileItems = computed(() =>
  (props.filePaths ?? []).map((p) => {
    const s = String(p).trim();
    const i = Math.max(s.lastIndexOf("/"), s.lastIndexOf("\\"));
    const name = i >= 0 ? s.slice(i + 1) : s;
    return { path: s, name: name || s };
  }),
);

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
    v-if="paths.length || fileItems.length"
    class="composerThumbRow qaReplyAttachStrip"
    role="group"
    :aria-label="zoomTitle"
  >
    <div v-for="p in paths" :key="'img-' + p" class="composerThumbWrap">
      <button
        v-if="thumbs[p] || failed.has(p)"
        type="button"
        class="qaReplyThumbBtn"
        :title="zoomTitle"
        @click="onThumbClick(p)"
      >
        <img
          v-if="thumbs[p]"
          class="composerThumb composerThumb--zoom"
          :src="thumbs[p]"
          alt=""
        />
        <span v-else class="qaReplyThumbFail">?</span>
      </button>
      <div v-else class="qaReplyThumbLoading" aria-hidden="true">
        <span class="qaReplyThumbSpinner" />
      </div>
    </div>
    <div
      v-for="it in fileItems"
      :key="'file-' + it.path"
      class="composerThumbWrap composerFileDropWrap"
      :title="it.path"
    >
      <div class="composerFileDropChip">
        <svg
          class="composerFileDropIcon"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.75"
          aria-hidden="true"
        >
          <path
            d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"
          />
          <path d="M14 2v6h6" />
        </svg>
        <span class="composerFileDropName">{{ it.name }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.qaReplyThumbBtn {
  display: block;
  width: 100%;
  height: 100%;
  padding: 0;
  margin: 0;
  border: none;
  background: rgba(7, 13, 25, 0.5);
  cursor: zoom-in;
}

.qaReplyThumbFail {
  font-size: 1.1rem;
  color: #94a3b8;
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
}

.qaReplyThumbLoading {
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
  background: rgba(7, 13, 25, 0.45);
}

.qaReplyThumbSpinner {
  width: 18px;
  height: 18px;
  border: 2px solid rgba(148, 163, 184, 0.25);
  border-top-color: #a5b4fc;
  border-radius: 50%;
  animation: qaReplySpin 0.7s linear infinite;
}

@keyframes qaReplySpin {
  to {
    transform: rotate(360deg);
  }
}
</style>
