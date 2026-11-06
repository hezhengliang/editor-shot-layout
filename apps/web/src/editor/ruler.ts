// 共享标尺/无限网格绘制:DieLayoutEditor 与 BitMapView 同款
// 坐标模型:screen = zoomLayer 平移/缩放(leafer 管理),世界坐标 μm
// 每次调用整体重建图层(编辑器既有模式,帧内调用频率低,可接受)

import { App, Group, Line, Rect, Text } from 'leafer-editor'
import { PALETTES, palette } from '../theme'
import type { CanvasPalette } from '../theme'

export const RULER_SIZE = 26

export interface RulerInput {
  /** 指针画布内坐标(屏幕,不含标尺);null 则不画十字线 */
  mouse: { x: number; y: number } | null
  /** 选中内容的世界包围盒(画标尺高亮带 + 尺寸 pill);不需要则传 null */
  selection?: { x: number; y: number; w: number; h: number } | null
  /** 是否绘制无限网格(默认 true) */
  grid?: boolean
  /** 强制暗色调色板(独立暗色视图如 BitMapView);缺省跟随编辑器主题 */
  dark?: boolean
  /** 坐标读数尾部备注,如 "(cell 500 μm)" / "(bit 1 μm)" */
  cellNote?: string
  /** 是否画右下角 X/Y 坐标读数(默认 true;有状态栏的视图可关) */
  readout?: boolean
  /** Y 标尺带宽度按可见最大读数自适应(编辑器开;BitMap 等保持 26) */
  dynamicYBand?: boolean
  /** 强调色(十字线/读数/pill 文字),默认 #38bdf8 */
  accent?: string
  /** 主刻度候选表(升序,末位为最大刻度),缺省用通用 1-2-5 表 */
  steps?: number[]
  /** 次刻度下限(如 BitMap 最小刻度 1μm) */
  minStep?: number
  /** 块边界步长(如 256):该倍数处画加长加粗刻度 + 大标签,中缩放比下展示块结构 */
  blockStep?: number
}

const sessions = new WeakMap<App, { ruler: Group | null; grid: Group | null }>()

function session(app: App) {
  let s = sessions.get(app)
  if (!s) {
    s = { ruler: null, grid: null }
    sessions.set(app, s)
  }
  return s
}

const DEFAULT_STEPS = [
  0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1, 2, 5, 10, 20, 50, 100,
  200, 500, 1000, 2000, 5000, 10000, 20000, 50000, 100000,
]

/** 主刻度:候选表中第一个满足 步长×缩放 ≥ 目标像素 的项;都不满足取末位(最大刻度) */
function niceStepUm(pxPerUm: number, targetPx: number, steps?: number[]): number {
  const table = steps ?? DEFAULT_STEPS
  for (const s of table) {
    if (s * pxPerUm >= targetPx) return s
  }
  return table[table.length - 1]
}

interface TickCfg {
  major: number
  minor: number
  block: number
}

function tickCfg(sc: number, input: RulerInput): TickCfg {
  const major = niceStepUm(sc, 70, input.steps)
  const minor = Math.max(major / 5, input.minStep ?? 0.0001)
  const block = input.blockStep ?? 0
  return { major, minor: minor >= major ? major : minor, block }
}

function num(v: unknown, fallback = 0): number {
  return typeof v === 'number' && isFinite(v) ? v : fallback
}

// 刻度标签按量级分档,避免大数字挤满标尺:≥1e6 → M,≥1e4 → k,其余原值(去尾零)
const trim = (v: number) => v.toFixed(3).replace(/\.?0+$/, '')
function labelFmt(v: number) {
  const a = Math.abs(v)
  if (a >= 1e6) return `${trim(v / 1e6)}M`
  if (a >= 1e4) return `${trim(v / 1e3)}k`
  return trim(v)
}

/** Y 标尺带宽度:容纳视口内最长标签(动态 26~64px),避免 "100000" 被裁掉前半(桌面同款) */
export function bandW(maxVisibleY: number): number {
  return Math.min(64, Math.max(RULER_SIZE, labelFmt(Math.max(0, maxVisibleY)).length * 6 + 20))
}

/** 当前视口下的 Y 标尺带实际宽度(dynamic=false 时恒为 RULER_SIZE) */
export function currentBandW(app: App, dynamic: boolean): number {
  if (!dynamic) return RULER_SIZE
  const layer = app.tree.zoomLayer
  const sc = num(layer.scale, 1)
  const maxY = sc > 0 ? ((app.height ?? 0) - num(layer.y)) / sc : 0
  return bandW(maxY)
}

