<script setup lang="ts">
import { useI18n } from "~/composables/useI18n";
const { t, message, locale, number, plural } = useI18n();
import {
  ref,
  shallowRef,
  computed,
  watch,
  onMounted,
  onUnmounted,
  nextTick,
} from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  RemoteEntry,
  SyncPair,
  LibraryView,
  ComputerRegistration,
} from "~/types";
import ComputerConnection from "~/components/ComputerConnection.vue";
import type { IconName } from "~/components/AppIcon.vue";
const props = defineProps<{
  connected: boolean;
  native: boolean;
  computer?: ComputerRegistration | null;
  pairs?: SyncPair[];
  command: <T = void>(
    name: string,
    args?: Record<string, unknown>,
  ) => Promise<T>;
}>();
const emit = defineEmits<{
  connect: [];
  sync: [preset: Partial<SyncPair>];
  full: [];
  queue: [];
  manage: [];
  registered: [];
  notify: [message: string, error?: boolean];
}>();
type Tab = "files" | "computers" | "documents" | "photos" | "albums";
const tab = ref<Tab>("files");
const paths: Record<Tab, string> = {
  files: "/my-files",
  computers: "/devices",
  documents: "/documents",
  photos: "/photos",
  albums: "/albums",
};
const tabs: { id: Tab; name: string; icon: IconName }[] = [
  { id: "files", name: "Arquivos", icon: "Folder" },
  { id: "computers", name: "Computadores", icon: "Monitor" },
  { id: "documents", name: "Documentos", icon: "FileText" },
  { id: "photos", name: "Fotos", icon: "Image" },
  { id: "albums", name: "Álbuns", icon: "Images" },
];
const path = ref("/my-files");
const computerName = ref(t("Meu PC Linux"));
const registering = ref(false);
const localComputer = ref<ComputerRegistration | null>(props.computer ?? null);
watch(
  () => props.computer,
  (value) => {
    localComputer.value = value ?? null;
  },
);
async function registerComputer(name: string, existingUid?: string) {
  if (registering.value) return;
  registering.value = true;
  error.value = "";
  try {
    const device = await props.command<RemoteEntry>("register_computer", {
      name,
      ...(existingUid ? { existingUid } : {}),
    });
    localComputer.value = { name: device.name, deviceUid: device.uid };
    emit("registered");
    emit(
      "notify",
      t("{0} vinculado. Agora escolha as pastas para sincronizar.", [
        device.name,
      ]),
    );
    if (tab.value === "computers" && path.value === "/devices") {
      await load("/devices", true);
    }
  } catch (e) {
    if (tab.value === "computers" && path.value === "/devices")
      error.value = String(e);
    else emit("notify", String(e), true);
  } finally {
    registering.value = false;
  }
}
const computerRoot = computed(
  () =>
    path.value.startsWith("/devices/") && path.value.split("/").length === 3,
);
const activeComputer = ref<RemoteEntry>();
const ownComputerRoot = computed(
  () =>
    computerRoot.value &&
    !!localComputer.value?.deviceUid &&
    activeComputer.value?.uid === localComputer.value.deviceUid,
);
const configuredPair = computed(() =>
  props.pairs?.find(
    (pair) =>
      path.value === pair.remotePath ||
      path.value.startsWith(`${pair.remotePath}/`),
  ),
);
const libraryConfigured = computed(() =>
  props.pairs?.some((pair) => pair.remotePath === "/my-files"),
);
function syncComputer(device: RemoteEntry) {
  emit("sync", {
    name: "",
    remotePath: device.path,
    deviceUid: device.uid,
    mode: "bidirectional",
  });
}

