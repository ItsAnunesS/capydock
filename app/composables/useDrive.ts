import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DriveState } from "~/types";
import {
  useI18n,
  defaultLocale,
  savedLocale,
  isLocale,
  encodeMessage,
  type Locale,
} from "~/composables/useI18n";

export function useDrive() {
  const { language, setLocale } = useI18n();
  const localeChanging = ref(false);
  const preferencesSaving = ref(false);
  let localeRevision = 0;
  const native = ref(false);
  const loading = ref(true);
  const notification = ref<{ message: string; error: boolean } | null>(null);
  const state = ref<DriveState>({
    config: {
      locale: savedLocale(),
      pairs: [],
      autoUpdate: true,
      paused: false,
      closeToTray: false,
      computerRegistrations: {},
      lastUpdateCheck: null,
      events: [],
      accountId: null,
      accountEmail: null,
    },
    runtime: {
      connected: false,
      busy: false,
      operation: "",
      currentPair: null,
      currentFile: null,
      cliVersion: "",
      error: null,
    },
    dataPath: "",
    autostart: false,
    trayAvailable: false,
    computer: null,
    queue: { paused: false, items: [] },
  });
  let unlisten: UnlistenFn | undefined;
  let poll: ReturnType<typeof setInterval> | undefined;
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  let refreshPending = false;

  function toast(message: string, error = false) {
    notification.value = { message, error };
    clearTimeout(toastTimer);
    toastTimer = setTimeout(
      () => {
        notification.value = null;
      },
      error ? 12000 : 6000,
    );
  }
  async function refresh() {
    if (!native.value || refreshPending) return;
    refreshPending = true;
    const revision = localeRevision;
    try {
      state.value = await invoke<DriveState>("get_state");
      if (!localeChanging.value && revision === localeRevision)
        setLocale(
          isLocale(state.value.config.locale)
            ? state.value.config.locale
            : defaultLocale,
        );
    } catch (error) {
      toast(String(error), true);
    } finally {
      refreshPending = false;
    }
  }
  async function command<T = void>(
    name: string,
    args?: Record<string, unknown>,
  ): Promise<T> {
    if (!native.value)
      throw new Error(
        encodeMessage(
          "Abra o aplicativo desktop para conectar sua conta e acessar os arquivos do computador.",
        ),
      );
    return invoke<T>(name, args);
  }
  async function act(
    name: string,
    args?: Record<string, unknown>,
    success?: string,
  ) {
    try {
      await command(name, args);
      if (success) toast(success);
    } catch (error) {
      if (String(error) !== "Operação cancelada.") toast(String(error), true);
    } finally {
      await refresh();
    }
  }
  async function preferences(
    changes: Partial<{
      paused: boolean;
      autoUpdate: boolean;
      autostart: boolean;
      closeToTray: boolean;
    }>,
  ) {
    if (preferencesSaving.value) return;
    preferencesSaving.value = true;
    try {
      await act("set_preferences", {
        paused: state.value.config.paused,
        autoUpdate: state.value.config.autoUpdate,
        autostart: state.value.autostart,
        ...changes,
      });
    } finally {
      preferencesSaving.value = false;
    }
  }
  async function changeLanguage(next: Locale) {
    if (!isLocale(next) || localeChanging.value || language.value === next)
      return;
    const previous = language.value;
    localeChanging.value = true;
    localeRevision++;
    setLocale(next);
    try {
      if (native.value) await command("set_locale", { locale: next });
      state.value.config.locale = next;
    } catch (error) {
      setLocale(previous);
      toast(
        encodeMessage("Não foi possível salvar o idioma: {0}", [
          { message: String(error) },
        ]),
        true,
      );
    } finally {
      localeChanging.value = false;
    }
  }
  onMounted(async () => {
    setLocale(savedLocale());
    native.value = isTauri();
    if (native.value) {
      unlisten = await listen("drive-changed", refresh);
      await refresh();
      poll = setInterval(refresh, 5000);
    }
    loading.value = false;
  });
  onUnmounted(() => {
    unlisten?.();
    clearInterval(poll);
    clearTimeout(toastTimer);
  });
  return {
    state,
    native,
    loading,
    notification,
    refresh,
    command,
    act,
    preferences,
    preferencesSaving,
    toast,
    changeLanguage,
    localeChanging,
  };
}
