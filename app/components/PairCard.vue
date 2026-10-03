<script setup lang="ts">
import { useI18n } from "~/composables/useI18n";
const { t, locale } = useI18n();
import type { SyncPair } from "~/types";
const props = defineProps<{
  pair: SyncPair;
  queued?: boolean;
  changing?: boolean;
  syncing: boolean;
  paused: boolean;
  connected: boolean;
}>();
defineEmits<{
  sync: [];
  toggle: [];
  edit: [];
  remove: [];
  open: [];
  recovery: [];
}>();
const sourceLabel = computed(() =>
  props.pair.remotePath.startsWith("/devices/")
    ? props.pair.remotePath.replace("/devices/", t("Computers · "))
    : props.pair.remotePath === "/photos"
      ? t("Proton Photos · todas as fotos")
      : props.pair.remotePath === "/albums"
        ? t("Proton Photos · todos os álbuns")
        : props.pair.remotePath.startsWith("/albums/")
          ? t("Álbum · {0}", [props.pair.name])
          : props.pair.remotePath,
);
const sourceIcon = computed(() =>
  props.pair.remotePath.startsWith("/devices/")
    ? "Monitor"
    : props.pair.remotePath === "/photos"
      ? "Image"
      : props.pair.remotePath.startsWith("/albums")
        ? "Images"
        : "Folder",
);
const mode = computed(
  () =>
    ({
      bidirectional: t("Nos dois sentidos"),
      upload: t("Computador → Drive"),
      download: t("Drive → Computador"),
    })[props.pair.mode],
);
const lastRun = computed(() =>
  props.pair.lastRun
    ? new Date(props.pair.lastRun * 1000).toLocaleString(locale.value, {
        day: "2-digit",
        month: "short",
        hour: "2-digit",
        minute: "2-digit",
      })
    : t("Aguardando primeira sincronização"),
);
</script>
<template>
  <article
    class="pair-card"
    :class="{ 'pair-paused': !pair.enabled || paused }"
  >
    <div class="pair-heading">
      <div class="folder-icon"><AppIcon :name="sourceIcon" :size="25" /></div>
      <div class="pair-name">
        <h3>{{ pair.name }}</h3>
        <span class="pair-state" :class="{ working: syncing }"
          ><span class="status-dot" />{{
            syncing
              ? t("Sincronizando")
              : queued
                ? t("Na fila")
                : !pair.enabled || paused
                  ? t("Pausada")
                  : !connected
                    ? t("Aguardando conexão")
                    : t("Automática")
          }}</span
        >
      </div>
      <details class="dropdown dropdown-end pair-menu">
        <summary
          class="btn btn-ghost btn-square btn-sm"
          :aria-label="t('Opções de {0}', [pair.name])"
        >
          <AppIcon name="Ellipsis" />
        </summary>
        <ul
          class="dropdown-content menu bg-base-100 rounded-box shadow-lg border border-base-300 w-52 p-2 z-20"
        >
          <li>
            <button :disabled="changing" @click="$emit('edit')">
              <AppIcon name="Settings2" :size="16" />
              {{ t("Editar sincronização") }}
            </button>
          </li>
          <li>
            <button @click="$emit('open')">
              <AppIcon name="FolderOpen" :size="16" />
              {{ t("Abrir pasta local") }}
            </button>
          </li>
          <li>
            <button @click="$emit('recovery')">
              <AppIcon name="History" :size="16" /> {{ t("Abrir recuperação") }}
            </button>
          </li>
          <li>
            <button
              :disabled="changing"
              class="text-error"
              @click="$emit('remove')"
            >
              <AppIcon name="Trash2" :size="16" /> {{ t("Remover pareamento") }}
            </button>
          </li>
        </ul>
      </details>
    </div>
    <div class="pair-paths">
      <p :title="pair.localPath">
        <AppIcon name="Monitor" :size="16" /><span>{{ pair.localPath }}</span>
      </p>
      <div class="path-connector" />
      <p :title="pair.remotePath">
        <AppIcon name="Cloud" :size="16" /><span>{{ sourceLabel }}</span>
      </p>
    </div>
    <div class="pair-meta">
      <span><AppIcon name="ArrowLeftRight" :size="14" />{{ mode }}</span
      ><span
        :title="
          t('No PC: ao alterar. No Drive: verificação a cada {0} min.', [
            pair.intervalMinutes,
          ])
        "
        ><AppIcon name="RefreshCw" :size="14" /> {{ t("Ao alterar") }}
      </span>
    </div>
    <div class="pair-bottom">
      <span>{{ lastRun }}</span>
      <div>
        <button
          class="btn btn-ghost btn-square btn-sm"
          :aria-label="
            pair.enabled
              ? t('Pausar {0}', [pair.name])
              : t('Ativar {0}', [pair.name])
          "
          :disabled="changing"
          @click="$emit('toggle')"
        >
          <AppIcon :name="pair.enabled ? 'Pause' : 'Play'" :size="16" /></button
        ><button
          class="btn btn-ghost btn-square btn-sm"
          :aria-label="t('Sincronizar {0}', [pair.name])"
          :title="
            queued
              ? t('Sincronização na fila')
              : t('Adicionar sincronização à fila')
          "
          :disabled="queued || !pair.enabled || !connected"
          @click="$emit('sync')"
        >
          <AppIcon
            name="RefreshCw"
            :size="16"
            :class="{ 'spin-slow': syncing }"
          />
        </button>
      </div>
    </div>
  </article>
</template>
