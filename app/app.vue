<script setup lang="ts">
import { version as appVersion } from "../package.json";
import { useI18n } from "~/composables/useI18n";
const { t, message, locale, number, plural } = useI18n();
useHead(() => ({ htmlAttrs: { lang: locale.value } }));
import type { SyncPair } from "~/types";
import type { IconName } from "~/components/AppIcon.vue";
const {
  state,
  native,
  loading,
  notification,
  command,
  act,
  refresh,
  preferences,
  preferencesSaving,
  toast,
  changeLanguage,
  localeChanging,
} = useDrive();
type Page =
  "overview" | "folders" | "activity" | "drive" | "settings" | "queue";
const page = ref<Page>("overview");
const search = ref("");
const filter = ref("all");
const activityFilter = ref("all");
const showPair = ref(false);
const showLibrary = ref(false);
const pairPreset = ref<Partial<SyncPair>>();
const editing = ref<SyncPair>();
const removing = ref<SyncPair>();
const removeDialog = ref<HTMLDialogElement>();
const connecting = ref(false);
const updating = ref(false);
const searchInput = ref<HTMLInputElement>();
useTrayNavigation(
  (target) => {
    showPair.value = false;
    showLibrary.value = false;
    removeDialog.value?.close();
    removing.value = undefined;
    page.value = target;
    nextTick(() =>
      document.getElementById("main-content")?.focus({ preventScroll: true }),
    );
  },
  (error) => toast(error, true),
);
const nav: { id: Page; label: string; icon: IconName }[] = [
  { id: "overview", label: "Overview", icon: "LayoutDashboard" },
  { id: "folders", label: "Synced folders", icon: "Folder" },
  { id: "queue", label: "Operation queue", icon: "ListOrdered" },
  { id: "activity", label: "Activity", icon: "History" },
  { id: "drive", label: "Library", icon: "Cloud" },
];
const titles: Record<Page, string> = {
  overview: "Overview",
  folders: "Synced folders",
  activity: "Activity",
  queue: "Operation queue",
  drive: "Library",
  settings: "Settings",
};
const enabled = computed(
  () => state.value.config.pairs.filter((pair) => pair.enabled).length,
);
const pairs = computed(() =>
  state.value.config.pairs.filter(
    (pair) =>
      `${pair.name} ${pair.localPath} ${pair.remotePath}`
        .toLowerCase()
        .includes(search.value.toLowerCase()) &&
      (filter.value === "all" ||
        (filter.value === "active" ? pair.enabled : !pair.enabled)),
  ),
);
const lastRun = computed(() =>
  Math.max(0, ...state.value.config.pairs.map((pair) => pair.lastRun ?? 0)),
);
const events = computed(() =>
  state.value.config.events.filter(
    (event) =>
      activityFilter.value === "all" ||
      (activityFilter.value === "attention"
        ? ["error", "conflict"].includes(event.kind)
        : event.kind === "success"),
  ),
);
const recent = computed(() => state.value.config.events.slice(0, 5));
const pairQueued = (id: string) =>
  state.value.queue.items.some(
    (item) =>
      item.kind === "sync" && item.pairId === id && item.status === "queued",
  );
const pairChanging = (id: string) =>
  state.value.queue.items.some(
    (item) =>
      item.kind === "configure" &&
      item.pairId === id &&
      ["queued", "running"].includes(item.status),
  );
const statusText = computed(() =>
  state.value.runtime.busy
    ? ({
        sync: t("Syncing"),
        login: t("Connecting account"),
        update: t("Updating CLI"),
      }[state.value.runtime.operation] ?? t("Working"))
    : !state.value.runtime.connected
      ? t("Account not connected")
      : state.value.config.paused
        ? t("Sync paused")
        : t("Ready to sync"),
);
const time = (timestamp: number | null) =>
  timestamp
    ? new Date(timestamp * 1000).toLocaleTimeString(locale.value, {
        hour: "2-digit",
        minute: "2-digit",
      })
    : "—";
