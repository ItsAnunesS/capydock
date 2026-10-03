import { libraryView } from "./library-fixture";
import { describe, it, expect, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import PairDialog from "../app/components/PairDialog.vue";
import ComputerConnection from "../app/components/ComputerConnection.vue";
import PairCard from "../app/components/PairCard.vue";
import type { SyncPair } from "../app/types";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(async () => "/home/test/Documentos"),
}));
const global = { stubs: { AppIcon: true } };
const pair: SyncPair = {
  id: "pair-1",
  name: "Documentos",
  localPath: "/home/test/Documentos",
  remotePath: "/my-files/Documentos",
  mode: "bidirectional",
  intervalMinutes: 5,
  enabled: true,
  lastRun: null,
  propagateDeletions: false,
};

describe("folder setup", () => {
  it("picks a native directory, browses Drive and saves the chosen direction", async () => {
    const command = vi.fn(async (name: string) =>
      name === "list_remote"
        ? [
            {
              name: "Documentos",
              path: "/my-files/Documentos",
              directory: true,
            },
          ]
        : undefined,
    );
    const wrapper = mount(PairDialog, {
      props: { command: command as never },
      global,
    });
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Escolher")!
      .trigger("click");
    await flushPromises();
    expect((wrapper.get("#local-path").element as HTMLInputElement).value).toBe(
      "/home/test/Documentos",
    );
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Explorar")!
      .trigger("click");
    await flushPromises();
    await wrapper.get(".folder-list button").trigger("click");
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Usar esta pasta")!
      .trigger("click");
    await wrapper.get("#sync-mode").setValue("upload");
    await wrapper.get("#sync-interval").setValue("15");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(command).toHaveBeenLastCalledWith("save_pair", {
      pair: expect.objectContaining({
        name: "Documentos",
        localPath: "/home/test/Documentos",
        remotePath: "/my-files/Documentos",
        mode: "upload",
        intervalMinutes: 15,
      }),
    });
    expect(wrapper.emitted("saved")).toHaveLength(1);
    wrapper.unmount();
  });

  it("keeps the form open and explains a backend validation failure", async () => {
    const command = vi.fn(async () => {
      throw new Error("Pastas sobrepostas");
    });
    const wrapper = mount(PairDialog, { props: { pair, command }, global });
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain(
      "Pastas sobrepostas",
    );
    expect(wrapper.emitted("saved")).toBeUndefined();
    expect(wrapper.get("dialog").attributes("open")).toBeDefined();
    wrapper.unmount();
  });

  it("prevents editing folder locations under an existing baseline", () => {
    const wrapper = mount(PairDialog, {
      props: { pair, command: vi.fn() },
      global,
    });
    expect(wrapper.get("#local-path").attributes("readonly")).toBeDefined();
    expect(wrapper.get("#remote-path").attributes("readonly")).toBeDefined();
    wrapper.unmount();
  });
});

describe("folder controls", () => {
  it("disables sync while disconnected and enables it after connection", async () => {
    const wrapper = mount(PairCard, {
      props: {
        pair,
        queued: false,
        changing: false,
        syncing: false,
        paused: false,
        connected: false,
      },
      global,
    });
    expect(
      wrapper
        .get('[aria-label="Sincronizar Documentos"]')
        .attributes("disabled"),
    ).toBeDefined();
    await wrapper.setProps({ connected: true });
    await wrapper.get('[aria-label="Sincronizar Documentos"]').trigger("click");
    expect(wrapper.emitted("sync")).toHaveLength(1);
    await wrapper.setProps({ changing: true });
    expect(
      wrapper.get('[aria-label="Pausar Documentos"]').attributes("disabled"),
    ).toBeDefined();
    wrapper.unmount();
  });

  it("shows an explicit paused state and offers resume", () => {
    const wrapper = mount(PairCard, {
      props: {
        pair: { ...pair, enabled: false },
        queued: false,
        changing: false,
        syncing: false,
        paused: false,
        connected: true,
      },
      global,
    });
    expect(wrapper.text()).toContain("Pausada");
    expect(wrapper.find('[aria-label="Ativar Documentos"]').exists()).toBe(
      true,
    );
    expect(
      wrapper
        .get('[aria-label="Sincronizar Documentos"]')
        .attributes("disabled"),
    ).toBeDefined();
    wrapper.unmount();
  });
});

