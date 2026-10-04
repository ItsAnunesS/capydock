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
  { id: "files", name: "Files", icon: "Folder" },
  { id: "computers", name: "Computers", icon: "Monitor" },
  { id: "documents", name: "Documents", icon: "FileText" },
  { id: "photos", name: "Photos", icon: "Image" },
  { id: "albums", name: "Albums", icon: "Images" },
];
const path = ref("/my-files");
const computerName = ref(t("My Linux PC"));
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
      t("{0} linked. Now choose the folders to sync.", [device.name]),
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
              ? t("Computers")
              : t("My Drive"),
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
      ? plural("{0} photo", "{0} photos", entry.photoCount)
      : entry.directory
        ? t("Folder")
        : entry.nativeDocument
          ? t("Proton editor · online")
          : entry.mediaType?.startsWith("video/")
            ? t("Video")
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
      title: t("Save file in this folder"),
    });
    if (typeof folder !== "string") return;
    const saved = await props.command<string>("download_file", {
      path: selected.value.path,
      destination: folder,
    });
    emit("notify", t("File saved to {0}", [saved]));
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
          ? t("My photos")
          : tab.value === "albums"
            ? t("My albums")
            : path.value.split("/").at(-1) === "my-files"
              ? t("My Drive")
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
      <div class="eyebrow">{{ t("YOUR LIBRARY") }}</div>
      <h1>{{ t("Everything that's yours. Here.") }}</h1>
      <p>{{ t("Documents, memories and projects, all in one place.") }}</p>
    </div>
    <button
      class="btn btn-primary"
      :disabled="!connected"
      @click="libraryConfigured ? emit('manage') : emit('full')"
    >
      <AppIcon name="HardDrive" :size="18" />
      {{ libraryConfigured ? t("View syncs") : t("Configure library") }}
    </button>
  </div>
  <div class="library-intro">
    <span class="library-emblem"><AppIcon name="Cloud" :size="29" /></span>
    <div>
      <strong> {{ t("Your Drive, closer.") }} </strong>
      <p>{{ t("Browse the cloud or keep a copy on your computer.") }}</p>
    </div>
    <button class="btn btn-sm btn-ghost" :disabled="!native" @click="web">
      {{ t("Open web version") }} <AppIcon name="ArrowUpRight" :size="17" />
    </button>
  </div>
  <section class="surface library-surface">
    <div class="library-tabs" role="tablist" :aria-label="t('Drive library')">
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
          :aria-label="t('Search library')"
          :placeholder="t('Search this view…')"
      /></label>
      <button
        class="btn btn-ghost btn-square btn-sm"
        :aria-label="t('Refresh library')"
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
            ? t("View sync")
            : computerRoot
              ? t("Add a folder from this PC")
              : tab === "albums" && path === "/albums"
                ? t("Sync albums")
                : t("Sync here")
        }}
      </button>
    </div>
    <div class="library-location">
      <nav v-if="crumbs.length" :aria-label="t('Path in Drive')">
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
      <nav v-else-if="albumName" :aria-label="t('Album')">
        <button
          :disabled="loading"
          @click="
            albumName = '';
            load('/albums');
          "
        >
          {{ t("Albums") }}</button
        ><AppIcon name="ChevronRight" :size="14" /><span>{{ albumName }}</span>
      </nav>
      <span v-else>{{
        tab === "documents"
          ? t("Documents across all folders")
          : tab === "photos"
            ? t("All photos and videos")
            : t("Your albums")
      }}</span>
      <span v-if="connected && !loading">{{
        plural("{0} item", "{0} items", filtered.length)
      }}</span>
    </div>
    <div
      v-if="connected && refreshing"
      class="library-refresh-note"
      role="status"
    >
      <span class="loading loading-spinner loading-sm" />
      <div>
        <strong>{{ t("Updating in the background") }}</strong>
        <p v-if="tab === 'documents'">
          {{
            t("{0} folders checked · {1} documents · {2} folders pending", [
              view?.foldersDone ?? 0,
              entries.length,
              view?.foldersPending ?? 0,
            ])
          }}
        </p>
        <p v-else>
          {{ t("You can keep browsing while the library updates.") }}
        </p>
      </div>
      <button class="btn btn-ghost btn-sm" @click="emit('queue')">
        {{ t("View queue") }}
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
        {{ t("Try again") }}
      </button>
    </div>
    <p v-if="connected && view?.updatedAt" class="library-cache-time">
      {{
        t("Saved library · updated {0}", [
          new Intl.DateTimeFormat(locale, {
            dateStyle: "short",
            timeStyle: "short",
          }).format(new Date(view.updatedAt * 1000)),
        ])
      }}<span v-if="view.partial"> · {{ t("Partial results") }}</span>
    </p>
    <div v-if="tab === 'documents'" class="library-note">
      <AppIcon name="FileText" :size="18" />
      <p>
        {{
          t(
            "PDFs open here; Office files can be downloaded. Docs and Sheets open in the integrated Proton editor and require an internet connection.",
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
              ? "Open a folder from this computer to sync a copy to your PC. To add new folders from this PC, link it in the computer list."
              : "These folders appear in Computers in Proton Drive. Local changes start syncing automatically.",
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
        <h2>{{ t("Your library starts with a connection.") }}</h2>
        <p>
          {{
            t(
              tab === "albums"
                ? "Sign in with your Proton account to see your albums here."
                : tab === "photos"
                  ? "Sign in with your Proton account to see your photos here."
                  : "Sign in with your Proton account to see your files here.",
            )
          }}
        </p>
        <button
          class="btn btn-primary"
          :disabled="!native"
          @click="emit('connect')"
        >
          <AppIcon name="LogIn" :size="18" /> {{ t("Connect Proton account") }}
        </button>
      </div>
      <div
        v-else-if="(loading || refreshing) && !entries.length"
        class="library-empty"
        role="status"
      >
        <span class="loading loading-spinner text-primary loading-lg" />
        <h2>{{ t("Loading your library…") }}</h2>
        <p>
          {{
            tab === "documents"
              ? t("Looking for documents across all folders.")
              : t("Reading and decrypting metadata on your computer.")
          }}
        </p>
      </div>
      <div
        v-else-if="error && !entries.length"
        class="library-empty"
        role="alert"
      >
        <AppIcon name="TriangleAlert" :size="36" />
        <h2>{{ t("Couldn't load.") }}</h2>
        <p>{{ message(error) }}</p>
        <button class="btn btn-soft" @click="load(path, true)">
          {{ t("Try again") }}
        </button>
      </div>
      <div v-else-if="!visible.length" class="library-empty">
        <span class="empty-library-icon"
          ><AppIcon name="FolderOpen" :size="40"
        /></span>
        <h2>
          {{ search ? t("No results here.") : t("This space is still empty.") }}
        </h2>
        <p>
          {{
            search
              ? t("Try another file name.")
              : t(
                  "Items from your account will appear here as soon as they are added.",
                )
          }}
        </p>
      </div>
      <div v-else-if="gallery" class="media-grid">
        <button
          v-for="(entry, index) in visible"
          :key="entry.path"
          class="media-card"
          :aria-label="t('Open {0}', [entry.name])"
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
                  ? plural("{0} photo", "{0} photos", entry.photoCount)
                  : t("Preview")
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
              t("Contents of {0}", [path])
            }}
          </caption>
          <thead>
            <tr>
              <th scope="col">{{ t("Name") }}</th>
              <th scope="col">{{ t("Type / size") }}</th>
              <th scope="col">
                <span class="sr-only"> {{ t("Action") }} </span>
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
                      >{{ t("This computer") }}</small
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
                  :aria-label="t('Open {0}', [entry.name])"
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
        :aria-label="t('Library pages')"
      >
        <button
          class="btn btn-sm btn-ghost"
          :disabled="pageNumber <= 1"
          @click="pageNumber--"
        >
          {{ t("Previous") }}</button
        ><span role="status">{{
          t("Page {0} of {1}", [pageNumber, pages])
        }}</span
        ><button
          class="btn btn-sm btn-ghost"
          :disabled="pageNumber >= pages"
          @click="pageNumber++"
        >
          {{ t("Next") }}
        </button>
      </nav>
    </div>
  </section>
  <p class="library-footnote">
    <AppIcon name="ShieldCheck" :size="15" />
    {{
      t(
        "Previews are downloaded when opened. Your content is decrypted only on this computer.",
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
            selected?.nativeDocument ? t("ONLINE EDITOR") : t("PREVIEW")
          }}</span>
          <h2 id="preview-title">{{ selected?.name }}</h2>
        </div>
        <button
          class="btn btn-ghost btn-square btn-sm"
          :aria-label="t('Close preview')"
          @click="previewDialog?.close()"
        >
          <AppIcon name="X" />
        </button>
      </div>
      <div v-if="previewLoading" class="preview-placeholder" role="status">
        <span class="loading loading-spinner loading-lg" />
        <p>{{ t("Preparing file…") }}</p>
      </div>
      <div v-else-if="selected?.nativeDocument" class="preview-placeholder">
        <AppIcon name="FileText" :size="54" />
        <h3>{{ t("Continue in the Proton editor.") }}</h3>
        <p>
          {{
            t(
              "Open and edit this document in the integrated window. Native content cannot be synced offline by the CLI.",
            )
          }}
        </p>
        <button class="btn btn-primary" @click="openDocument">
          {{ t("Open document") }} <AppIcon name="ArrowUpRight" :size="18" />
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
        <h3>{{ t("Open in your preferred app.") }}</h3>
        <p>
          {{
            t(
              "Download this file to view Office documents, videos and other formats on your computer.",
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
            downloading ? t("Downloading…") : t("Download file")
          }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button :aria-label="t('Close')">{{ t("Close") }}</button>
    </form>
  </dialog>
</template>
