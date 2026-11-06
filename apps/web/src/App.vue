<script setup lang="ts">
import { ref } from 'vue'
import { theme, toggleTheme } from './theme'
import WaferMap from './components/WaferMap.vue'
import BitMapView from './components/BitMapView.vue'
import DieLayoutEditor from './components/DieLayoutEditor.vue'
import type { DieInfo } from './components/WaferMap.vue'

type View = 'editor' | 'wafer' | 'bitmap'

const view = ref<View>('editor')
const selectedDie = ref<DieInfo | null>(null)
const navCollapsed = ref(false)

const NAV_ITEMS: { key: View; label: string; icon: string }[] = [
  { key: 'editor', label: '布局编辑器', icon: '▦' },
  { key: 'wafer', label: '晶圆图', icon: '◍' },
  { key: 'bitmap', label: 'Bit Map', icon: '▩' },
]

function openBitmap(die: DieInfo | null) {
  selectedDie.value = die ?? { x: 0, y: 0, bin: 0 }
  view.value = 'bitmap'
}
</script>

<template>
  <main class="flex h-screen bg-bg text-fg" :data-theme="theme">
    <!-- 左侧导航:布局编辑器 / 晶圆图 / Bit Map 平级入口(shadcn token),支持折叠 -->
    <aside
      class="flex shrink-0 flex-col border-r border-iborder bg-panel transition-all duration-200"
      :class="navCollapsed ? 'w-14' : 'w-48'"
    >
      <div class="flex h-14 items-center gap-2 border-b border-iborder px-4" :class="navCollapsed ? 'justify-center px-0' : ''">
        <span class="text-lg leading-none text-sky-400">▦</span>
        <span v-if="!navCollapsed" class="text-sm font-semibold">Shot Layout</span>
      </div>
      <nav class="flex flex-col gap-0.5 p-2">
        <button
          v-for="item in NAV_ITEMS"
          :key="item.key"
          class="flex cursor-pointer items-center rounded-md border-0 py-2 text-left text-[13px]"
          :class="[
            view === item.key
              ? 'bg-btn font-semibold text-fg'
              : 'bg-transparent text-dim hover:bg-btn hover:text-fg',
            navCollapsed ? 'justify-center px-0' : 'gap-2.5 px-3',
          ]"
          :title="navCollapsed ? item.label : ''"
          @click="item.key === 'bitmap' ? openBitmap(selectedDie) : (view = item.key)"
        >
          <span class="w-4 text-center text-[15px]" :class="view === item.key ? 'text-sky-400' : ''">
            {{ item.icon }}
          </span>
          <span v-if="!navCollapsed">{{ item.label }}</span>
        </button>
      </nav>
      <div class="mt-auto flex items-center border-t border-iborder p-3" :class="navCollapsed ? 'flex-col gap-2' : 'gap-2'">
        <button
          class="flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center rounded-md border border-iborder bg-input text-sm text-fg hover:bg-btnhover"
          :title="theme === 'dark' ? '切换浅色主题' : '切换深色主题'"
          @click="toggleTheme"
        >
          {{ theme === 'dark' ? '☀' : '☾' }}
        </button>
        <button
          class="flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center rounded-md border border-iborder bg-input text-sm text-fg hover:bg-btnhover"
          :title="navCollapsed ? '展开导航' : '折叠导航'"
          @click="navCollapsed = !navCollapsed"
        >
          {{ navCollapsed ? '»' : '«' }}
        </button>
        <span v-if="!navCollapsed" class="text-xs text-dim">light / dark</span>
      </div>
    </aside>

    <!-- 内容区 -->
    <section class="relative min-w-0 flex-1">
      <DieLayoutEditor v-if="view === 'editor'" />
      <div v-else-if="view === 'wafer'" class="flex h-full items-center justify-center overflow-auto p-4">
        <WaferMap @select-die="openBitmap($event)" />
      </div>
      <BitMapView
        v-else-if="selectedDie"
        :die="selectedDie"
        @close="view = 'wafer'"
      />
    </section>
  </main>
</template>
