<script setup lang="ts">
// BitMap 视图:编排层 —— leafer 生命周期 + 视图函数 + 事件,数据/渲染/侧栏/工具栏已拆分
import { ref, onMounted, onUnmounted } from 'vue'
import { paintRuler, RULER_SIZE as RULER } from '../editor/ruler'
import { App, Group, Rect, Text, Canvas as LeaferCanvas, MoveEvent, ZoomEvent } from 'leafer-editor'
import type { DieInfo } from './WaferMap.vue'
import { useBitmapData } from './bitmap/data'
import { createBitmapRenderer } from './bitmap/render'
import BitmapToolbar from './bitmap/BitmapToolbar.vue'
import BitmapSidebar from './bitmap/BitmapSidebar.vue'
import { BIT_PITCH, COLS, GRID_H, GRID_W, HIERARCHY, ROWS, SHEET_PAD, binColors, bitPath } from './bitmap/types'
import type { ShadingMode } from './bitmap/types'

interface Props {
  die: DieInfo
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'close'): void
}>()

const containerRef = ref<HTMLDivElement | null>(null)
const crispRef = ref<HTMLCanvasElement | null>(null)
const loading = ref(true)
let app: App | null = null
let content: Group | null = null
let hoverRect: Rect | null = null
let bitmapCanvas: LeaferCanvas | null = null
let resizeObserver: ResizeObserver | null = null

// ---------- 状态 ----------
const hoveredBit = ref<{ r: number; c: number; bin: number } | null>(null)
const zoomPct = ref(100)
const gotoRow = ref<number | null>(null)
const gotoCol = ref<number | null>(null)
const cursorWorld = ref<{ x: number; y: number }>({ x: 0, y: 0 })
const shadingMode = ref<ShadingMode>('yield')
const softContrast = ref(true)
const showWlbl = ref(false)
const hierVisibility = ref({ Bank: true, Array: true, Block: true })
// dev 环境默认指向本地 FastAPI 瓦片服务(scripts/dev.sh 随 pnpm dev 启动);清空则回本地渲染
const tileTemplate = ref(import.meta.env.DEV ? 'http://localhost:8000/tiles/{z}/{x}/{y}.png?mode=yield' : '')
const showTileBounds = ref(false)
let lastMouseX = -1
let lastMouseY = -1

// 读数坐标格式化(3 位小数去尾零)
function fmtUm(v: number): string {
  return v.toFixed(3).replace(/\.?0+$/, '')
}

function drawRuler() {
  if (!app) return
  // LeaferCanvas 初次布局会清掉过早绘制的内容:前几次重绘各刷一次纹理兜底
  renderer.maybePaintTexture()
  // 与 DieLayoutEditor 一致:世界原点钉在标尺左上角,不允许视野进入负坐标区
  const layer0 = app.tree.zoomLayer
  const lx = typeof layer0.x === 'number' ? layer0.x : 0
  const ly = typeof layer0.y === 'number' ? layer0.y : 0
  if (lx > RULER) layer0.x = RULER
  if (ly > RULER) layer0.y = RULER
  zoomPct.value = Math.round((typeof layer0.scale === 'number' ? layer0.scale : 1) * 100)
  // 瓦片模式:内容归后端,隐藏本地总览纹理与本地层级(避免加载间隙透出本地数据)
  const tileMode = tileTemplate.value.trim().length > 0
  if (bitmapCanvas) bitmapCanvas.visible = !tileMode
  if (!tileMode) renderer.drawHierarchy()
  renderer.paintCrisp(crispRef)
  paintRuler(app, {
    mouse: lastMouseX >= RULER && lastMouseY >= RULER ? { x: lastMouseX, y: lastMouseY } : null,
    selection: null,
    dark: true,
    cellNote: `(bit ${BIT_PITCH} μm)`,
    // 读数由模板底部 DOM 层展示(crisp 画布会盖住 sky 文本)
    readout: false,
    // 位图标尺:最小刻度 1μm(=bit 间距),最大 256×1000μm;256μm 块边界加粗强调,随缩放自动换档
    steps: [1, 2, 5, 10, 20, 50, 100, 200, 256, 500, 1000, 2000, 5000, 10000, 20000, 50000, 100000, 256000],
    minStep: 1,
    blockStep: 256,
  })
  // 位图领域读数(X/Y + row/col/bin + 层级路径 + WL/BL)走模板底部 DOM 覆盖层:
  // crisp 画布 z 层高于 leafer sky,读数画在 sky 里会被位图内容盖住
}

// ---------- 数据与渲染(拆分模块) ----------
const data = useBitmapData(props.die.bin, drawRuler)
const renderer = createBitmapRenderer({
  getApp: () => app,
  getContent: () => content,
  getBitmapCanvas: () => bitmapCanvas,
  data,
  tile: { template: tileTemplate, showBounds: showTileBounds },
  ui: { shadingMode, softContrast, showWlbl, hierVisibility },
  requestRedraw: drawRuler,
})

