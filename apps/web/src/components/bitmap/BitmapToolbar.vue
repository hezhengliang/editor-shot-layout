<script setup lang="ts">
// 画布工具栏:缩小/百分比/放大/自适应(与 DieLayoutEditor 同款)
defineProps<{ zoomPct: number }>()
const emit = defineEmits<{
  (e: 'zoom', factor: number): void
  (e: 'fit'): void
}>()
</script>

<template>
  <div class="canvas-toolbar">
    <button class="tb-btn" title="缩小" aria-label="缩小" @click="emit('zoom', 0.8)">
      <svg viewBox="0 0 16 16" width="14" height="14"><path d="M3 8h10" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" /></svg>
    </button>
    <span class="tb-zoom">{{ zoomPct }}%</span>
    <button class="tb-btn" title="放大" aria-label="放大" @click="emit('zoom', 1.25)">
      <svg viewBox="0 0 16 16" width="14" height="14"><path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" /></svg>
    </button>
    <span class="tb-sep" />
    <button class="tb-btn" title="自适应(居中)" aria-label="自适应(居中)" @click="emit('fit')">
      <svg viewBox="0 0 16 16" width="14" height="14"><path d="M2 6V2h4M10 2h4v4M14 10v4h-4M6 14H2v-4" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </button>
  </div>
</template>

<style scoped>
/* 画布工具栏:与 DieLayoutEditor 同款(缩小/百分比/放大/自适应居中) */
.canvas-toolbar {
  position: absolute;
  right: 16px;
  bottom: 16px;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px 6px;
  background: #0f172a;
  border: 1px solid #334155;
  border-radius: 8px;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.35);
  z-index: 5;
}

.tb-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: #cbd5e1;
  cursor: pointer;
}

.tb-btn:hover {
  background: #334155;
  color: #0f172a;
}

.tb-zoom {
  min-width: 44px;
  text-align: center;
  font-size: 11px;
  color: #94a3b8;
  font-variant-numeric: tabular-nums;
}

.tb-sep {
  width: 1px;
  height: 16px;
  background: #334155;
  margin: 0 3px;
}
</style>
