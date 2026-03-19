/**
 * Settings: full install/uninstall, copy JSON, optional PATH-only and per-IDE actions.
 */
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { locale, t } from "../i18n";
import type { PathEnvStatus } from "../types/relay-app";

export type RefreshHubResult = {
  ok: boolean;
  mcpConfigReadFailed: boolean;
  fatalError?: string;
};

export function useMcpAndPathSettings() {
  const mcpJson = ref("");
  const mcpCursorInstalled = ref(false);
  const cursorMcpPath = ref("");
  const mcpWindsurfInstalled = ref(false);
  const windsurfMcpPath = ref("");
  const mcpWindsurfBusy = ref(false);
  const hubMsg = ref("");
  const hubErr = ref("");
  const hubInstallBusy = ref(false);
  const hubUninstallBusy = ref(false);
  const mcpCursorBusy = ref(false);
  const copyToast = ref("");

  const pathEnv = ref<PathEnvStatus | null>(null);
  const pathEnvMsg = ref("");
  const pathEnvErr = ref("");
  const pathEnvBusy = ref(false);

  const ideHintsBlock = computed(() => {
    void locale.value;
    return [
      t("ideHintCursor", { cursorPath: cursorMcpPath.value || "~/.cursor/mcp.json" }),
      t("ideHintVscode"),
      t("ideHintWindsurf", {
        windsurfPath:
          windsurfMcpPath.value || "~/.codeium/windsurf/mcp_config.json",
      }),
      t("ideHintClaude"),
    ].join("\n\n———\n\n");
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
      mcpCursorInstalled.value = await invoke<boolean>("get_mcp_cursor_installed");
      mcpWindsurfInstalled.value = await invoke<boolean>(
        "get_mcp_windsurf_installed",
      );
      try {
        cursorMcpPath.value = await invoke<string>("get_cursor_mcp_json_path");
      } catch {
        cursorMcpPath.value = "~/.cursor/mcp.json";
      }
      try {
        windsurfMcpPath.value = await invoke<string>(
          "get_windsurf_mcp_json_path",
        );
      } catch {
        windsurfMcpPath.value = "~/.codeium/windsurf/mcp_config.json";
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

  async function doFullInstall() {
    hubInstallBusy.value = true;
    hubErr.value = "";
    hubMsg.value = "";
    try {
      const r = await invoke<Record<string, unknown>>("relay_full_install");
      hubMsg.value = t("mcpFullInstallOk");
      const pErr = r.pathError;
      const hasPathErr =
        pErr != null && typeof pErr === "string" && pErr.length > 0;
      if (r.pathAction === "skipped" && hasPathErr) {
        hubMsg.value += " " + t("mcpPathSkippedNote");
      }
      await refreshMcpHub();
    } catch (e) {
      hubErr.value =
        t("mcpFullErr") + " " + (e instanceof Error ? e.message : String(e));
    } finally {
      hubInstallBusy.value = false;
    }
  }

  /** No browser confirm — Tauri webview often blocks it; use in-app UI before calling. */
  async function runFullUninstall() {
    hubUninstallBusy.value = true;
    hubErr.value = "";
    hubMsg.value = "";
    try {
      await invoke("relay_full_uninstall");
      hubMsg.value = t("mcpFullUninstallOk");
      await refreshMcpHub();
    } catch (e) {
      hubErr.value =
        t("mcpFullErr") + " " + (e instanceof Error ? e.message : String(e));
    } finally {
      hubUninstallBusy.value = false;
    }
  }

  async function installCursorMcpOnly() {
    mcpCursorBusy.value = true;
    hubErr.value = "";
    try {
      await invoke("install_mcp_to_cursor");
      hubMsg.value = t("mcpCursorInstallOk");
      await refreshMcpHub();
    } catch (e) {
      hubErr.value =
        t("mcpFullErr") + " " + (e instanceof Error ? e.message : String(e));
    } finally {
      mcpCursorBusy.value = false;
    }
  }

  async function uninstallCursorMcpOnly() {
    mcpCursorBusy.value = true;
    hubErr.value = "";
    try {
      await invoke("uninstall_mcp_from_cursor");
      hubMsg.value = t("mcpCursorUninstallOk");
      await refreshMcpHub();
    } catch (e) {
      hubErr.value =
        t("mcpFullErr") + " " + (e instanceof Error ? e.message : String(e));
    } finally {
      mcpCursorBusy.value = false;
    }
  }

  async function installWindsurfMcpOnly() {
    mcpWindsurfBusy.value = true;
    hubErr.value = "";
    try {
      await invoke("install_mcp_to_windsurf");
      hubMsg.value = t("mcpWindsurfInstallOk");
      await refreshMcpHub();
    } catch (e) {
      hubErr.value =
        t("mcpFullErr") + " " + (e instanceof Error ? e.message : String(e));
    } finally {
      mcpWindsurfBusy.value = false;
    }
  }

  async function uninstallWindsurfMcpOnly() {
    mcpWindsurfBusy.value = true;
    hubErr.value = "";
    try {
      await invoke("uninstall_mcp_from_windsurf");
      hubMsg.value = t("mcpWindsurfUninstallOk");
      await refreshMcpHub();
    } catch (e) {
      hubErr.value =
        t("mcpFullErr") + " " + (e instanceof Error ? e.message : String(e));
    } finally {
      mcpWindsurfBusy.value = false;
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
    mcpCursorInstalled,
    cursorMcpPath,
    mcpWindsurfInstalled,
    windsurfMcpPath,
    mcpWindsurfBusy,
    hubMsg,
    hubErr,
    hubInstallBusy,
    hubUninstallBusy,
    mcpCursorBusy,
    copyToast,
    pathEnv,
    pathEnvMsg,
    pathEnvErr,
    pathEnvBusy,
    ideHintsBlock,
    refreshMcpHub,
    copyMcpJson,
    doFullInstall,
    runFullUninstall,
    installCursorMcpOnly,
    uninstallCursorMcpOnly,
    installWindsurfMcpOnly,
    uninstallWindsurfMcpOnly,
    configureRelayPath,
  };
}
