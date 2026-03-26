import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { IdeKind, IdeCapabilities } from "../types/relay-app";
import { getRelayRulePromptBilingual } from "../ideRulesTemplates";

const ideKind = ref<IdeKind | null>(null);
const ideCapabilities = ref<IdeCapabilities | null>(null);
const loaded = ref(false);

export function useIdeBinding() {
  async function loadBinding() {
    try {
      const binding = await invoke<IdeKind | null>("get_ide_binding");
      ideKind.value = binding;
      if (binding) {
        ideCapabilities.value = await invoke<IdeCapabilities>(
          "get_ide_capabilities",
          { ide: binding },
        );
        recheckVersion();
      } else {
        ideCapabilities.value = null;
      }
    } catch {
      ideKind.value = null;
      ideCapabilities.value = null;
    } finally {
      loaded.value = true;
    }
  }

  async function switchIde(ide: IdeKind) {
    await invoke("set_ide_binding", { ide });
    ideKind.value = ide;
    ideCapabilities.value = await invoke<IdeCapabilities>(
      "get_ide_capabilities",
      { ide },
    );
    try {
      const title = await invoke<string>("get_window_title");
      await getCurrentWindow().setTitle(title);
    } catch { /* best effort */ }
    recheckVersion();
  }

  function recheckVersion() {
    const ruleContent = getRelayRulePromptBilingual("loop", ideKind.value || undefined);
    invoke("recheck_version_upgrade", { ruleContent }).catch(() => {});
  }

  const supportsMcpInject = computed(
    () => ideCapabilities.value?.supportsMcpInject ?? false,
  );
  const supportsRulePrompt = computed(
    () => ideCapabilities.value?.supportsRulePrompt ?? false,
  );
  const supportsUsage = computed(
    () => ideCapabilities.value?.supportsUsage ?? false,
  );

  const ideLabel = computed(() => {
    const labels: Record<IdeKind, string> = {
      cursor: "Cursor",
      claude_code: "Claude Code",
      windsurf: "Windsurf",
      other: "Other",
    };
    return ideKind.value ? labels[ideKind.value] : "";
  });

  onMounted(() => {
    if (!loaded.value) void loadBinding();
  });

  return {
    ideKind,
    ideCapabilities,
    loaded,
    supportsMcpInject,
    supportsRulePrompt,
    supportsUsage,
    ideLabel,
    loadBinding,
    switchIde,
  };
}
