<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { ComputerRegistration, RemoteEntry } from "~/types";
import { useI18n } from "~/composables/useI18n";
const { t } = useI18n();
const props = defineProps<{
  registration?: ComputerRegistration | null;
  devices: RemoteEntry[];
  suggestedName: string;
  busy: boolean;
  loading: boolean;
  failed: boolean;
  native: boolean;
}>();
const emit = defineEmits<{
  register: [name: string, existingUid?: string];
  open: [device: RemoteEntry];
  sync: [device: RemoteEntry];
  web: [];
}>();
const name = ref(props.suggestedName);
const choice = ref("");
const linuxDevices = computed(() =>
  props.devices.filter((d) => d.kind === "device" && d.mediaType === "Linux"),
);
const current = computed(() =>
  props.devices.find((d) => d.uid === props.registration?.deviceUid),
);
const pending = computed(
  () => !!props.registration && !props.registration.deviceUid,
);
const missing = computed(
  () =>
    !!props.registration?.deviceUid &&
    !current.value &&
    !props.loading &&
    !props.failed,
);
watch(
  () => props.suggestedName,
  (next, previous) => {
    if (name.value === previous) name.value = next;
  },
);
const canSubmit = computed(
  () =>
    props.native &&
    !props.busy &&
    !props.loading &&
    !props.failed &&
    (pending.value ||
      (linuxDevices.value.length && choice.value !== "new"
        ? !!choice.value
        : !!name.value.trim())),
);
function submit() {
  if (!canSubmit.value) return;
  const existing = linuxDevices.value.find((d) => d.uid === choice.value);
  emit(
    "register",
    props.registration?.name ?? existing?.name ?? name.value.trim(),
    pending.value ? undefined : existing?.uid,
  );
}
</script>
<template>
  <section
    class="computer-connection"
    aria-labelledby="this-computer-heading"
    :aria-busy="busy || loading"
  >
    <div class="computer-connection-heading">
      <span class="folder-icon"><AppIcon name="Monitor" :size="24" /></span>
      <div>
        <p class="eyebrow">{{ t("ESTE COMPUTADOR") }}</p>
        <h2 id="this-computer-heading">
          {{
            registration?.deviceUid
              ? (current?.name ?? registration.name)
              : t("Conecte este PC uma única vez")
          }}
        </h2>
      </div>
      <span
        v-if="registration?.deviceUid"
        class="computer-status"
        :class="{ unavailable: missing }"
      >
        <AppIcon :name="current ? 'Check' : 'Clock3'" :size="16" />
        {{
          current
            ? t("Vinculado à conta")
            : missing
              ? t("Registro indisponível")
              : t("Verificando vínculo")
        }}
      </span>
    </div>
    <template v-if="registration?.deviceUid">
      <p v-if="missing" role="status">
        {{
          t(
            "O registro deste PC não apareceu no Drive. Confira a versão web antes de criar outro.",
          )
        }}
      </p>
      <p v-else>
        {{
          t(
            "Este PC já está conectado. Escolha as pastas para sincronizar; não é necessário registrá-lo novamente.",
          )
        }}
      </p>
      <div class="button-row">
        <button
          v-if="current"
          class="btn btn-primary btn-sm"
          @click="emit('sync', current)"
        >
          <AppIcon name="FolderPlus" :size="18" />{{
            t("Adicionar pasta deste PC")
          }}
        </button>
        <button
          v-if="current"
          class="btn btn-outline btn-sm"
          @click="emit('open', current)"
        >
          {{ t("Ver pastas") }}<AppIcon name="ChevronRight" :size="18" />
        </button>
        <button
          v-if="missing"
          class="btn btn-outline btn-sm"
          @click="emit('web')"
        >
          {{ t("Abrir versão web") }}
        </button>
      </div>
    </template>
    <form v-else class="computer-register" @submit.prevent="submit">
      <p>
        {{
          t(
            "Já aparece na lista? Vincule o registro existente. Crie um novo apenas se este PC ainda não estiver no Drive.",
          )
        }}
      </p>
      <template v-if="linuxDevices.length && !pending">
        <label class="field-label" for="computer-existing">{{
          t("Qual destes é este computador?")
        }}</label>
        <select
          id="computer-existing"
          v-model="choice"
          class="select w-full"
          :disabled="busy || loading"
        >
          <option disabled value="">{{ t("Selecione um computador") }}</option>
          <option
            v-for="device in linuxDevices"
            :key="device.uid"
            :value="device.uid"
          >
            {{ device.name }}
          </option>
          <option value="new">
            {{ t("Este PC ainda não está na lista") }}
          </option>
        </select>
      </template>
      <template v-if="(!linuxDevices.length || choice === 'new') && !pending">
        <label class="field-label" for="computer-name">{{
          t("Nome do computador")
        }}</label>
        <input
          id="computer-name"
          v-model="name"
          class="input w-full"
          required
          maxlength="120"
          :disabled="busy || loading"
          autocomplete="off"
        />
      </template>
      <p v-if="pending" role="status">
        {{
          t(
            "Vamos concluir o vínculo de {0}, aproveitando o registro da tentativa anterior.",
            [registration!.name],
          )
        }}
      </p>
      <button class="btn btn-primary" :disabled="!canSubmit">
        <span v-if="busy" class="loading loading-spinner loading-xs" />
        <AppIcon v-else name="Monitor" :size="18" />
        {{
          busy
            ? t("Conectando computador…")
            : pending
              ? t("Concluir conexão")
              : t("Vincular este computador")
        }}
      </button>
    </form>
  </section>
</template>
