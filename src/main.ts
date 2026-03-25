/** Vue app bootstrap for the Tauri webview. */
import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";

function syncRelayDocHiddenClass() {
  document.documentElement.classList.toggle(
    "relay-doc-hidden",
    document.visibilityState === "hidden",
  );
}

/** Pause decorative CSS animations when the webview loses focus (cheap idle CPU). */
function syncRelayWindowFocusClass() {
  document.documentElement.classList.toggle(
    "relay-window-unfocused",
    !document.hasFocus(),
  );
}

syncRelayDocHiddenClass();
syncRelayWindowFocusClass();
document.addEventListener("visibilitychange", syncRelayDocHiddenClass);
window.addEventListener("focus", syncRelayWindowFocusClass);
window.addEventListener("blur", syncRelayWindowFocusClass);

createApp(App).mount("#app");
