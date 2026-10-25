<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import {
  App,
  Group,
  Rect,
  Line,
  Text,
  Canvas as LeaferCanvas,
  MoveEvent,
  ZoomEvent,
} from 'leafer-editor'
import type { DieInfo } from './WaferMap.vue'

interface Props {
  die: DieInfo
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'close'): void
}>()

const containerRef = ref<HTMLDivElement | null>(null)
const loading = ref(true)
let app: App | null = null
let content: Group | null = null
let rulerGroup: Group | null = null
let hoverRect: Rect | null = null
let resizeObserver: ResizeObserver | null = null

const bitPitch = 1
const rows = 2048
const cols = 2048
const gridWidth = cols * bitPitch
const gridHeight = rows * bitPitch
const sheetPad = 1024

const binColors: Record<number, string> = {
  0: '#22c55e', // pass - green
  1: '#ef4444', // fail - red (defect)
  2: '#eab308', // warn - yellow
  3: '#3b82f6', // skip - blue
  4: '#1e293b', // null - dark slate
}

const binLabels: Record<number, string> = {
  0: 'PASS',
  1: 'FAIL',
  2: 'WARN',
  3: 'SKIP',
  4: 'NULL',
}

const hoveredBit = ref<{ r: number; c: number; bin: number } | null>(null)
const cursorWorld = ref<{ x: number; y: number }>({ x: 0, y: 0 })
let lastMouseX = -1
let lastMouseY = -1

function generateBitData(dieBin: number): Uint8Array {
  const total = rows * cols
  const data = new Uint8Array(total)
  const weights = [0.02, 0.02, 0.02, 0.02, 0.02]
  weights[dieBin] = 0.9
  const wTotal = weights.reduce((a, b) => a + b, 0)

  for (let i = 0; i < total; i++) {
    let r = Math.random() * wTotal
    let bin = 4
    for (let j = 0; j < weights.length; j++) {
      r -= weights[j]
      if (r <= 0) {
        bin = j
        break
      }
    }
    data[i] = bin
  }
  return data
}

const bitData = generateBitData(props.die.bin)

const defects = computed(() => {
  const list: { r: number; c: number; bin: number }[] = []
  const target = 300
  let attempts = 0
  while (list.length < target && attempts < target * 20) {
    attempts++
    const idx = Math.floor(Math.random() * bitData.length)
    const bin = bitData[idx]
    if (bin === 1 || bin === 2) {
      const r = Math.floor(idx / cols)
      const c = idx % cols
      list.push({ r, c, bin })
    }
  }
  return list
})

const defectTotal = computed(() => {
  let count = 0
  const sampleSize = 1000000
  for (let i = 0; i < sampleSize; i++) {
    const idx = Math.floor(Math.random() * bitData.length)
    const bin = bitData[idx]
    if (bin === 1 || bin === 2) count++
  }
  return Math.round((count / sampleSize) * bitData.length)
})

// 2048×2048 bit → 离屏 canvas 像素,再一次性贴到 leafer Canvas 元素(平移缩放只重绘纹理,不重建)
function paintBitMap(canvas: LeaferCanvas) {
  const off = document.createElement('canvas')
  off.width = cols
  off.height = rows
  const ctx = off.getContext('2d')!
  const imageData = ctx.createImageData(cols, rows)
  const pixels = imageData.data
  const rgb = [0, 0, 0]
  for (let i = 0; i < bitData.length; i++) {
    const hex = binColors[bitData[i]] ?? '#94a3b8'
    rgb[0] = parseInt(hex.slice(1, 3), 16)
    rgb[1] = parseInt(hex.slice(3, 5), 16)
    rgb[2] = parseInt(hex.slice(5, 7), 16)
    pixels[i * 4] = rgb[0]
    pixels[i * 4 + 1] = rgb[1]
    pixels[i * 4 + 2] = rgb[2]
    pixels[i * 4 + 3] = 255
  }
  ctx.putImageData(imageData, 0, 0)
  canvas.context.drawImage(off, 0, 0)
  canvas.paint()
}

