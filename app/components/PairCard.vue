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
      ? t("Proton Photos · all photos")
      : props.pair.remotePath === "/albums"
        ? t("Proton Photos · all albums")
        : props.pair.remotePath.startsWith("/albums/")
          ? t("Album · {0}", [props.pair.name])
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
      bidirectional: t("Both ways"),
      upload: t("Computer → Drive"),
      download: t("Drive → Computer"),
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
    : t("Waiting for first sync"),
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
              ? t("Syncing")
              : queued
                ? t("Queued")
                : !pair.enabled || paused
                  ? t("Paused")
                  : !connected
                    ? t("Waiting for connection")
                    : t("Automatic")
          }}</span
        >
      </div>
      <details class="dropdown dropdown-end pair-menu">
        <summary
          class="btn btn-ghost btn-square btn-sm"
          :aria-label="t('Options for {0}', [pair.name])"
        >
          <AppIcon name="Ellipsis" />
        </summary>
        <ul
          class="dropdown-content menu bg-base-100 rounded-box shadow-lg border border-base-300 w-52 p-2 z-20"
        >
          <li>
            <button :disabled="changing" @click="$emit('edit')">
              <AppIcon name="Settings2" :size="16" />
              {{ t("Edit sync") }}
            </button>
          </li>
          <li>
            <button @click="$emit('open')">
              <AppIcon name="FolderOpen" :size="16" />
              {{ t("Open local folder") }}
            </button>
          </li>
          <li>
            <button @click="$emit('recovery')">
              <AppIcon name="History" :size="16" />
              {{ t("Open recovery folder") }}
            </button>
          </li>
          <li>
            <button
              :disabled="changing"
              class="text-error"
              @click="$emit('remove')"
            >
              <AppIcon name="Trash2" :size="16" /> {{ t("Remove pairing") }}
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
          t('On PC: on change. In Drive: check every {0} min.', [
            pair.intervalMinutes,
          ])
        "
        ><AppIcon name="RefreshCw" :size="14" /> {{ t("On change") }}
      </span>
    </div>
    <div class="pair-bottom">
      <span>{{ lastRun }}</span>
      <div>
        <button
          class="btn btn-ghost btn-square btn-sm"
          :aria-label="
            pair.enabled
              ? t('Pause {0}', [pair.name])
              : t('Enable {0}', [pair.name])
          "
          :disabled="changing"
          @click="$emit('toggle')"
        >
          <AppIcon :name="pair.enabled ? 'Pause' : 'Play'" :size="16" /></button
        ><button
          class="btn btn-ghost btn-square btn-sm"
          :aria-label="t('Sync {0}', [pair.name])"
          :title="queued ? t('Sync queued') : t('Add sync to queue')"
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
