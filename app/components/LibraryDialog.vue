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
      title: t("Pasta para sua biblioteca Proton"),
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
          :aria-label="t('Fechar')"
          @click="dialog?.close()"
        >
          <AppIcon name="X" />
        </button>
      </div>
      <h2 id="library-setup-title">{{ t("Sua biblioteca, também no PC.") }}</h2>
      <p class="dialog-subtitle">
        {{
          t(
            "Escolha uma pasta para Arquivos, Fotos e Álbuns desta conta. Computadores são configurados separadamente; Docs e Sheets continuam online.",
          )
        }}
      </p>
      <form @submit.prevent="save">
        <label class="field-label" for="library-root">
          {{ t("Onde guardar sua biblioteca") }}
        </label>
        <div class="input-with-button">
          <input
            id="library-root"
            :value="localPath"
            readonly
            class="input w-full"
            :placeholder="t('Escolha uma pasta no computador')"
          /><button
            type="button"
            class="btn btn-soft"
            :disabled="busy"
            @click="choose"
          >
            {{ t("Escolher") }}
          </button>
        </div>
        <div class="sync-plan">
          <div>
            <AppIcon name="Folder" /><span
              ><strong> {{ t("Arquivos") }} </strong
              ><small>
                {{ t("Todas as pastas · computador ↔ Drive") }}
              </small></span
            ><AppIcon name="Check" :size="17" />
          </div>
          <div>
            <AppIcon name="Image" /><span
              ><strong> {{ t("Fotos") }} </strong
              ><small>
                {{ t("Receber a biblioteca e enviar novas fotos") }}
              </small></span
            ><input
              v-model="includePhotos"
              type="checkbox"
              class="checkbox checkbox-primary checkbox-sm"
              :aria-label="t('Incluir fotos e álbuns')"
              :disabled="busy"
            />
          </div>
          <div :class="{ 'opacity-40': !includePhotos }">
            <AppIcon name="Images" /><span
              ><strong> {{ t("Álbuns") }} </strong
              ><small>
                {{ t("Cópia contínua organizada por álbum · Drive → PC") }}
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
            ><strong> {{ t("Propagar exclusões de arquivos") }} </strong
            ><small>
              {{
                t(
                  "Excluir no PC move a outra cópia para a lixeira do Drive. Excluir no Drive move a cópia local para a pasta de recuperação.",
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
                "Docs e Sheets continuam online. Fotos editadas e exclusões de fotos/álbuns são preservadas para revisão. A cópia por álbum também ocupa espaço no PC.",
              )
            }}
          </p>
        </div>
        <p v-if="error" class="form-error" role="alert">{{ message(error) }}</p>
        <div class="modal-action">
          <button type="button" class="btn btn-ghost" @click="dialog?.close()">
            {{ t("Cancelar") }}</button
          ><button class="btn btn-primary" :disabled="!localPath || busy">
            <span
              v-if="busy"
              class="loading loading-spinner loading-xs"
            /><AppIcon v-else name="RefreshCw" :size="17" />
            {{ t("Ativar sincronização") }}
          </button>
        </div>
      </form>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button :aria-label="t('Fechar')">{{ t("Fechar") }}</button>
    </form>
  </dialog>
</template>
