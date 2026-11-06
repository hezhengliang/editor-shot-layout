<script setup lang="ts">
// BitMap 侧栏:纯展示 + 交互转发(数据/状态由父层 v-model + props 注入)
import type {
  BinStat,
  DefectItem,
  DieCoord,
  PatternStat,
  ShadingMode,
  TopItem,
  WaferDiesInfo,
} from './types'

const props = defineProps<{
  die: DieCoord
  waferDies: WaferDiesInfo
  binStats: BinStat[]
  patternStats: PatternStat
  wlTop: TopItem[]
  blTop: TopItem[]
  defects: DefectItem[]
  defectTotal: number
  binColors: Record<number, string>
  binLabels: Record<number, string>
  hoveredBit: DefectItem | null
  cursorWorld: { x: number; y: number }
  rows: number
  cols: number
}>()

const shadingMode = defineModel<ShadingMode>('shadingMode', { required: true })
const softContrast = defineModel<boolean>('softContrast', { required: true })
const showWlbl = defineModel<boolean>('showWlbl', { required: true })
const hierVisibility = defineModel<{ Bank: boolean; Array: boolean; Block: boolean }>('hierVisibility', {
  required: true,
})
const tileTemplate = defineModel<string>('tileTemplate', { required: true })
const showTileBounds = defineModel<boolean>('showTileBounds', { required: true })
const binFilter = defineModel<boolean[]>('binFilter', { required: true })
const gotoRow = defineModel<number | null>('gotoRow', { required: true })
const gotoCol = defineModel<number | null>('gotoCol', { required: true })

const emit = defineEmits<{
  (e: 'focus-region', x: number, y: number, size: number): void
  (e: 'focus-wlbl', kind: 'wl' | 'bl', i: number): void
  (e: 'focus-defect', r: number, c: number): void
  (e: 'goto'): void
  (e: 'export'): void
  (e: 'bin-toggle', b: number, event: Event): void
  /** 状态变更后请求父层重绘画布(原 @change="drawRuler") */
  (e: 'redraw'): void
  /** Tile Source 变更/清除:父层重置瓦片缓存并重绘 */
  (e: 'tile-reset'): void
}>()

function clearTiles() {
  tileTemplate.value = ''
  emit('tile-reset')
}
</script>

