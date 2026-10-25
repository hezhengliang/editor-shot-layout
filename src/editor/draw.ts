import { Group, Rect, Line, Text } from 'leafer-editor'
import { palette } from '../theme'
import {
  params,
  placedDies,
  shots,
  app,
  markerLayer,
  shotDimsLayer,
  rulerGroup,
  gridLayerApp,
  hoverGroup,
  zoomPct,
  selSpan,
  selGap,
  selected,
  isolatedShotUid,
  resizeHandles,
  lastMouseX,
  lastMouseY,
  setMarkerLayer,
  setShotDimsLayer,
  setRulerGroup,
  setGridLayerApp,
  setHoverGroup,
  num,
  fmtUm,
  viewScale,
} from './state'
import { REF_FIELDS } from './types'
import type { LRect, PlacedDie, Shot } from './types'
import { shotW, shotH, shotRect, dieSize, dieContentRect, selectionContentBounds } from './geometry'
import { editorDies, refreshShotLayout } from './ops'

// ---------- Shot 内部尺寸标注(相邻 die 间距 + 内边距) ----------

function dieLocalRect(die: PlacedDie, _cell: number): LRect {
  const s = dieSize(die)
  return { x: num(die.group.x), y: num(die.group.y), w: s.w, h: s.h }
}

function drawGapDim(layer: Group, axis: 'x' | 'y', a: LRect, b: LRect, gap: number, cell: number) {
  // 文字/线宽为屏幕固定尺寸(不随缩放变大),标注线本身保持世界长度
  const vs = Math.max(viewScale(), 1e-6)
  const fontSize = 11 / vs
  const lw = 1.2 / vs
  const tick = 5 / vs
  const color = '#f472b6'
  const labelW = Math.min(gap * 2, cell * 10)
  if (axis === 'x') {
    const x1 = a.x + a.w
    const x2 = b.x
    const y = (Math.max(a.y, b.y) + Math.min(a.y + a.h, b.y + b.h)) / 2
    layer.add(new Line({ x: x1, y, points: [0, 0, gap, 0], stroke: color, strokeWidth: lw }))
    layer.add(new Line({ x: x1, y, points: [0, -tick, 0, tick], stroke: color, strokeWidth: lw }))
    layer.add(new Line({ x: x2, y, points: [0, -tick, 0, tick], stroke: color, strokeWidth: lw }))
    layer.add(
      new Text({
        x: (x1 + x2) / 2 - labelW / 2,
        y: y - fontSize * 1.35,
        width: labelW,
        textAlign: 'center',
        text: fmtUm(gap),
        fill: color,
        fontSize,
      }),
    )
  } else {
    const y1 = a.y + a.h
    const y2 = b.y
    const x = (Math.max(a.x, b.x) + Math.min(a.x + a.w, b.x + b.w)) / 2
    layer.add(new Line({ x, y: y1, points: [0, 0, 0, gap], stroke: color, strokeWidth: lw }))
    layer.add(new Line({ x, y: y1, points: [-tick, 0, tick, 0], stroke: color, strokeWidth: lw }))
    layer.add(new Line({ x, y: y2, points: [-tick, 0, tick, 0], stroke: color, strokeWidth: lw }))
    // 左上角锚点 + 手动计算:-90° 后文字条水平占据 [x, x+1.2fs],贴线约 4px
    layer.add(
      new Text({
        x: x - fontSize * 1.55,
        y: (y1 + y2) / 2 + labelW / 2,
        width: labelW,
        height: fontSize * 1.2,
        rotation: -90,
        textAlign: 'center',
        verticalAlign: 'middle',
        text: fmtUm(gap),
        fill: color,
        fontSize,
      }),
    )
  }
}

