import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import OperationQueue from "../app/components/OperationQueue.vue";
import PairCard from "../app/components/PairCard.vue";
import type { Operation, SyncPair } from "../app/types";
const global = { stubs: { AppIcon: true } };
const job = (id: string, status: Operation["status"]): Operation => ({
  id,
  status,
  lane: "transfer",
  kind: "sync",
  title: `Sincronizar ${id}`,
  detail: `Documentos/${id}.pdf`,
  pairId: id,
  automatic: false,
  createdAt: 10,
  startedAt: status === "queued" ? null : 15,
  finishedAt: null,
  error: null,
  canCancelRunning: true,
});
describe("operation queue", () => {
  it("shows transfers, navigation and the index together with independent cancellation", async () => {
    const wrapper = mount(OperationQueue, {
      props: {
        native: true,
        queue: {
          paused: false,
          items: [
            job("upload", "running"),
            { ...job("documents", "running"), lane: "background" },
            { ...job("browse", "running"), lane: "interactive" },
          ],
        },
      },
      global,
    });
    expect(wrapper.findAll(".queue-active-card")).toHaveLength(3);
    const index = wrapper
      .findAll(".queue-active-card")
      .find((card) => card.text().includes("Sincronizar documents"))!;
    expect(index.text()).toContain("Índice em segundo plano");
    await index.get("button").trigger("click");
    expect(wrapper.emitted("cancel")).toEqual([["documents"]]);
    expect(index.get("button").text()).toBe("Cancelar consulta");
    wrapper.unmount();
  });
  it("separates active, ordered waiting and history, and cancels the exact queued item", async () => {
    const wrapper = mount(OperationQueue, {
      props: {
        native: true,
        queue: {
          paused: false,
          items: [
            job("A", "running"),
            job("B", "queued"),
            job("C", "queued"),
            { ...job("D", "failed"), error: "Sem conexão" },
          ],
        },
      },
      global,
    });
    expect(wrapper.get(".queue-running").text()).toContain("Sincronizar A");
    expect(
      wrapper.findAll(".queue-waiting li").map((row) => row.text()),
    ).toEqual([
      expect.stringContaining("Sincronizar B"),
      expect.stringContaining("Sincronizar C"),
    ]);
    expect(wrapper.get(".queue-history").text()).toContain("Sem conexão");
    await wrapper.get('[aria-label="Cancelar Sincronizar C"]').trigger("click");
    expect(wrapper.emitted("cancel")).toEqual([["C"]]);
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Pausar fila")!
      .trigger("click");
    expect(wrapper.emitted("pause")).toEqual([[true]]);
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Limpar histórico")!
      .trigger("click");
    expect(wrapper.emitted("clear")).toHaveLength(1);
    wrapper.unmount();
  });
  it("reflects completion and safe cancellation without a fake percentage", async () => {
    const wrapper = mount(OperationQueue, {
      props: {
        native: true,
        queue: {
          paused: false,
          items: [job("A", "running"), job("B", "queued")],
        },
      },
      global,
    });
    expect(
      wrapper.get('[role="progressbar"]').attributes("aria-valuenow"),
    ).toBeUndefined();
    await wrapper.setProps({
      queue: {
        paused: false,
        items: [job("A", "completed"), job("B", "cancelling")],
      },
    });
    expect(wrapper.get(".queue-running").text()).toContain("Parando…");
    expect(wrapper.get(".queue-waiting").text()).toContain(
      "Nenhuma operação esperando",
    );
    expect(
      wrapper.get(".queue-running button").attributes("disabled"),
    ).toBeDefined();
    expect(wrapper.get(".queue-history").text()).toContain("Concluída");
    wrapper.unmount();
  });
  it("resumes a paused queue and does not interrupt non-cancellable work", async () => {
    const wrapper = mount(OperationQueue, {
      props: {
        native: true,
        queue: {
          paused: true,
          items: [{ ...job("Update", "running"), canCancelRunning: false }],
        },
      },
      global,
    });
    expect(wrapper.find(".queue-running button").exists()).toBe(false);
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Retomar fila")!
      .trigger("click");
    expect(wrapper.emitted("pause")).toEqual([[false]]);
    wrapper.unmount();
  });
  it("keeps the current operation and waiting count visible outside the queue page", async () => {
    const wrapper = mount(OperationQueue, {
      props: {
        compact: true,
        native: true,
        queue: {
          paused: false,
          items: [job("A", "running"), job("B", "queued")],
        },
      },
      global,
    });
    expect(wrapper.text()).toContain("Sincronizar A");
    expect(wrapper.text()).toContain("1 na fila");
    await wrapper.get("button").trigger("click");
    expect(wrapper.emitted("open")).toHaveLength(1);
    wrapper.unmount();
  });
  it("allows requesting another sync while one runs, preventing a duplicate waiting sync", async () => {
    const pair: SyncPair = {
      id: "p",
      name: "Documentos",
      localPath: "/tmp/docs",
      remotePath: "/my-files/docs",
      mode: "bidirectional",
      intervalMinutes: 5,
      enabled: true,
      lastRun: null,
      propagateDeletions: false,
    };
    const wrapper = mount(PairCard, {
      props: { pair, syncing: true, paused: false, connected: true },
      global,
    });
    expect(
      wrapper
        .get('[aria-label="Sincronizar Documentos"]')
        .attributes("disabled"),
    ).toBeUndefined();
    await wrapper.get('[aria-label="Sincronizar Documentos"]').trigger("click");
    expect(wrapper.emitted("sync")).toHaveLength(1);
    await wrapper.setProps({ syncing: false, queued: true });
    expect(wrapper.text()).toContain("Na fila");
    expect(
      wrapper
        .get('[aria-label="Sincronizar Documentos"]')
        .attributes("disabled"),
    ).toBeDefined();
    expect(
      wrapper.get('[aria-label="Pausar Documentos"]').attributes("disabled"),
    ).toBeUndefined();
    wrapper.unmount();
  });
});