/** 无限画布网格:覆盖可见视口,世界坐标,任何缩放下恒 1px 线宽 */
function paintGrid(app: App, pal: CanvasPalette, inputRef: RulerInput, s: { grid: Group | null }, bw: number) {
  s.grid?.destroy()
  const g = new Group({ hittable: false })
  s.grid = g
  app.tree.add(g, 0)
  const layer = app.tree.zoomLayer
  const sc = num(layer.scale, 1)
  const ox = num(layer.x)
  const oy = num(layer.y)
  const w = app.width ?? 0
  const h = app.height ?? 0
  if (w <= 0 || h <= 0 || sc <= 0) return
  const x0 = (bw - ox) / sc
  const x1 = (w - ox) / sc
  const y0 = (RULER_SIZE - oy) / sc
  const y1 = (h - oy) / sc
  const { major, minor, block } = tickCfg(sc, inputRef)
  const lw = 1 / sc
  let k = Math.floor(x0 / minor)
  while (k * minor <= x1) {
    const um = k * minor
    if (um >= x0 && um <= x1) {
      const isMajor = Math.abs(um / major - Math.round(um / major)) < 1e-9
      const isBlock = block > 0 && Math.abs(um / block - Math.round(um / block)) < 1e-9
      const w = isBlock ? 2 / sc : lw
      g.add(new Rect({ x: um - w / 2, y: y0, width: w, height: y1 - y0, fill: isBlock ? pal.gridMajor : isMajor ? pal.gridMajor : pal.gridMinor }))
    }
    k += 1
  }
  k = Math.floor(y0 / minor)
  while (k * minor <= y1) {
    const um = k * minor
    if (um >= y0 && um <= y1) {
      const isMajor = Math.abs(um / major - Math.round(um / major)) < 1e-9
      const isBlock = block > 0 && Math.abs(um / block - Math.round(um / block)) < 1e-9
      const w = isBlock ? 2 / sc : lw
      g.add(new Rect({ x: x0, y: um - w / 2, width: x1 - x0, height: w, fill: isMajor ? pal.gridMajor : pal.gridMinor }))
    }
    k += 1
  }
}