const albumName = ref("");
const entries = shallowRef<RemoteEntry[]>([]);
const view = shallowRef<LibraryView>();
let pollTimer: ReturnType<typeof setTimeout> | undefined;
let searchTimer: ReturnType<typeof setTimeout> | undefined;
const searchTerm = ref("");
const refreshing = computed(() => view.value?.refreshing ?? false);
const collator = computed(
  () => new Intl.Collator(locale.value, { numeric: true, sensitivity: "base" }),
);
const search = ref("");
const loading = ref(false);
const error = ref("");
const selected = ref<RemoteEntry>();
const preview = ref<{
  kind: string;
  mime: string;
  content: string;
  name: string;
}>();
const previewLoading = ref(false);
const previewError = ref("");
const downloading = ref(false);
const previewDialog = ref<HTMLDialogElement>();
const pageNumber = ref(1);
const pageSize = 48;
let request = 0;
let previewRequest = 0;
const filtered = computed(() =>
  entries.value
    .filter((entry) =>
      `${entry.name} ${entry.path}`
        .toLocaleLowerCase(locale.value)
        .includes(searchTerm.value.toLocaleLowerCase(locale.value)),
    )
    .sort(
      (a, b) =>
        Number(b.directory) - Number(a.directory) ||
        collator.value.compare(a.name, b.name),
    ),
);
const visible = computed(() =>
  filtered.value.slice(
    (pageNumber.value - 1) * pageSize,
    pageNumber.value * pageSize,
  ),
);
const pages = computed(() =>
  Math.max(1, Math.ceil(filtered.value.length / pageSize)),
);
const gallery = computed(
  () => tab.value === "photos" || tab.value === "albums",
);
const crumbs = computed(() =>
  path.value.startsWith("/my-files") || path.value.startsWith("/devices")
    ? path.value
        .split("/")
        .filter(Boolean)
        .map((part, index, parts) => ({
          label: index
            ? part
            : tab.value === "computers"
              ? t("Computadores")
              : t("Meu Drive"),
          path: "/" + parts.slice(0, index + 1).join("/"),
        }))
    : [],
);
const bytes = (n: number) =>
  n < 1024
    ? `${number(n)} B`
    : n < 1024 ** 2
      ? `${number(n / 1024, { maximumFractionDigits: 1 })} KB`
      : n < 1024 ** 3
        ? `${number(n / 1024 ** 2, { maximumFractionDigits: 1 })} MB`
        : `${number(n / 1024 ** 3, { maximumFractionDigits: 1 })} GB`;
const icon = (entry: RemoteEntry): IconName =>
  entry.kind === "device"
    ? "Monitor"
    : entry.kind === "album"
      ? "Images"
      : entry.directory
        ? "Folder"
        : entry.nativeDocument
          ? "FileText"
          : entry.mediaType?.startsWith("image/")
            ? "Image"
            : entry.mediaType?.startsWith("video/")
              ? "Film"
              : "File";
const kind = (entry: RemoteEntry) =>
  entry.kind === "device"
    ? entry.mediaType
    : entry.kind === "album"
      ? plural("{0} foto", "{0} fotos", entry.photoCount)
      : entry.directory
        ? t("Pasta")
        : entry.nativeDocument
          ? t("Editor Proton · online")
          : entry.mediaType?.startsWith("video/")
            ? t("Vídeo")
            : bytes(entry.size);
