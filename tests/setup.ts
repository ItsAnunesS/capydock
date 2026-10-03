import { ref, computed, onMounted, onUnmounted, nextTick, watch } from "vue";
Object.assign(globalThis, {
  ref,
  computed,
  onMounted,
  onUnmounted,
  nextTick,
  watch,
});
HTMLDialogElement.prototype.showModal = function () {
  this.setAttribute("open", "");
};
HTMLDialogElement.prototype.close = function () {
  this.removeAttribute("open");
  this.dispatchEvent(new Event("close"));
};