/** 标尺(屏幕坐标,固定在 sky 层,不随内容缩放):刻度 + 十字线 + 读数 + 选中高亮带 */
function paintRulerLayer(
  app: App,
  pal: CanvasPalette,
  accent: string,
  input: RulerInput,
  s: { ruler: Group | null },
  bw: number,
) {
  s.ruler?.destroy()
  const ruler = new Group({ hittable: false })
  s.ruler = ruler
  app.sky.add(ruler)

  const w = app.width ?? 0
  const h = app.height ?? 0
  const layer = app.tree.zoomLayer
  const sc = num(layer.scale, 1)
  const ox = num(layer.x)
  const oy = num(layer.y)
  const cellNote = input.cellNote ?? ''

  ruler.add(new Rect({ x: 0, y: 0, width: w, height: RULER_SIZE, fill: pal.rulerBg }))
  ruler.add(new Rect({ x: 0, y: 0, width: bw, height: h, fill: pal.rulerBg }))
  ruler.add(new Line({ x: 0, y: RULER_SIZE, points: [0, 0, w, 0], stroke: pal.rulerLine, strokeWidth: 1 }))
  ruler.add(new Line({ x: bw, y: 0, points: [0, 0, 0, h], stroke: pal.rulerLine, strokeWidth: 1 }))

  const { major, minor, block } = tickCfg(sc, input)
  const isMajorAt = (um: number) => Math.abs(um / major - Math.round(um / major)) < 1e-9
  const isBlockAt = (um: number) => block > 0 && Math.abs(um / block - Math.round(um / block)) < 1e-9

  // 相邻标签最小间距(px),不足则跳过后画者(防块标签与主刻度标签重叠)
  const LABEL_GAP = 34
  const startUmX = Math.floor((RULER_SIZE - ox) / sc / minor) * minor
  let lastLabelX = -Infinity
  for (let um = startUmX; ; um += minor) {
    const sx = ox + um * sc
    if (sx > w) break
    if (sx < RULER_SIZE) continue
    const isBlock = isBlockAt(um)
    const isMajor = isMajorAt(um) || isBlock
    const len = isBlock ? 16 : isMajor ? 10 : 5
    ruler.add(new Line({ x: sx, y: RULER_SIZE, points: [0, 0, 0, -len], stroke: isMajor ? pal.rulerText : pal.rulerMinor, strokeWidth: isBlock ? 2 : 1 }))
    if ((isBlock || isMajorAt(um)) && sx - lastLabelX >= LABEL_GAP) {
      lastLabelX = sx
      ruler.add(
        new Text({
          x: sx - 40,
          y: isBlock ? 1 : 2,
          width: 80,
          textAlign: 'center',
          text: labelFmt(um),
          fill: isBlock ? accent : pal.rulerText,
          fontSize: isBlock ? 10 : 9,
        }),
      )
    }
  }

  const startUmY = Math.floor((bw - oy) / sc / minor) * minor
  let lastLabelY = -Infinity
  for (let um = startUmY; ; um += minor) {
    const sy = oy + um * sc
    if (sy > h) break
    if (sy < bw) continue
    const isBlock = isBlockAt(um)
    const isMajor = isMajorAt(um) || isBlock
    const len = isBlock ? 16 : isMajor ? 10 : 5
    ruler.add(new Line({ x: bw, y: sy, points: [0, 0, -len, 0], stroke: isMajor ? pal.rulerText : pal.rulerMinor, strokeWidth: isBlock ? 2 : 1 }))
    if ((isBlock || isMajorAt(um)) && sy - lastLabelY >= LABEL_GAP && um !== 0) {
      lastLabelY = sy
      // 竖排字符(逐字换行):规避 leafer rotation 文本在部分环境不渲染的问题
      const label = labelFmt(um).split('').join('\n')
      ruler.add(
        new Text({
          x: bw / 2 - 5,
          y: sy - 11 * 2.2,
          width: 10,
          height: 22,
          around: 'center',
          textAlign: 'center',
          verticalAlign: 'middle',
          text: label,
          fill: isBlock ? accent : pal.rulerText,
          fontSize: isBlock ? 10 : 9,
          lineHeight: 11,
        }),
      )
    }
  }

  if (input.mouse && input.mouse.x >= bw && input.mouse.y >= RULER_SIZE) {
    const mx = input.mouse.x
    const my = input.mouse.y
    ruler.add(new Line({ x: mx, y: RULER_SIZE, points: [0, 0, 0, h - RULER_SIZE], stroke: pal.rulerMinor, strokeWidth: 1 }))
    ruler.add(new Line({ x: bw, y: my, points: [0, 0, w - bw, 0], stroke: pal.rulerMinor, strokeWidth: 1 }))
    ruler.add(new Line({ x: mx, y: 0, points: [0, 0, 0, RULER_SIZE], stroke: accent, strokeWidth: 2 }))
    ruler.add(new Line({ x: 0, y: my, points: [0, 0, bw, 0], stroke: accent, strokeWidth: 2 }))
    const umX = (mx - ox) / sc
    const umY = (my - oy) / sc
    if (input.readout !== false) {
      ruler.add(
        new Text({
          x: w - 308,
          y: h - 16,
          width: 300,
          textAlign: 'right',
          text: `X: ${umX.toFixed(3)} μm  Y: ${umY.toFixed(3)} μm${cellNote ? '  ' + cellNote : ''}`,
          fill: accent,
          fontSize: 11,
        }),
      )
    }
  }

  const sel = input.selection ?? null
  if (sel) {
    const p1x = ox + sel.x * sc
    const p2x = ox + (sel.x + sel.w) * sc
    const p1y = oy + sel.y * sc
    const p2y = oy + (sel.y + sel.h) * sc
    const hl = 'rgba(56,189,248,0.4)'
    if (p2x > RULER_SIZE && p1x < w) {
      const bx1 = Math.max(p1x, RULER_SIZE)
      const bx2 = Math.min(p2x, w)
      ruler.add(new Rect({ x: bx1, y: 0, width: bx2 - bx1, height: RULER_SIZE, fill: hl }))
      const cx = (bx1 + bx2) / 2
      ruler.add(new Rect({ x: cx - 45, y: 4, width: 90, height: RULER_SIZE - 8, fill: pal.pillBg, cornerRadius: 2 }))
      ruler.add(
        new Text({
          x: cx - 45,
          y: 4,
          width: 90,
          height: RULER_SIZE - 8,
          textAlign: 'center',
          verticalAlign: 'middle',
          text: `${sel.w.toFixed(3).replace(/\.?0+$/, '')} μm`,
          fill: accent,
          fontSize: 10,
        }),
      )
    }
    if (p2y > bw && p1y < h) {
      const by1 = Math.max(p1y, bw)
      const by2 = Math.min(p2y, h)
      ruler.add(new Rect({ x: 0, y: by1, width: bw, height: by2 - by1, fill: hl }))
      const cy = (by1 + by2) / 2
      ruler.add(new Rect({ x: 4, y: cy - 45, width: bw - 8, height: 90, fill: pal.pillBg, cornerRadius: 2 }))
      const label = `${sel.h.toFixed(3).replace(/\.?0+$/, '')} μm`.split('').join('\n')
      ruler.add(
        new Text({
          x: 4 + (bw - 8) / 2 - 5,
          y: cy - 9,
          width: 10,
          height: 18,
          around: 'center',
          textAlign: 'center',
          verticalAlign: 'middle',
          text: label,
          fill: accent,
          fontSize: 9,
          lineHeight: 10,
        }),
      )
    }
  }
}

/** 绘制(或重绘)标尺 + 网格;视图侧在指针/缩放/选中变化时调用 */
export function paintRuler(app: App | null, input: RulerInput): void {
  if (!app) return
  const pal = input.dark ? PALETTES.dark : palette()
  const accent = input.accent ?? '#38bdf8'
  const s = session(app)
  const bw = currentBandW(app, input.dynamicYBand === true)
  if (input.grid === false) {
    s.grid?.destroy()
    s.grid = null
  } else {
    paintGrid(app, pal, input, s, bw)
  }
  paintRulerLayer(app, pal, accent, input, s, bw)
}
