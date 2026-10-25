<script setup lang="ts">
import { ref } from 'vue'
import { theme } from './theme'
import WaferMap from './components/WaferMap.vue'
import BitMapView from './components/BitMapView.vue'
import DieLayoutEditor from './components/DieLayoutEditor.vue'
import type { DieInfo } from './components/WaferMap.vue'

const selectedDie = ref<DieInfo | null>(null)
const view = ref<'editor' | 'wafer'>('editor')
</script>

<template>
  <main
    class="app flex h-screen flex-col bg-[#eef1f6] text-slate-900 dark:bg-[#020617] dark:text-slate-200"
    :data-theme="theme"
  >
    <header
      v-if="!selectedDie"
      class="flex shrink-0 items-center gap-4 border-b border-slate-200 px-4 py-2.5 dark:border-slate-800"
    >
      <h1 class="m-0 font-[Arial] text-lg">Wafer Map Demo</h1>
      <nav class="ml-auto flex gap-2">
        <button
          class="cursor-pointer rounded-md border bg-white px-4 py-[7px] text-sm text-slate-500 hover:bg-slate-200 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400 dark:hover:bg-slate-700"
          :class="view === 'editor' ? 'border-blue-600! bg-blue-600! text-white!' : 'border-slate-200'"
          @click="view = 'editor'"
        >
          Leafer Editor 大 Die 布局
        </button>
        <button
          class="cursor-pointer rounded-md border bg-white px-4 py-[7px] text-sm text-slate-500 hover:bg-slate-200 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400 dark:hover:bg-slate-700"
          :class="view === 'wafer' ? 'border-blue-600! bg-blue-600! text-white!' : 'border-slate-200'"
          @click="view = 'wafer'"
        >
          晶圆图 Wafer Map
        </button>
      </nav>
    </header>
    <div class="relative min-h-0 flex-1">
      <template v-if="!selectedDie">
        <DieLayoutEditor v-if="view === 'editor'" />
        <div v-else class="flex h-full items-center justify-center overflow-auto p-4">
          <WaferMap @select-die="selectedDie = $event" />
        </div>
      </template>
      <BitMapView
        v-else
        :die="selectedDie"
        @close="selectedDie = null"
      />
    </div>
  </main>
</template>
