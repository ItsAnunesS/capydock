import { libraryView } from "../library-fixture";
// Explicitly isolated fixtures: this harness never connects to a Proton account.
import { createApp, h, ref } from "vue";
import LanguageSetting from "../../app/components/LanguageSetting.vue";
import {
  encodeMessage,
  savedLocale,
  setLocale,
} from "../../app/composables/useI18n";
setLocale(savedLocale());
import OperationQueue from "../../app/components/OperationQueue.vue";
import type { OperationQueue as QueueState, Operation } from "../../app/types";
import CloudLibrary from "../../app/components/CloudLibrary.vue";
import AppIcon from "../../app/components/AppIcon.vue";
import PdfPreview from "../../app/components/PdfPreview.vue";
import type { RemoteEntry } from "../../app/types";
import pdf from "./sample.json";
import "../../app/assets/main.css";
import "../../app/assets/accessibility.css";
const base: RemoteEntry = {
  uid: "fixture",
  name: "Guia de viagem.pdf",
  path: "/my-files/Guia de viagem.pdf",
  directory: false,
  revision: "r",
  size: 1000,
  kind: "file",
  mediaType: "application/pdf",
  nativeDocument: false,
  photoCount: 0,
  modified: null,
};
const fixtures: Record<string, RemoteEntry[]> = {
  "/devices": [
    {
      ...base,
      name: "Meu PC Linux",
      path: "/devices/Meu PC Linux",
      directory: true,
      kind: "device",
      mediaType: "Linux",
    },
  ],
  "/devices/Meu PC Linux": [
    {
      ...base,
      name: "Documentos",
      path: "/devices/Meu PC Linux/Documentos",
      directory: true,
      kind: "folder",
    },
  ],
  "/my-files": [
    base,
    {
      ...base,
      name: "Ideias para a viagem",
      path: "/my-files/Ideias",
      nativeDocument: true,
    },
    {
      ...base,
      name: "Projetos",
      path: "/my-files/Projetos",
      directory: true,
      kind: "folder",
    },
  ],
  "/documents": [
    base,
    {
      ...base,
      name: "Ideias para a viagem",
      path: "/my-files/Ideias",
      nativeDocument: true,
    },
  ],
  "/albums": [
    "Verão em Portugal",
    "Fim de semana",
    "Memórias de família",
    "Pelos caminhos",
  ].map((name, i) => ({
    ...base,
    name,
    path: `/albums/fixture-${i}`,
    directory: true,
    kind: "album",
    photoCount: 12 + i,
  })),
  "/photos": [
    "Costa portuguesa.jpg",
    "Entre montanhas.jpg",
    "A caminho.jpg",
  ].map((name, i) => ({
    ...base,
    name,
    path: `/photos/fixture-${i}`,
    kind: "photo",
    mediaType: "image/jpeg",
  })),
};
const demoComputer = ref<{ name: string; deviceUid: string | null } | null>(
  new URLSearchParams(location.search).get("computer") === "unlinked"
    ? null
    : { name: "Meu PC Linux", deviceUid: "fixture" },
);
const command = async <T = void>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> => {
  if (name === "computer_name") return "Meu PC Linux" as T;
  if (name === "register_computer") {
    demoComputer.value = { name: "Meu PC Linux", deviceUid: "fixture" };
    return fixtures["/devices"]![0] as T;
  }
  if (name === "list_library") {
    const documents = args?.path === "/documents";
    const items =
      fixtures[String(args?.path)] ??
      (String(args?.path).startsWith("/albums/") ? fixtures["/photos"]! : []);
    return libraryView(
      items,
      documents
        ? {
            refreshing: true,
            partial: true,
            foldersDone: 128,
            foldersPending: 36,
            updatedAt: Math.floor(Date.now() / 1000) - 60,
          }
        : {},
    ) as T;
  }
  if (name === "preview_file" && args?.path === base.path)
    return {
      kind: "pdf",
      mime: "application/pdf",
      content: pdf.base64,
      name: base.name,
    } as T;
  throw new Error("Action unavailable in the isolated test.");
};
const testNow = Math.floor(Date.now() / 1000);
function testJob(
  id: string,
  title: string,
  detail: string,
  status: Operation["status"],
  age: number,
): Operation {
  return {
    id,
    title,
    detail,
    status,
    lane: "transfer",
    kind: "sync",
    pairId: null,
    automatic: id === "sync",
    createdAt: testNow - age,
    startedAt: status === "queued" ? null : testNow - age,
    finishedAt: null,
    error: null,
    canCancelRunning: true,
  };
}
const queueState = ref<QueueState>({
  paused: false,
  items: [
    testJob(
      "sync",
      encodeMessage("Sync {0}", ["Documents"]),
      encodeMessage("Uploading · {0}", ["Projetos/Apresentação.pdf"]),
      "running",
      42,
    ),
    testJob(
      "download",
      "Download file",
      "/devices/Meu PC Linux/Projetos/Briefing.pdf",
      "queued",
      18,
    ),
    {
      ...testJob("albums", "Load library", "/albums", "queued", 12),
      lane: "interactive",
    },
    {
      ...testJob(
        "index",
        "Index documents",
        encodeMessage(
          "{0} folders checked · {1} documents · {2} folders pending",
          [128, 246, 36],
        ),
        "running",
        8,
      ),
      lane: "background",
      kind: "index",
      automatic: true,
    },
    {
      ...testJob("browse", "Load library", "/devices", "running", 1),
      lane: "interactive",
      kind: "browse",
    },
    testJob(
      "photos",
      encodeMessage("Sync {0}", ["Photos"]),
      encodeMessage("Downloading · {0}", ["Fotos da viagem.jpg"]),
      "completed",
      110,
    ),
  ],
});
const queuePreview =
  new URLSearchParams(location.search).get("view") === "queue";
createApp({
  render: () =>
    h("main", { style: "max-width:1150px;padding:32px;margin:auto;" }, [
      h(
        "p",
        { style: "color:#9b6295;margin-bottom:20px;font-size:12px" },
        "ISOLATED TEST · fictional data · no account connected",
      ),
      h(LanguageSetting, { onChange: setLocale }),
      queuePreview
        ? h(OperationQueue, {
            queue: queueState.value,
            native: true,
            onCancel: (id: string) => {
              const item = queueState.value.items.find((i) => i.id === id);
              if (item)
                item.status =
                  item.status === "queued" ? "cancelled" : "cancelling";
            },
            onPause: (paused: boolean) => {
              queueState.value.paused = paused;
            },
            onClear: () => {
              queueState.value.items = queueState.value.items.filter((i) =>
                ["queued", "running", "cancelling"].includes(i.status),
              );
            },
          })
        : h(CloudLibrary, {
            native: true,
            connected: true,
            command,
            computer: demoComputer.value,
          }),
    ]),
})
  .component("AppIcon", AppIcon)
  .component("PdfPreview", PdfPreview)
  .mount("#app");
