import { ref, reactive } from 'vue'
import type { App, Group } from 'leafer-editor'
import type { Pane } from 'tweakpane'
import type { PlacedDie, Preset, Shot, UIGroup } from './types'

export const params = reactive({
  bigCols: 16,
  bigRows: 16,
  cellSizeUm: 500,
})

export const containerRef = ref<HTMLDivElement | null>(null)
export const viewHostRef = ref<HTMLDivElement | null>(null)
// 选中 Die 的四角 resize 手柄(DOM 覆盖层,屏幕坐标;样式对齐 leafer 原生控制点)
export const placedDies = ref<PlacedDie[]>([])
export const shots = ref<Shot[]>([])
export const selected = ref<PlacedDie | null>(null)
export const selectedShot = ref<Shot | null>(null)
export const status = ref('从左侧把小 Die 拖入网格或 Shot,或点击卡片自动放置')

export const zoomPct = ref(100)

// 右键上下文菜单
export const ctxMenu = ref<{ x: number; y: number; die: PlacedDie | null; shot: Shot | null } | null>(null)

export const dragState = ref<{
  cardId: string
  x: number
  y: number
  startX: number
  startY: number
  moved: boolean
} | null>(null)

export const dropHint = ref<{ x: number; y: number; w: number; h: number; valid: boolean } | null>(null)

export const selSpan = ref<{ w: number; h: number } | null>(null)
export const selGap = ref<{ x: number; y: number } | null>(null)

// Figma 式隔离:双击 Shot 只显示它,其余淡化(Esc/双击退出)
export const isolatedShotUid = ref<number | null>(null)

export const paneRef = ref<HTMLDivElement | null>(null)
export const paneLeftRef = ref<HTMLDivElement | null>(null)

export let app: App | null = null
export let dieLayer: Group | null = null
export let markerLayer: Group | null = null
export let previewLayer: Group | null = null
export let shotDimsLayer: Group | null = null
export let resizeObserver: ResizeObserver | null = null
export let rulerGroup: Group | null = null
export let gridLayerApp: Group | null = null
export let hoverGroup: Group | null = null
export let pane: Pane | null = null
export let paneLeft: Pane | null = null
export let uidCounter = 0
export let lastRejectReason = ''
export let lastMouseX = -1
export let lastMouseY = -1

export const dieMap = new Map<UIGroup, PlacedDie>()
export const shotMap = new Map<UIGroup, Shot>()

// —— 参考设计器/CAD 的交互:复制、粘贴、悬停高亮、拖动实时读数 ——
export let clipboard: { preset: Preset; wUm: number; hUm: number }[] = []

export function setApp(v: App | null) {
  app = v
}
export function setDieLayer(v: Group | null) {
  dieLayer = v
}
export function setMarkerLayer(v: Group | null) {
  markerLayer = v
}
export function setPreviewLayer(v: Group | null) {
  previewLayer = v
}
export function setShotDimsLayer(v: Group | null) {
  shotDimsLayer = v
}
export function setResizeObserver(v: ResizeObserver | null) {
  resizeObserver = v
}
export function setRulerGroup(v: Group | null) {
  rulerGroup = v
}
export function setGridLayerApp(v: Group | null) {
  gridLayerApp = v
}
export function setHoverGroup(v: Group | null) {
  hoverGroup = v
}
export function setPane(v: Pane | null) {
  pane = v
}
export function setPaneLeft(v: Pane | null) {
  paneLeft = v
}
export function setUidCounter(v: number) {
  uidCounter = v
  return v
}
export function setLastRejectReason(v: string) {
  lastRejectReason = v
}
export function setLastMouse(x: number, y: number) {
  lastMouseX = x
  lastMouseY = y
}
export function setClipboard(v: { preset: Preset; wUm: number; hUm: number }[]) {
  clipboard = v
}

export function num(v: unknown, fallback = 0): number {
  return typeof v === 'number' && isFinite(v) ? v : fallback
}

export function round3(v: number): number {
  return Math.round(v * 1000) / 1000
}

export function fmtUm(v: number): string {
  return `${v.toFixed(3).replace(/\.?0+$/, '')} μm`
}

export function viewScale(): number {
  return num(app?.tree.zoomLayer.scale, 1)
}

export function setStatus(msg: string) {
  status.value = msg
}

export const resizeHandles = ref<{ key: string; x: number; y: number; cursor: string }[]>([])