const date = (timestamp: number | null) =>
  timestamp
    ? new Date(timestamp * 1000).toLocaleString(locale.value, {
        day: "2-digit",
        month: "short",
        hour: "2-digit",
        minute: "2-digit",
      })
    : t("Not checked yet");
const eventIcon = (kind: string): IconName =>
  (
    ({
      success: "CheckCheck",
      error: "TriangleAlert",
      conflict: "ArrowLeftRight",
      info: "RefreshCw",
    }) as Record<string, IconName>
  )[kind] ?? "Clock3";
function addPair() {
  if (!native.value) {
    toast(t("Open the desktop app to select and sync folders."), true);
    return;
  }
  if (!state.value.runtime.connected) {
    toast(t("Connect your Proton account before adding a folder."), true);
    return;
  }
  editing.value = undefined;
  pairPreset.value = undefined;
  showPair.value = true;
}
function syncLibrary(preset: Partial<SyncPair>) {
  addPair();
  pairPreset.value = preset;
}
async function savedLibrary() {
  showLibrary.value = false;
  await refresh();
  page.value = "folders";
  toast(t("Library configured. Sync will start automatically."));
}
function editPair(pair: SyncPair) {
  editing.value = pair;
  showPair.value = true;
}
async function savedPair() {
  showPair.value = false;
  await refresh();
  toast(
    t("Folder connected. Changes on your computer will sync automatically."),
  );
}
async function connect() {
  connecting.value = true;
  await act("login");
  connecting.value = false;
}
async function update() {
  updating.value = true;
  await act("update_cli", undefined, t("CLI check complete."));
  updating.value = false;
}
async function togglePair(pair: SyncPair) {
  await act("save_pair", { pair: { ...pair, enabled: !pair.enabled } });
}
async function removePair() {
  if (!removing.value) return;
  await act("remove_pair", { id: removing.value.id });
  removeDialog.value?.close();
  removing.value = undefined;
}
async function confirmRemove(pair: SyncPair) {
  removing.value = pair;
  await nextTick();
  removeDialog.value?.showModal();
}
function keydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === "k") {
    event.preventDefault();
    searchInput.value?.focus();
  }
}
onMounted(() => window.addEventListener("keydown", keydown));
onUnmounted(() => window.removeEventListener("keydown", keydown));
watch(page, async () => {
  if (document.activeElement === searchInput.value) return;
  await nextTick();
  document.getElementById("main-content")?.focus({ preventScroll: true });
});
</script>

