import { params, num, fmtUm, dieMap, shotMap } from './state'
import { REF_FIELDS } from './types'
import type { LRect, PlacedDie, Shot, ShotSlot, UIGroup } from './types'
import { editorTargets } from './ops'

// ---------- 几何 ----------

// 列宽 = 该列最宽 die(空列默认一格);shot 尺寸由内部 die + scribe 导出
export function colWidth(shot: Shot, c: number): number {
  const cell = params.cellSizeUm
  let w = 0
  for (const d of shot.dies) {
    if (d.slot?.i === c) w = Math.max(w, d.wUm)
  }
  return Math.max(w, cell)
}

export function rowHeight(shot: Shot, r: number): number {
  const cell = params.cellSizeUm
  let h = 0
  for (const d of shot.dies) {
    if (d.slot?.j === r) h = Math.max(h, d.hUm)
  }
  return Math.max(h, cell)
}

// 外形 = 外圈 scribe×2 + Σ列宽 + (列数−1)×inner scribe
// die 内容块在 shot 内水平/垂直居中:外圈精确 = pad,相邻 die 间距精确 = gap。
export function shotGridBaseW(shot: Shot): number {
  let sum = 0
  for (let c = 0; c < shot.cols; c++) sum += colWidth(shot, c)
  return sum + Math.max(0, shot.cols - 1) * shot.gapX
}

export function shotGridBaseH(shot: Shot): number {
  let sum = 0
  for (let r = 0; r < shot.rows; r++) sum += rowHeight(shot, r)
  return sum + Math.max(0, shot.rows - 1) * shot.gapY
}

export function shotW(shot: Shot): number {
  return shot.padX * 2 + shotGridBaseW(shot)
}

export function shotH(shot: Shot): number {
  return shot.padY * 2 + shotGridBaseH(shot)
}

export function slotOriginX(shot: Shot, i: number): number {
  let x = shot.padX
  for (let k = 0; k < i; k++) x += colWidth(shot, k) + shot.gapX
  return x
}

export function slotOriginY(shot: Shot, j: number): number {
  let y = shot.padY
  for (let k = 0; k < j; k++) y += rowHeight(shot, k) + shot.gapY
  return y
}

export function slotOrigin(shot: Shot, i: number, j: number): { x: number; y: number } {
  return { x: slotOriginX(shot, i), y: slotOriginY(shot, j) }
}

// 槽位单元格 = die 区域(列宽/行高);inner scribe 是单元格之间的间隙带
export function slotSpan(shot: Shot, i: number, j: number): { w: number; h: number } {
  return { w: colWidth(shot, i), h: rowHeight(shot, j) }
}

// 槽位网格布局:die 固定尺寸,在所属槽位单元格内水平垂直居中。
// inner scribe(相邻 die 间距)= 两单元格居中留白之和;outer scribe(die 到边框)= pad + 首格留白,
// 均由 step/pad/die 尺寸自然导出;gapX/gapY 作为最小 scribe 要求校验。
export function cellBox(shot: Shot, i: number, j: number, wUm: number, hUm: number): LRect | null {
  const span = slotSpan(shot, i, j)
  const w = wUm
  const h = hUm
  // 约束:die 不超过所在列宽(span = 列宽 + inner scribe)
  if (w > span.w + 1e-9) return null
  if (h > span.h + 1e-9) return null
  const o = slotOrigin(shot, i, j)
  return { x: o.x + (span.w - w) / 2, y: o.y + (span.h - h) / 2, w, h }
}

export function layoutRects(
  shot: Shot,
  extra?: { slot: ShotSlot; wUm: number; hUm: number; ignore?: PlacedDie },
): Map<PlacedDie | 'candidate', LRect> | null {
  const result = new Map<PlacedDie | 'candidate', LRect>()
  for (const d of shot.dies) {
    if (extra && d === extra.ignore) continue
    if (!d.slot) continue
    const box = cellBox(shot, d.slot.i, d.slot.j, d.wUm, d.hUm)
    if (!box) return null
    result.set(d, box)
  }
  if (extra) {
    const box = cellBox(shot, extra.slot.i, extra.slot.j, extra.wUm, extra.hUm)
    if (!box) return null
    result.set('candidate', box)
  }
  return result
}

// 曝光场适配描述
export function fieldFitText(shot: Shot): string {
  const t = REF_FIELDS[shot.refField]
  if (!t) return '未选参考场'
  const fit = shotW(shot) <= t.w && shotH(shot) <= t.h
  return `${t.label} · ${fit ? '适配 ✓' : '超出 ✗'}`
}

// 场利用率 = Σdie 面积 / shot 面积(行业关注,直接影响成本)
export function shotUtilization(shot: Shot): number {
  let dieArea = 0
  for (const d of shot.dies) dieArea += d.wUm * d.hUm
  const a = shotW(shot) * shotH(shot)
  return a > 0 ? Math.round((dieArea / a) * 100) : 0
}

