<script setup lang="ts">
import { useI18n } from "~/composables/useI18n";
const { t, message } = useI18n();
import { ref, onMounted, onUnmounted } from "vue";
import {
  getDocument,
  GlobalWorkerOptions,
  type PDFDocumentProxy,
  type PDFDocumentLoadingTask,
  type RenderTask,
} from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
const props = defineProps<{ content: string }>();
GlobalWorkerOptions.workerSrc = workerUrl;
const canvas = ref<HTMLCanvasElement>();
const page = ref(1);
const total = ref(0);
const busy = ref(false);
const error = ref("");
const textContent = ref("");
const showText = ref(false);
let document: PDFDocumentProxy | undefined;
let loadingTask: PDFDocumentLoadingTask | undefined;
let renderTask: RenderTask | undefined;
let closed = false;
onMounted(async () => {
  busy.value = true;
  try {
    const data = Uint8Array.from(atob(props.content), (char) =>
      char.charCodeAt(0),
    );
    // Data-only PDF rendering: no PDF scripts, remote links or external assets.
    loadingTask = getDocument({
      data,
      useSystemFonts: true,
      useWasm: false,
    });
    document = await loadingTask.promise;
    if (closed) {
      await loadingTask.destroy();
      return;
    }
    total.value = document.numPages;
    await render();
  } catch (e) {
    error.value = t("Não foi possível ler este PDF: {0}", [String(e)]);
  } finally {
    busy.value = false;
  }
});
onUnmounted(() => {
  closed = true;
  renderTask?.cancel();
  void loadingTask?.destroy();
});
async function render() {
  if (!document || !canvas.value) return;
  busy.value = true;
  try {
    const pdfPage = await document.getPage(page.value);
    if (closed || !canvas.value) return;
    const base = pdfPage.getViewport({ scale: 1 });
    const scale = Math.min(2, 1500 / Math.max(base.width, base.height));
    const viewport = pdfPage.getViewport({ scale });
    const context = canvas.value.getContext("2d");
    if (!context) throw new Error(t("Canvas indisponível"));
    canvas.value.width = viewport.width;
    canvas.value.height = viewport.height;
    renderTask = pdfPage.render({
      canvas: canvas.value,
      canvasContext: context,
      viewport,
    });
    await renderTask.promise;
    const text = await pdfPage.getTextContent();
    textContent.value = text.items
      .map((item) =>
        "str" in item ? item.str + (item.hasEOL ? "\n" : " ") : "",
      )
      .join("");
  } catch (e) {
    if (!closed) error.value = String(e);
  } finally {
    busy.value = false;
  }
}
async function navigate(delta: number) {
  if (busy.value) return;
  page.value += delta;
  await render();
}
</script>
<template>
  <div class="pdf-viewer" :aria-busy="busy">
    <div class="pdf-tools">
      <button
        class="btn btn-sm btn-ghost"
        :disabled="busy || page <= 1"
        :aria-label="t('Página anterior')"
        @click="navigate(-1)"
      >
        <AppIcon name="ArrowLeft" :size="17" /></button
      ><span>{{ t("Página {0} de {1}", [page, total || "…"]) }}</span
      ><button
        class="btn btn-sm btn-ghost"
        :disabled="busy || page >= total"
        :aria-label="t('Próxima página')"
        @click="navigate(1)"
      >
        <AppIcon name="ArrowRight" :size="17" /></button
      ><span v-if="busy" class="loading loading-spinner loading-xs" /><button
        class="btn btn-xs btn-ghost"
        :aria-pressed="showText"
        @click="showText = !showText"
      >
        {{ t("Texto acessível") }}
      </button>
    </div>
    <p v-if="error" class="form-error" role="alert">{{ message(error) }}</p>
    <div class="pdf-page">
      <canvas
        ref="canvas"
        role="img"
        :aria-label="t('Página {0} do documento PDF', [page])"
      />
    </div>
    <pre v-if="showText" class="pdf-text" aria-live="polite">{{
      textContent || t("Esta página não contém texto selecionável.")
    }}</pre>
  </div>
</template>