// ---------- 视图函数 ----------
function fitInitial() {
  if (!app) return
  const w = app.width ?? 0
  const h = app.height ?? 0
  const s = Math.max(0.05, Math.min((w - 80) / GRID_W, (h - 80) / GRID_H, 1))
  const layer = app.tree.zoomLayer
  layer.scale = s
  layer.x = w / 2 - (GRID_W * s) / 2 + 12
  layer.y = h / 2 - (GRID_H * s) / 2 + 12
  drawRuler()
}

function focusDefect(r: number, c: number) {
  if (!app) return
  const s = 20
  const layer = app.tree.zoomLayer
  layer.scale = s
  layer.x = (app.width ?? 0) / 2 - (c + 0.5) * s
  layer.y = (app.height ?? 0) / 2 - (r + 0.5) * s
  drawRuler()
}

/** 聚焦某层级区域(侧栏导航用) */
function focusRegion(x: number, y: number, size: number) {
  if (!app) return
  const w = app.width ?? 0
  const h = app.height ?? 0
  const s = Math.min(100, Math.max(0.02, (Math.min(w, h) * 0.7) / size))
  const layer = app.tree.zoomLayer
  layer.scale = s
  layer.x = w / 2 - (x + size / 2) * s
  layer.y = h / 2 - (y + size / 2) * s
  drawRuler()
}

/** 聚焦某条 WL(行带)/ BL(列带) */
function focusWlbl(kind: 'wl' | 'bl', i: number) {
  if (!app) return
  const w = app.width ?? 0
  const h = app.height ?? 0
  const layer = app.tree.zoomLayer
  if (kind === 'wl') {
    const s = Math.min(100, Math.max(0.02, (w - 100) / GRID_W))
    layer.scale = s
    layer.x = w / 2 - (GRID_W / 2) * s
    layer.y = h / 2 - (i + 0.5) * s
  } else {
    const s = Math.min(100, Math.max(0.02, (h - 100) / GRID_H))
    layer.scale = s
    layer.y = h / 2 - (GRID_H / 2) * s
    layer.x = w / 2 - (i + 0.5) * s
  }
  drawRuler()
}

/** 跳转到指字线/位线交叉的 bit(中倍率居中) */
function gotoWlbl(r: number, c: number) {
  if (!app) return
  const w = app.width ?? 0
  const h = app.height ?? 0
  const s = 20
  const layer = app.tree.zoomLayer
  layer.scale = s
  layer.x = w / 2 - (c + 0.5) * s
  layer.y = h / 2 - (r + 0.5) * s
  drawRuler()
}

/**
 * 行业下钻:双击层级格缩放到该格(最深可见层),再双击返回全景。
 * 层级格在视野中 ≥500px 视为"已在该层",下钻时跳过更深层。
 */
function drillAt(wx: number, wy: number) {
  if (!app) return
  if (wx < 0 || wx >= GRID_W || wy < 0 || wy >= GRID_H) {
    fitInitial()
    return
  }
  const layer = app.tree.zoomLayer
  const vs = typeof layer.scale === 'number' ? layer.scale : 1
  // 从浅到深找第一个"尚未占满视野"的层级,下钻到指针所在格;全部占满则回全景
  for (const lv of HIERARCHY) {
    if (lv.span * vs >= 500) continue
    const i = Math.floor(wx / lv.span)
    const j = Math.floor(wy / lv.span)
    focusRegion(i * lv.span, j * lv.span, lv.span)
    return
  }
  fitInitial()
}

// 工具栏:以画布中心为锚点缩放(与 DieLayoutEditor 的 zoomBy 一致)
function zoomBy(factor: number) {
  if (!app) return
  const layer = app.tree.zoomLayer
  const sc = typeof layer.scale === 'number' ? layer.scale : 1
  const w = app.width ?? 0
  const h = app.height ?? 0
  const cx = w / 2
  const cy = h / 2
  const ns = Math.min(100, Math.max(0.02, sc * factor))
  const wx = (cx - (typeof layer.x === 'number' ? layer.x : 0)) / sc
  const wy = (cy - (typeof layer.y === 'number' ? layer.y : 0)) / sc
  layer.scale = ns
  layer.x = cx - wx * ns
  layer.y = cy - wy * ns
  drawRuler()
}