// 标注统一绘制在独立图层(世界坐标),与 Shot 组(网格/背景/Die)互不干扰
export function buildAllShotDims() {
  if (!app) return
  shotDimsLayer?.destroy()
  setShotDimsLayer(new Group({ hittable: false }))
  app.tree.add(shotDimsLayer!)
  // 参考曝光场(行业标准场,琥珀色虚线,居中于 shot)
  const vs0 = Math.max(viewScale(), 1e-6)
  const isoUid = isolatedShotUid.value
  for (const shot of shots.value) {
    if (isoUid !== null && shot.uid !== isoUid) continue
    const t = REF_FIELDS[shot.refField]
    if (!t) continue
    const ox = num(shot.group.x)
    const oy = num(shot.group.y)
    const cx = ox + shotW(shot) / 2
    const cy = oy + shotH(shot) / 2
    shotDimsLayer!.add(
      new Rect({
        x: cx - t.w / 2,
        y: cy - t.h / 2,
        width: t.w,
        height: t.h,
        stroke: '#f59e0b',
        strokeWidth: 2 / vs0,
        dashPattern: [8 / vs0, 5 / vs0],
      }),
    )
    shotDimsLayer!.add(
      new Text({
        x: cx - 80 / vs0,
        y: cy - t.h / 2 + 4 / vs0,
        width: 160 / vs0,
        textAlign: 'center',
        text: `曝光场 ${t.label}`,
        fill: '#f59e0b',
        fontSize: 10 / vs0,
      }),
    )
  }
  const cell = params.cellSizeUm
  for (const shot of shots.value) {
    if (!shot.showDims) continue
    if (isoUid !== null && shot.uid !== isoUid) continue
    const ox = num(shot.group.x)
    const oy = num(shot.group.y)
    const dies = [...shot.dies].sort(
      (a, b) => (a.slot?.j ?? 0) - (b.slot?.j ?? 0) || (a.slot?.i ?? 0) - (b.slot?.i ?? 0),
    )
    const rects = new Map<PlacedDie, LRect>()
    for (const d of dies) {
      const r = dieLocalRect(d, cell)
      rects.set(d, { x: r.x + ox, y: r.y + oy, w: r.w, h: r.h })
    }
    for (const a of dies) {
      const ra = rects.get(a)!
      let right: PlacedDie | null = null
      let gapR = Infinity
      let below: PlacedDie | null = null
      let gapB = Infinity
      for (const b of dies) {
        if (b === a) continue
        const rb = rects.get(b)!
        if (ra.y < rb.y + rb.h && ra.y + ra.h > rb.y) {
          const gap = rb.x - (ra.x + ra.w)
          if (gap > 1e-6 && gap < gapR) {
            gapR = gap
            right = b
          }
        }
        if (ra.x < rb.x + rb.w && ra.x + ra.w > rb.x) {
          const gap = rb.y - (ra.y + ra.h)
          if (gap > 1e-6 && gap < gapB) {
            gapB = gap
            below = b
          }
        }
      }
      if (right) drawGapDim(shotDimsLayer!, 'x', ra, rects.get(right)!, gapR, cell)
      if (below) drawGapDim(shotDimsLayer!, 'y', ra, rects.get(below)!, gapB, cell)
    }

    if (shot.showOuter) drawOuterAndSizeDims(shot, rects, ox, oy)
  }
}