function fitInitial() {
  if (!app) return
  const w = app.width ?? 0
  const h = app.height ?? 0
  const s = Math.max(0.05, Math.min((w - 80) / gridWidth, (h - 80) / gridHeight, 1))
  const layer = app.tree.zoomLayer
  layer.scale = s
  layer.x = w / 2 - (gridWidth * s) / 2 + 12
  layer.y = h / 2 - (gridHeight * s) / 2 + 12
  drawRuler()
}

function focusDefect(r: number, c: number) {
  if (!app) return
  const s = 20
  const wx = c + 0.5
  const wy = r + 0.5
  const layer = app.tree.zoomLayer
  layer.scale = s
  layer.x = (app.width ?? 0) / 2 - wx * s
  layer.y = (app.height ?? 0) / 2 - wy * s
  drawRuler()
}

// ---------- 标尺(屏幕坐标,固定在 sky 层,不随内容缩放) ----------
const RULER = 24

function drawRuler() {
  if (!app) return
  rulerGroup?.destroy()
  rulerGroup = new Group({ hittable: false })
  app.sky.add(rulerGroup)
  const g = rulerGroup
  const w = app.width ?? 0
  const h = app.height ?? 0
  const layer = app.tree.zoomLayer
  const s = typeof layer.scale === 'number' ? layer.scale : 1
  const ox = typeof layer.x === 'number' ? layer.x : 0
  const oy = typeof layer.y === 'number' ? layer.y : 0

  g.add(new Rect({ x: 0, y: 0, width: w, height: RULER, fill: '#0f172a' }))
  g.add(new Rect({ x: 0, y: 0, width: RULER, height: h, fill: '#0f172a' }))
  g.add(new Line({ x: 0, y: RULER, points: [0, 0, w, 0], stroke: '#334155', strokeWidth: 1 }))
  g.add(new Line({ x: RULER, y: 0, points: [0, 0, 0, h], stroke: '#334155', strokeWidth: 1 }))

  let majorStep: number
  let minorStep: number
  if (s > 20) {
    majorStep = 10
    minorStep = 1
  } else if (s > 5) {
    majorStep = 50
    minorStep = 10
  } else if (s > 1) {
    majorStep = 100
    minorStep = 25
  } else if (s > 0.2) {
    majorStep = 500
    minorStep = 100
  } else {
    majorStep = 1000
    minorStep = 250
  }

  const minCol = Math.floor(-sheetPad / bitPitch)
  const maxCol = Math.ceil((gridWidth + sheetPad) / bitPitch)
  const minRow = Math.floor(-sheetPad / bitPitch)
  const maxRow = Math.ceil((gridHeight + sheetPad) / bitPitch)
  const startCol = Math.max(minCol, Math.floor((RULER - ox) / s / bitPitch))
  const endCol = Math.min(maxCol, Math.ceil((w - ox) / s / bitPitch))
  const startRow = Math.max(minRow, Math.floor((RULER - oy) / s / bitPitch))
  const endRow = Math.min(maxRow, Math.ceil((h - oy) / s / bitPitch))

  for (let c = startCol - (startCol % minorStep); c <= endCol; c += minorStep) {
    const sx = ox + c * bitPitch * s
    if (sx < RULER) continue
    const isMajor = c % majorStep === 0
    const len = isMajor ? 10 : 5
    g.add(new Line({ x: sx, y: RULER, points: [0, 0, 0, -len], stroke: isMajor ? '#cbd5e1' : '#475569', strokeWidth: 1 }))
    if (isMajor) {
      g.add(
        new Text({
          x: sx - 40,
          y: RULER - len - 13,
          width: 80,
          textAlign: 'center',
          text: String(c),
          fill: '#cbd5e1',
          fontSize: 9,
        }),
      )
    }
  }
  for (let r = startRow - (startRow % minorStep); r <= endRow; r += minorStep) {
    const sy = oy + r * bitPitch * s
    if (sy < RULER) continue
    const isMajor = r % majorStep === 0
    const len = isMajor ? 10 : 5
    g.add(new Line({ x: RULER, y: sy, points: [0, 0, -len, 0], stroke: isMajor ? '#cbd5e1' : '#475569', strokeWidth: 1 }))
    if (isMajor) {
      g.add(
        new Text({
          x: RULER / 2 - 40,
          y: sy - 11,
          width: 80,
          height: 22,
          around: 'center',
          rotation: -90,
          textAlign: 'center',
          verticalAlign: 'middle',
          text: String(r),
          fill: '#cbd5e1',
          fontSize: 9,
        }),
      )
    }
  }

  if (lastMouseX >= RULER && lastMouseY >= RULER) {
    g.add(new Line({ x: lastMouseX, y: RULER, points: [0, 0, 0, h - RULER], stroke: '#475569', strokeWidth: 1 }))
    g.add(new Line({ x: RULER, y: lastMouseY, points: [0, 0, w - RULER, 0], stroke: '#475569', strokeWidth: 1 }))
    g.add(new Line({ x: lastMouseX, y: 0, points: [0, 0, 0, RULER], stroke: '#38bdf8', strokeWidth: 2 }))
    g.add(new Line({ x: 0, y: lastMouseY, points: [0, 0, RULER, 0], stroke: '#38bdf8', strokeWidth: 2 }))
    const coordText = hoveredBit.value
      ? `row:${hoveredBit.value.r} col:${hoveredBit.value.c} bin:${hoveredBit.value.bin} (${binLabels[hoveredBit.value.bin]})`
      : `X:${Math.round(cursorWorld.value.x)} Y:${Math.round(cursorWorld.value.y)}`
    g.add(
      new Text({
        x: w - 400,
        y: h - 18,
        width: 392,
        textAlign: 'right',
        text: coordText,
        fill: '#38bdf8',
        fontSize: 10,
      }),
    )
  }
}