<template>
  <aside class="w-60 shrink-0 overflow-y-auto border-l border-slate-800 bg-slate-900 p-4 text-slate-200">
    <!-- 晶圆位置指示器:当前 die 在 wafer 上的位置(与 WaferMap 同几何) -->
    <div class="mb-6">
      <h3 class="m-0 mb-3 border-b border-slate-700 pb-1.5 text-[11px] font-semibold tracking-wide text-slate-300 uppercase">Wafer Location</h3>
      <div class="relative mx-auto h-32 w-32">
        <svg viewBox="-410 -410 820 820" class="h-full w-full">
          <circle r="400" fill="#1e293b" stroke="#64748b" stroke-width="8" />
          <g fill="#475569">
            <circle
              v-for="d in waferDies.list"
              :key="d.x + ',' + d.y"
              :cx="d.x * 4.6"
              :cy="d.y * 4.6"
              r="3.4"
            />
          </g>
          <rect
            :x="die.x * 41.4 - 19"
            :y="die.y * 41.4 - 19"
            width="38"
            height="38"
            fill="none"
            stroke="#38bdf8"
            stroke-width="5"
          />
        </svg>
        <span class="absolute bottom-0 left-1/2 h-2 w-4 -translate-x-1/2 bg-slate-900" />
      </div>
      <p class="m-0 mt-1 text-center text-[11px] text-slate-500">Die ({{ die.x }}, {{ die.y }}) · bin={{ die.bin }}</p>
    </div>

    <!-- Bin 过滤:未勾选 bin 调暗(isolate 轻量版) -->
    <div class="mb-6">
      <h3 class="m-0 mb-3 border-b border-slate-700 pb-1.5 text-[11px] font-semibold tracking-wide text-slate-300 uppercase">Bin Filter</h3>
      <div
        class="flex flex-wrap gap-x-3 gap-y-1 text-[12px] text-slate-300"
        :class="shadingMode !== 'bin' ? 'pointer-events-none opacity-40' : ''"
        :title="tileTemplate.trim() ? '瓦片模式:shading 由后端渲染,本地面板不适用' : (shadingMode !== 'bin' ? '该着色模式下 Bin Filter 不适用' : '')"
      >
        <label v-for="b in [0, 1, 2, 3, 4]" :key="b" class="flex cursor-pointer items-center gap-1 select-none">
          <input
            :checked="binFilter[b]"
            type="checkbox"
            class="accent-sky-500"
            @change="emit('bin-toggle', b, $event)"
          />
          <span class="h-2 w-2 rounded-[2px]" :style="{ background: binColors[b] }" />
          {{ binLabels[b] }}
        </label>
      </div>
    </div>

    <!-- Bin 统计表:数量 + 占比 -->
    <div class="mb-6">
      <h3 class="m-0 mb-3 border-b border-slate-700 pb-1.5 text-[11px] font-semibold tracking-wide text-slate-300 uppercase">Bin Summary</h3>
      <div class="flex flex-col gap-1">
        <div v-for="b in binStats" :key="b.bin" class="flex items-center gap-2 text-[12px]">
          <span class="h-2.5 w-2.5 shrink-0 rounded-[2px]" :style="{ background: binColors[b.bin] }" />
          <span class="text-slate-300">{{ binLabels[b.bin] }}</span>
          <span class="ml-auto text-slate-500">{{ b.count.toLocaleString() }}</span>
          <span class="w-12 text-right text-slate-400">{{ b.pct.toFixed(1) }}%</span>
        </div>
      </div>
    </div>

    <!-- 失效模式分类 -->
    <div class="mb-6">
      <h3 class="m-0 mb-3 border-b border-slate-700 pb-1.5 text-[11px] font-semibold tracking-wide text-slate-300 uppercase">Fail Patterns</h3>
      <div class="grid grid-cols-2 gap-1 text-[12px]">
        <div class="rounded bg-slate-800/60 px-2 py-1.5">
          <div class="text-slate-400">Row Fail</div>
          <div class="text-slate-100">{{ patternStats.rowFailRows }} 行</div>
        </div>
        <div class="rounded bg-slate-800/60 px-2 py-1.5">
          <div class="text-slate-400">Col Fail</div>
          <div class="text-slate-100">{{ patternStats.colFailCols }} 列</div>
        </div>
        <div class="rounded bg-slate-800/60 px-2 py-1.5">
          <div class="text-slate-400">Cluster</div>
          <div class="text-slate-100">{{ patternStats.clustered.toLocaleString() }}</div>
        </div>
        <div class="rounded bg-slate-800/60 px-2 py-1.5">
          <div class="text-slate-400">Single Bit</div>
          <div class="text-slate-100">{{ patternStats.isolated.toLocaleString() }}</div>
        </div>
      </div>
    </div>

    <!-- WL / BL 失效 TOP8 直方图(点击定位最差字线) -->
    <div class="mb-6">
      <h3 class="m-0 mb-3 border-b border-slate-700 pb-1.5 text-[11px] font-semibold tracking-wide text-slate-300 uppercase">Worst WL / BL</h3>
      <div class="mb-1 text-[11px] text-slate-400">Wordline(行)</div>
      <div class="mb-2 flex flex-col gap-0.5">
        <button
          v-for="t in wlTop"
          :key="'wl' + t.i"
          class="flex cursor-pointer items-center gap-1 text-[11px] hover:opacity-80"
          title="点击定位该字线"
          @click="emit('focus-wlbl', 'wl', t.i)"
        >
          <span class="w-8 shrink-0 text-right text-slate-400">WL{{ t.i }}</span>
          <span class="h-2.5 rounded-sm bg-sky-500/80" :style="{ width: Math.max(4, (t.n / (wlTop[0]?.n || 1)) * 88) + 'px' }" />
          <span class="text-slate-500">{{ t.n }}</span>
        </button>
      </div>
      <div class="mb-1 text-[11px] text-slate-400">Bitline(列)</div>
      <div class="flex flex-col gap-0.5">
        <button
          v-for="t in blTop"
          :key="'bl' + t.i"
          class="flex cursor-pointer items-center gap-1 text-[11px] hover:opacity-80"
          title="点击定位该位线"
          @click="emit('focus-wlbl', 'bl', t.i)"
        >
          <span class="w-8 shrink-0 text-right text-slate-400">BL{{ t.i }}</span>
          <span class="h-2.5 rounded-sm bg-pink-500/80" :style="{ width: Math.max(4, (t.n / (blTop[0]?.n || 1)) * 88) + 'px' }" />
          <span class="text-slate-500">{{ t.n }}</span>
        </button>
      </div>
      <div class="mt-2 flex items-center gap-1 text-[12px]">
        <input
          v-model.number="gotoRow"
          type="number"
          min="0"
          :max="rows - 1"
          placeholder="WL"
          class="w-full min-w-0 rounded border border-slate-700 bg-slate-800/60 px-1.5 py-1 text-slate-200 outline-none"
        />
        <input
          v-model.number="gotoCol"
          type="number"
          min="0"
          :max="cols - 1"
          placeholder="BL"
          class="w-full min-w-0 rounded border border-slate-700 bg-slate-800/60 px-1.5 py-1 text-slate-200 outline-none"
        />
        <button
          class="shrink-0 cursor-pointer rounded border border-slate-700 bg-slate-800/60 px-2 py-1 text-slate-300 hover:border-sky-500 hover:text-sky-400"
          @click="emit('goto')"
        >
          跳转
        </button>
      </div>
    </div>

    <div class="mb-6">
      <h3 class="m-0 mb-3 border-b border-slate-700 pb-1.5 text-[11px] font-semibold tracking-wide text-slate-300 uppercase">Inspection</h3>
      <div class="mb-2 flex flex-col gap-0.5">
        <span class="text-xs text-slate-400 uppercase">Cursor World</span>
        <span class="text-sm break-all">
          X={{ Math.round(cursorWorld.x) }} Y={{ Math.round(cursorWorld.y) }}
        </span>
      </div>
      <div class="mb-2 flex flex-col gap-0.5">
        <span class="text-xs text-slate-400 uppercase">Hovered Bit</span>
        <span v-if="hoveredBit" class="text-sm break-all">
          row={{ hoveredBit.r }} col={{ hoveredBit.c }} bin={{ hoveredBit.bin }} ({{ binLabels[hoveredBit.bin] }})
        </span>
        <span v-else class="text-sm break-all text-slate-500">--</span>
      </div>
      <div class="mb-2 flex flex-col gap-0.5">
        <span class="text-xs text-slate-400 uppercase">Die</span>
        <span class="text-sm break-all">({{ die.x }}, {{ die.y }}) bin={{ die.bin }}</span>
      </div>
      <div class="mb-2 flex flex-col gap-1.5">
        <span class="text-xs text-slate-400 uppercase">Shading</span>
        <select
          v-model="shadingMode"
          class="w-full cursor-pointer rounded border border-slate-700 bg-slate-800/60 px-1.5 py-1 text-[12px] text-slate-200 outline-none"
          @change="emit('redraw')"
        >
          <option value="bin">Bin 多数色(硬边界)</option>
          <option value="yield">Yield 良率渐变</option>
          <option value="defect">Defect 类型分离(FAIL红 / WARN黄)</option>
        </select>
        <label class="flex cursor-pointer items-center gap-2 text-[12px] text-slate-300 select-none">
          <input v-model="softContrast" type="checkbox" class="accent-sky-500" @change="emit('redraw')" />
          柔和对比(√ 色阶,稀疏缺陷更清晰)
        </label>
      </div>
      <div class="mb-2 flex flex-col gap-1">
        <span class="text-xs text-slate-400 uppercase">Tile Source(后端瓦片)</span>
        <div class="flex items-center gap-1">
          <input
            v-model="tileTemplate"
            type="text"
            placeholder="http://…/tiles/{z}/{x}/{y}.png"
            class="w-full min-w-0 rounded border border-slate-700 bg-slate-800/60 px-1.5 py-1 text-[11px] text-slate-200 outline-none placeholder:text-slate-600"
            @change="emit('tile-reset')"
          />
          <button
            v-if="tileTemplate"
            class="shrink-0 cursor-pointer rounded border border-slate-700 bg-slate-800/60 px-1.5 py-1 text-[11px] text-slate-300 hover:border-sky-500"
            title="清除,回本地渲染"
            @click="clearTiles"
          >
            ✕
          </button>
        </div>
        <label class="mt-1 flex cursor-pointer items-center gap-1.5 text-[11px] text-slate-400 select-none">
          <input v-model="showTileBounds" type="checkbox" class="accent-slate-500" @change="emit('redraw')" />
          显示瓦片边界(调试切片寻址)
        </label>
      </div>
    </div>
    <div class="mb-6">
      <h3 class="m-0 mb-3 border-b border-slate-700 pb-1.5 text-[11px] font-semibold tracking-wide text-slate-300 uppercase">Hierarchy</h3>
      <div class="mb-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[12px] text-slate-300">
        <label class="flex cursor-pointer items-center gap-1 select-none">
          <input v-model="hierVisibility.Bank" type="checkbox" class="accent-sky-500" @change="emit('redraw')" /> Bank
        </label>
        <label class="flex cursor-pointer items-center gap-1 select-none">
          <input v-model="hierVisibility.Array" type="checkbox" class="accent-sky-500" @change="emit('redraw')" /> Array
        </label>
        <label class="flex cursor-pointer items-center gap-1 select-none">
          <input v-model="hierVisibility.Block" type="checkbox" class="accent-sky-500" @change="emit('redraw')" /> Block
        </label>
        <label class="flex cursor-pointer items-center gap-1 select-none">
          <input v-model="showWlbl" type="checkbox" class="accent-pink-500" @change="emit('redraw')" /> WL/BL 十字
        </label>
      </div>
      <div class="mb-2 flex flex-col gap-1 text-[13px]">
        <div class="flex items-center gap-2">
          <span class="h-2 w-2 shrink-0 rounded-[2px]" style="background: #38bdf8" />
          <span class="text-slate-400">Die</span>
          <span class="ml-auto text-slate-500">2048×2048</span>
        </div>
        <div class="flex items-center gap-2">
          <span class="h-2 w-2 shrink-0 rounded-[2px]" style="background: #38bdf8" />
          <span>Bank ×4</span>
          <span class="ml-auto text-slate-500">1024²</span>
        </div>
        <div class="grid grid-cols-4 gap-1">
          <button
            v-for="i in 4"
            :key="'b' + i"
            class="cursor-pointer rounded border border-slate-700 bg-slate-800/60 px-1 py-0.5 text-[11px] text-slate-300 hover:border-sky-500 hover:text-sky-400"
            @click="emit('focus-region', (i - 1) % 2 * 1024, Math.floor((i - 1) / 2) * 1024, 1024)"
          >
            B{{ i - 1 }}
          </button>
        </div>
        <div class="flex items-center gap-2">
          <span class="h-2 w-2 shrink-0 rounded-[2px]" style="background: #60a5fa" />
          <span>Array ×16</span>
          <span class="ml-auto text-slate-500">512²</span>
        </div>
        <div class="grid grid-cols-4 gap-1">
          <button
            v-for="i in 4"
            :key="'a' + i"
            class="cursor-pointer rounded border border-slate-700 bg-slate-800/60 px-1 py-0.5 text-[11px] text-slate-300 hover:border-sky-500 hover:text-sky-400"
            @click="emit('focus-region', (i - 1) % 2 * 512, Math.floor((i - 1) / 2) * 512, 512)"
          >
            A{{ i - 1 }}
          </button>
        </div>
        <div class="flex items-center gap-2">
          <span class="h-2 w-2 shrink-0 rounded-[2px]" style="background: #3b82f6" />
          <span class="text-slate-400">Block ×64</span>
          <span class="ml-auto text-slate-500">256²</span>
        </div>
      </div>
    </div>
    <div class="mb-6">
      <h3 class="m-0 mb-3 border-b border-slate-700 pb-1.5 text-[11px] font-semibold tracking-wide text-slate-300 uppercase">Defects (≈{{ defectTotal.toLocaleString() }} total)</h3>
      <ul class="m-0 max-h-[60vh] list-none overflow-y-auto p-0">
        <li
          v-for="d in defects"
          :key="`${d.r}-${d.c}`"
          class="flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-[13px] hover:bg-slate-800"
          @click="emit('focus-defect', d.r, d.c)"
        >
          <span class="h-2 w-2 shrink-0 rounded-[2px]" :style="{ background: binColors[d.bin] }" />
          row {{ d.r }} col {{ d.c }}
        </li>
      </ul>
    </div>
  </aside>
</template>
