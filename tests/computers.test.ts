import { describe, expect, it, vi } from "vitest";
// The bridge's small registration policy is tested without real credentials.
// @ts-expect-error the compiled helper is intentionally plain JavaScript
import { ensureDevice } from "../scripts/computers/ensure-device.mjs";

const device = {
  uid: "pc-1",
  rootFolderUid: "root-1",
  name: { ok: true, value: "Meu PC" },
  type: "Linux",
};
function sdk(devices: unknown[]) {
  return {
    async *iterateDevices() {
      yield* devices;
    },
    createDevice: vi.fn(async () => device),
  };
}
describe("computer registration", () => {
  it("registers Linux and reuses a previous successful registration on retry", async () => {
    const empty = sdk([]);
    expect(await ensureDevice(empty, "Meu PC")).toEqual(device);
    expect(empty.createDevice).toHaveBeenCalledWith("Meu PC", "Linux");
    const existing = sdk([device]);
    expect(await ensureDevice(existing, "Meu PC")).toEqual(device);
    expect(existing.createDevice).not.toHaveBeenCalled();
  });
  it("refuses ambiguous names, unverified names and a different OS", async () => {
    for (const devices of [
      [device, device],
      [{ ...device, type: "Windows" }],
      [{ ...device, name: { ok: false } }],
    ]) {
      const api = sdk(devices);
      await expect(ensureDevice(api, "Meu PC")).rejects.toThrow();
      expect(api.createDevice).not.toHaveBeenCalled();
    }
  });
  it("validates names before accessing the account", async () => {
    for (const name of ["", "..", "a/b", "a\\b", "a\nb"]) {
      const api = sdk([]);
      await expect(ensureDevice(api, name)).rejects.toThrow();
      expect(api.createDevice).not.toHaveBeenCalled();
    }
  });
});