/** 导出当前视图为 PNG(leafer 画布 + crisp 覆盖层合成) */
function exportPng() {
  if (!app || !containerRef.value) return
  const leaferCanvas = containerRef.value.querySelector('canvas')
  const crisp = crispRef.value
  if (!leaferCanvas) return
  const dpr = window.devicePixelRatio || 1
  const w = app.width ?? 0
  const h = app.height ?? 0
  const out = document.createElement('canvas')
  out.width = Math.round(w * dpr)
  out.height = Math.round(h * dpr)
  const ctx = out.getContext('2d')!
  ctx.drawImage(leaferCanvas, 0, 0, out.width, out.height)
  if (crisp) ctx.drawImage(crisp, 0, 0, out.width, out.height)
  const a = document.createElement('a')
  a.href = out.toDataURL('image/png')
  a.download = `bitmap-die(${props.die.x},${props.die.y}).png`
  a.click()
}

function updateHover(clientX: number, clientY: number) {
  if (!app || !containerRef.value) return
  const rect = containerRef.value.getBoundingClientRect()
  lastMouseX = clientX - rect.left
  lastMouseY = clientY - rect.top
  const world = app.tree.getPagePointByClient({ clientX, clientY })
  cursorWorld.value = {
    x: Math.max(-SHEET_PAD, Math.min(GRID_W + SHEET_PAD, world.x)),
    y: Math.max(-SHEET_PAD, Math.min(GRID_H + SHEET_PAD, world.y)),
  }
  const c = Math.floor(world.x / BIT_PITCH)
  const r = Math.floor(world.y / BIT_PITCH)
  const bit = r >= 0 && r < ROWS && c >= 0 && c < COLS ? { r, c, bin: data.bitDataRef.value[r * COLS + c] } : null
  hoveredBit.value = bit
  if (hoverRect) {
    if (bit) {
      hoverRect.set({ x: bit.c * BIT_PITCH, y: bit.r * BIT_PITCH, visible: true })
    } else {
      hoverRect.visible = false
    }
  }
  renderer.drawWlbl(bit ? bit.r : null, bit ? bit.c : null)
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

  // 双击下钻走原生 DOM dblclick(leafer double_tap 在本视图手势配置下不触发,原生事件可靠)
  const onDblClick = (e: MouseEvent) => {
    if (!app) return
    const world = app.tree.getPagePointByClient({ clientX: e.clientX, clientY: e.clientY })
    drillAt(world.x, world.y)
  }
  host.addEventListener('dblclick', onDblClick)

  // 图纸背景 + 位图区底
  content.add(
    new Rect({
      x: -SHEET_PAD,
      y: -SHEET_PAD,
      width: GRID_W + SHEET_PAD * 2,
      height: GRID_H + SHEET_PAD * 2,
      fill: '#0b1120',
    }),
  )
  content.add(
    new Rect({ x: 0, y: 0, width: GRID_W, height: GRID_H, fill: '#050914', stroke: '#334155', strokeWidth: 2 }),
  )

  // 总览纹理:线性采样(缩小 = 平均 shading,行业总览观感);高倍率锐度由 crisp 覆盖层负责
  const bitmap = new LeaferCanvas({ x: 0, y: 0, width: GRID_W, height: GRID_H, smooth: true, hittable: false })
  content.add(bitmap)
  bitmapCanvas = bitmap

  // hover 高亮框(世界坐标)
  hoverRect = new Rect({ x: 0, y: 0, width: BIT_PITCH, height: BIT_PITCH, stroke: '#38bdf8', strokeWidth: 0.25, visible: false })
  content.add(hoverRect)

  // 左上角信息(屏幕层)
  const info = new Text({
    x: RULER + 8,
    y: 6,
    text: `Die(${props.die.x}, ${props.die.y})  bin=${props.die.bin}  bits=${ROWS.toLocaleString()}×${COLS.toLocaleString()}`,
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
    renderer.drawWlbl(null, null)
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

  // 数据源:本地 FastAPI 服务(/bits)优先,失败回退合成随机;随后刷总览纹理
  ;(async () => {
    const remote = await data.loadBitDataFromServer()
    if (remote) {
      data.bitDataRef.value = remote
      data.dataSource.value = 'image'
    }
    data.rebuildMips()
    data.invalidateAnalysis()
    loading.value = false
    drawRuler()
  })()

  // dev 调试钩子(仅开发环境):供端到端测试驱动
  if (import.meta.env.DEV) {
    ;(window as unknown as Record<string, unknown>).__bm = {
      state: () => ({
        hovered: hoveredBit.value,
        cursor: cursorWorld.value,
        zoom: app ? { s: app.tree.zoomLayer.scale, x: app.tree.zoomLayer.x, y: app.tree.zoomLayer.y } : null,
        contentChildren: content ? content.children.length : 0,
        dataSource: data.dataSource.value,
        crispCells: (() => {
          const cv = crispRef.value
          return cv ? { w: cv.width, h: cv.height } : null
        })(),
      }),
      focusDefect: (r: number, c: number) => focusDefect(r, c),
      setZoom: (sc: number) => {
        if (!app) return
        app.tree.zoomLayer.scale = sc
        drawRuler()
      },
      fit: () => fitInitial(),
      hoverAt: (clientX: number, clientY: number) => updateHover(clientX, clientY),
      appRef: () => app,
      bitPath: (r: number, c: number) => bitPath(r, c),
      focusRegion: (x: number, y: number, size: number) => focusRegion(x, y, size),
    }
  }

  onUnmounted(() => {
    window.removeEventListener('pointermove', onMove)
    host.removeEventListener('pointerleave', onLeave)
    host.removeEventListener('dblclick', onDblClick)
    resizeObserver?.disconnect()
    resizeObserver = null
    renderer.clearOverlays()
    if (app) {
      app.destroy()
      app = null
    }
    content = null
    hoverRect = null
    bitmapCanvas = null
  })
})
</script>

<template>
  <div class="absolute inset-0 flex flex-col bg-[#020617] font-[Arial,sans-serif]">
    <header class="flex items-center gap-4 border-b border-slate-800 bg-slate-900 px-4 py-3 text-slate-200">
      <button class="cursor-pointer rounded border-0 bg-slate-700 px-3 py-1.5 text-sm text-white hover:bg-slate-600" @click="emit('close')">
        ← Back to Wafer
      </button>
      <span class="font-bold">Bit Map (CAD Infinite Canvas)</span>
      <span class="ml-auto text-sm text-slate-400">scroll to zoom · drag to pan · hover to inspect</span>
      <button
        class="ml-3 cursor-pointer rounded border border-slate-600 bg-slate-800 px-2.5 py-1 text-xs text-slate-200 hover:border-sky-500 hover:text-sky-400"
        title="导出当前视图为 PNG"
        @click="exportPng"
      >
        导出 PNG
      </button>
    </header>
    <div class="flex flex-1 overflow-hidden">
      <div ref="containerRef" class="relative flex-1 overflow-hidden">
        <div v-if="loading" class="absolute inset-0 flex items-center justify-center bg-slate-950/80 text-base text-slate-400">
          Loading 2048×2048 bit map(本地服务 /bits,失败则合成)...
        </div>
        <canvas ref="crispRef" class="pointer-events-none absolute inset-0 z-[2]" style="image-rendering: pixelated"></canvas>
        <!-- 领域读数层:高于 crisp 画布,悬停 bit 时显示坐标 + row/col/bin + 层级路径 + WL/BL -->
        <div class="pointer-events-none absolute right-3 bottom-12 z-[3] text-right font-mono text-[10px] leading-4 text-sky-400">
          <div>
            X {{ fmtUm(cursorWorld.x) }} μm · Y {{ fmtUm(cursorWorld.y) }} μm
            <template v-if="hoveredBit">
              · row:{{ hoveredBit.r }} col:{{ hoveredBit.c }} bin:{{ hoveredBit.bin }}
              ({{ ['PASS', 'FAIL', 'WARN', 'SKIP', 'NULL'][hoveredBit.bin] }})
            </template>
          </div>
          <div v-if="hoveredBit">{{ bitPath(hoveredBit.r, hoveredBit.c) }} · WL={{ hoveredBit.r }} BL={{ hoveredBit.c }}</div>
        </div>
        <BitmapToolbar :zoom-pct="zoomPct" @zoom="zoomBy" @fit="fitInitial" />
      </div>
      <BitmapSidebar
        :die="props.die"
        :wafer-dies="data.waferDies.value"
        :bin-stats="data.binStats.value"
        :pattern-stats="data.patternStats.value"
        :wl-top="data.wlTop.value"
        :bl-top="data.blTop.value"
        :defects="data.defects.value"
        :defect-total="data.defectTotal.value"
        :bin-colors="binColors"
        :bin-labels="['PASS', 'FAIL', 'WARN', 'SKIP', 'NULL']"
        :hovered-bit="hoveredBit"
        :cursor-world="cursorWorld"
        :rows="ROWS"
        :cols="COLS"
        v-model:shading-mode="shadingMode"
        v-model:soft-contrast="softContrast"
        v-model:show-wlbl="showWlbl"
        v-model:hier-visibility="hierVisibility"
        v-model:tile-template="tileTemplate"
        v-model:show-tile-bounds="showTileBounds"
        v-model:bin-filter="data.binFilter.value"
        v-model:goto-row="gotoRow"
        v-model:goto-col="gotoCol"
        @focus-region="focusRegion"
        @focus-wlbl="focusWlbl"
        @focus-defect="focusDefect"
        @goto="gotoWlbl(Math.round(gotoRow ?? 0), Math.round(gotoCol ?? 0))"
        @bin-toggle="data.onBinToggle"
        @redraw="drawRuler"
        @tile-reset="renderer.resetTiles(); drawRuler()"
      />
    </div>
  </div>
</template>
