import { Group, Rect, Line, Text, Ellipse } from 'leafer-editor'
import { palette, theme } from '../theme'
import {
  params,
  placedDies,
  shots,
  app,
  markerLayer,
  shotDimsLayer,
  hoverGroup,
  setHoverGroup,
  zoomPct,
  selSpan,
  selGap,
  selected,
  selectedShot,
  isolatedShotUid,
  resizeHandles,
  lastMouseX,
  lastMouseY,
  measureRulers,
  setMarkerLayer,
  setShotDimsLayer,
  num,
  fmtUm,
  viewScale,
} from './state'
import { REF_FIELDS } from './types'
import type { LRect, PlacedDie, Shot } from './types'
import { shotW, shotH, shotRect, dieSize, dieContentRect, selectionContentBounds } from './geometry'
import { editorDies, refreshShotLayout } from './ops'
import { paintRuler, RULER_SIZE, currentBandW } from './ruler'

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

// ---------- 标尺(共享实现见 ./ruler.ts;此处仅做视图钳制与参数装配) ----------

export function drawRuler() {
  if (!app) return
  // Y 标尺带宽度(动态:按可见最大读数 26~64px)
  const bw = currentBandW(app, true)
  // 视图钳制:世界原点固定在标尺左上角,不允许视野进入负坐标区
  const layer0 = app.tree.zoomLayer
  if (num(layer0.x) > bw) layer0.x = bw
  if (num(layer0.y) > RULER_SIZE) layer0.y = RULER_SIZE
  zoomPct.value = Math.round(num(app.tree.zoomLayer.scale, 1) * 100)
  paintRuler(app, {
    mouse: lastMouseX.value >= bw && lastMouseY.value >= RULER_SIZE ? { x: lastMouseX.value, y: lastMouseY.value } : null,
    selection: selectionContentBounds(),
    cellNote: `(cell ${params.cellSizeUm} μm)`,
    // 坐标读数收进底部状态栏,不再画在画布右下角(BitMap 视图仍用共享读数)
    readout: false,
    dynamicYBand: true,
  })
  drawMeasureRulers()
  drawSelectionBadge()
  updateResizeHandles()
}

// ---------- 测量标尺(工具栏 ╱ / R 键;琥珀色;右键点删) ----------

let measureGroup: Group | null = null
let badgeGroup: Group | null = null

// 单条测量标尺形状(committed 与拖拽预览共用;preview 用虚线);世界坐标节点 + 屏幕固定字号/线宽
export function drawMeasureShape(
  g: Group,
  a: { x: number; y: number },
  b: { x: number; y: number },
  preview: boolean,
) {
  const vs = Math.max(viewScale(), 1e-6)
  const dark = theme.value === 'dark'
  const lineC = dark ? '#fbbf24' : '#d97706'
  const textC = dark ? '#fde68a' : '#b45309'
  const lw = 1.5 / vs
  const dot = 3 / vs
  g.add(
    new Line({
      x: a.x,
      y: a.y,
      points: [0, 0, b.x - a.x, b.y - a.y],
      stroke: lineC,
      strokeWidth: lw,
      ...(preview ? { dashPattern: [6 / vs, 4 / vs] } : {}),
    }),
  )
  for (const p of [a, b]) {
    g.add(new Ellipse({ x: p.x - dot, y: p.y - dot, width: dot * 2, height: dot * 2, fill: lineC }))
  }
  // 长度 pill:中点上方,屏幕固定(字号 10、内边距 6/3)
  const fs = 10 / vs
  const padX = 6 / vs
  const padY = 3 / vs
  const label = fmtUm(Math.hypot(b.x - a.x, b.y - a.y))
  const pillW = label.length * fs * 0.68 + padX * 2
  const pillH = fs * 1.2 + padY * 2
  const px = (a.x + b.x) / 2 - pillW / 2
  const py = (a.y + b.y) / 2 - pillH - 6 / vs
  g.add(
    new Rect({
      x: px,
      y: py,
      width: pillW,
      height: pillH,
      fill: palette().pillBg,
      cornerRadius: 3 / vs,
      stroke: lineC,
      strokeWidth: 1 / vs,
    }),
  )
  g.add(
    new Text({
      x: px,
      y: py,
      width: pillW,
      height: pillH,
      textAlign: 'center',
      verticalAlign: 'middle',
      text: label,
      fill: textC,
      fontSize: fs,
    }),
  )
}

// 已提交的测量标尺(挂在 tree 上,随 zoom/pan/主题切换经 drawRuler 重绘)
export function drawMeasureRulers() {
  if (!app) return
  if (measureGroup) measureGroup.destroy()
  measureGroup = null
  if (!measureRulers.value.length) return
  const g = new Group({ hittable: false })
  for (const r of measureRulers.value) drawMeasureShape(g, r.a, r.b, false)
  app.tree.add(g)
  measureGroup = g
}

// 单选 Die/Shot 的实测尺寸徽标:对象屏幕矩形顶边上方 4px 居中 pill(桌面同款)
export function drawSelectionBadge() {
  if (!app) return
  if (badgeGroup) badgeGroup.destroy()
  badgeGroup = null
  const iso = isolatedShotUid.value
  let wr: { x: number; y: number; w: number; h: number } | null = null
  const die = selected.value
  const shot = selectedShot.value
  if (die) {
    // 所在 Shot 被隔离淡化时跳过
    if (iso !== null && (!die.shot || die.shot.uid !== iso)) return
    wr = dieContentRect(die)
  } else if (shot) {
    if (iso !== null && shot.uid !== iso) return
    wr = shotRect(shot)
  } else {
    return
  }
  // 多选(≥2)维持现有包围盒文字,不画 pill
  if (editorDies().length >= 2) return

  const s = Math.max(num(app.tree.zoomLayer.scale, 1), 1e-6)
  const ox = num(app.tree.zoomLayer.x)
  const oy = num(app.tree.zoomLayer.y)
  const { w: cw } = viewSize()
  // 世界坐标节点 + 屏幕固定字号/线宽(同 drawMeasureShape 模式)
  const fs = 10 / s
  const padX = 6 / s
  const padY = 3 / s
  const label = `${fmtUm(wr.w)} × ${fmtUm(wr.h)}`
  const pillW = label.length * fs * 0.68 + padX * 2
  const pillH = fs * 1.2 + padY * 2
  // 顶边上方空间不足(会进 X 标尺带)时改放底边下方
  const topS = oy + wr.y * s
  const below = topS - pillH * s - 4 < RULER_SIZE + 2
  const py = below ? wr.y + wr.h + 4 / s : wr.y - pillH - 4 / s
  // 水平方向钳制在画布内(左界为 Y 标尺带)
  const bw = currentBandW(app, true)
  const cxS = Math.min(
    Math.max(ox + (wr.x + wr.w / 2) * s, bw + (pillW * s) / 2 + 2),
    cw - (pillW * s) / 2 - 2,
  )
  const px = (cxS - ox) / s - pillW / 2
  const blue = palette().shotStroke
  const g = new Group({ hittable: false })
  g.add(
    new Rect({
      x: px,
      y: py,
      width: pillW,
      height: pillH,
      fill: palette().pillBg,
      cornerRadius: 3 / s,
      stroke: blue,
      strokeWidth: 1 / s,
    }),
  )
  g.add(
    new Text({
      x: px,
      y: py,
      width: pillW,
      height: pillH,
      textAlign: 'center',
      verticalAlign: 'middle',
      text: label,
      fill: blue,
      fontSize: fs,
    }),
  )
  app.tree.add(g)
  badgeGroup = g
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