// 由 step/pad/die 尺寸导出实际 scribe 值(内圈=相邻 die 间距,外圈=die 到边框)
export function scribeInfo(shot: Shot): { ix: string; iy: string; ox: string; oy: string } {
  if (!shot.dies.length) {
    return { ix: '—', iy: '—', ox: fmtUm(shot.padX), oy: fmtUm(shot.padY) }
  }
  const colMaxW = new Array(shot.cols).fill(0)
  const rowMaxH = new Array(shot.rows).fill(0)
  for (const d of shot.dies) {
    if (!d.slot) continue
    colMaxW[d.slot.i] = Math.max(colMaxW[d.slot.i], d.wUm)
    rowMaxH[d.slot.j] = Math.max(rowMaxH[d.slot.j], d.hUm)
  }
  const innerX =
    shot.cols >= 2
      ? Math.max(
          shot.gapX + (colWidth(shot, 0) - colMaxW[0]) / 2 + (colWidth(shot, 1) - colMaxW[1]) / 2,
          0,
        )
      : 0
  const innerY =
    shot.rows >= 2
      ? Math.max(
          shot.gapY + (rowHeight(shot, 0) - rowMaxH[0]) / 2 + (rowHeight(shot, 1) - rowMaxH[1]) / 2,
          0,
        )
      : 0
  const outerX = shot.padX + (slotSpan(shot, 0, 0).w - colMaxW[0]) / 2
  const outerY = shot.padY + (slotSpan(shot, 0, 0).h - rowMaxH[0]) / 2
  return { ix: fmtUm(innerX), iy: fmtUm(innerY), ox: fmtUm(outerX), oy: fmtUm(outerY) }
}

// 由 die+scribe 导出的各列/行槽位间距(列宽+inner scribe)
export function derivedPitchesX(shot: Shot): string {
  const parts: string[] = []
  for (let c = 0; c < shot.cols; c++) parts.push(fmtUm(colWidth(shot, c) + shot.gapX))
  return parts.join(' / ')
}

export function derivedPitchesY(shot: Shot): string {
  const parts: string[] = []
  for (let r = 0; r < shot.rows; r++) parts.push(fmtUm(rowHeight(shot, r) + shot.gapY))
  return parts.join(' / ')
}

// 按落点找最近的槽位
export function nearestSlot(shot: Shot, localX: number, localY: number): ShotSlot {
  let bi = 0
  let bd = Infinity
  for (let i = 0; i < shot.cols; i++) {
    const d = Math.abs(slotOriginX(shot, i) - localX)
    if (d < bd) {
      bd = d
      bi = i
    }
  }
  let bj = 0
  bd = Infinity
  for (let j = 0; j < shot.rows; j++) {
    const d = Math.abs(slotOriginY(shot, j) - localY)
    if (d < bd) {
      bd = d
      bj = j
    }
  }
  return { i: bi, j: bj }
}

export function shotRect(shot: Shot): { x: number; y: number; w: number; h: number } {
  return { x: num(shot.group.x), y: num(shot.group.y), w: shotW(shot), h: shotH(shot) }
}

export function dieSize(die: PlacedDie): { w: number; h: number } {
  return { w: die.wUm, h: die.hUm }
}

function dieContentPos(die: PlacedDie): { x: number; y: number } {
  if (die.shot) {
    return {
      x: num(die.shot.group.x) + num(die.group.x),
      y: num(die.shot.group.y) + num(die.group.y),
    }
  }
  return { x: num(die.group.x), y: num(die.group.y) }
}

export function dieContentRect(die: PlacedDie): { x: number; y: number; w: number; h: number } {
  const p = dieContentPos(die)
  const s = dieSize(die)
  return { x: p.x, y: p.y, w: s.w, h: s.h }
}

export function rectsOverlap(
  a: { x: number; y: number; w: number; h: number },
  b: { x: number; y: number; w: number; h: number },
): boolean {
  return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y
}

// 当前选中元素(可多选)的内容坐标包围盒
export function selectionContentBounds(): { x: number; y: number; w: number; h: number } | null {
  const targets = editorTargets()
  let minX = Infinity
  let minY = Infinity
  let maxX = -Infinity
  let maxY = -Infinity
  let any = false
  for (const t of targets) {
    const die = dieMap.get(t as UIGroup)
    const shot = shotMap.get(t as UIGroup)
    const r = die ? dieContentRect(die) : shot ? shotRect(shot) : null
    if (r) {
      any = true
      minX = Math.min(minX, r.x)
      minY = Math.min(minY, r.y)
      maxX = Math.max(maxX, r.x + r.w)
      maxY = Math.max(maxY, r.y + r.h)
    }
  }
  return any ? { x: minX, y: minY, w: maxX - minX, h: maxY - minY } : null
}
