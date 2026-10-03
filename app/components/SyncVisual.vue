<template>
  <div class="sync-visual" aria-hidden="true">
    <div class="orbit orbit-one" />
    <div class="orbit orbit-two" />
    <div class="visual-cloud">
      <AppIcon name="Cloud" :size="55" :stroke-width="1.3" /><span
        class="visual-check"
        ><AppIcon name="Check" :size="12" :stroke-width="3"
      /></span>
    </div>
    <div class="visual-transfer">
      <AppIcon name="ArrowLeftRight" :size="20" />
    </div>
    <div class="visual-folder">
      <div class="folder-tab" />
      <AppIcon name="ShieldCheck" :size="34" :stroke-width="1.3" />
    </div>
    <span class="visual-dot dot-one" /><span class="visual-dot dot-two" /><span
      class="visual-star"
      >+</span
    >
  </div>
</template>
