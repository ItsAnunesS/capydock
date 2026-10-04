import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export function useTrayNavigation(
  navigate: (page: "settings") => void,
  reportError: (error: string) => void,
) {
  let disposed = false;
  const listeners: UnlistenFn[] = [];
  async function consume() {
    try {
      const page = await invoke<string | null>("take_tray_navigation");
      if (!disposed && page === "settings") navigate(page);
    } catch (error) {
      if (!disposed) reportError(String(error));
    }
  }
  onMounted(async () => {
    if (!isTauri()) return;
    try {
      for (const [event, callback] of [
        ["tray-navigation", consume],
        [
          "tray-error",
          (event: { payload: string }) => {
            if (!disposed) reportError(event.payload);
          },
        ],
      ] as const) {
        const unlisten = await listen<string>(event, callback);
        if (disposed) {
          unlisten();
          return;
        }
        listeners.push(unlisten);
      }
      // Read after registering: a tray click during frontend startup must not
      // disappear just because the webview wasn't listening to events yet.
      await consume();
    } catch (error) {
      if (!disposed) reportError(String(error));
    }
  });
  onUnmounted(() => {
    disposed = true;
    for (const unlisten of listeners) unlisten();
  });
}
