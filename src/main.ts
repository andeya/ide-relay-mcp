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
syncRelayDocHiddenClass();
document.addEventListener("visibilitychange", syncRelayDocHiddenClass);

createApp(App).mount("#app");
