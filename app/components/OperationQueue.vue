<script setup lang="ts">
import { useI18n } from "~/composables/useI18n";
const { t, message, number } = useI18n();
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { Operation, OperationQueue } from "~/types";
const props = defineProps<{
  queue: OperationQueue;
  compact?: boolean;
  native: boolean;
}>();
defineEmits<{
  open: [];
  cancel: [id: string];
  pause: [paused: boolean];
  clear: [];
}>();
const lanePriority = {
  transfer: 0,
  exclusive: 1,
  interactive: 2,
  background: 3,
};
const active = computed(() =>
  props.queue.items
    .filter((item) => ["running", "cancelling"].includes(item.status))
    .sort((a, b) => lanePriority[a.lane] - lanePriority[b.lane]),
);
const running = computed(() => active.value[0]);
const waiting = computed(() =>
  props.queue.items
    .filter((item) => item.status === "queued")
    .sort((a, b) => Number(a.automatic) - Number(b.automatic)),
);
const laneLabel = (item: Operation) =>
  ({
    transfer: t("Transferências e alterações"),
    interactive: t("Navegação"),
    background: t("Índice em segundo plano"),
    exclusive: t("Manutenção da sessão"),
  })[item.lane];
const history = computed(() =>
  props.queue.items
    .filter((item) =>
      ["completed", "failed", "cancelled"].includes(item.status),
    )
    .reverse(),
);
const clock = ref(Date.now() / 1000);
let timer: ReturnType<typeof setInterval>;
onMounted(() => {
  timer = setInterval(() => {
    clock.value = Date.now() / 1000;
  }, 1000);
});
onUnmounted(() => clearInterval(timer));
function elapsed(since: number) {
  const seconds = Math.max(0, Math.floor(clock.value - since));
  return seconds < 60
    ? t("{0}s", [seconds])
    : seconds < 3600
      ? t("{0}min {1}s", [Math.floor(seconds / 60), seconds % 60])
      : t("{0}h {1}min", [
          Math.floor(seconds / 3600),
          Math.floor((seconds % 3600) / 60),
        ]);
}
const label = (item: Operation) =>
  ({
    completed: t("Concluída"),
    failed: t("Falhou"),
    cancelled: t("Cancelada"),
    queued: t("Na fila"),
    running: t("Em execução"),
    cancelling: t("Parando…"),
  })[item.status];
</script>

