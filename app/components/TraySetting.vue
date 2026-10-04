<script setup lang="ts">
import { useI18n } from "~/composables/useI18n";
const { t } = useI18n();
const props = defineProps<{
  enabled: boolean;
  available: boolean;
  native: boolean;
  saving: boolean;
}>();
const emit = defineEmits<{ change: [enabled: boolean] }>();
function change(event: Event) {
  const input = event.target as HTMLInputElement;
  const enabled = input.checked;
  // Reflect the persisted value, including when saving fails.
  input.checked = props.enabled;
  emit("change", enabled);
}
</script>
<template>
  <div class="setting-row" :aria-busy="saving">
    <div>
      <strong
        ><label for="close-to-tray">{{ t("System tray") }}</label></strong
      >
      <p id="tray-help">
        {{
          t(
            "When you click X, hide the app and keep syncing in the background.",
          )
        }}
      </p>
      <p id="tray-status">
        {{
          native && !available
            ? t("The system tray is unavailable in this session.")
            : t("Use the tray icon to open the app or quit.")
        }}
      </p>
    </div>
    <input
      id="close-to-tray"
      type="checkbox"
      role="switch"
      class="toggle toggle-primary toggle-sm"
      aria-describedby="tray-help tray-status"
      :checked="enabled"
      :disabled="!native || !available || saving"
      @change="change"
    />
  </div>
</template>
