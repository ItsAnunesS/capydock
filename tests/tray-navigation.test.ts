import { beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent, h } from "vue";
import { mount, flushPromises } from "@vue/test-utils";
import { useTrayNavigation } from "../app/composables/useTrayNavigation";

const api = vi.hoisted(() => ({
  native: true,
  invoke: vi.fn(),
  listen: vi.fn(),
  unlisten: vi.fn(),
  handlers: new Map<string, (event: { payload: string }) => void>(),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: api.invoke,
  isTauri: () => api.native,
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: api.listen }));

beforeEach(() => {
  api.native = true;
  api.handlers.clear();
  api.invoke.mockReset().mockResolvedValue(null);
  api.unlisten.mockReset();
  api.listen.mockReset().mockImplementation(async (event, handler) => {
    api.handlers.set(event, handler);
    return () => {
      api.handlers.delete(event);
      api.unlisten(event);
    };
  });
});

function harness() {
  const navigate = vi.fn();
  const error = vi.fn();
  const component = defineComponent({
    setup() {
      useTrayNavigation(navigate, error);
      return () => h("div");
    },
  });
  return { wrapper: mount(component), navigate, error };
}

describe("tray navigation", () => {
  it("handles a settings click made before the frontend finished loading", async () => {
    api.invoke.mockImplementation(async () => {
      expect(api.handlers.has("tray-navigation")).toBe(true);
      return "settings";
    });
    const { wrapper, navigate } = harness();
    await flushPromises();
    expect(api.invoke).toHaveBeenCalledWith("take_tray_navigation");
    expect(navigate).toHaveBeenCalledExactlyOnceWith("settings");
    wrapper.unmount();
    expect(api.handlers.size).toBe(0);
  });

  it("opens settings from a later tray event and surfaces native errors", async () => {
    const { wrapper, navigate, error } = harness();
    await flushPromises();
    api.invoke.mockResolvedValueOnce("settings");
    api.handlers.get("tray-navigation")!({ payload: "" });
    await flushPromises();
    expect(navigate).toHaveBeenCalledExactlyOnceWith("settings");
    api.handlers.get("tray-error")!({ payload: "Could not open folder" });
    expect(error).toHaveBeenCalledWith("Could not open folder");
    api.invoke.mockResolvedValueOnce("unknown-page");
    api.handlers.get("tray-navigation")!({ payload: "" });
    await flushPromises();
    expect(navigate).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });

  it("cleans up a listener whose registration finishes after unmount", async () => {
    let resolve!: (unlisten: () => void) => void;
    api.listen.mockReturnValueOnce(
      new Promise((done) => {
        resolve = done;
      }),
    );
    const { wrapper, navigate } = harness();
    wrapper.unmount();
    resolve(api.unlisten);
    await flushPromises();
    expect(api.unlisten).toHaveBeenCalledTimes(1);
    expect(api.invoke).not.toHaveBeenCalled();
    expect(navigate).not.toHaveBeenCalled();
  });

  it("does not navigate after unmount or register native events in browser preview", async () => {
    let resolve!: (page: string) => void;
    api.invoke.mockReturnValueOnce(
      new Promise((done) => {
        resolve = done;
      }),
    );
    const { wrapper, navigate } = harness();
    await flushPromises();
    wrapper.unmount();
    resolve("settings");
    await flushPromises();
    expect(navigate).not.toHaveBeenCalled();
    api.native = false;
    api.listen.mockClear();
    const browser = harness();
    await flushPromises();
    expect(api.listen).not.toHaveBeenCalled();
    browser.wrapper.unmount();
  });
});