<template>
  <button
    v-if="compact"
    class="queue-summary"
    @click="$emit('open')"
    :aria-label="t('Ver fila de operações: {0} aguardando', [waiting.length])"
  >
    <span class="queue-summary-icon"
      ><AppIcon
        :name="queue.paused ? 'Pause' : running ? 'RefreshCw' : 'ListOrdered'"
        :size="18"
        :class="{ 'spin-slow': running && !queue.paused }"
    /></span>
    <span class="queue-summary-copy"
      ><strong>{{
        queue.paused
          ? t("Fila pausada")
          : running
            ? message(running.title)
            : t("Fila de operações")
      }}</strong
      ><span>{{
        running
          ? message(running.detail)
          : t("Acompanhe suas transferências e outras operações")
      }}</span></span
    >
    <span class="queue-count"
      >{{ number(waiting.length) }} {{ t("na fila") }} </span
    ><span class="queue-summary-link">
      {{ t("Ver fila") }} <AppIcon name="ChevronRight" :size="16"
    /></span>
  </button>
  <section v-else class="queue-page" aria-labelledby="queue-title">
    <div class="page-heading">
      <div>
        <div class="eyebrow">{{ t("TUDO EM SEU TEMPO") }}</div>
        <h1 id="queue-title">{{ t("Fila de operações") }}</h1>
        <p>
          {{
            t(
              "Navegação e indexação têm filas próprias. As alterações de arquivos continuam protegidas.",
            )
          }}
        </p>
      </div>
      <button
        class="btn"
        :class="queue.paused ? 'btn-primary' : 'btn-soft'"
        :disabled="!native"
        @click="$emit('pause', !queue.paused)"
      >
        <AppIcon :name="queue.paused ? 'Play' : 'Pause'" :size="17" />{{
          queue.paused ? t("Retomar fila") : t("Pausar fila")
        }}
      </button>
    </div>
    <div v-if="queue.paused" class="queue-pause-note" role="status">
      <AppIcon name="Pause" :size="18" />
      <p>
        {{
          t(
            "Fila pausada. As operações em andamento podem terminar; as próximas aguardam.",
          )
        }}
      </p>
    </div>
    <div class="queue-columns">
      <section class="surface queue-running" aria-labelledby="running-title">
        <div class="queue-section-heading">
          <h2 id="running-title">
            {{ t("Em execução") }}
            <span class="count-badge">{{ number(active.length) }}</span>
          </h2>
          <span class="queue-live-dot" :class="{ active: running }" />
        </div>
        <div v-if="active.length" class="queue-active-list">
          <article
            v-for="running in active"
            :key="running.id"
            class="queue-active-card"
          >
            <span class="queue-lane">{{ laneLabel(running) }}</span>
            <div class="queue-operation-icon">
              <AppIcon
                :name="running.status === 'cancelling' ? 'Pause' : 'RefreshCw'"
                :size="30"
                :class="{ 'spin-slow': running.status === 'running' }"
              />
            </div>
            <span class="queue-status" role="status"
              >{{ label(running)
              }}<span v-if="running.automatic">
                {{ t("· Automática") }}
              </span></span
            >
            <h3>{{ message(running.title) }}</h3>
            <p class="queue-detail" :title="message(running.detail)">
              {{ message(running.detail) }}
            </p>
            <div
              class="queue-progress"
              :class="{ stopping: running.status === 'cancelling' }"
              role="progressbar"
              :aria-label="t('Operação em andamento')"
            >
              <span />
            </div>
            <div class="queue-running-footer">
              <span>
                {{ t("Em execução há") }}
                {{ elapsed(running.startedAt ?? running.createdAt) }}</span
              ><button
                v-if="running.canCancelRunning"
                class="btn btn-ghost btn-sm"
                :disabled="running.status === 'cancelling'"
                @click="$emit('cancel', running.id)"
              >
                {{
                  running.status === "cancelling"
                    ? t("Parando com segurança…")
                    : ["interactive", "background"].includes(running.lane)
                      ? t("Cancelar consulta")
                      : t("Parar após este arquivo")
                }}
              </button>
            </div>
            <p v-if="!running.canCancelRunning" class="queue-fine-print">
              {{
                t(
                  "Esta operação será concluída com segurança antes da próxima alteração.",
                )
              }}
            </p>
          </article>
        </div>
        <div v-else class="queue-idle">
          <AppIcon :name="queue.paused ? 'Pause' : 'CheckCheck'" :size="38" />
          <h3>
            {{
              queue.paused
                ? t("Tudo pronto para continuar.")
                : t("Nenhuma operação em execução.")
            }}
          </h3>
          <p>
            {{
              queue.paused
                ? t("Retome a fila quando quiser.")
                : t("As próximas operações aparecem aqui assim que começarem.")
            }}
          </p>
        </div>
      </section>
      <section class="surface queue-waiting" aria-labelledby="waiting-title">
        <div class="queue-section-heading">
          <h2 id="waiting-title">
            {{ t("Na fila") }}
            <span class="count-badge">{{ number(waiting.length) }}</span>
          </h2>
          <span> {{ t("Prioridade e ordem de chegada") }} </span>
        </div>
        <ol
          v-if="waiting.length"
          class="queue-list"
          :aria-label="t('Operações aguardando')"
        >
          <li v-for="(item, index) in waiting" :key="item.id">
            <span class="queue-position">{{ number(index + 1) }}</span>
            <div class="queue-item-copy">
              <strong>{{ message(item.title) }}</strong>
              <p :title="message(item.detail)">{{ message(item.detail) }}</p>
              <span
                >{{ laneLabel(item) }} ·
                {{ item.automatic ? t("Automática · ") : "" }}
                {{ t("Aguardando há") }} {{ elapsed(item.createdAt) }}</span
              >
            </div>
            <button
              class="btn btn-ghost btn-square btn-sm"
              :aria-label="t('Cancelar {0}', [message(item.title)])"
              @click="$emit('cancel', item.id)"
            >
              <AppIcon name="X" :size="17" />
            </button>
          </li>
        </ol>
        <div v-else class="queue-idle small">
          <AppIcon name="ListOrdered" :size="30" />
          <h3>{{ t("Nenhuma operação esperando.") }}</h3>
          <p>
            {{
              t(
                "Você pode abrir arquivos, baixar e solicitar novas sincronizações mesmo durante uma transferência.",
              )
            }}
          </p>
        </div>
      </section>
    </div>
    <section
      v-if="history.length"
      class="surface queue-history"
      aria-labelledby="queue-history-title"
    >
      <div class="queue-section-heading">
        <h2 id="queue-history-title">{{ t("Finalizadas nesta sessão") }}</h2>
        <button class="btn btn-ghost btn-sm" @click="$emit('clear')">
          {{ t("Limpar histórico") }}
        </button>
      </div>
      <ul class="queue-list">
        <li
          v-for="item in history"
          :key="item.id"
          :class="`queue-${item.status}`"
        >
          <span class="queue-result-icon"
            ><AppIcon
              :name="
                item.status === 'completed'
                  ? 'CheckCheck'
                  : item.status === 'failed'
                    ? 'TriangleAlert'
                    : 'X'
              "
              :size="19"
          /></span>
          <div class="queue-item-copy">
            <strong>{{ message(item.title) }}</strong>
            <p>{{ message(item.error ?? item.detail) }}</p>
          </div>
          <span class="queue-result-label">{{ label(item) }}</span>
        </li>
      </ul>
    </section>
    <p class="queue-session-note">
      {{
        t(
          "A fila pertence a esta sessão do aplicativo. A sincronização automática volta a comparar suas pastas ao reabrir.",
        )
      }}
    </p>
  </section>
</template>
