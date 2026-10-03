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
const nav: { id: Page; label: string; icon: IconName }[] = [
  { id: "overview", label: "Visão geral", icon: "LayoutDashboard" },
  { id: "folders", label: "Pastas sincronizadas", icon: "Folder" },
  { id: "queue", label: "Fila de operações", icon: "ListOrdered" },
  { id: "activity", label: "Atividade", icon: "History" },
  { id: "drive", label: "Biblioteca", icon: "Cloud" },
];
const titles: Record<Page, string> = {
  overview: "Visão geral",
  folders: "Pastas sincronizadas",
  activity: "Atividade",
  queue: "Fila de operações",
  drive: "Biblioteca",
  settings: "Configurações",
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
        sync: t("Sincronizando"),
        login: t("Conectando conta"),
        update: t("Atualizando CLI"),
      }[state.value.runtime.operation] ?? t("Trabalhando"))
    : !state.value.runtime.connected
      ? t("Conta não conectada")
      : state.value.config.paused
        ? t("Sincronização pausada")
        : t("Pronto para sincronizar"),
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
    : t("Ainda não verificado");
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
    toast(
      t("Abra o aplicativo desktop para selecionar e sincronizar pastas."),
      true,
    );
    return;
  }
  if (!state.value.runtime.connected) {
    toast(t("Conecte sua conta Proton antes de adicionar uma pasta."), true);
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
  toast(
    t("Biblioteca configurada. A sincronização será iniciada automaticamente."),
  );
}
function editPair(pair: SyncPair) {
  editing.value = pair;
  showPair.value = true;
}
async function savedPair() {
  showPair.value = false;
  await refresh();
  toast(
    t(
      "Pasta conectada. Alterações no computador serão sincronizadas automaticamente.",
    ),
  );
}
async function connect() {
  connecting.value = true;
  await act("login");
  connecting.value = false;
}
async function update() {
  updating.value = true;
  await act("update_cli", undefined, t("Verificação do CLI concluída."));
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
    <a class="skip-link" href="#main-content">{{
      t("Pular para o conteúdo")
    }}</a>
    <aside class="sidebar">
      <a
        href="#"
        class="brand"
        :aria-label="t('CapyDock — início')"
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
      <div class="workspace-label">{{ t("ESPAÇO PESSOAL") }}</div>
      <nav :aria-label="t('Navegação principal')">
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
          <strong> {{ t("Seus arquivos. Só seus.") }} </strong>
          <p>
            {{ t("Protegidos pela criptografia de ponta a ponta da Proton.") }}
          </p>
          <span> {{ t("PRIVACIDADE POR PADRÃO") }} </span>
        </div>
        <button
          class="nav-item"
          :class="{ active: page === 'settings' }"
          :aria-current="page === 'settings' ? 'page' : undefined"
          :aria-label="t('Configurações')"
          :title="t('Configurações')"
          @click="page = 'settings'"
        >
          <AppIcon name="Settings2" :size="19" /> {{ t("Configurações") }}
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
                ? (state.config.accountEmail ?? t("Conta Proton"))
                : t("Sua conta Proton")
            }}</strong
            ><span>{{
              state.runtime.connected
                ? t("Conectada ao desktop")
                : t("Conecte para começar")
            }}</span>
          </div>
          <button
            class="btn btn-ghost btn-square btn-sm"
            :aria-label="
              state.runtime.connected
                ? t('Configurações da conta')
                : t('Conectar conta')
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
            {{ t("Meu computador") }} </span
          ><span class="slash">/</span><strong>{{ t(titles[page]) }}</strong>
        </div>
        <div class="topbar-right">
          <label class="search-box"
            ><AppIcon name="Search" :size="16" /><input
              ref="searchInput"
              v-model="search"
              :aria-label="t('Buscar pastas')"
              :placeholder="t('Buscar pastas...')"
              @input="page = 'folders'"
            /><kbd>Ctrl K</kbd></label
          ><button
            class="btn btn-ghost btn-square btn-sm"
            :aria-label="t('Ajuda e informações')"
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
                "Prévia da interface · conexão e sincronização disponíveis no aplicativo desktop.",
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
            <strong> {{ t("Continue no navegador") }} </strong>
            <p>
              {{
                t(
                  "Faça login na Proton. Esta janela será atualizada quando a conexão terminar.",
                )
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
            <strong> {{ t("Sincronizando seus arquivos") }} </strong>
            <p>
              {{
                state.runtime.currentFile
                  ? message(state.runtime.currentFile)
                  : t("Preparando a sincronização…")
              }}
            </p>
          </div>
          <button class="btn btn-sm btn-ghost" @click="act('cancel_sync')">
            {{ t("Parar após este arquivo") }}
          </button>
        </div>

        <template v-if="page === 'overview' || page === 'folders'">
          <div class="page-heading">
            <div>
              <div class="eyebrow">{{ t("SEU ESPAÇO, EM SINCRONIA") }}</div>
              <h1>
                {{
                  page === "overview"
                    ? t("Tudo no seu lugar.")
                    : t("Suas pastas, conectadas.")
                }}
              </h1>
              <p>
                {{
                  page === "overview"
                    ? t(
                        "Do seu computador para a nuvem. Com toda a sua privacidade.",
                      )
                    : t(
                        "Escolha o que fica em sintonia com o seu Proton Drive.",
                      )
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
                ><span /> {{ t("SEU DRIVE, MAIS PERTO") }}
              </span>
              <h2>
                {{ t("Um só fluxo.") }} <br />
                {{ t("Onde você estiver.") }}
              </h2>
              <p>
                {{ t("Suas pastas do Linux e seu Proton Drive,") }}
                <br class="desktop-break" />
                {{ t("conectados de um jeito simples e seguro.") }}
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
                      ? t("Conectando…")
                      : t("Na fila…")
                    : t("Conectar conta Proton")
                }}<AppIcon name="ArrowRight" :size="16" /></button
              ><button
                v-else
                class="btn btn-primary"
                :disabled="!native"
                @click="addPair"
              >
                <AppIcon name="Plus" :size="18" /> {{ t("Conectar uma pasta") }}
                <AppIcon name="ArrowRight" :size="16" /></button
              ><span class="welcome-footnote"
                ><AppIcon name="ShieldCheck" :size="13" />
                {{ t("Login seguro pelo navegador") }}
              </span>
            </div>
            <SyncVisual />
            <div class="hero-caption">
              <AppIcon name="Monitor" :size="14" /> {{ t("Seu computador") }}
              <span>↔</span><AppIcon name="Cloud" :size="15" />Proton Drive
            </div>
          </section>
          <div class="stats-grid">
            <div class="metric">
              <span class="metric-icon lavender"
                ><AppIcon name="Folder" :size="20"
              /></span>
              <div>
                <span> {{ t("Pastas conectadas") }} </span>
                <div class="metric-value">
                  {{ number(state.config.pairs.length)
                  }}<small
                    >{{ plural("{0} ativa", "{0} ativas", enabled) }}
                  </small>
                </div>
              </div>
            </div>
            <div class="metric">
              <span class="metric-icon blue"
                ><AppIcon name="RefreshCw" :size="20"
              /></span>
              <div>
                <span> {{ t("Última verificação") }} </span>
                <div class="metric-value" :class="{ 'metric-empty': !lastRun }">
                  {{ lastRun ? time(lastRun) : t("Tudo começa aqui")
                  }}<small>{{
                    lastRun
                      ? new Date(lastRun * 1000).toLocaleDateString(locale)
                      : t("Conecte sua primeira pasta")
                  }}</small>
                </div>
              </div>
            </div>
            <div class="metric">
              <span class="metric-icon green"
                ><AppIcon name="ShieldCheck" :size="21"
              /></span>
              <div>
                <span> {{ t("Proteção dos arquivos") }} </span>
                <div class="metric-value metric-label">
                  {{ t("Ponta a ponta") }}
                  <small> {{ t("Criptografia da Proton") }} </small>
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
                    {{ t("Pastas sincronizadas") }}
                    <span class="count-badge">{{
                      number(state.config.pairs.length)
                    }}</span>
                  </h2>
                  <p>
                    {{ t("Pastas, fotos e álbuns sempre por perto.") }}
                    <button class="text-primary" @click="page = 'drive'">
                      {{ t("Explorar biblioteca →") }}
                    </button>
                  </p>
                </div>
                <button
                  class="btn btn-primary btn-sm"
                  :disabled="!native"
                  @click="addPair"
                >
                  <AppIcon name="Plus" :size="16" /> {{ t("Adicionar pasta") }}
                </button>
              </div>
              <div class="folder-toolbar">
                <div
                  class="filter-tabs"
                  role="group"
                  :aria-label="t('Filtrar pastas')"
                >
                  <button
                    :class="{ selected: filter === 'all' }"
                    :aria-pressed="filter === 'all'"
                    @click="filter = 'all'"
                  >
                    {{ t("Todas") }}</button
                  ><button
                    :class="{ selected: filter === 'active' }"
                    :aria-pressed="filter === 'active'"
                    @click="filter = 'active'"
                  >
                    {{ t("Ativas") }}</button
                  ><button
                    :class="{ selected: filter === 'paused' }"
                    :aria-pressed="filter === 'paused'"
                    @click="filter = 'paused'"
                  >
                    {{ t("Pausadas") }}
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
                  />{{
                    state.config.paused ? t("Retomar tudo") : t("Pausar tudo")
                  }}
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
                      ? t("Nenhuma pasta encontrada")
                      : t("Sua primeira pasta é o começo.")
                  }}
                </h3>
                <p>
                  {{
                    state.config.pairs.length
                      ? t("Tente outro nome ou altere o filtro.")
                      : t(
                          "Conecte uma pasta e deixe seus arquivos sempre por perto.",
                        )
                  }}
                </p>
                <button
                  v-if="!state.config.pairs.length"
                  class="btn btn-outline btn-sm"
                  :disabled="!native"
                  @click="addPair"
                >
                  <AppIcon name="FolderPlus" :size="16" />
                  {{ t("Escolher uma pasta") }}</button
                ><button
                  v-else
                  class="btn btn-ghost btn-sm"
                  @click="
                    search = '';
                    filter = 'all';
                  "
                >
                  {{ t("Limpar filtros") }}
                </button>
                <div v-if="!state.config.pairs.length" class="empty-footnote">
                  <AppIcon name="Check" :size="13" />
                  {{ t("Seus arquivos continuam no computador") }}
                </div>
              </div>
              <div class="sync-note">
                <AppIcon name="ShieldCheck" :size="16" />
                <p>
                  {{
                    t(
                      "Você no controle. Configure as exclusões por pasta; conflitos preservam as duas cópias.",
                    )
                  }}
                </p>
              </div>
            </section>
            <aside v-if="page === 'overview'" class="activity-panel">
              <div class="section-heading">
                <h2>{{ t("Atividade recente") }}</h2>
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
                <h3>{{ t("Um pouco de tranquilidade.") }}</h3>
                <p>
                  {{ t("As sincronizações e atualizações") }} <br />
                  {{ t("vão aparecer por aqui.") }}
                </p>
              </div>
              <button class="activity-link" @click="page = 'activity'">
                {{ t("Ver toda a atividade") }}
                <AppIcon name="ArrowRight" :size="15" />
              </button>
            </aside>
          </div>
        </template>

        <template v-else-if="page === 'activity'"
          ><div class="page-heading">
            <div>
              <div class="eyebrow">{{ t("CADA ARQUIVO, CADA ETAPA") }}</div>
              <h1>{{ t("O que acontece por aqui.") }}</h1>
              <p>
                {{
                  t(
                    "Acompanhe sincronizações, atualizações e arquivos que precisam de atenção.",
                  )
                }}
              </p>
            </div>
          </div>
          <section class="surface activity-surface">
            <div
              class="filter-tabs"
              role="group"
              :aria-label="t('Filtrar atividade')"
            >
              <button
                :class="{ selected: activityFilter === 'all' }"
                :aria-pressed="activityFilter === 'all'"
                @click="activityFilter = 'all'"
              >
                {{ t("Tudo") }}</button
              ><button
                :class="{ selected: activityFilter === 'success' }"
                :aria-pressed="activityFilter === 'success'"
                @click="activityFilter = 'success'"
              >
                {{ t("Concluídas") }}</button
              ><button
                :class="{ selected: activityFilter === 'attention' }"
                :aria-pressed="activityFilter === 'attention'"
                @click="activityFilter = 'attention'"
              >
                {{ t("Precisam de atenção") }}
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
                    ? t("Concluído")
                    : event.kind === "error"
                      ? t("Erro")
                      : event.kind === "conflict"
                        ? t("Revisar")
                        : t("Informação")
                }}</span>
              </div>
            </div>
            <div v-else class="large-empty">
              <AppIcon name="History" :size="40" />
              <h2>{{ t("Nenhuma atividade ainda.") }}</h2>
              <p>
                {{
                  t(
                    "Assim que houver uma sincronização, você acompanha tudo aqui.",
                  )
                }}
              </p>
            </div>
          </section>
          <div class="help-note">
            <AppIcon name="CircleHelp" :size="18" />
            <p>
              {{
                t(
                  "Um conflito significa que um arquivo mudou nos dois lados ou que uma cópia foi removida. Compare as cópias no computador e no Drive e deixe o conteúdo desejado nos dois lados. A próxima sincronização reconhecerá que os arquivos são iguais.",
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
              <div class="eyebrow">{{ t("DO SEU JEITO") }}</div>
              <h1>{{ t("Pequenos ajustes. Tudo certo.") }}</h1>
              <p>
                {{
                  t(
                    "Gerencie sua conta e deixe o aplicativo trabalhar para você.",
                  )
                }}
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
                <h2>{{ t("Aplicativo e sincronização") }}</h2>
              </div>
              <div class="setting-row">
                <div>
                  <strong> {{ t("Iniciar com o computador") }} </strong>
                  <p>
                    {{ t("Abra o aplicativo ao entrar na sua sessão Linux.") }}
                  </p>
                </div>
                <input
                  type="checkbox"
                  class="toggle toggle-primary toggle-sm"
                  :aria-label="t('Iniciar com o computador')"
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
                  <strong> {{ t("Pausar sincronizações") }} </strong>
                  <p>{{ t("A operação atual termina antes da pausa.") }}</p>
                </div>
                <input
                  type="checkbox"
                  class="toggle toggle-primary toggle-sm"
                  :aria-label="t('Pausar sincronizações')"
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
                      "Com System tray ativado, a sincronização continua mesmo com a janela oculta. Alterações locais disparam a sincronização; a busca de mudanças no Drive usa o intervalo de cada pasta.",
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
                      ? t("Carregando")
                      : t("Desktop")
                }}</span>
              </div>
              <div class="setting-row">
                <div>
                  <strong> {{ t("Atualizar automaticamente") }} </strong>
                  <p>
                    {{ t("Verifica novas versões estáveis a cada 24 horas.") }}
                  </p>
                </div>
                <input
                  type="checkbox"
                  class="toggle toggle-primary toggle-sm"
                  :aria-label="t('Atualizar CLI automaticamente')"
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
                  {{ t("Fonte oficial · SHA-512 verificado") }}
                </span>
                <p>
                  {{ t("Última verificação:") }}
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
                        ? t("Verificando…")
                        : t("Na fila…")
                      : t("Verificar atualização")
                  }}
                </button>
              </div>
            </section>
            <section class="surface settings-card">
              <div class="settings-heading">
                <AppIcon name="ShieldCheck" :size="21" />
                <h2>{{ t("Conta Proton") }}</h2>
              </div>
              <div class="connection-detail">
                <span
                  class="status-pill"
                  :class="{ online: state.runtime.connected }"
                  ><span class="status-dot" />{{
                    state.runtime.connected
                      ? t("Conectada")
                      : t("Não conectada")
                  }}</span
                ><strong v-if="state.config.accountEmail">{{
                  state.config.accountEmail
                }}</strong>
                <p>
                  {{
                    t(
                      "O login acontece pelo navegador. As credenciais ficam no cofre seguro do sistema operacional.",
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
                    {{ t("Conectar conta") }}</button
                  ><button
                    v-else
                    class="btn btn-outline btn-sm"
                    :disabled="!native"
                    @click="act('logout')"
                  >
                    <AppIcon name="LogOut" :size="15" />
                    {{ t("Desconectar") }}</button
                  ><button
                    class="btn btn-ghost btn-sm"
                    :disabled="!native"
                    @click="act('refresh_connection')"
                  >
                    {{ t("Verificar conexão") }}
                  </button>
                </div>
              </div>
            </section>
            <section class="surface settings-card about-card">
              <div class="settings-heading">
                <AppIcon name="Sparkles" :size="21" />
                <h2>{{ t("Feito para o seu Linux") }}</h2>
              </div>
              <p>
                CapyDock <strong>{{ appVersion }}</strong>
              </p>
              <p>
                {{
                  t(
                    "Interface independente construída com Tauri, Nuxt e DaisyUI. Usa o CLI oficial da Proton.",
                  )
                }}
              </p>
              <p class="muted">
                {{
                  t("Este aplicativo não é um produto oficial da Proton AG.")
                }}
              </p>
              <div v-if="state.dataPath" class="data-path">
                <span> {{ t("Configurações e histórico") }} </span
                ><code>{{ state.dataPath }}</code>
              </div>
            </section>
          </div></template
        >

        <footer class="app-footer">
          <span
            ><AppIcon name="ShieldCheck" :size="14" />
            {{ t("Privacidade em cada sincronização.") }} </span
          ><span
            >Linux <span class="footer-dot">·</span>
            {{ t("Cliente independente") }} <span class="footer-dot">·</span>
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
          :aria-label="t('Fechar notificação')"
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
          {{ t("Remover {0}?", [removing?.name]) }}
        </h2>
        <p class="py-4 text-sm">
          {{
            t(
              "Os arquivos no computador e no Proton Drive serão mantidos. Apenas o pareamento de sincronização será removido.",
            )
          }}
        </p>
        <div class="modal-action">
          <button class="btn btn-ghost" @click="removeDialog?.close()">
            {{ t("Cancelar") }}</button
          ><button class="btn btn-error" @click="removePair">
            {{ t("Remover pareamento") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button>{{ t("Fechar") }}</button>
      </form>
    </dialog>
  </div>
</template>