import CloudLibrary from "../app/components/CloudLibrary.vue";
import LibraryDialog from "../app/components/LibraryDialog.vue";
const file = {
  name: "Manual.pdf",
  path: "/my-files/Manual.pdf",
  directory: false,
  revision: "r",
  size: 32,
  uid: "u",
  kind: "file",
  mediaType: "application/pdf",
  nativeDocument: false,
  photoCount: 0,
  modified: null,
};

describe("cloud library", () => {
  it("loads documents and previews PDFs through the native bridge", async () => {
    const command = vi.fn(async (name: string) =>
      name === "list_library"
        ? libraryView([file])
        : {
            kind: "pdf",
            mime: "application/pdf",
            content: "cGRm",
            name: file.name,
          },
    );
    const wrapper = mount(CloudLibrary, {
      props: { connected: true, native: true, command: command as never },
      global: { stubs: { AppIcon: true, PdfPreview: true } },
    });
    await flushPromises();
    await wrapper.get("#tab-documents").trigger("click");
    await flushPromises();
    expect(command).toHaveBeenCalledWith("list_library", {
      path: "/documents",
    });
    await wrapper.get(".file-name-button").trigger("click");
    await flushPromises();
    expect(command).toHaveBeenCalledWith("preview_file", { path: file.path });
    expect(wrapper.find("pdf-preview-stub").exists()).toBe(true);
    wrapper.unmount();
  });

  it("navigates an album by UID and prepares its sync pair", async () => {
    const album = {
      ...file,
      kind: "album",
      name: "Verão",
      path: "/albums/unique-uid",
      directory: true,
      photoCount: 2,
    };
    const command = vi.fn(
      async (_name: string, args?: Record<string, unknown>) =>
        libraryView(args?.path === "/albums" ? [album] : []),
    );
    const wrapper = mount(CloudLibrary, {
      props: { connected: true, native: true, command: command as never },
      global,
    });
    await flushPromises();
    await wrapper.get("#tab-albums").trigger("click");
    await flushPromises();
    await wrapper.get(".media-card").trigger("click");
    await flushPromises();
    expect(command).toHaveBeenLastCalledWith("list_library", {
      path: "/albums/unique-uid",
    });
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "Sincronizar aqui")!
      .trigger("click");
    expect(wrapper.emitted("sync")?.[0]).toEqual([
      expect.objectContaining({ remotePath: album.path, name: "Verão" }),
    ]);
    wrapper.unmount();
  });

  it("shows native documents online without attempting a binary download", async () => {
    const doc = {
      ...file,
      nativeDocument: true,
      name: "Notas",
      mediaType: "application/vnd.proton.doc",
    };
    const command = vi.fn(async (name: string) =>
      name === "list_library" ? libraryView([doc]) : undefined,
    );
    const wrapper = mount(CloudLibrary, {
      props: { connected: true, native: true, command: command as never },
      global,
    });
    await flushPromises();
    await wrapper.get(".file-name-button").trigger("click");
    await flushPromises();
    expect(command).not.toHaveBeenCalledWith("preview_file", expect.anything());
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "Abrir documento")!
      .trigger("click");
    await flushPromises();
    expect(command).toHaveBeenLastCalledWith("open_document", {
      path: doc.path,
    });
    expect(
      wrapper
        .findAll("button")
        .some((button) => button.text() === "Baixar arquivo"),
    ).toBe(false);
    wrapper.unmount();
  });

  it("does not show fake cloud items or enable sync without a connection", async () => {
    const command = vi.fn();
    const wrapper = mount(CloudLibrary, {
      props: { connected: false, native: true, command },
      global,
    });
    await wrapper.get("#tab-photos").trigger("click");
    expect(command).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("Sua biblioteca começa com uma conexão");
    expect(
      wrapper
        .findAll("button")
        .find((button) => button.text() === "Configurar biblioteca")!
        .attributes("disabled"),
    ).toBeDefined();
    wrapper.unmount();
  });
});

