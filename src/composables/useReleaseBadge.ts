import { computed, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** Keep in sync with `src-tauri/src/release_check.rs` RELAY_REPO_* (web fallback URLs). */
export const RELAY_REPO_HOME = "https://github.com/andeya/ide-relay-mcp";
export const RELAY_REPO_RELEASES_LATEST = `${RELAY_REPO_HOME}/releases/latest`;

export type ReleaseCheckPayload = {
  current_version: string;
  latest_version: string | null;
  update_available: boolean;
  check_error: string | null;
};

const CACHE_KEY = "relay_release_check_v1";
const TTL_MS = 4 * 60 * 60 * 1000;

export function useReleaseBadge() {
  const payload = ref<ReleaseCheckPayload | null>(null);
  const loading = ref(true);

  async function refresh() {
    loading.value = true;
    try {
      const raw = sessionStorage.getItem(CACHE_KEY);
      if (raw) {
        try {
          const parsed = JSON.parse(raw) as {
            at: number;
            data: ReleaseCheckPayload;
          };
          if (
            typeof parsed?.at === "number" &&
            Date.now() - parsed.at < TTL_MS &&
            parsed.data &&
            !parsed.data.check_error
          ) {
            payload.value = parsed.data;
            loading.value = false;
            return;
          }
        } catch {
          sessionStorage.removeItem(CACHE_KEY);
        }
      }
      const data = await invoke<ReleaseCheckPayload>("check_github_latest_release");
      payload.value = data;
      if (!data.check_error) {
        sessionStorage.setItem(
          CACHE_KEY,
          JSON.stringify({ at: Date.now(), data }),
        );
      }
    } catch {
      payload.value = null;
    } finally {
      loading.value = false;
    }
  }

  async function openRepo() {
    const releasesLatest = !!payload.value?.update_available;
    try {
      await invoke("open_relay_github_repo", { releasesLatest });
    } catch {
      const url = releasesLatest ? RELAY_REPO_RELEASES_LATEST : RELAY_REPO_HOME;
      window.open(url, "_blank");
    }
  }

  onMounted(() => {
    void refresh();
  });

  const badgeTitle = computed(() => {
    const p = payload.value;
    if (!p) return "";
    if (p.check_error) return p.check_error;
    if (p.update_available && p.latest_version) {
      return `GitHub · latest v${p.latest_version}`;
    }
    return `GitHub · v${p.current_version}`;
  });

  return { payload, loading, refresh, openRepo, badgeTitle };
}