function updateHover(clientX: number, clientY: number) {
  if (!app || !containerRef.value) return
  const rect = containerRef.value.getBoundingClientRect()
  lastMouseX = clientX - rect.left
  lastMouseY = clientY - rect.top
  const world = app.tree.getPagePointByClient({ clientX, clientY })
  cursorWorld.value = {
    x: Math.max(-sheetPad, Math.min(gridWidth + sheetPad, world.x)),
    y: Math.max(-sheetPad, Math.min(gridHeight + sheetPad, world.y)),
  }
  const c = Math.floor(world.x / bitPitch)
  const r = Math.floor(world.y / bitPitch)
  const bit = r >= 0 && r < rows && c >= 0 && c < cols ? { r, c, bin: bitData[r * cols + c] } : null
  hoveredBit.value = bit
  if (hoverRect) {
    if (bit) {
      hoverRect.set({ x: bit.c * bitPitch, y: bit.r * bitPitch, visible: true })
    } else {
      hoverRect.visible = false
    }
  }
  drawRuler()
}

onMounted(() => {
  const host = containerRef.value
  if (!host) return

  app = new App({
    view: host,
    tree: { type: 'design' },
    sky: {},
    fill: '#020617',
    wheel: { preventDefault: true },
    pointer: { preventDefaultMenu: true },
    move: { drag: true, dragEmpty: true, dragOut: true },
    zoom: { min: 0.02, max: 100 },
  })

  content = new Group()
  app.tree.add(content)

  // 图纸背景 + 位图区底
  content.add(
    new Rect({
      x: -sheetPad,
      y: -sheetPad,
      width: gridWidth + sheetPad * 2,
      height: gridHeight + sheetPad * 2,
      fill: '#0b1120',
    }),
  )
  content.add(
    new Rect({ x: 0, y: 0, width: gridWidth, height: gridHeight, fill: '#050914', stroke: '#334155', strokeWidth: 2 }),
  )

  // 2048×2048 bit 纹理(Canvas 元素:一次绘制,缩放时仅重采样)
  const bitmap = new LeaferCanvas({ x: 0, y: 0, width: gridWidth, height: gridHeight, smooth: false, hittable: false })
  content.add(bitmap)
  paintBitMap(bitmap)

  // hover 高亮框(世界坐标)
  hoverRect = new Rect({ x: 0, y: 0, width: bitPitch, height: bitPitch, stroke: '#38bdf8', strokeWidth: 0.25, visible: false })
  content.add(hoverRect)

  // 左上角信息(屏幕层)
  const info = new Text({
    x: RULER + 8,
    y: 6,
    text: `Die(${props.die.x}, ${props.die.y})  bin=${props.die.bin}  bits=${rows.toLocaleString()}×${cols.toLocaleString()}`,
    fill: '#ffffff',
    fontSize: 13,
  })
  app.sky.add(info)

  fitInitial()

  app.tree.on(MoveEvent.MOVE, drawRuler)
  app.tree.on(ZoomEvent.ZOOM, drawRuler)

  // leafer drag 模式会拦截 canvas 上的 pointermove 冒泡,挂 window 监听(编辑器拖拽同款)
  const onMove = (e: PointerEvent) => {
    const r = host.getBoundingClientRect()
    if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) return
    updateHover(e.clientX, e.clientY)
  }
  const onLeave = (e: PointerEvent) => {
    // 重绘 sky 标尺会导致指针下方 DOM 变化、误触发 pointerleave;仅当指针真的离开画布才清除
    const r = host.getBoundingClientRect()
    if (e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) return
    lastMouseX = -1
    lastMouseY = -1
    hoveredBit.value = null
    if (hoverRect) hoverRect.visible = false
    drawRuler()
  }
  window.addEventListener('pointermove', onMove)
  host.addEventListener('pointerleave', onLeave)

  resizeObserver = new ResizeObserver(() => {
    if (!app) return
    app.resize({ width: host.clientWidth, height: host.clientHeight })
    drawRuler()
  })
  resizeObserver.observe(host)

  loading.value = false

  // dev 调试钩子(仅开发环境):供端到端测试驱动
  if (import.meta.env.DEV) {
    ;(window as unknown as Record<string, unknown>).__bm = {
      state: () => ({
        hovered: hoveredBit.value,
        cursor: cursorWorld.value,
        zoom: app ? { s: app.tree.zoomLayer.scale, x: app.tree.zoomLayer.x, y: app.tree.zoomLayer.y } : null,
        contentChildren: content ? content.children.length : 0,
      }),
      focusDefect: (r: number, c: number) => focusDefect(r, c),
      fit: () => fitInitial(),
      hoverAt: (clientX: number, clientY: number) => updateHover(clientX, clientY),
    }
  }

  onUnmounted(() => {
    window.removeEventListener('pointermove', onMove)
    host.removeEventListener('pointerleave', onLeave)
    resizeObserver?.disconnect()
    resizeObserver = null
    if (app) {
      app.destroy()
      app = null
    }
    content = null
    rulerGroup = null
    hoverRect = null
  })
})
</script>