// 外圈 scribe(die↔shot 边框)与 shot 容器尺寸标注,灰色/蓝色
function drawOuterAndSizeDims(
  shot: Shot,
  rects: Map<PlacedDie | 'candidate', LRect>,
  ox: number,
  oy: number,
) {
  const vs = Math.max(viewScale(), 1e-6)
  const fs = 9 / vs
  const lw = 1 / vs
  const tick = 4 / vs
  const gray = palette().dimGray
  const blue = '#38bdf8'
  const w = shotW(shot)
  const h = shotH(shot)

  // 首/末 die 的左/上/右/下缘(相对 shot 边框的本地距离;无 die 时用 pad)
  let firstLeft = Infinity
  let firstTop = Infinity
  let lastRight = -Infinity
  let lastBottom = -Infinity
  for (const r of rects.values()) {
    firstLeft = Math.min(firstLeft, r.x - ox)
    firstTop = Math.min(firstTop, r.y - oy)
    lastRight = Math.max(lastRight, r.x + r.w - ox)
    lastBottom = Math.max(lastBottom, r.y + r.h - oy)
  }
  const outerXL = Number.isFinite(firstLeft) ? firstLeft : shot.padX
  const outerYT = Number.isFinite(firstTop) ? firstTop : shot.padY
  const outerXR = Number.isFinite(lastRight) ? w - lastRight : shot.padX
  const outerYB = Number.isFinite(lastBottom) ? h - lastBottom : shot.padY

  // X 方向(水平)外圈:在 shot 垂直中线(高的一半)处,左边框 → 首 die 左缘;右边框 ← 末 die 右缘
  const oyX = oy + h / 2
  shotDimsLayer!.add(new Line({ x: ox, y: oyX, points: [0, 0, outerXL, 0], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(new Line({ x: ox, y: oyX, points: [0, -tick, 0, tick], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(new Line({ x: ox + outerXL, y: oyX, points: [0, -tick, 0, tick], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(
    new Text({
      x: ox + outerXL / 2 - 45,
      y: oyX - fs * 1.3,
      width: 90,
      textAlign: 'center',
      text: fmtUm(outerXL),
      fill: gray,
      fontSize: fs,
    }),
  )
  shotDimsLayer!.add(new Line({ x: ox + w - outerXR, y: oyX, points: [0, 0, outerXR, 0], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(new Line({ x: ox + w - outerXR, y: oyX, points: [0, -tick, 0, tick], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(new Line({ x: ox + w, y: oyX, points: [0, -tick, 0, tick], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(
    new Text({
      x: ox + w - outerXR / 2 - 45,
      y: oyX - fs * 1.3,
      width: 90,
      textAlign: 'center',
      text: fmtUm(outerXR),
      fill: gray,
      fontSize: fs,
    }),
  )
  // Y 方向(垂直)外圈:在 shot 水平中线(宽的一半)处,上边框 → 首 die 上缘;下边框 ← 末 die 下缘
  const oxY = ox + w / 2
  shotDimsLayer!.add(new Line({ x: oxY, y: oy, points: [0, 0, 0, outerYT], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(new Line({ x: oxY, y: oy, points: [-tick, 0, tick, 0], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(new Line({ x: oxY, y: oy + outerYT, points: [-tick, 0, tick, 0], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(
    new Text({
      x: oxY - fs * 1.55,
      y: oy + outerYT / 2 + 45,
      width: 90,
      height: fs * 1.2,
      rotation: -90,
      textAlign: 'center',
      verticalAlign: 'middle',
      text: fmtUm(outerYT),
      fill: gray,
      fontSize: fs,
    }),
  )
  shotDimsLayer!.add(new Line({ x: oxY, y: oy + h - outerYB, points: [0, 0, 0, outerYB], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(new Line({ x: oxY, y: oy + h - outerYB, points: [-tick, 0, tick, 0], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(new Line({ x: oxY, y: oy + h, points: [-tick, 0, tick, 0], stroke: gray, strokeWidth: lw }))
  shotDimsLayer!.add(
    new Text({
      x: oxY - fs * 1.55,
      y: oy + h - outerYB / 2 + 45,
      width: 90,
      height: fs * 1.2,
      rotation: -90,
      textAlign: 'center',
      verticalAlign: 'middle',
      text: fmtUm(outerYB),
      fill: gray,
      fontSize: fs,
    }),
  )

  // 容器尺寸:shot 底边下方居中,蓝色(底衬为屏幕固定宽度)
  const cy = oy + h + 12 / vs
  const pillW = 150 / vs
  shotDimsLayer!.add(new Line({ x: ox, y: cy, points: [0, 0, w, 0], stroke: blue, strokeWidth: lw }))
  shotDimsLayer!.add(new Line({ x: ox, y: cy, points: [0, -tick, 0, tick], stroke: blue, strokeWidth: lw }))
  shotDimsLayer!.add(new Line({ x: ox + w, y: cy, points: [0, -tick, 0, tick], stroke: blue, strokeWidth: lw }))
  shotDimsLayer!.add(
    new Rect({ x: ox + w / 2 - pillW / 2, y: cy + tick * 0.6, width: pillW, height: fs * 1.4, fill: palette().pillBg, cornerRadius: 2 }),
  )
  shotDimsLayer!.add(
    new Text({
      x: ox + w / 2 - pillW / 2,
      y: cy + tick * 0.6,
      width: pillW,
      height: fs * 1.4,
      textAlign: 'center',
      verticalAlign: 'middle',
      text: `Shot ${fmtUm(w)} × ${fmtUm(h)}`,
      fill: blue,
      fontSize: fs,
    }),
  )
}

// 以画布中心为锚点缩放
export function zoomBy(factor: number) {
  if (!app) return
  const layer = app.tree.zoomLayer
  const s = num(layer.scale, 1)
  const { w, h } = viewSize()
  const cx = w / 2
  const cy = h / 2
  const ns = Math.min(64, Math.max(0.02, s * factor))
  const wx = (cx - num(layer.x)) / s
  const wy = (cy - num(layer.y)) / s
  layer.scale = ns
  layer.x = cx - wx * ns
  layer.y = cy - wy * ns
  drawRuler()
}

// ---------- 标尺 ----------

const RULER_SIZE = 26

function niceStepUm(pxPerUm: number, targetPx: number): number {
  const steps = [
    0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1, 2, 5, 10, 20, 50, 100,
    200, 500, 1000, 2000, 5000, 10000, 20000, 50000, 100000,
  ]
  for (const s of steps) {
    if (s * pxPerUm >= targetPx) return s
  }
  return steps[steps.length - 1]
}

// 悬停高亮框(Figma hover 反馈)
export function hoverHighlight(rect: { x: number; y: number; w: number; h: number } | null) {
  if (!app) return
  if (hoverGroup) hoverGroup.destroy()
  setHoverGroup(null)
  if (!rect) return
  const vs = Math.max(viewScale(), 1e-6)
  setHoverGroup(new Group({ hittable: false }))
  hoverGroup!.add(
    new Rect({
      x: rect.x,
      y: rect.y,
      width: rect.w,
      height: rect.h,
      stroke: '#f472b6',
      strokeWidth: 1.5 / vs,
    }),
  )
  app.tree.add(hoverGroup!)
}

// 无限画布网格:覆盖整个可见视口,世界坐标,任何缩放下恒为 1px 线宽
function drawInfiniteGrid() {
  if (!app) return
  gridLayerApp?.destroy()
  setGridLayerApp(new Group({ hittable: false }))
  app.tree.add(gridLayerApp!, 0)
  const layer = app.tree.zoomLayer
  const s = num(layer.scale, 1)
  const ox = num(layer.x)
  const oy = num(layer.y)
  const w = app.width ?? 0
  const h = app.height ?? 0
  if (w <= 0 || h <= 0 || s <= 0) return
  // 可见世界范围(μm)
  const x0 = (RULER_SIZE - ox) / s
  const x1 = (w - ox) / s
  const y0 = (RULER_SIZE - oy) / s
  const y1 = (h - oy) / s
  const major = niceStepUm(s, 70)
  const minor = major / 5
  const lw = 1 / s
  const minorColor = palette().gridMinor
  const majorColor = palette().gridMajor
  const kx0 = Math.floor(x0 / minor)
  const kx1 = Math.ceil(x1 / minor)
  for (let k = kx0; k <= kx1; k++) {
    const um = k * minor
    if (um < x0 || um > x1) continue
    const isMajor = Math.abs(um / major - Math.round(um / major)) < 1e-9
    gridLayerApp!.add(
      new Rect({ x: um - lw / 2, y: y0, width: lw, height: y1 - y0, fill: isMajor ? majorColor : minorColor }),
    )
  }
  const ky0 = Math.floor(y0 / minor)
  const ky1 = Math.ceil(y1 / minor)
  for (let k = ky0; k <= ky1; k++) {
    const um = k * minor
    if (um < y0 || um > y1) continue
    const isMajor = Math.abs(um / major - Math.round(um / major)) < 1e-9
    gridLayerApp!.add(
      new Rect({ x: x0, y: um - lw / 2, width: x1 - x0, height: lw, fill: isMajor ? majorColor : minorColor }),
    )
  }
}

export function drawRuler() {
  if (!app) return
  // 视图钳制:世界原点固定在标尺左上角,不允许视野进入负坐标区
  // (screen = ox + world*s,绘图区左缘 screen=RULER_SIZE 对应 world≥0 ⇒ ox ≤ RULER_SIZE)
  const layer0 = app.tree.zoomLayer
  if (num(layer0.x) > RULER_SIZE) layer0.x = RULER_SIZE
  if (num(layer0.y) > RULER_SIZE) layer0.y = RULER_SIZE
  drawInfiniteGrid()
  if (rulerGroup) rulerGroup.destroy()
  setRulerGroup(new Group({ hittable: false }))
  app.sky.add(rulerGroup!)

  const w = app.width ?? 0
  const h = app.height ?? 0
  const layer = app.tree.zoomLayer
  const s = num(layer.scale, 1)
  const ox = num(layer.x)
  const oy = num(layer.y)
  const cell = params.cellSizeUm
  zoomPct.value = Math.round(s * 100)

  const bg = new Group()
  bg.add(new Rect({ x: 0, y: 0, width: w, height: RULER_SIZE, fill: palette().rulerBg }))
  bg.add(new Rect({ x: 0, y: 0, width: RULER_SIZE, height: h, fill: palette().rulerBg }))
  bg.add(new Line({ x: 0, y: RULER_SIZE, points: [0, 0, w, 0], stroke: palette().rulerLine, strokeWidth: 1 }))
  bg.add(new Line({ x: RULER_SIZE, y: 0, points: [0, 0, 0, h], stroke: palette().rulerLine, strokeWidth: 1 }))
  rulerGroup!.add(bg)

  const major = niceStepUm(s, 70)
  const minor = major / 5
  const labelFmt = (v: number) => v.toFixed(3).replace(/\.?0+$/, '')

  const umAt = (screen: number, offset: number) => (screen - offset) / s

  const startUmX = Math.floor(umAt(RULER_SIZE, ox) / minor) * minor
  for (let um = startUmX; ; um += minor) {
    const sx = ox + um * s
    if (sx > w) break
    if (sx < RULER_SIZE) continue
    const isMajor = Math.abs(um / major - Math.round(um / major)) < 1e-9
    const len = isMajor ? 10 : 5
    rulerGroup!.add(
      new Line({ x: sx, y: RULER_SIZE, points: [0, 0, 0, -len], stroke: isMajor ? palette().rulerText : palette().rulerMinor, strokeWidth: 1 }),
    )
    if (isMajor) {
      rulerGroup!.add(
        new Text({
          x: sx - 40,
          y: 2,
          width: 80,
          textAlign: 'center',
          text: labelFmt(um),
          fill: '#cbd5e1',
          fontSize: 9,
        }),
      )
    }
  }

  const startUmY = Math.floor(umAt(RULER_SIZE, oy) / minor) * minor
  for (let um = startUmY; ; um += minor) {
    const sy = oy + um * s
    if (sy > h) break
    if (sy < RULER_SIZE) continue
    const isMajor = Math.abs(um / major - Math.round(um / major)) < 1e-9
    const len = isMajor ? 10 : 5
    rulerGroup!.add(
      new Line({ x: RULER_SIZE, y: sy, points: [0, 0, -len, 0], stroke: isMajor ? palette().rulerText : palette().rulerMinor, strokeWidth: 1 }),
    )
    if (isMajor) {
      rulerGroup!.add(
        new Text({
          x: RULER_SIZE / 2 - 40,
          y: sy - 11,
          width: 80,
          height: 22,
          around: 'center',
          rotation: -90,
          textAlign: 'center',
          verticalAlign: 'middle',
          text: labelFmt(um),
          fill: '#cbd5e1',
          fontSize: 9,
        }),
      )
    }
  }

  if (lastMouseX >= RULER_SIZE && lastMouseY >= RULER_SIZE) {
    rulerGroup!.add(
      new Line({ x: lastMouseX, y: RULER_SIZE, points: [0, 0, 0, h - RULER_SIZE], stroke: '#475569', strokeWidth: 1 }),
    )
    rulerGroup!.add(
      new Line({ x: RULER_SIZE, y: lastMouseY, points: [0, 0, w - RULER_SIZE, 0], stroke: '#475569', strokeWidth: 1 }),
    )
    rulerGroup!.add(
      new Line({ x: lastMouseX, y: 0, points: [0, 0, 0, RULER_SIZE], stroke: '#38bdf8', strokeWidth: 2 }),
    )
    rulerGroup!.add(
      new Line({ x: 0, y: lastMouseY, points: [0, 0, RULER_SIZE, 0], stroke: '#38bdf8', strokeWidth: 2 }),
    )
    const umX = (lastMouseX - ox) / s
    const umY = (lastMouseY - oy) / s
    rulerGroup!.add(
      new Text({
        x: w - 308,
        y: h - 16,
        width: 300,
        textAlign: 'right',
        text: `X: ${umX.toFixed(3)} μm  Y: ${umY.toFixed(3)} μm  (cell ${cell} μm)`,
        fill: '#38bdf8',
        fontSize: 11,
      }),
    )
  }

  // 选中元素范围:标尺高亮带 + 范围读数
  const sel = selectionContentBounds()
  if (sel) {
    const x1 = ox + sel.x * s
    const x2 = ox + (sel.x + sel.w) * s
    const y1 = oy + sel.y * s
    const y2 = oy + (sel.y + sel.h) * s
    const hl = 'rgba(56,189,248,0.4)'
    if (x2 > RULER_SIZE && x1 < w) {
      const bx1 = Math.max(x1, RULER_SIZE)
      const bx2 = Math.min(x2, w)
      rulerGroup!.add(new Rect({ x: bx1, y: 0, width: bx2 - bx1, height: RULER_SIZE, fill: hl }))
      // 尺寸 pill 放在标尺带内部,避免压住画布内容(外圈标注等)
      const cx = (bx1 + bx2) / 2
      rulerGroup!.add(
        new Rect({ x: cx - 45, y: 4, width: 90, height: RULER_SIZE - 8, fill: palette().pillBg, cornerRadius: 2 }),
      )
      rulerGroup!.add(
        new Text({ x: cx - 45, y: 4, width: 90, height: RULER_SIZE - 8, textAlign: 'center', verticalAlign: 'middle', text: fmtUm(sel.w), fill: '#38bdf8', fontSize: 10 }),
      )
    }
    if (y2 > RULER_SIZE && y1 < h) {
      const by1 = Math.max(y1, RULER_SIZE)
      const by2 = Math.min(y2, h)
      rulerGroup!.add(new Rect({ x: 0, y: by1, width: RULER_SIZE, height: by2 - by1, fill: hl }))
      const cy = (by1 + by2) / 2
      rulerGroup!.add(
        new Rect({ x: 4, y: cy - 45, width: RULER_SIZE - 8, height: 90, fill: palette().pillBg, cornerRadius: 2 }),
      )
      rulerGroup!.add(
        new Text({
          x: 4 + (RULER_SIZE - 8) / 2 - 45,
          y: cy - 9,
          width: 90,
          height: 18,
          around: 'center',
          rotation: -90,
          textAlign: 'center',
          verticalAlign: 'middle',
          text: fmtUm(sel.h),
          fill: '#38bdf8',
          fontSize: 10,
        }),
      )
    }
  }
  updateResizeHandles()
}

// 选中 Die 的四角 resize 手柄(DOM 覆盖层;leafer 内置 resize 已关闭)
export function updateResizeHandles() {
  resizeHandles.value = []
  const die = selected.value
  if (!app || !die || !die.shot) return
  const layer = app.tree.zoomLayer
  const s = num(layer.scale, 1)
  const ox = num(layer.x)
  const oy = num(layer.y)
  const r = dieContentRect(die)
  const corners: { key: string; x: number; y: number; cursor: string }[] = [
    { key: 'top-left', x: r.x, y: r.y, cursor: 'nwse-resize' },
    { key: 'top-right', x: r.x + r.w, y: r.y, cursor: 'nesw-resize' },
    { key: 'bottom-left', x: r.x, y: r.y + r.h, cursor: 'nesw-resize' },
    { key: 'bottom-right', x: r.x + r.w, y: r.y + r.h, cursor: 'nwse-resize' },
  ]
  resizeHandles.value = corners.map((c) => ({ key: c.key, x: ox + c.x * s, y: oy + c.y * s, cursor: c.cursor }))
}

// ---------- 多选距离标记 ----------

export function updateSelectionMarker() {
  if (!app) return
  if (markerLayer) markerLayer.destroy()
  setMarkerLayer(new Group({ hittable: false }))
  app.tree.add(markerLayer!)
  selSpan.value = null
  selGap.value = null

  const dies = editorDies()
  if (dies.length < 2) return

  const rects = dies.map((d) => ({ die: d, ...dieContentRect(d) }))
  let minX = Infinity
  let minY = Infinity
  let maxX = -Infinity
  let maxY = -Infinity
  for (const r of rects) {
    minX = Math.min(minX, r.x)
    minY = Math.min(minY, r.y)
    maxX = Math.max(maxX, r.x + r.w)
    maxY = Math.max(maxY, r.y + r.h)
  }
  const bw = maxX - minX
  const bh = maxY - minY
  selSpan.value = { w: bw, h: bh }

  if (rects.length === 2) {
    const [a, b] = rects
    const gapX =
      b.x >= a.x + a.w
        ? b.x - (a.x + a.w)
        : a.x >= b.x + b.w
          ? a.x - (b.x + b.w)
          : 0
    const gapY =
      b.y >= a.y + a.h
        ? b.y - (a.y + a.h)
        : a.y >= b.y + b.h
          ? a.y - (b.y + b.h)
          : 0
    selGap.value = { x: gapX, y: gapY }
  }

  markerLayer!.add(
    new Rect({
      x: minX,
      y: minY,
      width: bw,
      height: bh,
      fill: 'rgba(56,189,248,0.06)',
      stroke: '#38bdf8',
      strokeWidth: Math.max(bw, bh) / 400,
      dashPattern: [bw / 30, bw / 60],
    }),
  )
  const fontSize = Math.max(bh * 0.09, bw * 0.03)
  markerLayer!.add(
    new Text({
      x: minX,
      y: minY - fontSize * 1.4,
      width: bw,
      textAlign: 'center',
      text: `${fmtUm(bw)} × ${fmtUm(bh)}`,
      fill: '#38bdf8',
      fontSize,
    }),
  )
}

// ---------- 棋盘 / 视图 ----------

export function viewSize() {
  return { w: app?.width ?? 0, h: app?.height ?? 0 }
}

// 所有 Shot 的内容包围盒(棋盘已移除,die 都在 shot 内)
function contentBounds(): { x: number; y: number; w: number; h: number } | null {
  let minX = Infinity
  let minY = Infinity
  let maxX = -Infinity
  let maxY = -Infinity
  let any = false
  for (const shot of shots.value) {
    const r = shotRect(shot)
    any = true
    minX = Math.min(minX, r.x)
    minY = Math.min(minY, r.y)
    maxX = Math.max(maxX, r.x + r.w)
    maxY = Math.max(maxY, r.y + r.h)
  }
  return any ? { x: minX, y: minY, w: maxX - minX, h: maxY - minY } : null
}

export function fitView() {
  if (!app) return
  const { w, h } = viewSize()
  const pad = 120
  const b = contentBounds()
  if (!b) {
    // 空画布:默认视野约 8000μm 见方,世界原点居中
    const s = Math.min(w, h) / 8000
    app.tree.zoomLayer.scale = s
    app.tree.zoomLayer.x = w / 2
    app.tree.zoomLayer.y = h / 2
    drawRuler()
    return
  }
  const s = Math.max(0.02, Math.min((w - pad) / b.w, (h - pad) / b.h, 2))
  app.tree.zoomLayer.scale = s
  app.tree.zoomLayer.x = (w - b.w * s) / 2 - b.x * s
  app.tree.zoomLayer.y = (h - b.h * s) / 2 - b.y * s
  drawRuler()
}

// 主题切换:重着色画布所有元素
export function applyCanvasTheme() {
  if (!app) return
  const pal = palette()
  ;(app as unknown as { fill?: string }).fill = pal.canvasBg
  for (const die of placedDies.value) {
    die.rect.stroke = pal.dieStroke
    if (die.label) die.label.fill = '#0f172a'
  }
  for (const shot of [...shots.value]) refreshShotLayout(shot)
  buildAllShotDims()
  updateSelectionMarker()
  drawRuler()
}
