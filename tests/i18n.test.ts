import { libraryView } from "./library-fixture";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import { defineComponent, h, nextTick } from "vue";
import { readFileSync } from "node:fs";
import { parse } from "@vue/compiler-sfc";
import legacyKeys from "../app/i18n/legacy-keys.json";
import {
  catalogs,
  defaultLocale,
  encodeMessage,
  localeStorageKey,
  messagePrefix,
  savedLocale,
  setLocale,
  useI18n,
  type Locale,
} from "../app/composables/useI18n";
import { useDrive } from "../app/composables/useDrive";
import LanguageSetting from "../app/components/LanguageSetting.vue";
import TraySetting from "../app/components/TraySetting.vue";
import OperationQueue from "../app/components/OperationQueue.vue";
import PairCard from "../app/components/PairCard.vue";
import CloudLibrary from "../app/components/CloudLibrary.vue";
import type { DriveState, Operation, SyncPair } from "../app/types";

const api = vi.hoisted(() => ({ invoke: vi.fn(), native: true }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: api.invoke,
  isTauri: () => api.native,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));
const global = { stubs: { AppIcon: true, PdfPreview: true } };
beforeEach(() => {
  setLocale("pt");
  api.native = true;
  api.invoke.mockReset();
});
afterEach(() => {
  setLocale("pt");
});
const i18n = useI18n();
const pair: SyncPair = {
  id: "folder-1",
  name: "Documentos",
  localPath: "/home/test/Documentos",
  remotePath: "/my-files/Documentos",
  mode: "bidirectional",
  intervalMinutes: 5,
  enabled: true,
  lastRun: 1791045000,
  propagateDeletions: false,
};
const job: Operation = {
  id: "job-1",
  lane: "transfer",
  kind: "sync",
  title: encodeMessage("Sincronizar {0}", ["Documentos"]),
  detail: encodeMessage("Enviando · {0}", ["Documentos/Relatório {0}.pdf"]),
  pairId: pair.id,
  automatic: true,
  status: "running",
  createdAt: 10,
  startedAt: 15,
  finishedAt: null,
  error: null,
  canCancelRunning: true,
};
function driveState(): DriveState {
  return {
    config: {
      locale: "pt",
      pairs: [pair],
      autoUpdate: false,
      paused: false,
      closeToTray: false,
      computerRegistrations: {},
      lastUpdateCheck: null,
      events: [],
      accountId: null,
      accountEmail: null,
    },
    runtime: {
      connected: true,
      busy: true,
      operation: "sync",
      currentPair: pair.id,
      currentFile: job.detail,
      cliVersion: "0.8.0",
      error: null,
    },
    dataPath: "/tmp/test",
    autostart: false,
    trayAvailable: true,
    computer: null,
    queue: { paused: false, items: [job] },
  };
}
describe("translations", () => {
  it("reads English source messages and every legacy key without translating user data", () => {
    for (const lang of ["en", "pt", "es"] as const) {
      setLocale(lang);
      for (const [oldKey, key] of Object.entries(legacyKeys)) {
        expect(Object.hasOwn(catalogs.en, key), key).toBe(true);
        expect(i18n.message(oldKey)).toBe(i18n.t(key));
        expect(
          i18n.message(encodeMessage(oldKey, ["Documentos/{0}.pdf"])),
        ).toBe(i18n.message(encodeMessage(key, ["Documentos/{0}.pdf"])));
      }
    }
    setLocale("en");
    expect(i18n.message(encodeMessage("Sync {0}", ["Documentos"]))).toBe(
      "Sync Documentos",
    );
    expect(i18n.message("constructor")).toBe("constructor");
    expect(i18n.message("__proto__")).toBe("__proto__");
  });
  it("uses English for a fresh profile, invalid preferences and unavailable storage", () => {
    expect(defaultLocale).toBe("en");
    localStorage.removeItem(localeStorageKey);
    expect(savedLocale()).toBe("en");
    for (const unsupported of ["", "unsupported", "null"]) {
      localStorage.setItem(localeStorageKey, unsupported);
      expect(savedLocale()).toBe("en");
    }
    const storage = vi
      .spyOn(Storage.prototype, "getItem")
      .mockImplementation(() => {
        throw new Error("Storage is unavailable");
      });
    try {
      expect(savedLocale()).toBe("en");
    } finally {
      storage.mockRestore();
    }
    setLocale("unsupported" as Locale);
    expect(i18n.language.value).toBe("en");
    expect(document.documentElement.lang).toBe("en-US");
    expect(i18n.t("Configurações")).toBe("Settings");
    expect(i18n.message("Caminho inválido.")).toBe("Invalid path.");
  });
  it("falls back to English for missing or empty translations, including nested native errors", () => {
    const key = "Couldn't save the language: {0}";
    for (const lang of ["pt", "es"] as const) {
      setLocale(lang);
      const original = catalogs[lang][key]!;
      const nested = catalogs[lang]["Invalid path."]!;
      try {
        delete catalogs[lang][key];
        catalogs[lang]["Invalid path."] = "  ";
        expect(
          i18n.message(encodeMessage(key, [{ message: "Caminho inválido." }])),
        ).toBe("Couldn't save the language: Invalid path.");
        catalogs[lang][key] = "";
        expect(i18n.t(key, ["ECONNRESET"])).toBe(
          "Couldn't save the language: ECONNRESET",
        );
        expect(i18n.t("Unknown external diagnostic")).toBe(
          "Unknown external diagnostic",
        );
      } finally {
        catalogs[lang][key] = original;
        catalogs[lang]["Invalid path."] = nested;
      }
    }
  });
  it("has matching catalogs and parameters in all three languages", () => {
    const parameters = (s: string) =>
      [...s.matchAll(/\{\d+\}/g)].map((x) => x[0]).sort();
    for (const dictionary of Object.values(catalogs)) {
      expect(Object.keys(dictionary).sort()).toEqual(
        Object.keys(catalogs.pt).sort(),
      );
      for (const [key, value] of Object.entries(dictionary)) {
        expect(value.trim(), key).not.toBe("");
        expect(parameters(value), key).toEqual(parameters(key));
      }
    }
  });
  it("keeps all visible Vue text and accessible labels translated", () => {
    const files = [
      "app.vue",
      ...[
        "CloudLibrary",
        "ComputerConnection",
        "LanguageSetting",
        "TraySetting",
        "LibraryDialog",
        "OperationQueue",
        "PairCard",
        "PairDialog",
        "PdfPreview",
      ].map((n) => `components/${n}.vue`),
    ];
    const invariant = new Set([
      "Proton Drive",
      "CapyDock",
      "Proton Drive CLI",
      "DESKTOP",
      "Ctrl K",
      "Linux",
    ]);
    function visit(node: any, file: string) {
      if (node.type === 2 && /[A-Za-zÀ-ÿ]/.test(node.content))
        expect(
          invariant.has(node.content.trim()),
          `${file}: untranslated text ${node.content}`,
        ).toBe(true);
      if (node.type === 1)
        for (const prop of node.props) {
          if (
            prop.type === 6 &&
            !(prop.name === "alt" && prop.value?.content === "") &&
            ["aria-label", "placeholder", "title", "alt"].includes(prop.name)
          )
            throw new Error(`${file}: static ${prop.name}`);
        }
      for (const child of node.children ?? []) visit(child, file);
    }
    for (const file of files) {
      const source = readFileSync(`${process.cwd()}/app/${file}`, "utf8");
      visit(parse(source).descriptor.template!.ast, file);
      for (const match of source.matchAll(
        /\bt\(\s*(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)')/g,
      )) {
        const key = (match[1] ?? match[2]!).replace(/\\(['"\\])/g, "$1");
        expect(Object.hasOwn(catalogs.en, key), `${file}: missing ${key}`).toBe(
          true,
        );
      }
    }
  });
  it("formats plurals, numbers and document language, and restores the saved choice", () => {
    for (const lang of ["pt", "en", "es"] as const) {
      setLocale(lang);
      expect(savedLocale()).toBe(lang);
      expect(document.documentElement.lang).toBe(i18n.locale.value);
      expect(i18n.number(12345.6)).toBe(
        new Intl.NumberFormat(i18n.locale.value).format(12345.6),
      );
    }
    setLocale("en");
    expect(i18n.plural("{0} foto", "{0} fotos", 1)).toBe("1 photo");
    expect(i18n.plural("{0} foto", "{0} fotos", 2)).toBe("2 photos");
    setLocale("pt");
    expect(i18n.plural("{0} ativa", "{0} ativas", 0)).toBe("0 ativas");
    localStorage.setItem(localeStorageKey, "unsupported");
    expect(savedLocale()).toBe("en");
  });
  it("translates nested native diagnostics without interpreting filenames, markup or unknown CLI text", () => {
    const path = "Documentos/<script>alert(1)</script>/{0}.pdf";
    const encoded = encodeMessage("{0}: {1}", [
      path,
      {
        message: encodeMessage("Atualização: {0}", [
          { message: "Caminho inválido." },
        ]),
      },
    ]);
    setLocale("en");
    expect(i18n.message(encoded)).toBe(`${path}: Update: Invalid path.`);
    setLocale("es");
    expect(i18n.message(encoded)).toBe(
      `${path}: Actualización: Ruta no válida.`,
    );
    expect(i18n.message(encodeMessage("{0}", ["Documentos"]))).toBe(
      "Documentos",
    );
    expect(i18n.message("raw CLI ECONNRESET 123")).toBe(
      "raw CLI ECONNRESET 123",
    );
    expect(i18n.message(messagePrefix + "broken")).toBe(
      messagePrefix + "broken",
    );
    expect(
      i18n.message(String(new Error(encodeMessage("Caminho inválido.")))),
    ).toBe("Ruta no válida.");
  });
  it("switches a mounted queue, controls and persisted activity messages without changing operations", async () => {
    const queue = {
      paused: false,
      items: [job, { ...job, id: "job-2", status: "queued" as const }],
    };
    const original = JSON.stringify(queue);
    const wrapper = mount(OperationQueue, {
      props: { queue, native: true },
      global,
    });
    for (const [lang, heading, running, cancel] of [
      ["pt", "Fila de operações", "Sincronizar Documentos", "Cancelar"],
      ["en", "Operation queue", "Sync Documentos", "Cancel"],
      ["es", "Cola de operaciones", "Sincronizar Documentos", "Cancelar"],
    ] as const) {
      setLocale(lang);
      await nextTick();
      expect(wrapper.get("h1").text()).toBe(heading);
      expect(wrapper.get(".queue-running h3").text()).toBe(running);
      expect(wrapper.get(".queue-detail").text()).toContain(
        "Documentos/Relatório {0}.pdf",
      );
      expect(
        wrapper.get(".queue-waiting button").attributes("aria-label"),
      ).toBe(`${cancel} ${running}`);
    }
    expect(JSON.stringify(queue)).toBe(original);
    wrapper.unmount();
  });
  it("updates folder dates and statuses while preserving folder names and paths", async () => {
    const wrapper = mount(PairCard, {
      props: { pair, syncing: false, paused: false, connected: true },
      global,
    });
    setLocale("en");
    await nextTick();
    expect(wrapper.get("h3").text()).toBe("Documentos");
    expect(wrapper.text()).toContain("Both ways");
    expect(wrapper.text()).toContain(
      new Date(pair.lastRun! * 1000).toLocaleString("en-US", {
        day: "2-digit",
        month: "short",
        hour: "2-digit",
        minute: "2-digit",
      }),
    );
    setLocale("es");
    await nextTick();
    expect(wrapper.text()).toContain("En ambos sentidos");
    expect(wrapper.text()).toContain(pair.localPath);
    wrapper.unmount();
  });
  it("changes library tabs live and keeps native file data intact", async () => {
    const command = vi.fn(async () =>
      libraryView([
        {
          uid: "1",
          name: "Documentos",
          path: "/my-files/Documentos",
          directory: true,
          kind: "folder",
          size: 0,
        },
      ]),
    );
    const wrapper = mount(CloudLibrary, {
      props: { command: command as never, connected: true, native: true },
      global,
    });
    await flushPromises();
    setLocale("en");
    await nextTick();
    expect(wrapper.findAll('[role="tab"]').map((t) => t.text())).toEqual([
      "Files",
      "Computers",
      "Documents",
      "Photos",
      "Albums",
    ]);
    expect(wrapper.get(".file-name-button strong").text()).toBe("Documentos");
    setLocale("es");
    await nextTick();
    expect(wrapper.findAll('[role="tab"]').map((t) => t.text())).toEqual([
      "Archivos",
      "Ordenadores",
      "Documentos",
      "Fotos",
      "Álbumes",
    ]);
    expect(command).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });
});
describe("language preference", () => {
  it("starts the web preview in English without a saved preference", async () => {
    localStorage.removeItem(localeStorageKey);
    api.native = false;
    let drive!: ReturnType<typeof useDrive>;
    const wrapper = mount(
      defineComponent({
        setup() {
          drive = useDrive();
          return () => h(LanguageSetting, { onChange: drive.changeLanguage });
        },
      }),
      { global },
    );
    await flushPromises();
    expect(drive.state.value.config.locale).toBe("en");
    expect(i18n.language.value).toBe("en");
    expect(wrapper.get("h2").text()).toBe("Language");
    expect((wrapper.get("select").element as HTMLSelectElement).value).toBe(
      "en",
    );
    expect(wrapper.findAll("option")[0]!.text()).toBe("English");
    expect(api.invoke).not.toHaveBeenCalled();
    wrapper.unmount();
  });
  it("falls back to English if native settings return an unsupported locale", async () => {
    const state = driveState();
    state.config.locale = "unsupported" as Locale;
    api.invoke.mockResolvedValue(state);
    const wrapper = mount(
      defineComponent({
        setup() {
          useDrive();
          return () => h(LanguageSetting);
        },
      }),
      { global },
    );
    await flushPromises();
    expect(i18n.language.value).toBe("en");
    expect(savedLocale()).toBe("en");
    expect(wrapper.get("h2").text()).toBe("Language");
    wrapper.unmount();
  });
  it("restores native preferences and saves through the actual selector without touching transfers", async () => {
    const state = driveState();
    state.config.locale = "es";
    api.invoke.mockImplementation(async (name) =>
      name === "get_state" ? structuredClone(state) : undefined,
    );
    let drive!: ReturnType<typeof useDrive>;
    const wrapper = mount(
      defineComponent({
        setup() {
          drive = useDrive();
          return () =>
            h(LanguageSetting, {
              onChange: drive.changeLanguage,
              saving: drive.localeChanging.value,
            });
        },
      }),
      { global },
    );
    await flushPromises();
    expect(wrapper.get("h2").text()).toBe("Idioma");
    expect(i18n.language.value).toBe("es");
    await wrapper.get("select").setValue("en");
    await flushPromises();
    expect(wrapper.get("h2").text()).toBe("Language");
    expect(api.invoke).toHaveBeenCalledWith("set_locale", { locale: "en" });
    expect(savedLocale()).toBe("en");
    expect(drive.state.value.config.pairs).toEqual([pair]);
    expect(drive.state.value.queue).toEqual(state.queue);
    expect(drive.state.value.config.autoUpdate).toBe(false);
    wrapper.unmount();
  });
  it("rolls back the selection if native saving fails", async () => {
    api.invoke.mockImplementation(async (name) => {
      if (name === "get_state") return driveState();
      throw "Caminho inválido.";
    });
    let drive!: ReturnType<typeof useDrive>;
    const wrapper = mount(
      defineComponent({
        setup() {
          drive = useDrive();
          return () => h(LanguageSetting, { onChange: drive.changeLanguage });
        },
      }),
      { global },
    );
    await flushPromises();
    await wrapper.get("select").setValue("es");
    await flushPromises();
    expect(i18n.language.value).toBe("pt");
    expect(savedLocale()).toBe("pt");
    expect(i18n.message(drive.notification.value!.message)).toBe(
      "Não foi possível salvar o idioma: Caminho inválido.",
    );
    wrapper.unmount();
  });
});

describe("system tray preference", () => {
  function mountTray() {
    let drive!: ReturnType<typeof useDrive>;
    const wrapper = mount(
      defineComponent({
        setup() {
          drive = useDrive();
          return () =>
            h(TraySetting, {
              enabled: drive.state.value.config.closeToTray,
              available: drive.state.value.trayAvailable,
              native: drive.native.value,
              saving: drive.preferencesSaving.value,
              onChange: (closeToTray: boolean) =>
                drive.preferences({ closeToTray }),
            });
        },
      }),
      { global },
    );
    return { wrapper, drive };
  }

  it("saves from the switch, restores after reopening settings and leaves transfers intact", async () => {
    const state = driveState();
    let finishSaving!: () => void;
    api.invoke.mockImplementation(async (name, args) => {
      if (name === "get_state") return structuredClone(state);
      if (name === "set_preferences") {
        await new Promise<void>((resolve) => {
          finishSaving = resolve;
        });
        state.config.closeToTray = args.closeToTray;
      }
    });
    const { wrapper, drive } = mountTray();
    await flushPromises();
    await wrapper.get("input").setValue(true);
    expect(wrapper.get("input").attributes("disabled")).toBeDefined();
    expect(api.invoke).toHaveBeenCalledWith("set_preferences", {
      paused: false,
      autoUpdate: false,
      autostart: false,
      closeToTray: true,
    });
    finishSaving();
    await flushPromises();
    expect((wrapper.get("input").element as HTMLInputElement).checked).toBe(
      true,
    );
    expect(drive.state.value.queue).toEqual(state.queue);
    expect(drive.state.value.config.pairs).toEqual([pair]);
    expect(drive.state.value.config.paused).toBe(false);
    expect(drive.state.value.config.autoUpdate).toBe(false);
    wrapper.unmount();
    const restored = mountTray();
    await flushPromises();
    expect(
      (restored.wrapper.get("input").element as HTMLInputElement).checked,
    ).toBe(true);
    await restored.wrapper.get("input").setValue(false);
    finishSaving();
    await flushPromises();
    expect(
      (restored.wrapper.get("input").element as HTMLInputElement).checked,
    ).toBe(false);
    expect(
      api.invoke.mock.calls.every(([name]) =>
        ["get_state", "set_preferences"].includes(name),
      ),
    ).toBe(true);
    restored.wrapper.unmount();
  });

  it("keeps the previous closing behavior selected if saving fails", async () => {
    api.invoke.mockImplementation(async (name) => {
      if (name === "get_state") return driveState();
      throw "Falha ao salvar preferências";
    });
    const { wrapper, drive } = mountTray();
    await flushPromises();
    await wrapper.get("input").setValue(true);
    await flushPromises();
    expect((wrapper.get("input").element as HTMLInputElement).checked).toBe(
      false,
    );
    expect(wrapper.get("input").attributes("disabled")).toBeUndefined();
    expect(drive.notification.value).toEqual({
      message: "Falha ao salvar preferências",
      error: true,
    });
    wrapper.unmount();
  });

  it("disables hiding if the native tray is unavailable and explains it in all languages", async () => {
    const state = driveState();
    state.trayAvailable = false;
    api.invoke.mockResolvedValue(state);
    const { wrapper } = mountTray();
    await flushPromises();
    for (const locale of ["pt", "en", "es"] as const) {
      setLocale(locale);
      await nextTick();
      expect(wrapper.get("#tray-status").text()).toBe(
        catalogs[locale]["The system tray is unavailable in this session."],
      );
      expect(wrapper.get("input").attributes("disabled")).toBeDefined();
    }
    expect(api.invoke).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });
});
