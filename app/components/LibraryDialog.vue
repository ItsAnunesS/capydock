<script setup lang="ts">
import { useI18n } from "~/composables/useI18n";
const { t, message } = useI18n();
import { open } from "@tauri-apps/plugin-dialog";
const props = defineProps<{
  command: <T = void>(
    name: string,
    args?: Record<string, unknown>,
  ) => Promise<T>;
}>();
const emit = defineEmits<{ close: []; saved: [] }>();
const dialog = ref<HTMLDialogElement>();
const localPath = ref("");
const includePhotos = ref(true);
const propagateDeletions = ref(false);
const busy = ref(false);
const error = ref("");
onMounted(() => dialog.value?.showModal());
async function choose() {
  try {
    const path = await open({
      directory: true,
      multiple: false,
      title: t("Folder for your Proton library"),
    });
    if (typeof path === "string") localPath.value = path;
  } catch (e) {
    error.value = String(e);
  }
}
async function save() {
  busy.value = true;
  error.value = "";
  try {
    await props.command("setup_library", {
      localPath: localPath.value,
      includePhotos: includePhotos.value,
      propagateDeletions: propagateDeletions.value,
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
    aria-labelledby="library-setup-title"
    @close="emit('close')"
  >
    <div class="modal-box pair-dialog">
      <div class="dialog-heading">
        <div class="folder-icon"><AppIcon name="HardDrive" :size="25" /></div>
        <button
          class="btn btn-ghost btn-square btn-sm"
          :aria-label="t('Close')"
          @click="dialog?.close()"
        >
          <AppIcon name="X" />
        </button>
      </div>
      <h2 id="library-setup-title">{{ t("Your library, on your PC too.") }}</h2>
      <p class="dialog-subtitle">
        {{
          t(
            "Choose a folder for this account’s Files, Photos and Albums. Computers are configured separately; Docs and Sheets remain online.",
          )
        }}
      </p>
      <form @submit.prevent="save">
        <label class="field-label" for="library-root">
          {{ t("Where to keep your library") }}
        </label>
        <div class="input-with-button">
          <input
            id="library-root"
            :value="localPath"
            readonly
            class="input w-full"
            :placeholder="t('Choose a folder on your computer')"
          /><button
            type="button"
            class="btn btn-soft"
            :disabled="busy"
            @click="choose"
          >
            {{ t("Choose") }}
          </button>
        </div>
        <div class="sync-plan">
          <div>
            <AppIcon name="Folder" /><span
              ><strong> {{ t("Files") }} </strong
              ><small>
                {{ t("All folders · computer ↔ Drive") }}
              </small></span
            ><AppIcon name="Check" :size="17" />
          </div>
          <div>
            <AppIcon name="Image" /><span
              ><strong> {{ t("Photos") }} </strong
              ><small>
                {{ t("Download the library and upload new photos") }}
              </small></span
            ><input
              v-model="includePhotos"
              type="checkbox"
              class="checkbox checkbox-primary checkbox-sm"
              :aria-label="t('Include photos and albums')"
              :disabled="busy"
            />
          </div>
          <div :class="{ 'opacity-40': !includePhotos }">
            <AppIcon name="Images" /><span
              ><strong> {{ t("Albums") }} </strong
              ><small>
                {{ t("Continuous copy organized by album · Drive → PC") }}
              </small></span
            ><AppIcon name="CloudDownload" :size="17" />
          </div>
        </div>
        <label class="deletion-option"
          ><input
            v-model="propagateDeletions"
            class="checkbox checkbox-sm checkbox-primary"
            type="checkbox"
            :disabled="busy"
          /><span
            ><strong> {{ t("Propagate file deletions") }} </strong
            ><small>
              {{
                t(
                  "Deleting on your PC moves the other copy to Drive trash. Deleting in Drive moves the local copy to the recovery folder.",
                )
              }}
            </small></span
          ></label
        >
        <div class="safety-note">
          <AppIcon name="CircleHelp" :size="19" />
          <p>
            {{
              t(
                "Docs and Sheets remain online. Edited photos and photo/album deletions are preserved for review. Album copies also take up space on your PC.",
              )
            }}
          </p>
        </div>
        <p v-if="error" class="form-error" role="alert">{{ message(error) }}</p>
        <div class="modal-action">
          <button type="button" class="btn btn-ghost" @click="dialog?.close()">
            {{ t("Cancel") }}</button
          ><button class="btn btn-primary" :disabled="!localPath || busy">
            <span
              v-if="busy"
              class="loading loading-spinner loading-xs"
            /><AppIcon v-else name="RefreshCw" :size="17" />
            {{ t("Enable sync") }}
          </button>
        </div>
      </form>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button :aria-label="t('Close')">{{ t("Close") }}</button>
    </form>
  </dialog>
</template>