<template>
  <div class="fixed inset-0 z-20 flex flex-col bg-[#020617] font-[Arial,sans-serif]">
    <header class="flex items-center gap-4 border-b border-slate-800 bg-slate-900 px-4 py-3 text-slate-200">
      <button class="cursor-pointer rounded border-0 bg-slate-700 px-3 py-1.5 text-sm text-white hover:bg-slate-600" @click="emit('close')">
        ← Back to Wafer
      </button>
      <span class="font-bold">Bit Map (CAD Infinite Canvas)</span>
      <span class="ml-auto text-sm text-slate-400">scroll to zoom · drag to pan · hover to inspect</span>
    </header>
    <div class="flex flex-1 overflow-hidden">
      <div ref="containerRef" class="relative flex-1 overflow-hidden">
        <div v-if="loading" class="absolute inset-0 flex items-center justify-center bg-slate-950/80 text-base text-slate-400">
          Generating 2048×2048 bit map...
        </div>
      </div>
      <aside class="w-60 shrink-0 overflow-y-auto border-l border-slate-800 bg-slate-900 p-4 text-slate-200">
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
        </div>
        <div class="mb-6">
          <h3 class="m-0 mb-3 border-b border-slate-700 pb-1.5 text-[11px] font-semibold tracking-wide text-slate-300 uppercase">Defects (≈{{ defectTotal.toLocaleString() }} total)</h3>
          <ul class="m-0 max-h-[60vh] list-none overflow-y-auto p-0">
            <li
              v-for="d in defects"
              :key="`${d.r}-${d.c}`"
              class="flex cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-[13px] hover:bg-slate-800"
              @click="focusDefect(d.r, d.c)"
            >
              <span class="h-2 w-2 shrink-0 rounded-[2px]" :style="{ background: binColors[d.bin] }" />
              row {{ d.r }} col {{ d.c }}
            </li>
          </ul>
        </div>
      </aside>
    </div>
  </div>
</template>