describe("complete library setup", () => {
  it("requires explicit opt-in for deletions and sends complete sync options", async () => {
    const command = vi.fn();
    const wrapper = mount(LibraryDialog, { props: { command }, global });
    expect(
      (wrapper.get(".deletion-option input").element as HTMLInputElement)
        .checked,
    ).toBe(false);
    await wrapper
      .findAll("button")
      .find((button) => button.text() === "Escolher")!
      .trigger("click");
    await flushPromises();
    await wrapper.get(".deletion-option input").setValue(true);
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(command).toHaveBeenCalledWith("setup_library", {
      localPath: "/home/test/Documentos",
      includePhotos: true,
      propagateDeletions: true,
    });
    expect(wrapper.emitted("saved")).toHaveLength(1);
    wrapper.unmount();
  });
});

describe("computers UI", () => {
  it("registers the computer, opens its folders and offers a new local pair", async () => {
    const device = {
      ...file,
      name: "Meu PC",
      path: "/devices/Meu PC",
      uid: "pc-1",
      kind: "device",
      directory: true,
      mediaType: "Linux",
    };
    let registered = false;
    const command = vi.fn(async (name: string) => {
      if (name === "computer_name") return "Meu PC";
      if (name === "register_computer") {
        registered = true;
        return device;
      }
      return libraryView(registered ? [device] : []);
    });
    const wrapper = mount(CloudLibrary, {
      props: { connected: true, native: true, command: command as never },
      global,
    });
    await flushPromises();
    await wrapper.get("#tab-computers").trigger("click");
    await flushPromises();
    expect(command).toHaveBeenCalledWith("list_library", { path: "/devices" });
    expect(wrapper.text()).not.toContain("Sincronizar aqui");
    await wrapper.get(".computer-register").trigger("submit");
    await flushPromises();
    expect(command).toHaveBeenCalledWith("register_computer", {
      name: "Meu PC",
    });
    expect(command).toHaveBeenLastCalledWith("list_library", {
      path: "/devices",
      force: true,
      revision: 1,
    });
    expect(wrapper.find(".computer-register").exists()).toBe(false);
    expect(wrapper.text()).toContain("Vinculado à conta");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Adicionar pasta deste PC")!
      .trigger("click");
    expect(wrapper.emitted("sync")?.[0]).toEqual([
      expect.objectContaining({ remotePath: device.path, name: "" }),
    ]);
    wrapper.unmount();
  });
  it("saves a new folder under the selected computer with an explicit root", async () => {
    const command = vi.fn();
    const wrapper = mount(PairDialog, {
      props: { preset: { remotePath: "/devices/Meu PC" }, command },
      global,
    });
    await wrapper.get("#pair-name").setValue("Documentos");
    await wrapper.get("#local-path").setValue("/home/test/Documentos");
    expect(wrapper.text()).toContain("Será criada a pasta");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(command).toHaveBeenCalledWith("save_pair", {
      computerRoot: "/devices/Meu PC",
      pair: expect.objectContaining({
        name: "Documentos",
        localPath: "/home/test/Documentos",
      }),
    });
    wrapper.unmount();
  });
  it("never offers the virtual Computers collection as a sync folder", async () => {
    const wrapper = mount(PairDialog, {
      props: { command: vi.fn(async () => []) as never },
      global,
    });
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Computadores")!
      .trigger("click");
    await flushPromises();
    expect(
      wrapper
        .findAll("button")
        .find((b) => b.text() === "Usar esta pasta")!
        .attributes("disabled"),
    ).toBeDefined();
    expect(wrapper.find(".new-folder").exists()).toBe(false);
    wrapper.unmount();
  });
});

