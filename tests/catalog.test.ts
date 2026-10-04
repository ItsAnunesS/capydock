import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import CloudLibrary from "../app/components/CloudLibrary.vue";
import { libraryView } from "./library-fixture";
// @ts-expect-error compiled helper is plain JavaScript
import { readFolderBatch } from "../scripts/computers/read-folder-batch.mjs";
const global = { stubs: { AppIcon: true, PdfPreview: true } };
const file = {
  name: "Projeto.pdf",
  path: "/my-files/Projeto.pdf",
  size: 10,
  mediaType: "application/pdf",
};
afterEach(() => vi.useRealTimers());
describe("responsive catalog", () => {
  it("keeps cached documents visible, deduplicates revision polling and cancels stale polling on navigation", async () => {
    vi.useFakeTimers();
    const command = vi.fn(
      async (_name: string, args?: Record<string, unknown>) => {
        if (_name === "computer_name") return "Meu PC";
        if (args?.path === "/documents")
          return libraryView(args.revision ? [] : [file], {
            entries: args.revision ? null : ([file] as never),
            refreshing: true,
            partial: true,
            foldersDone: 12,
            foldersPending: 90,
          });
        return libraryView();
      },
    );
    const wrapper = mount(CloudLibrary, {
      props: { connected: true, native: true, command: command as never },
      global,
    });
    await flushPromises();
    await wrapper.get("#tab-documents").trigger("click");
    await flushPromises();
    expect(wrapper.get(".file-name-button").text()).toContain(file.name);
    expect(wrapper.get(".library-refresh-note").text()).toContain(
      "12 folders checked",
    );
    await vi.advanceTimersByTimeAsync(1200);
    await flushPromises();
    expect(command).toHaveBeenLastCalledWith("list_library", {
      path: "/documents",
      revision: 1,
    });
    expect(wrapper.get(".file-name-button").text()).toContain(file.name);
    await wrapper.get("#tab-computers").trigger("click");
    await flushPromises();
    const calls = command.mock.calls.length;
    await vi.advanceTimersByTimeAsync(5000);
    expect(command).toHaveBeenCalledTimes(calls);
    wrapper.unmount();
  });
  it("keeps usable cached results on a refresh failure and permits explicit retry", async () => {
    const command = vi.fn(async () =>
      libraryView([file], { error: "Sem conexão" }),
    );
    const wrapper = mount(CloudLibrary, {
      props: { connected: true, native: true, command: command as never },
      global,
    });
    await flushPromises();
    expect(wrapper.get(".file-name-button").text()).toContain(file.name);
    expect(wrapper.get('[role="alert"]').text()).toContain("Sem conexão");
    await wrapper.get('[aria-label="Refresh library"]').trigger("click");
    await flushPromises();
    expect(command).toHaveBeenLastCalledWith("list_library", {
      path: "/my-files",
      force: true,
      revision: 1,
    });
    wrapper.unmount();
  });
});
describe("batched metadata SDK bridge", () => {
  it("fetches four folders concurrently in one session and uses UIDs without resolving ancestors", async () => {
    const releases: (() => void)[] = [];
    const sdk = {
      iterateFolderChildren: vi.fn(async function* (uid: string) {
        await new Promise<void>((resolve) => releases.push(resolve));
        yield { uid, name: "item" };
      }),
    };
    const paths = { getNode: vi.fn() };
    const requests = [1, 2, 3, 4].map((n) => ({
      path: `/my-files/folder${n}`,
      uid: `uid${n}`,
    }));
    const pending = readFolderBatch(sdk, paths, JSON.stringify(requests));
    await Promise.resolve();
    expect(releases).toHaveLength(4);
    releases.forEach((resolve) => resolve());
    expect((await pending).map((page: any) => page.path)).toEqual(
      requests.map((r) => r.path),
    );
    expect(paths.getNode).not.toHaveBeenCalled();
  });
  it("resolves the root once, propagates errors and rejects unsafe or oversized batches before SDK access", async () => {
    const paths = { getNode: vi.fn(async () => "root") };
    const sdk = {
      iterateFolderChildren: vi.fn(async function* () {
        throw new Error("offline");
        yield;
      }),
    };
    await expect(
      readFolderBatch(
        sdk,
        paths,
        JSON.stringify([{ path: "/my-files", uid: null }]),
      ),
    ).rejects.toThrow("offline");
    expect(paths.getNode).toHaveBeenCalledExactlyOnceWith("/my-files");
    sdk.iterateFolderChildren.mockClear();
    for (const input of [
      [],
      Array(5).fill({ path: "/my-files", uid: null }),
      [{ path: "/my-files/../private", uid: null }],
      [{ path: "/devices", uid: null }],
      [{ path: "/my-files", uid: 42 }],
    ]) {
      await expect(
        readFolderBatch(sdk, paths, JSON.stringify(input)),
      ).rejects.toThrow("Lote");
    }
    expect(sdk.iterateFolderChildren).not.toHaveBeenCalled();
  });
});