<template>
  <div class="desktop-shell" data-theme="drive">
    <a class="skip-link" href="#main-content">{{ t("Skip to content") }}</a>
    <aside class="sidebar">
      <a
        href="#"
        class="brand"
        :aria-label="t('CapyDock — home')"
        @click.prevent="page = 'overview'"
        ><img
          class="brand-capy"
          src="/icon.svg"
          alt=""
          width="42"
          height="42"
          aria-hidden="true"
        />
        <div><strong>CapyDock</strong><span>DESKTOP</span></div></a
      >
      <div class="workspace-label">{{ t("PERSONAL SPACE") }}</div>
      <nav :aria-label="t('Main navigation')">
        <button
          v-for="item in nav"
          :key="item.id"
          class="nav-item"
          :class="{ active: page === item.id }"
          :aria-label="t(item.label)"
          :title="t(item.label)"
          :aria-current="page === item.id ? 'page' : undefined"
          @click="page = item.id"
        >
          <AppIcon :name="item.icon" :size="19" /><span>{{
            t(item.label)
          }}</span
          ><span
            v-if="item.id === 'folders' && state.config.pairs.length"
            class="nav-count"
            >{{ number(state.config.pairs.length) }}</span
          >
        </button>
      </nav>
      <div class="sidebar-bottom">
        <div class="privacy-card">
          <div class="privacy-icon">
            <AppIcon name="ShieldCheck" :size="20" />
          </div>
          <strong> {{ t("Your files. Yours alone.") }} </strong>
          <p>
            {{ t("Protected by Proton end-to-end encryption.") }}
          </p>
          <span> {{ t("PRIVATE BY DEFAULT") }} </span>
        </div>
        <button
          class="nav-item"
          :class="{ active: page === 'settings' }"
          :aria-current="page === 'settings' ? 'page' : undefined"
          :aria-label="t('Settings')"
          :title="t('Settings')"
          @click="page = 'settings'"
        >
          <AppIcon name="Settings2" :size="19" /> {{ t("Settings") }}
        </button>
        <div class="account">
          <div class="avatar placeholder">
            <div>
              {{
                state.runtime.connected
                  ? (state.config.accountEmail?.[0]?.toUpperCase() ?? "P")
                  : "P"
              }}
            </div>
          </div>
          <div class="account-copy">
            <strong>{{
              state.runtime.connected
                ? (state.config.accountEmail ?? t("Proton account"))
                : t("Your Proton account")
            }}</strong
            ><span>{{
              state.runtime.connected
                ? t("Connected to desktop")
                : t("Connect to get started")
            }}</span>
          </div>
          <button
            class="btn btn-ghost btn-square btn-sm"
            :aria-label="
              state.runtime.connected
                ? t('Account settings')
                : t('Connect account')
            "
            :disabled="connecting || !native"
            @click="state.runtime.connected ? (page = 'settings') : connect()"
          >
            <AppIcon
              :name="state.runtime.connected ? 'ChevronRight' : 'LogIn'"
              :size="18"
            />
          </button>
        </div>
      </div>
    </aside>

    <div class="workspace">
      <header class="topbar">
        <div class="breadcrumb">
          <AppIcon name="Monitor" :size="17" /><span>
            {{ t("My computer") }} </span
          ><span class="slash">/</span><strong>{{ t(titles[page]) }}</strong>
        </div>
        <div class="topbar-right">
          <label class="search-box"
            ><AppIcon name="Search" :size="16" /><input
              ref="searchInput"
              v-model="search"
              :aria-label="t('Search folders')"
              :placeholder="t('Search folders...')"
              @input="page = 'folders'"
            /><kbd>Ctrl K</kbd></label
          ><button
            class="btn btn-ghost btn-square btn-sm"
            :aria-label="t('Help and information')"
            @click="page = 'settings'"
          >
            <AppIcon name="CircleHelp" :size="19" />
          </button>
        </div>
      </header>
      <main id="main-content" tabindex="-1">
        <div v-if="!native && !loading" class="preview-banner">
          <AppIcon name="Monitor" :size="16" /><span>
            {{
              t(
                "Interface preview · connection and sync are available in the desktop app.",
              )
            }}
          </span>
        </div>
        <OperationQueue
          v-if="
            page !== 'queue' && (state.queue.items.length || state.queue.paused)
          "
          :queue="state.queue"
          :native="native"
          compact
          @open="page = 'queue'"
        />
        <OperationQueue
          v-if="page === 'queue'"
          :queue="state.queue"
          :native="native"
          @cancel="(id) => act('cancel_operation', { id })"
          @pause="(paused) => act('pause_operation_queue', { paused })"
          @clear="act('clear_operation_history')"
        />
        <div
          v-if="state.runtime.operation === 'login'"
          class="operation-banner"
          role="status"
        >
          <span class="loading loading-spinner loading-sm" />
          <div>
            <strong> {{ t("Continue in your browser") }} </strong>
            <p>
              {{
                t("Sign in to Proton. This window will update once connected.")
              }}
            </p>
          </div>
        </div>
        <div
          v-if="state.runtime.operation === 'sync' && page !== 'queue'"
          class="operation-banner"
          role="status"
        >
          <span class="loading loading-spinner loading-sm" />
          <div>
            <strong> {{ t("Syncing your files") }} </strong>
            <p>
              {{
                state.runtime.currentFile
                  ? message(state.runtime.currentFile)
                  : t("Preparing to sync…")
              }}
            </p>
          </div>
          <button class="btn btn-sm btn-ghost" @click="act('cancel_sync')">
            {{ t("Stop after this file") }}
          </button>
        </div>

        <template v-if="page === 'overview' || page === 'folders'">
          <div class="page-heading">
            <div>
              <div class="eyebrow">{{ t("YOUR SPACE, IN SYNC") }}</div>
              <h1>
                {{
                  page === "overview"
                    ? t("Everything in its place.")
                    : t("Your folders, connected.")
                }}
              </h1>
              <p>
                {{
                  page === "overview"
                    ? t(
                        "From your computer to the cloud. With all your privacy.",
                      )
                    : t("Choose what stays in sync with your Proton Drive.")
                }}
              </p>
            </div>
            <div
              class="status-pill"
              :class="{
                online: state.runtime.connected && !state.config.paused,
                syncing: state.runtime.busy,
              }"
            >
              <span class="status-dot" />{{ statusText }}
            </div>
          </div>
          <section v-if="page === 'overview'" class="welcome-card">
            <div class="welcome-copy">
              <span class="mini-label"
                ><span /> {{ t("YOUR DRIVE, CLOSER") }}
              </span>
              <h2>
                {{ t("One seamless flow.") }} <br />
                {{ t("Wherever you are.") }}
              </h2>
              <p>
                {{ t("Your Linux folders and your Proton Drive,") }}
                <br class="desktop-break" />
                {{ t("connected simply and securely.") }}
              </p>
              <button
                v-if="!state.runtime.connected"
                class="btn btn-primary"
                :disabled="connecting || !native"
                @click="connect"
              >
                <AppIcon name="LogIn" :size="17" />{{
                  connecting
                    ? state.runtime.operation === "login"
                      ? t("Connecting…")
                      : t("Queued…")
                    : t("Connect Proton account")
                }}<AppIcon name="ArrowRight" :size="16" /></button
              ><button
                v-else
                class="btn btn-primary"
                :disabled="!native"
                @click="addPair"
              >
                <AppIcon name="Plus" :size="18" /> {{ t("Connect a folder") }}
                <AppIcon name="ArrowRight" :size="16" /></button
              ><span class="welcome-footnote"
                ><AppIcon name="ShieldCheck" :size="13" />
                {{ t("Secure browser sign-in") }}
              </span>
            </div>
            <SyncVisual />
            <div class="hero-caption">
              <AppIcon name="Monitor" :size="14" /> {{ t("Your computer") }}
              <span>↔</span><AppIcon name="Cloud" :size="15" />Proton Drive
            </div>
          </section>
          <div class="stats-grid">
            <div class="metric">
              <span class="metric-icon lavender"
                ><AppIcon name="Folder" :size="20"
              /></span>
              <div>
                <span> {{ t("Connected folders") }} </span>
                <div class="metric-value">
                  {{ number(state.config.pairs.length)
                  }}<small
                    >{{
                      plural("{0} active folder", "{0} active folders", enabled)
                    }}
                  </small>
                </div>
              </div>
            </div>
            <div class="metric">
              <span class="metric-icon blue"
                ><AppIcon name="RefreshCw" :size="20"
              /></span>
              <div>
                <span> {{ t("Last checked") }} </span>
                <div class="metric-value" :class="{ 'metric-empty': !lastRun }">
                  {{ lastRun ? time(lastRun) : t("It all starts here")
                  }}<small>{{
                    lastRun
                      ? new Date(lastRun * 1000).toLocaleDateString(locale)
                      : t("Connect your first folder")
                  }}</small>
                </div>
              </div>
            </div>
            <div class="metric">
              <span class="metric-icon green"
                ><AppIcon name="ShieldCheck" :size="21"
              /></span>
              <div>
                <span> {{ t("File protection") }} </span>
                <div class="metric-value metric-label">
                  {{ t("End-to-end") }}
                  <small> {{ t("Proton encryption") }} </small>
                </div>
              </div>
            </div>
          </div>
          <div
            class="content-columns"
            :class="{ 'full-width': page === 'folders' }"
          >
            <section class="folders-section">
              <div class="section-heading">
                <div>
                  <h2>
                    {{ t("Synced folders") }}
                    <span class="count-badge">{{
                      number(state.config.pairs.length)
                    }}</span>
                  </h2>
                  <p>
                    {{ t("Folders, photos and albums always within reach.") }}
                    <button class="text-primary" @click="page = 'drive'">
                      {{ t("Explore library →") }}
                    </button>
                  </p>
                </div>
                <button
                  class="btn btn-primary btn-sm"
                  :disabled="!native"
                  @click="addPair"
                >
                  <AppIcon name="Plus" :size="16" /> {{ t("Add folder") }}
                </button>
              </div>
              <div class="folder-toolbar">
                <div
                  class="filter-tabs"
                  role="group"
                  :aria-label="t('Filter folders')"
                >
                  <button
                    :class="{ selected: filter === 'all' }"
                    :aria-pressed="filter === 'all'"
                    @click="filter = 'all'"
                  >
                    {{ t("All folders") }}</button
                  ><button
                    :class="{ selected: filter === 'active' }"
                    :aria-pressed="filter === 'active'"
                    @click="filter = 'active'"
                  >
                    {{ t("Active") }}</button
                  ><button
                    :class="{ selected: filter === 'paused' }"
                    :aria-pressed="filter === 'paused'"
                    @click="filter = 'paused'"
                  >
                    {{ t("Paused folders") }}
                  </button>
                </div>
                <button
                  class="text-button"
                  :disabled="!native || !state.config.pairs.length"
                  @click="preferences({ paused: !state.config.paused })"
                >
                  <AppIcon
                    :name="state.config.paused ? 'Play' : 'Pause'"
                    :size="14"
                  />{{ state.config.paused ? t("Resume all") : t("Pause all") }}
                </button>
              </div>
              <div v-if="pairs.length" class="pair-grid">
                <PairCard
                  v-for="pair in pairs"
                  :key="pair.id"
                  :pair="pair"
                  :queued="pairQueued(pair.id)"
                  :changing="pairChanging(pair.id)"
                  :connected="state.runtime.connected"
                  :paused="state.config.paused"
                  :syncing="state.runtime.currentPair === pair.id"
                  @sync="act('sync_now', { id: pair.id })"
                  @toggle="togglePair(pair)"
                  @edit="editPair(pair)"
                  @remove="confirmRemove(pair)"
                  @open="act('open_local', { id: pair.id })"
                  @recovery="act('open_recovery', { id: pair.id })"
                />
              </div>
              <div v-else class="empty-folders">
                <div class="empty-folder-art">
                  <AppIcon name="Folder" :size="47" :stroke-width="1.2" /><span
                    ><AppIcon name="Plus" :size="15"
                  /></span>
                </div>
                <h3>
                  {{
                    state.config.pairs.length
                      ? t("No folders found")
                      : t("Your first folder is the beginning.")
                  }}
                </h3>
                <p>
                  {{
                    state.config.pairs.length
                      ? t("Try another name or change the filter.")
                      : t("Connect a folder and keep your files within reach.")
                  }}
                </p>
                <button
                  v-if="!state.config.pairs.length"
                  class="btn btn-outline btn-sm"
                  :disabled="!native"
                  @click="addPair"
                >
                  <AppIcon name="FolderPlus" :size="16" />
                  {{ t("Choose a folder") }}</button
                ><button
                  v-else
                  class="btn btn-ghost btn-sm"
                  @click="
                    search = '';
                    filter = 'all';
                  "
                >
                  {{ t("Clear filters") }}
                </button>
                <div v-if="!state.config.pairs.length" class="empty-footnote">
                  <AppIcon name="Check" :size="13" />
                  {{ t("Your files stay on your computer") }}
                </div>
              </div>
              <div class="sync-note">
                <AppIcon name="ShieldCheck" :size="16" />
                <p>
                  {{
                    t(
                      "You're in control. Configure deletions for each folder; conflicts preserve both copies.",
                    )
                  }}
                </p>
              </div>
            </section>
            <aside v-if="page === 'overview'" class="activity-panel">
              <div class="section-heading">
                <h2>{{ t("Recent activity") }}</h2>
                <span class="live-dot" />
              </div>
              <div v-if="recent.length" class="compact-events">
                <div v-for="event in recent" :key="event.id" class="event-row">
                  <span class="event-icon" :class="event.kind"
                    ><AppIcon :name="eventIcon(event.kind)" :size="16"
                  /></span>
                  <div>
                    <p>{{ message(event.message) }}</p>
                    <span>{{ time(event.timestamp) }}</span>
                  </div>
                </div>
              </div>
              <div v-else class="activity-empty">
                <div class="activity-lines">
                  <span /><span /><span /><AppIcon name="Check" :size="18" />
                </div>
                <h3>{{ t("A little peace of mind.") }}</h3>
                <p>
                  {{ t("Syncs and updates") }} <br />
                  {{ t("will appear here.") }}
                </p>
              </div>
              <button class="activity-link" @click="page = 'activity'">
                {{ t("View all activity") }}
                <AppIcon name="ArrowRight" :size="15" />
              </button>
            </aside>
          </div>
        </template>

        <template v-else-if="page === 'activity'"
          ><div class="page-heading">
            <div>
              <div class="eyebrow">{{ t("EVERY FILE, EVERY STEP") }}</div>
              <h1>{{ t("What's happening here.") }}</h1>
              <p>
                {{ t("Follow syncs, updates and files that need attention.") }}
              </p>
            </div>
          </div>
          <section class="surface activity-surface">
            <div
              class="filter-tabs"
              role="group"
              :aria-label="t('Filter activity')"
            >
              <button
                :class="{ selected: activityFilter === 'all' }"
                :aria-pressed="activityFilter === 'all'"
                @click="activityFilter = 'all'"
              >
                {{ t("All activity") }}</button
              ><button
                :class="{ selected: activityFilter === 'success' }"
                :aria-pressed="activityFilter === 'success'"
                @click="activityFilter = 'success'"
              >
                {{ t("Completed operations") }}</button
              ><button
                :class="{ selected: activityFilter === 'attention' }"
                :aria-pressed="activityFilter === 'attention'"
                @click="activityFilter = 'attention'"
              >
                {{ t("Needs attention") }}
              </button>
            </div>
            <div v-if="events.length" class="event-table">
              <div v-for="event in events" :key="event.id" class="event-row">
                <span class="event-icon" :class="event.kind"
                  ><AppIcon :name="eventIcon(event.kind)" :size="18"
                /></span>
                <div>
                  <p>{{ message(event.message) }}</p>
                  <span>{{ date(event.timestamp) }}</span>
                </div>
                <span class="event-tag">{{
                  event.kind === "success"
                    ? t("Completed")
                    : event.kind === "error"
                      ? t("Error")
                      : event.kind === "conflict"
                        ? t("Review")
                        : t("Information")
                }}</span>
              </div>
            </div>
            <div v-else class="large-empty">
              <AppIcon name="History" :size="40" />
              <h2>{{ t("No activity yet.") }}</h2>
              <p>
                {{ t("As soon as a sync runs, you can follow it here.") }}
              </p>
            </div>
          </section>
          <div class="help-note">
            <AppIcon name="CircleHelp" :size="18" />
            <p>
              {{
                t(
                  "A conflict means a file changed on both sides or a copy was removed. Compare the copies on your computer and in Drive, and keep the content you want on both sides. The next sync will recognize that the files are identical.",
                )
              }}
            </p>
          </div></template
        >

        <CloudLibrary
          @queue="page = 'queue'"
          v-else-if="page === 'drive'"
          :connected="state.runtime.connected"
          :native="native"
          :command="command"
          :computer="state.computer"
          :pairs="state.config.pairs"
          @registered="refresh"
          @manage="page = 'folders'"
          @connect="connect"
          @sync="syncLibrary"
          @full="showLibrary = true"
          @notify="toast"
        />

        <template v-else-if="page === 'settings'"
          ><div class="page-heading">
            <div>
              <div class="eyebrow">{{ t("YOUR WAY") }}</div>
              <h1>{{ t("Small adjustments. All set.") }}</h1>
              <p>
                {{ t("Manage your account and let the app work for you.") }}
              </p>
            </div>
          </div>
          <div class="settings-grid">
            <LanguageSetting
              :saving="localeChanging"
              @change="changeLanguage"
            />
            <section class="surface settings-card">
              <div class="settings-heading">
                <AppIcon name="Monitor" :size="21" />
                <h2>{{ t("App and sync") }}</h2>
              </div>
              <div class="setting-row">
                <div>
                  <strong> {{ t("Start with your computer") }} </strong>
                  <p>
                    {{
                      t("Open the app when you sign in to your Linux session.")
                    }}
                  </p>
                </div>
                <input
                  type="checkbox"
                  class="toggle toggle-primary toggle-sm"
                  :aria-label="t('Start with your computer')"
                  :checked="state.autostart"
                  :disabled="!native || preferencesSaving"
                  @change="
                    preferences({
                      autostart: ($event.target as HTMLInputElement).checked,
                    })
                  "
                />
              </div>
              <TraySetting
                :enabled="state.config.closeToTray"
                :available="state.trayAvailable"
                :native="native"
                :saving="preferencesSaving"
                @change="preferences({ closeToTray: $event })"
              />
              <div class="setting-row">
                <div>
                  <strong> {{ t("Pause syncs") }} </strong>
                  <p>
                    {{ t("The current operation finishes before pausing.") }}
                  </p>
                </div>
                <input
                  type="checkbox"
                  class="toggle toggle-primary toggle-sm"
                  :aria-label="t('Pause syncs')"
                  :checked="state.config.paused"
                  :disabled="!native || preferencesSaving"
                  @change="
                    preferences({
                      paused: ($event.target as HTMLInputElement).checked,
                    })
                  "
                />
              </div>
              <div class="settings-info">
                <AppIcon name="CircleHelp" :size="17" />
                <p>
                  {{
                    t(
                      "With System tray enabled, syncing continues even when the window is hidden. Local changes trigger syncing; checking for changes in Drive uses each folder’s interval.",
                    )
                  }}
                </p>
              </div>
            </section>
            <section class="surface settings-card">
              <div class="settings-heading">
                <AppIcon name="RefreshCw" :size="21" />
                <h2>Proton Drive CLI</h2>
                <span class="badge badge-sm badge-soft">{{
                  state.runtime.cliVersion
                    ? `v${state.runtime.cliVersion}`
                    : native
                      ? t("Loading")
                      : t("Desktop")
                }}</span>
              </div>
              <div class="setting-row">
                <div>
                  <strong> {{ t("Update automatically") }} </strong>
                  <p>
                    {{ t("Checks for new stable versions every 24 hours.") }}
                  </p>
                </div>
                <input
                  type="checkbox"
                  class="toggle toggle-primary toggle-sm"
                  :aria-label="t('Update CLI automatically')"
                  :checked="state.config.autoUpdate"
                  :disabled="!native || preferencesSaving"
                  @change="
                    preferences({
                      autoUpdate: ($event.target as HTMLInputElement).checked,
                    })
                  "
                />
              </div>
              <div class="update-details">
                <span
                  ><AppIcon name="ShieldCheck" :size="15" />
                  {{ t("Official source · SHA-512 verified") }}
                </span>
                <p>
                  {{ t("Last checked:") }}
                  {{ date(state.config.lastUpdateCheck) }}
                </p>
                <button
                  class="btn btn-outline btn-sm"
                  :disabled="updating || !native"
                  @click="update"
                >
                  <AppIcon
                    name="RefreshCw"
                    :size="15"
                    :class="{ 'spin-slow': updating }"
                  />{{
                    updating
                      ? state.runtime.operation === "update"
                        ? t("Checking…")
                        : t("Queued…")
                      : t("Check for updates")
                  }}
                </button>
              </div>
            </section>
            <section class="surface settings-card">
              <div class="settings-heading">
                <AppIcon name="ShieldCheck" :size="21" />
                <h2>{{ t("Proton account") }}</h2>
              </div>
              <div class="connection-detail">
                <span
                  class="status-pill"
                  :class="{ online: state.runtime.connected }"
                  ><span class="status-dot" />{{
                    state.runtime.connected
                      ? t("Connected")
                      : t("Not connected")
                  }}</span
                ><strong v-if="state.config.accountEmail">{{
                  state.config.accountEmail
                }}</strong>
                <p>
                  {{
                    t(
                      "Sign-in happens in your browser. Credentials are stored in your operating system's secure keyring.",
                    )
                  }}
                </p>
                <p v-if="state.runtime.error" class="connection-error">
                  {{ message(state.runtime.error) }}
                </p>
                <div class="button-row">
                  <button
                    v-if="!state.runtime.connected"
                    class="btn btn-primary btn-sm"
                    :disabled="connecting || !native"
                    @click="connect"
                  >
                    {{ t("Connect account") }}</button
                  ><button
                    v-else
                    class="btn btn-outline btn-sm"
                    :disabled="!native"
                    @click="act('logout')"
                  >
                    <AppIcon name="LogOut" :size="15" />
                    {{ t("Disconnect") }}</button
                  ><button
                    class="btn btn-ghost btn-sm"
                    :disabled="!native"
                    @click="act('refresh_connection')"
                  >
                    {{ t("Check connection") }}
                  </button>
                </div>
              </div>
            </section>
            <section class="surface settings-card about-card">
              <div class="settings-heading">
                <AppIcon name="Sparkles" :size="21" />
                <h2>{{ t("Made for your Linux") }}</h2>
              </div>
              <p>
                CapyDock <strong>{{ appVersion }}</strong>
              </p>
              <p>
                {{
                  t(
                    "Independent interface built with Tauri, Nuxt and DaisyUI. Uses the official Proton CLI.",
                  )
                }}
              </p>
              <p class="muted">
                {{ t("This app is not an official Proton AG product.") }}
              </p>
              <div v-if="state.dataPath" class="data-path">
                <span> {{ t("Settings and history") }} </span
                ><code>{{ state.dataPath }}</code>
              </div>
            </section>
          </div></template
        >

        <footer class="app-footer">
          <span
            ><AppIcon name="ShieldCheck" :size="14" />
            {{ t("Privacy in every sync.") }} </span
          ><span
            >Linux <span class="footer-dot">·</span>
            {{ t("Independent client") }} <span class="footer-dot">·</span>
            {{
              state.runtime.cliVersion
                ? t("CLI {0}", [state.runtime.cliVersion])
                : "CapyDock"
            }}</span
          >
        </footer>
      </main>
    </div>
    <Transition name="toast"
      ><div
        v-if="notification"
        class="notification"
        :class="{ error: notification.error }"
        :role="notification.error ? 'alert' : 'status'"
      >
        <AppIcon
          :name="notification.error ? 'TriangleAlert' : 'CheckCheck'"
          :size="20"
        />
        <p>{{ message(notification.message) }}</p>
        <button
          class="btn btn-ghost btn-square btn-xs"
          :aria-label="t('Dismiss notification')"
          @click="notification = null"
        >
          <AppIcon name="X" :size="16" />
        </button></div
    ></Transition>
    <PairDialog
      v-if="showPair"
      :pair="editing"
      :preset="pairPreset"
      :command="command"
      @close="showPair = false"
      @saved="savedPair"
    />
    <LibraryDialog
      v-if="showLibrary"
      :command="command"
      @close="showLibrary = false"
      @saved="savedLibrary"
    />
    <dialog ref="removeDialog" class="modal" aria-labelledby="remove-title">
      <div class="modal-box">
        <h2 id="remove-title" class="text-xl font-semibold">
          {{ t("Remove {0}?", [removing?.name]) }}
        </h2>
        <p class="py-4 text-sm">
          {{
            t(
              "Files on your computer and in Proton Drive will be kept. Only the sync pairing will be removed.",
            )
          }}
        </p>
        <div class="modal-action">
          <button class="btn btn-ghost" @click="removeDialog?.close()">
            {{ t("Cancel") }}</button
          ><button class="btn btn-error" @click="removePair">
            {{ t("Remove pairing") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button>{{ t("Close") }}</button>
      </form>
    </dialog>
  </div>
</template>