describe("computer connection states", () => {
  const device = {
    ...file,
    name: "Meu PC",
    path: "/devices/Meu PC",
    uid: "pc-1",
    kind: "device",
    directory: true,
    mediaType: "Linux",
  };
  const props = {
    devices: [device],
    suggestedName: "hostname",
    loading: false,
    failed: false,
    native: true,
    busy: false,
  };
  it("requires an explicit choice when legacy computers exist and links by UID", async () => {
    const wrapper = mount(ComputerConnection, { props, global });
    expect(wrapper.get("button").attributes("disabled")).toBeDefined();
    expect(wrapper.find("#computer-name").exists()).toBe(false);
    await wrapper.get("select").setValue("pc-1");
    await wrapper.get("form").trigger("submit");
    expect(wrapper.emitted("register")).toEqual([["Meu PC", "pc-1"]]);
    wrapper.unmount();
  });
  it("restores the bound state after reopening, including a remote rename", async () => {
    const wrapper = mount(ComputerConnection, {
      props: {
        ...props,
        registration: { name: "Old hostname", deviceUid: "pc-1" },
      },
      global,
    });
    expect(wrapper.find("form").exists()).toBe(false);
    expect(wrapper.get("h2").text()).toBe("Meu PC");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Adicionar pasta deste PC")!
      .trigger("click");
    expect(wrapper.emitted("sync")).toEqual([[device]]);
    expect(wrapper.emitted("register")).toBeUndefined();
    wrapper.unmount();
  });
  it("keeps missing identities and pending retries from offering another registration", async () => {
    const wrapper = mount(ComputerConnection, {
      props: {
        ...props,
        registration: { name: "Missing", deviceUid: "missing" },
      },
      global,
    });
    expect(wrapper.find("form").exists()).toBe(false);
    expect(wrapper.text()).toContain("Registro indisponível");
    await wrapper.setProps({
      registration: { name: "Original", deviceUid: null },
    });
    expect(wrapper.find("input").exists()).toBe(false);
    await wrapper.get("form").trigger("submit");
    expect(wrapper.emitted("register")).toEqual([["Original", undefined]]);
    wrapper.unmount();
  });
  it("directs an already synchronized folder to management instead of a duplicate pair", async () => {
    const command = vi.fn(async () => libraryView());
    const wrapper = mount(CloudLibrary, {
      props: {
        connected: true,
        native: true,
        command: command as never,
        pairs: [{ ...pair, remotePath: "/my-files" }],
      },
      global,
    });
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Ver sincronização")!
      .trigger("click");
    expect(wrapper.emitted("manage")).toHaveLength(1);
    expect(wrapper.emitted("sync")).toBeUndefined();
    expect(wrapper.text()).not.toContain("Sincronizar tudo");
    wrapper.unmount();
  });
});

describe("registration while navigating", () => {
  it("does not replace another tab when the queued registration finishes", async () => {
    let finish!: (device: unknown) => void;
    const command = vi.fn(async (name: string) => {
      if (name === "computer_name") return "Meu PC";
      if (name === "register_computer")
        return new Promise((resolve) => {
          finish = resolve;
        });
      return libraryView();
    });
    const wrapper = mount(CloudLibrary, {
      props: { native: true, connected: true, command: command as never },
      global,
    });
    await flushPromises();
    await wrapper.get("#tab-computers").trigger("click");
    await flushPromises();
    await wrapper.get(".computer-register").trigger("submit");
    await wrapper.get("#tab-photos").trigger("click");
    await flushPromises();
    finish({
      ...file,
      name: "Meu PC",
      uid: "pc-1",
      path: "/devices/Meu PC",
      kind: "device",
      directory: true,
    });
    await flushPromises();
    expect(wrapper.get("#tab-photos").attributes("aria-selected")).toBe("true");
    expect(command).toHaveBeenLastCalledWith("list_library", {
      path: "/photos",
    });
    expect(wrapper.emitted("registered")).toHaveLength(1);
    wrapper.unmount();
  });
});
