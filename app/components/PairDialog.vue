<script setup lang="ts">
import { useI18n } from "~/composables/useI18n";
const { t, message } = useI18n();
import { open } from "@tauri-apps/plugin-dialog";
import type { SyncPair, Mode, RemoteEntry } from "~/types";
const props = defineProps<{
  pair?: SyncPair;
  preset?: Partial<SyncPair>;
  command: <T = void>(
    name: string,
    args?: Record<string, unknown>,
  ) => Promise<T>;
}>();
const emit = defineEmits<{ close: []; saved: [] }>();
const dialog = ref<HTMLDialogElement>();
const name = ref(props.pair?.name ?? props.preset?.name ?? "");
const localPath = ref(props.pair?.localPath ?? "");
const remotePath = ref(
  props.pair?.remotePath ?? props.preset?.remotePath ?? "/my-files",
);
const mode = ref<Mode>(
  props.pair?.mode ?? props.preset?.mode ?? "bidirectional",
);
const propagateDeletions = ref(props.pair?.propagateDeletions ?? false);
const computerRoot = computed(() =>
  !props.pair &&
  remotePath.value.startsWith("/devices/") &&
  remotePath.value.split("/").length === 3
    ? remotePath.value
    : undefined,
);
const isPhotos = computed(
  () =>
    remotePath.value === "/photos" || remotePath.value.startsWith("/albums"),
);
const intervalMinutes = ref(props.pair?.intervalMinutes ?? 5);
const error = ref("");
const busy = ref(false);
const browsing = ref(false);
const folders = ref<RemoteEntry[]>([]);
const browsePath = ref(remotePath.value);
const newFolder = ref("");
onMounted(() => dialog.value?.showModal());
async function chooseLocal() {
  try {
    const path = await open({
      directory: true,
      multiple: false,
      title: t("Choose the folder to sync"),
    });
    if (typeof path === "string") {
      localPath.value = path;
      if (!name.value)
        name.value = path.split("/").filter(Boolean).at(-1) ?? t("My folder");
    }
  } catch (e) {
    error.value = String(e);
  }
}
async function browse(path: string) {
  busy.value = true;
  error.value = "";
  try {
    folders.value = (
      await props.command<RemoteEntry[]>("list_remote", { path })
    ).filter((item) => item.directory);
    browsePath.value = path;
    browsing.value = true;
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
async function createFolder() {
  if (!newFolder.value.trim()) return;
  busy.value = true;
  error.value = "";
  try {
    await props.command("create_remote_folder", {
      parent: browsePath.value,
      name: newFolder.value.trim(),
    });
    newFolder.value = "";
    await browse(browsePath.value);
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
async function save() {
  busy.value = true;
  error.value = "";
  try {
    await props.command("save_pair", {
      ...(computerRoot.value ? { computerRoot: computerRoot.value } : {}),
      pair: {
        id: props.pair?.id ?? "",
        name: name.value.trim(),
        localPath: localPath.value,
        remotePath: remotePath.value.replace(/\/$/, ""),
        mode: mode.value,
        intervalMinutes: Number(intervalMinutes.value),
        enabled: props.pair?.enabled ?? true,
        lastRun: props.pair?.lastRun ?? null,
        deviceUid: props.pair?.deviceUid ?? null,
        propagateDeletions: !isPhotos.value && propagateDeletions.value,
      },
    });
    emit("saved");
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <dialog
    ref="dialog"
    class="modal"
    aria-labelledby="pair-dialog-title"
    @close="emit('close')"
  >
    <div class="modal-box pair-dialog">
      <div class="dialog-heading">
        <div class="folder-icon"><AppIcon name="FolderPlus" :size="24" /></div>
        <button
          class="btn btn-ghost btn-square btn-sm"
          :aria-label="t('Close')"
          @click="dialog?.close()"
        >
          <AppIcon name="X" />
        </button>
      </div>
      <h2 id="pair-dialog-title">
        {{ pair ? t("Edit sync") : t("A place for every file.") }}
      </h2>
      <p class="dialog-subtitle">
        {{ t("Connect a folder on your computer to your Proton Drive.") }}
      </p>
      <form @submit.prevent="save">
        <label class="field-label" for="pair-name"> {{ t("Sync name") }} </label
        ><input
          id="pair-name"
          v-model="name"
          class="input w-full"
          maxlength="120"
          :placeholder="t('E.g. Work documents')"
          required
          autofocus
        />
        <label class="field-label" for="local-path">
          {{ t("On your computer") }}
        </label>
        <div class="input-with-button">
          <input
            id="local-path"
            v-model="localPath"
            class="input w-full"
            :placeholder="t('/home/you/Documents')"
            :readonly="!!pair"
            required
          /><button
            type="button"
            class="btn btn-soft"
            :disabled="!!pair"
            @click="chooseLocal"
          >
            <AppIcon name="FolderOpen" :size="17" /> {{ t("Choose") }}
          </button>
        </div>
        <label class="field-label" for="remote-path">
          {{ t("In Proton Drive") }}
        </label>
        <div
          v-if="!pair && !isPhotos"
          class="destination-tabs"
          role="group"
          :aria-label="t('Destination in Proton Drive')"
        >
          <button
            type="button"
            class="btn btn-sm"
            :class="
              remotePath.startsWith('/my-files') ? 'btn-soft' : 'btn-ghost'
            "
            :disabled="busy"
            @click="
              remotePath = '/my-files';
              browse('/my-files');
            "
          >
            <AppIcon name="Cloud" :size="16" /> {{ t("My files") }}
          </button>
          <button
            type="button"
            class="btn btn-sm"
            :class="
              remotePath.startsWith('/devices') ? 'btn-soft' : 'btn-ghost'
            "
            :disabled="busy"
            @click="browse('/devices')"
          >
            <AppIcon name="Monitor" :size="16" /> {{ t("Computers") }}
          </button>
        </div>
        <div class="input-with-button">
          <input
            id="remote-path"
            v-model="remotePath"
            class="input w-full"
            :readonly="!!pair"
            required
          /><button
            type="button"
            class="btn btn-soft"
            :disabled="!!pair || busy || isPhotos"
            @click="browse(remotePath)"
          >
            <AppIcon name="Cloud" :size="17" /> {{ t("Browse") }}
          </button>
        </div>
        <div v-if="browsing" class="folder-browser">
          <div class="browser-toolbar">
            <button
              type="button"
              class="btn btn-ghost btn-square btn-xs"
              :disabled="['/my-files', '/devices'].includes(browsePath) || busy"
              :aria-label="t('Parent folder')"
              @click="browse(browsePath.slice(0, browsePath.lastIndexOf('/')))"
            >
              <AppIcon name="ArrowLeft" :size="16" /></button
            ><span>{{ browsePath }}</span
            ><button
              type="button"
              class="btn btn-xs btn-primary"
              :disabled="browsePath === '/devices' || busy"
              @click="
                remotePath = browsePath;
                browsing = false;
              "
            >
              {{ t("Use this folder") }}
            </button>
          </div>
          <div class="folder-list">
            <button
              v-for="folder in folders"
              :key="folder.path"
              type="button"
              :disabled="busy"
              @click="browse(folder.path)"
            >
              <AppIcon name="Folder" :size="18" />{{ folder.name
              }}<AppIcon name="ChevronRight" :size="15" />
            </button>
            <p v-if="!folders.length">
              {{
                browsePath === "/devices"
                  ? t(
                      "Register this PC in My Drive → Computers to get started.",
                    )
                  : t("No subfolders here.")
              }}
            </p>
          </div>
          <div v-if="browsePath !== '/devices'" class="new-folder">
            <input
              v-model="newFolder"
              class="input input-sm"
              :placeholder="t('New folder name')"
              :aria-label="t('New folder name in Drive')"
            /><button
              type="button"
              class="btn btn-sm btn-ghost"
              :disabled="busy || !newFolder.trim()"
              @click="createFolder"
            >
              <AppIcon name="Plus" :size="16" /> {{ t("Create") }}
            </button>
          </div>
        </div>
        <p v-if="computerRoot" class="dialog-subtitle">
          {{
            t("The folder {0} will be created inside {1} in Computers.", [
              name || t("sync name"),
              computerRoot.split("/").at(-1),
            ])
          }}
        </p>
        <label class="field-label" for="sync-mode">
          {{ t("How to sync") }} </label
        ><select
          id="sync-mode"
          v-model="mode"
          class="select w-full"
          :disabled="remotePath === '/albums'"
        >
          <option value="bidirectional">
            {{ t("Both ways · computer ↔ Drive") }}
          </option>
          <option value="upload">{{ t("Upload · computer → Drive") }}</option>
          <option value="download">
            {{ t("Download · Drive → computer") }}
          </option>
        </select>
        <div class="safety-note">
          <AppIcon name="RefreshCw" :size="19" />
          <p>
            {{
              t(
                "On your computer, creating or editing files starts syncing automatically after about 2 seconds without new changes.",
              )
            }}
          </p>
        </div>
        <label class="field-label" for="sync-interval">
          {{ t("Check for changes in Drive every") }} </label
        ><select
          id="sync-interval"
          v-model="intervalMinutes"
          class="select w-full"
        >
          <option :value="1">{{ t("1 minute") }}</option>
          <option :value="5">{{ t("5 minutes") }}</option>
          <option :value="15">{{ t("15 minutes") }}</option>
          <option :value="30">{{ t("30 minutes") }}</option>
          <option :value="60">{{ t("1 hour") }}</option>
          <option :value="1440">{{ t("24 hours") }}</option>
        </select>
        <label v-if="!isPhotos" class="deletion-option"
          ><input
            v-model="propagateDeletions"
            type="checkbox"
            class="checkbox checkbox-sm checkbox-primary"
          /><span
            ><strong> {{ t("Propagate deletions") }} </strong
            ><small>
              {{
                t(
                  "Uses Drive trash and the local recovery folder. Only removes copies that haven't changed since the last sync.",
                )
              }}
            </small></span
          ></label
        >
        <div class="safety-note">
          <AppIcon name="ShieldCheck" :size="19" />
          <p>
            {{
              isPhotos
                ? t(
                    "New photos can be uploaded; edits and deletions of originals are preserved for review. All albums use a continuous copy to your PC.",
                  )
                : t(
                    "Conflicts preserve both copies. Docs and Sheets remain available in the online editor; other files are synced.",
                  )
            }}
          </p>
        </div>
        <p v-if="error" class="form-error" role="alert">{{ message(error) }}</p>
        <div class="modal-action">
          <button type="button" class="btn btn-ghost" @click="dialog?.close()">
            {{ t("Cancel") }}</button
          ><button
            class="btn btn-primary"
            :disabled="busy || !name.trim() || !localPath"
          >
            <span
              v-if="busy"
              class="loading loading-spinner loading-xs"
            /><AppIcon v-else name="Check" :size="17" />{{
              pair ? t("Save changes") : t("Connect folder")
            }}
          </button>
        </div>
      </form>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button :aria-label="t('Close window')">{{ t("Close") }}</button>
    </form>
  </dialog>
</template>
