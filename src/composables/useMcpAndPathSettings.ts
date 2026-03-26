/**
 * Settings: public install (PATH), IDE-specific install (MCP + rule), copy JSON.
 */
import { computed, ref, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { locale, t } from "../i18n";
import type { LocaleKey } from "../i18n";
import { getRelayRulePromptBilingual } from "../ideRulesTemplates";
import type { IdeKind, PathEnvStatus } from "../types/relay-app";

export type RefreshHubResult = {
  ok: boolean;
  mcpConfigReadFailed: boolean;
  fatalError?: string;
};

export function useMcpAndPathSettings(
  ideLabel: Ref<string>,
  ideKind: Ref<IdeKind | null>,
) {
  const mcpJson = ref("");
  const ideMcpInstalled = ref(false);
  const ideRuleInstalled = ref(false);
  const ideMcpPath = ref("");
  const hubMsg = ref("");
  const hubErr = ref("");
  const hubInstallBusy = ref(false);
  const hubUninstallBusy = ref(false);
  const copyToast = ref("");

  const pathEnv = ref<PathEnvStatus | null>(null);
  const pathEnvMsg = ref("");
  const pathEnvErr = ref("");
  const pathEnvBusy = ref(false);

  const IDE_HINT_KEYS: Record<IdeKind, LocaleKey> = {
    cursor: "ideHintCursor",
    claude_code: "ideHintClaude",
    windsurf: "ideHintWindsurf",
    other: "ideHintVscode",
  };

  const ideHintsBlock = computed(() => {
    void locale.value;
    const kind = ideKind.value;
    if (!kind) return "";
    const key = IDE_HINT_KEYS[kind];
    if (kind === "cursor") {
      return t(key, { cursorPath: ideMcpPath.value || "~/.cursor/mcp.json" });
    }
    if (kind === "windsurf") {
      return t(key, {
        windsurfPath:
          ideMcpPath.value || "~/.codeium/windsurf/mcp_config.json",
      });
    }
    return t(key);
  });

  async function refreshPathEnv() {
    try {
      pathEnv.value = await invoke<PathEnvStatus>("get_relay_path_env_status");
    } catch {
      pathEnv.value = null;
    }
  }

  async function refreshMcpHub(): Promise<RefreshHubResult> {
    let mcpConfigReadFailed = false;
    try {
      mcpJson.value = await invoke<string>("get_mcp_config_json");
    } catch (e) {
      mcpConfigReadFailed = true;
      mcpJson.value = "// " + (e instanceof Error ? e.message : String(e));
    }
    try {
      try {
        ideMcpInstalled.value = await invoke<boolean>("ide_has_relay_mcp");
      } catch {
        ideMcpInstalled.value = false;
      }
      try {
        ideRuleInstalled.value = await invoke<boolean>("ide_rule_installed");
      } catch {
        ideRuleInstalled.value = false;
      }
      try {
        ideMcpPath.value = await invoke<string>("ide_mcp_json_path");
      } catch {
        ideMcpPath.value = "";
      }
      await refreshPathEnv();
      return { ok: true, mcpConfigReadFailed };
    } catch (e) {
      return {
        ok: false,
        mcpConfigReadFailed,
        fatalError: e instanceof Error ? e.message : String(e),
      };
    }
  }

  async function copyMcpJson() {
    copyToast.value = "";
    try {
      await navigator.clipboard.writeText(mcpJson.value);
      copyToast.value = t("mcpCopied");
    } catch {
      copyToast.value = t("mcpCopyErr");
    }
    setTimeout(() => {
      copyToast.value = "";
    }, 2500);
  }

  const ideInstallBusy = ref(false);
  const ideUninstallBusy = ref(false);

  async function doPublicInstall() {
    hubInstallBusy.value = true;
    hubErr.value = "";
    hubMsg.value = "";
    try {
      await invoke<string>("configure_relay_path_env_permanent");
      hubMsg.value = t("publicInstallOk");
      await refreshMcpHub();
    } catch (e) {
      hubErr.value =
        t("mcpFullErr") + " " + (e instanceof Error ? e.message : String(e));
    } finally {
      hubInstallBusy.value = false;
    }
  }

  async function runPublicUninstall() {
    hubUninstallBusy.value = true;
    hubErr.value = "";
    hubMsg.value = "";
    try {
      await invoke("remove_relay_path_env");
      hubMsg.value = t("publicUninstallOk");
      await refreshMcpHub();
    } catch (e) {
      hubErr.value =
        t("mcpFullErr") + " " + (e instanceof Error ? e.message : String(e));
    } finally {
      hubUninstallBusy.value = false;
    }
  }

  async function doIdeInstall() {
    ideInstallBusy.value = true;
    hubErr.value = "";
    hubMsg.value = "";
    try {
      await invoke("ide_install_relay_mcp");
      const ruleContent = getRelayRulePromptBilingual("loop", ideKind.value || undefined);
      await invoke("ide_install_rule", { content: ruleContent });
      hubMsg.value = t("ideInstallOk", { ide: ideLabel.value || "IDE" });
      await refreshMcpHub();
    } catch (e) {
      hubErr.value =
        t("mcpFullErr") + " " + (e instanceof Error ? e.message : String(e));
    } finally {
      ideInstallBusy.value = false;
    }
  }

  async function runIdeUninstall() {
    ideUninstallBusy.value = true;
    hubErr.value = "";
    hubMsg.value = "";
    try {
      await invoke("ide_uninstall_relay_mcp");
      try { await invoke("ide_uninstall_rule"); } catch { /* best effort */ }
      hubMsg.value = t("ideUninstallOk", { ide: ideLabel.value || "IDE" });
      await refreshMcpHub();
    } catch (e) {
      hubErr.value =
        t("mcpFullErr") + " " + (e instanceof Error ? e.message : String(e));
    } finally {
      ideUninstallBusy.value = false;
    }
  }

  async function configureRelayPath() {
    if (!pathEnv.value || pathEnv.value.configured || pathEnvBusy.value) {
      return;
    }
    const plat = pathEnv.value.platform;
    pathEnvBusy.value = true;
    pathEnvErr.value = "";
    pathEnvMsg.value = "";
    try {
      const r = await invoke<string>("configure_relay_path_env_permanent");
      await refreshPathEnv();
      if (r === "already") {
        pathEnvMsg.value = t("pathEnvAlready");
      } else if (r === "windows") {
        pathEnvMsg.value = t("pathEnvDoneWin");
      } else if (plat === "macos") {
        pathEnvMsg.value = t("pathEnvDoneMac");
      } else if (plat === "linux") {
        pathEnvMsg.value = t("pathEnvDoneLinux");
      } else {
        pathEnvMsg.value = t("pathEnvDoneOther");
      }
    } catch (e) {
      pathEnvErr.value =
        t("pathEnvErrPrefix") +
        " " +
        (e instanceof Error ? e.message : String(e));
    } finally {
      pathEnvBusy.value = false;
    }
  }

  return {
    mcpJson,
    ideMcpInstalled,
    ideRuleInstalled,
    ideMcpPath,
    hubMsg,
    hubErr,
    hubInstallBusy,
    hubUninstallBusy,
    ideInstallBusy,
    ideUninstallBusy,
    copyToast,
    pathEnv,
    pathEnvMsg,
    pathEnvErr,
    pathEnvBusy,
    ideHintsBlock,
    refreshMcpHub,
    copyMcpJson,
    doPublicInstall,
    runPublicUninstall,
    doIdeInstall,
    runIdeUninstall,
    configureRelayPath,
  };
}
