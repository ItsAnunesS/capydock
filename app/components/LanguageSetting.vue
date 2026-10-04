<script setup lang="ts">
import { useI18n, type Locale } from "~/composables/useI18n";
const { t, language, languages } = useI18n();
defineProps<{ saving?: boolean }>();
defineEmits<{ change: [locale: Locale] }>();
</script>
<template>
  <section
    class="surface settings-card language-card"
    aria-labelledby="language-heading"
  >
    <div class="settings-heading">
      <AppIcon name="Languages" :size="21" />
      <h2 id="language-heading">{{ t("Language") }}</h2>
    </div>
    <div class="setting-row">
      <div>
        <label class="font-semibold" for="app-language">{{
          t("App language")
        }}</label>
        <p id="language-help">
          {{ t("Changes apply immediately and are saved on this computer.") }}
        </p>
      </div>
      <select
        id="app-language"
        class="select language-select"
        :value="language"
        :disabled="saving"
        aria-describedby="language-help"
        @change="
          $emit('change', ($event.target as HTMLSelectElement).value as Locale)
        "
      >
        <option
          v-for="item in languages"
          :key="item.value"
          :value="item.value"
          :lang="item.tag"
        >
          {{ item.label }}
        </option>
      </select>
    </div>
  </section>
</template>
