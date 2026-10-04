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
        <p class="eyebrow">{{ t("THIS COMPUTER") }}</p>
        <h2 id="this-computer-heading">
          {{
            registration?.deviceUid
              ? (current?.name ?? registration.name)
              : t("Connect this PC once")
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
            ? t("Linked to account")
            : missing
              ? t("Registration unavailable")
              : t("Checking connection")
        }}
      </span>
    </div>
    <template v-if="registration?.deviceUid">
      <p v-if="missing" role="status">
        {{
          t(
            "This PC’s registration was not found in Drive. Check the web app before creating another.",
          )
        }}
      </p>
      <p v-else>
        {{
          t(
            "This PC is already connected. Choose the folders to sync; there is no need to register it again.",
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
            t("Add a folder from this PC")
          }}
        </button>
        <button
          v-if="current"
          class="btn btn-outline btn-sm"
          @click="emit('open', current)"
        >
          {{ t("View folders") }}<AppIcon name="ChevronRight" :size="18" />
        </button>
        <button
          v-if="missing"
          class="btn btn-outline btn-sm"
          @click="emit('web')"
        >
          {{ t("Open web version") }}
        </button>
      </div>
    </template>
    <form v-else class="computer-register" @submit.prevent="submit">
      <p>
        {{
          t(
            "Already listed? Link the existing registration. Create a new one only if this PC is not in Drive yet.",
          )
        }}
      </p>
      <template v-if="linuxDevices.length && !pending">
        <label class="field-label" for="computer-existing">{{
          t("Which one is this computer?")
        }}</label>
        <select
          id="computer-existing"
          v-model="choice"
          class="select w-full"
          :disabled="busy || loading"
        >
          <option disabled value="">{{ t("Select a computer") }}</option>
          <option
            v-for="device in linuxDevices"
            :key="device.uid"
            :value="device.uid"
          >
            {{ device.name }}
          </option>
          <option value="new">
            {{ t("This PC is not listed yet") }}
          </option>
        </select>
      </template>
      <template v-if="(!linuxDevices.length || choice === 'new') && !pending">
        <label class="field-label" for="computer-name">{{
          t("Computer name")
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
            "We’ll finish linking {0}, reusing the registration from the previous attempt.",
            [registration!.name],
          )
        }}
      </p>
      <button class="btn btn-primary" :disabled="!canSubmit">
        <span v-if="busy" class="loading loading-spinner loading-xs" />
        <AppIcon v-else name="Monitor" :size="18" />
        {{
          busy
            ? t("Connecting computer…")
            : pending
              ? t("Finish connecting")
              : t("Link this computer")
        }}
      </button>
    </form>
  </section>
</template>