watch(search, (value) => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    searchTerm.value = value;
    pageNumber.value = 1;
  }, 120);
});
watch(pages, (value) => {
  pageNumber.value = Math.min(pageNumber.value, value);
});
watch(
  () => props.connected,
  (connected) => {
    if (connected) void load(path.value);
    else {
      request++;
      clearTimeout(pollTimer);
      view.value = undefined;
      entries.value = [];
    }
  },
);
onMounted(() => {
  if (props.connected) void load(path.value);
});
onUnmounted(() => {
  request++;
  previewRequest++;
  clearTimeout(pollTimer);
  clearTimeout(searchTimer);
});
async function load(next: string, force = false) {
  const device = entries.value.find(
    (entry) =>
      entry.kind === "device" &&
      (next === entry.path || next.startsWith(`${entry.path}/`)),
  );
  if (device) activeComputer.value = device;
  if (next === "/devices") activeComputer.value = undefined;
  const changedPath = next !== path.value;
  const token = ++request;
  clearTimeout(pollTimer);
  if (next !== path.value) {
    entries.value = [];
    view.value = undefined;
    pageNumber.value = 1;
  }
  loading.value = !entries.value.length;
  error.value = "";
  path.value = next;
  await readView(next, token, force);
  if (changedPath && token === request) {
    await nextTick();
    document.getElementById("library-content")?.focus({ preventScroll: true });
  }
}
async function readView(source: string, token: number, force = false) {
  try {
    const args: Record<string, unknown> = { path: source };
    if (force) args.force = true;
    if (view.value) args.revision = view.value.revision;
    const result = await props.command<LibraryView>("list_library", args);
    if (token !== request) return;
    if (result.entries !== null) entries.value = result.entries;
    view.value = result;
    error.value = result.error ?? "";
    if (result.refreshing) {
      pollTimer = setTimeout(() => void readView(source, token), 1200);
    }
  } catch (e) {
    if (token === request) error.value = String(e);
  } finally {
    if (token === request) loading.value = false;
  }
}
function switchTab(next: Tab) {
  tab.value = next;
  if (next === "computers" && props.native) {
    void props
      .command<string>("computer_name")
      .then((name) => {
        computerName.value = name;
      })
      .catch(() => {});
  }
  search.value = "";
  searchTerm.value = "";
  albumName.value = "";
  if (props.connected) void load(paths[next]);
  else {
    path.value = paths[next];
    entries.value = [];
  }
}
async function tabKey(event: KeyboardEvent) {
  const index = tabs.findIndex((item) => item.id === tab.value);
  const next =
    event.key === "ArrowRight"
      ? (index + 1) % tabs.length
      : event.key === "ArrowLeft"
        ? (index + tabs.length - 1) % tabs.length
        : event.key === "Home"
          ? 0
          : event.key === "End"
            ? tabs.length - 1
            : -1;
  if (next < 0) return;
  event.preventDefault();
  switchTab(tabs[next]!.id);
  await nextTick();
  document.getElementById(`tab-${tab.value}`)?.focus();
}
async function inspect(entry: RemoteEntry) {
  if (entry.directory) {
    if (entry.kind === "album") albumName.value = entry.name;
    await load(entry.path);
    return;
  }
  selected.value = entry;
  preview.value = undefined;
  previewError.value = "";
  await nextTick();
  previewDialog.value?.showModal();
  if (entry.nativeDocument) return;
  const token = ++previewRequest;
  previewLoading.value = true;
  try {
    const result = await props.command<typeof preview.value>("preview_file", {
      path: entry.path,
    });
    if (token === previewRequest) preview.value = result;
  } catch (e) {
    if (token === previewRequest) previewError.value = String(e);
  } finally {
    if (token === previewRequest) previewLoading.value = false;
  }
}
async function openDocument() {
  if (!selected.value) return;
  previewLoading.value = true;
  previewError.value = "";
  try {
    await props.command("open_document", { path: selected.value.path });
  } catch (e) {
    previewError.value = String(e);
  } finally {
    previewLoading.value = false;
  }
}
async function download() {
  if (!selected.value) return;
  downloading.value = true;
  previewError.value = "";
  try {
    const folder = await open({
      directory: true,
      multiple: false,
      title: t("Salvar arquivo nesta pasta"),
    });
    if (typeof folder !== "string") return;
    const saved = await props.command<string>("download_file", {
      path: selected.value.path,
      destination: folder,
    });
    emit("notify", t("Arquivo salvo em {0}", [saved]));
  } catch (e) {
    previewError.value = String(e);
  } finally {
    downloading.value = false;
  }
}
function syncCurrent() {
  if (configuredPair.value) {
    emit("manage");
    return;
  }
  emit("sync", {
    name: computerRoot.value
      ? ""
      : albumName.value ||
        (tab.value === "photos"
          ? t("Minhas fotos")
          : tab.value === "albums"
            ? t("Meus álbuns")
            : path.value.split("/").at(-1) === "my-files"
              ? t("Meu Drive")
              : path.value.split("/").at(-1)),
    remotePath: path.value,
    mode: path.value === "/albums" ? "download" : "bidirectional",
  });
}
async function web() {
  try {
    await props.command("open_drive");
  } catch (e) {
    error.value = String(e);
  }
}
</script>
<template>
  <div class="page-heading library-heading">
    <div>
      <div class="eyebrow">{{ t("SUA BIBLIOTECA") }}</div>
      <h1>{{ t("Tudo o que é seu. Aqui.") }}</h1>
      <p>{{ t("Documentos, memórias e projetos, em um só lugar.") }}</p>
    </div>
    <button
      class="btn btn-primary"
      :disabled="!connected"
      @click="libraryConfigured ? emit('manage') : emit('full')"
    >
      <AppIcon name="HardDrive" :size="18" />
      {{
        libraryConfigured ? t("Ver sincronizações") : t("Configurar biblioteca")
      }}
    </button>
  </div>
  <div class="library-intro">
    <span class="library-emblem"><AppIcon name="Cloud" :size="29" /></span>
    <div>
      <strong> {{ t("Seu Drive, mais perto.") }} </strong>
      <p>{{ t("Navegue pela nuvem ou mantenha uma cópia no computador.") }}</p>
    </div>
    <button class="btn btn-sm btn-ghost" :disabled="!native" @click="web">
      {{ t("Abrir versão web") }} <AppIcon name="ArrowUpRight" :size="17" />
    </button>
  </div>
  <section class="surface library-surface">
    <div
      class="library-tabs"
      role="tablist"
      :aria-label="t('Biblioteca do Drive')"
    >
      <button
        v-for="item in tabs"
        :id="`tab-${item.id}`"
        :key="item.id"
        role="tab"
        :tabindex="tab === item.id ? 0 : -1"
        @keydown="tabKey"
        :aria-selected="tab === item.id"
        aria-controls="library-content"
        :class="{ active: tab === item.id }"
        @click="switchTab(item.id)"
      >
        <AppIcon :name="item.icon" :size="18" />{{ t(item.name) }}
      </button>
    </div>
    <div class="library-toolbar">
      <label class="library-search"
        ><AppIcon name="Search" :size="18" /><input
          v-model="search"
          type="search"
          :aria-label="t('Buscar na biblioteca')"
          :placeholder="t('Buscar nesta visualização…')"
      /></label>
      <button
        class="btn btn-ghost btn-square btn-sm"
        :aria-label="t('Atualizar biblioteca')"
        :disabled="loading || !connected"
        @click="load(path, true)"
      >
        <AppIcon
          name="RefreshCw"
          :size="18"
          :class="{ spinning: refreshing || loading }"
        />
      </button>
      <button
        v-if="
          tab !== 'documents' &&
          path !== '/devices' &&
          (!computerRoot || ownComputerRoot)
        "
        class="btn btn-soft btn-sm"
        :disabled="!connected || loading"
        @click="syncCurrent"
      >
        <AppIcon name="RefreshCw" :size="16" />{{
          configuredPair
            ? t("Ver sincronização")
            : computerRoot
              ? t("Adicionar pasta deste PC")
              : tab === "albums" && path === "/albums"
                ? t("Sincronizar álbuns")
                : t("Sincronizar aqui")
        }}
      </button>
    </div>
    <div class="library-location">
      <nav v-if="crumbs.length" :aria-label="t('Caminho no Drive')">
        <template v-for="(crumb, i) in crumbs" :key="crumb.path"
          ><AppIcon v-if="i" name="ChevronRight" :size="14" /><button
            :aria-current="i === crumbs.length - 1 ? 'page' : undefined"
            :disabled="loading"
            @click="load(crumb.path)"
          >
            {{ crumb.label }}
          </button></template
        >
      </nav>
      <nav v-else-if="albumName" :aria-label="t('Álbum')">
        <button
          :disabled="loading"
          @click="
            albumName = '';
            load('/albums');
          "
        >
          {{ t("Álbuns") }}</button
        ><AppIcon name="ChevronRight" :size="14" /><span>{{ albumName }}</span>
      </nav>
      <span v-else>{{
        tab === "documents"
          ? t("Documentos em todas as pastas")
          : tab === "photos"
            ? t("Todas as fotos e vídeos")
            : t("Seus álbuns")
      }}</span>
      <span v-if="connected && !loading">{{
        plural("{0} item", "{0} itens", filtered.length)
      }}</span>
    </div>
    <div
      v-if="connected && refreshing"
      class="library-refresh-note"
      role="status"
    >
      <span class="loading loading-spinner loading-sm" />
      <div>
        <strong>{{ t("Atualizando em segundo plano") }}</strong>
        <p v-if="tab === 'documents'">
          {{
            t(
              "{0} pastas verificadas · {1} documentos · {2} pastas pendentes",
              [
                view?.foldersDone ?? 0,
                entries.length,
                view?.foldersPending ?? 0,
              ],
            )
          }}
        </p>
        <p v-else>
          {{
            t(
              "Você pode continuar navegando enquanto a biblioteca é atualizada.",
            )
          }}
        </p>
      </div>
      <button class="btn btn-ghost btn-sm" @click="emit('queue')">
        {{ t("Ver fila") }}
      </button>
    </div>
    <div
      v-if="connected && error && entries.length"
      class="library-refresh-note library-refresh-warning"
      role="alert"
    >
      <AppIcon name="TriangleAlert" :size="18" />
      <p>{{ message(error) }}</p>
      <button class="btn btn-ghost btn-sm" @click="load(path, true)">
        {{ t("Tentar novamente") }}
      </button>
    </div>
    <p v-if="connected && view?.updatedAt" class="library-cache-time">
      {{
        t("Biblioteca salva · atualizada em {0}", [
          new Intl.DateTimeFormat(locale, {
            dateStyle: "short",
            timeStyle: "short",
          }).format(new Date(view.updatedAt * 1000)),
        ])
      }}<span v-if="view.partial"> · {{ t("Resultados parciais") }}</span>
    </p>
    <div v-if="tab === 'documents'" class="library-note">
      <AppIcon name="FileText" :size="18" />
      <p>
        {{
          t(
            "PDFs abrem aqui; arquivos Office podem ser baixados. Docs e Sheets abrem no editor Proton integrado e precisam de internet.",
          )
        }}
      </p>
    </div>
    <ComputerConnection
      v-if="tab === 'computers' && path === '/devices' && connected"
      :registration="localComputer"
      :devices="entries"
      :suggested-name="computerName"
      :busy="registering"
      :loading="loading || refreshing"
      :failed="!!error"
      :native="native"
      @register="registerComputer"
      @open="load($event.path)"
      @sync="syncComputer"
      @web="web"
    />
    <div v-if="tab === 'computers' && path !== '/devices'" class="library-note">
      <AppIcon name="Monitor" :size="18" />
      <p>
        {{
          t(
            computerRoot && !ownComputerRoot
              ? "Abra uma pasta deste computador para sincronizar uma cópia no seu PC. Para adicionar novas pastas deste PC, vincule-o na lista de computadores."
              : "Estas pastas aparecem em Computers no Proton Drive. Alterações locais iniciam a sincronização automaticamente.",
          )
        }}
      </p>
    </div>
    <div
      id="library-content"
      tabindex="-1"
      role="tabpanel"
      :aria-labelledby="`tab-${tab}`"
      :aria-busy="loading"
    >
      <div v-if="!connected" class="library-empty">
        <span class="empty-library-icon"
          ><AppIcon :name="tabs.find((t) => t.id === tab)!.icon" :size="42"
        /></span>
        <h2>{{ t("Sua biblioteca começa com uma conexão.") }}</h2>
        <p>
          {{
            t(
              tab === "albums"
                ? "Entre com sua conta Proton para ver seus álbuns aqui."
                : tab === "photos"
                  ? "Entre com sua conta Proton para ver suas fotos aqui."
                  : "Entre com sua conta Proton para ver seus arquivos aqui.",
            )
          }}
        </p>
        <button
          class="btn btn-primary"
          :disabled="!native"
          @click="emit('connect')"
        >
          <AppIcon name="LogIn" :size="18" /> {{ t("Conectar conta Proton") }}
        </button>
      </div>
      <div
        v-else-if="(loading || refreshing) && !entries.length"
        class="library-empty"
        role="status"
      >
        <span class="loading loading-spinner text-primary loading-lg" />
        <h2>{{ t("Carregando sua biblioteca…") }}</h2>
        <p>
          {{
            tab === "documents"
              ? t("Procurando documentos em todas as pastas.")
              : t("Lendo e decifrando os metadados no seu computador.")
          }}
        </p>
      </div>
      <div
        v-else-if="error && !entries.length"
        class="library-empty"
        role="alert"
      >
        <AppIcon name="TriangleAlert" :size="36" />
        <h2>{{ t("Não foi possível carregar.") }}</h2>
        <p>{{ message(error) }}</p>
        <button class="btn btn-soft" @click="load(path, true)">
          {{ t("Tentar novamente") }}
        </button>
      </div>
      <div v-else-if="!visible.length" class="library-empty">
        <span class="empty-library-icon"
          ><AppIcon name="FolderOpen" :size="40"
        /></span>
        <h2>
          {{
            search
              ? t("Nenhum resultado por aqui.")
              : t("Este espaço ainda está vazio.")
          }}
        </h2>
        <p>
          {{
            search
              ? t("Tente outro nome de arquivo.")
              : t(
                  "Os itens da sua conta aparecerão aqui assim que forem adicionados.",
                )
          }}
        </p>
      </div>
      <div v-else-if="gallery" class="media-grid">
        <button
          v-for="(entry, index) in visible"
          :key="entry.path"
          class="media-card"
          :aria-label="t('Abrir {0}', [entry.name])"
          @click="inspect(entry)"
        >
          <span
            class="media-cover"
            :class="[
              `tone-${index % 4}`,
              { 'album-cover': entry.kind === 'album' },
            ]"
            ><AppIcon :name="icon(entry)" :size="45" :stroke-width="1.2" /><span
              class="media-cover-label"
              >{{
                entry.kind === "album"
                  ? plural("{0} foto", "{0} fotos", entry.photoCount)
                  : t("Visualizar")
              }}</span
            ></span
          >
          <span class="media-name">{{ entry.name }}</span
          ><span class="media-meta"
            >{{ kind(entry) }}<AppIcon name="ArrowUpRight" :size="15"
          /></span>
        </button>
      </div>
      <div v-else class="library-table-wrap">
        <table class="table library-table">
          <caption class="sr-only">
            {{
              t("Conteúdo de {0}", [path])
            }}
          </caption>
          <thead>
            <tr>
              <th scope="col">{{ t("Nome") }}</th>
              <th scope="col">{{ t("Tipo / tamanho") }}</th>
              <th scope="col">
                <span class="sr-only"> {{ t("Ação") }} </span>
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="entry in visible" :key="entry.path">
              <td>
                <button class="file-name-button" @click="inspect(entry)">
                  <span
                    class="file-symbol"
                    :class="{
                      folder: entry.directory,
                      document: entry.nativeDocument,
                    }"
                    ><AppIcon :name="icon(entry)" :size="23" /></span
                  ><span
                    ><strong>{{ entry.name }}</strong
                    ><small
                      v-if="
                        entry.kind === 'device' &&
                        entry.uid === localComputer?.deviceUid
                      "
                      class="this-pc-label"
                      >{{ t("Este computador") }}</small
                    ><small v-if="tab === 'documents'">{{
                      entry.path.replace("/my-files/", "")
                    }}</small></span
                  >
                </button>
              </td>
              <td>{{ kind(entry) }}</td>
              <td>
                <button
                  class="btn btn-ghost btn-square btn-sm"
                  :aria-label="t('Abrir {0}', [entry.name])"
                  @click="inspect(entry)"
                >
                  <AppIcon
                    :name="entry.directory ? 'ChevronRight' : 'ArrowUpRight'"
                    :size="17"
                  />
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <nav
        v-if="pages > 1"
        class="library-pagination"
        :aria-label="t('Páginas da biblioteca')"
      >
        <button
          class="btn btn-sm btn-ghost"
          :disabled="pageNumber <= 1"
          @click="pageNumber--"
        >
          {{ t("Anterior") }}</button
        ><span role="status">{{
          t("Página {0} de {1}", [pageNumber, pages])
        }}</span
        ><button
          class="btn btn-sm btn-ghost"
          :disabled="pageNumber >= pages"
          @click="pageNumber++"
        >
          {{ t("Próxima") }}
        </button>
      </nav>
    </div>
  </section>
  <p class="library-footnote">
    <AppIcon name="ShieldCheck" :size="15" />
    {{
      t(
        "Pré-visualizações são baixadas ao abrir. Seu conteúdo é decifrado apenas neste computador.",
      )
    }}
  </p>
  <dialog
    ref="previewDialog"
    class="modal"
    aria-labelledby="preview-title"
    @close="
      previewRequest++;
      previewLoading = false;
      preview = undefined;
    "
  >
    <div class="modal-box file-preview-dialog">
      <div class="preview-heading">
        <div>
          <span class="eyebrow">{{
            selected?.nativeDocument
              ? t("EDITOR ONLINE")
              : t("PRÉ-VISUALIZAÇÃO")
          }}</span>
          <h2 id="preview-title">{{ selected?.name }}</h2>
        </div>
        <button
          class="btn btn-ghost btn-square btn-sm"
          :aria-label="t('Fechar pré-visualização')"
          @click="previewDialog?.close()"
        >
          <AppIcon name="X" />
        </button>
      </div>
      <div v-if="previewLoading" class="preview-placeholder" role="status">
        <span class="loading loading-spinner loading-lg" />
        <p>{{ t("Preparando arquivo…") }}</p>
      </div>
      <div v-else-if="selected?.nativeDocument" class="preview-placeholder">
        <AppIcon name="FileText" :size="54" />
        <h3>{{ t("Continue no editor Proton.") }}</h3>
        <p>
          {{
            t(
              "Abra e edite este documento na janela integrada. O conteúdo nativo não pode ser sincronizado offline pelo CLI.",
            )
          }}
        </p>
        <button class="btn btn-primary" @click="openDocument">
          {{ t("Abrir documento") }} <AppIcon name="ArrowUpRight" :size="18" />
        </button>
      </div>
      <img
        v-else-if="preview?.kind === 'image'"
        class="image-preview"
        :src="`data:${preview.mime};base64,${preview.content}`"
        :alt="selected?.name"
      />
      <PdfPreview
        v-else-if="preview?.kind === 'pdf'"
        :content="preview.content"
      />
      <pre v-else-if="preview?.kind === 'text'" class="text-preview">{{
        preview.content
      }}</pre>
      <div v-else-if="preview?.kind === 'external'" class="preview-placeholder">
        <AppIcon name="FileText" :size="52" />
        <h3>{{ t("Abra no seu aplicativo preferido.") }}</h3>
        <p>
          {{
            t(
              "Baixe este arquivo para visualizar documentos Office, vídeos e outros formatos no computador.",
            )
          }}
        </p>
      </div>
      <p v-if="previewError" class="form-error" role="alert">
        {{ message(previewError) }}
      </p>
      <div class="preview-footer">
        <span>{{ selected ? kind(selected) : "" }}</span
        ><button
          v-if="selected && !selected.nativeDocument"
          class="btn btn-primary btn-sm"
          :disabled="downloading || previewLoading"
          @click="download"
        >
          <span
            v-if="downloading"
            class="loading loading-spinner loading-xs"
          /><AppIcon v-else name="CloudDownload" :size="17" />{{
            downloading ? t("Baixando…") : t("Baixar arquivo")
          }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button :aria-label="t('Fechar')">{{ t("Fechar") }}</button>
    </form>
  </dialog>
</template>
