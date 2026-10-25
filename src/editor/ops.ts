import { Group, Rect, Text, EditorMoveEvent } from 'leafer-editor'
import { palette } from '../theme'
import {
  params,
  placedDies,
  shots,
  selected,
  selectedShot,
  app,
  dieLayer,
  uidCounter,
  dieMap,
  shotMap,
  clipboard,
  ctxMenu,
  setUidCounter,
  setLastRejectReason,
  setClipboard,
  num,
  round3,

  fmtUm,
  viewScale,
  setStatus,
  isolatedShotUid,
} from './state'
import type { LRect, PlacedDie, Preset, Shot, UIGroup, UIRect, UIText } from './types'
import {
  rectsOverlap,
  dieContentRect,
  shotRect,
  layoutRects,
  shotW,
  shotH,
  nearestSlot,
  slotOrigin,
  slotSpan,
  slotOriginX,
  slotOriginY,
  colWidth,
  rowHeight,
  dieSize,
} from './geometry'
import { buildAllShotDims, updateSelectionMarker, drawRuler, hoverHighlight, viewSize } from './draw'
import { rebuildPane } from './panes'
import { dieLibrary, saveLibrary } from './library'

export function editorTargets(): unknown[] {
  if (!app) return []
  const list = app.editor.list as unknown[]
  if (list.length) return list
  const t = app.editor.target as unknown
  return t ? [t] : []
}

// ---------- 放置校验 ----------

export function canPlaceBoardDie(
  col: number,
  row: number,
  wUm: number,
  hUm: number,
  ignore?: PlacedDie,
): boolean {
  const cell = params.cellSizeUm
  if (col < 0 || row < 0 || (col + 1) * cell > params.bigCols * cell || (row + 1) * cell > params.bigRows * cell) {
    return false
  }
  const rect = { x: col * cell, y: row * cell, w: wUm, h: hUm }
  for (const d of placedDies.value) {
    if (d === ignore || d.shot) continue
    if (rectsOverlap(rect, dieContentRect(d))) return false
  }
  for (const s of shots.value) {
    if (rectsOverlap(rect, shotRect(s))) return false
  }
  return true
}

export function canPlaceRect(
  x: number,
  y: number,
  w: number,
  h: number,
  ignoreShot?: Shot,
): boolean {
  // 标尺起点即世界原点(左上):不允许任何元素进入负坐标区
  if (x < 0 || y < 0) return false
  const rect = { x, y, w, h }
  for (const d of placedDies.value) {
    if (d.shot) continue
    if (rectsOverlap(rect, dieContentRect(d))) return false
  }
  for (const s of shots.value) {
    if (s === ignoreShot) continue
    if (rectsOverlap(rect, shotRect(s))) return false
  }
  return true
}

export function slotOccupied(shot: Shot, i: number, j: number, ignore?: PlacedDie): boolean {
  // 注意:shot.dies 经 Vue 响应式代理,必须用 uid 比较而非引用(Vue 代理 !== 原始对象)
  return shot.dies.some((d) => d.uid !== ignore?.uid && d.slot?.i === i && d.slot?.j === j)
}

export function canPlaceInShot(
  shot: Shot,
  i: number,
  j: number,
  wUm: number,
  hUm: number,
  ignore?: PlacedDie,
): boolean {
  if (i < 0 || j < 0 || i >= shot.cols || j >= shot.rows) {
    setLastRejectReason('槽位越界')
    return false
  }
  if (slotOccupied(shot, i, j, ignore)) {
    setLastRejectReason('槽位已被占用')
    return false
  }
  // 临时把候选 die 放入副本(shot 尺寸由 die 推导,必须计入;绝不能污染真实数组)
  const preset = ignore
    ? ignore.preset
    : ({ id: 'p', name: 'Die', wUm, hUm, color: '#60a5fa' } as Preset)
  const pseudo = { uid: -1, preset, col: 0, row: 0, wUm, hUm, group: null, rect: null, label: null, shot, slot: { i, j } } as unknown as PlacedDie
  const backup = shot.dies
  shot.dies = ignore ? backup.filter((d) => d.uid !== ignore.uid) : [...backup, pseudo]
  if (ignore) shot.dies.push(pseudo)
  const rects = layoutRects(shot)
  let ok = rects !== null
  if (ok && !canPlaceRect(num(shot.group.x), num(shot.group.y), shotW(shot), shotH(shot), shot)) {
    ok = false
    setLastRejectReason('Shot 扩展后与棋盘元素重叠或越界')
  }
  if (ok && rects) {
    // 全部按 uid 定位(Vue 响应式代理下引用比较不可靠)
    let mine: LRect | undefined
    for (const [k, r] of rects) {
      if ((k as PlacedDie).uid === pseudo.uid) mine = r
    }
    for (const [k, r] of rects) {
      if ((k as PlacedDie).uid === pseudo.uid) continue
      if (mine && rectsOverlap(mine, r)) {
        ok = false
        setLastRejectReason('Die 之间重叠')
        break
      }
    }
  }
  shot.dies = backup
  return ok
}

// ---------- Die 创建 ----------

export function createDieParts(preset: Preset, wUm: number, hUm: number): { group: UIGroup; rect: UIRect; label: UIText | null } {
  const group = new Group({ x: 0, y: 0, editable: true })
  const rect = new Rect({
    x: 0,
    y: 0,
    width: wUm,
    height: hUm,
    fill: preset.color,
    cornerRadius: 2,
    stroke: palette().dieStroke,
    strokeWidth: 2,
  })
  let label: UIText | null = null
  if (hUm * viewScale() >= 16) {
    label = new Text({
      text: preset.name.replace('Die ', ''),
      fill: '#0f172a',
      fontSize: Math.min(wUm, hUm) * 0.22,
      textAlign: 'center',
      verticalAlign: 'middle',
      width: wUm,
      height: hUm,
      hittable: false,
    })
  }
  group.add(rect)
  if (label) group.add(label)
  return { group, rect, label }
}

export function applyGeometry(die: PlacedDie) {
  const cell = params.cellSizeUm
  if (die.shot && die.slot) {
    const r = layoutRects(die.shot)?.get(die)
    if (r) {
      die.group.x = r.x
      die.group.y = r.y
    }
  } else {
    die.group.x = die.col * cell
    die.group.y = die.row * cell
  }
  const s = dieSize(die)
  die.rect.width = s.w
  die.rect.height = s.h
  if (die.label) {
    die.label.width = die.rect.width
    die.label.height = die.rect.height
    die.label.fontSize = cell * 0.22
    die.label.visible = (die.rect.height ?? 0) * viewScale() >= 16
  }
}

export function registerDie(die: PlacedDie) {
  die.group.on('pointerenter', () => {
    hoverHighlight(dieContentRect(die))
    setStatus(
      die.shot
        ? `${die.preset.name} @ Shot#${die.shot.uid} 槽位 (${die.slot?.i}, ${die.slot?.j})`
        : `${die.preset.name} @ (${die.col}, ${die.row})`,
    )
  })
  die.group.on('pointerleave', () => hoverHighlight(null))
  const parent = die.shot ? die.shot.group : dieLayer
  parent?.add(die.group)
  placedDies.value.push(die)
  dieMap.set(die.group, die)
}

export function placeInShot(shot: Shot, preset: Preset, i: number, j: number): PlacedDie | null {
  const wUm = round3(preset.wUm)
  const hUm = round3(preset.hUm)
  if (!canPlaceInShot(shot, i, j, wUm, hUm)) return null
  const parts = createDieParts(preset, wUm, hUm)
  const die: PlacedDie = {
    uid: setUidCounter(uidCounter + 1),
    preset,
    col: 0,
    row: 0,
    wUm,
    hUm,
    ...parts,
    shot,
    slot: { i, j },
  }
  parts.group.x = shot.padX
  parts.group.y = shot.padY
  registerDie(die)
  shot.dies.push(die)
  if (!canPlaceRect(num(shot.group.x), num(shot.group.y), shotW(shot), shotH(shot), shot)) {
    // 推导后的 shot 变大与棋盘元素冲突,撤销本次放置
    shot.dies.pop()
    dieMap.delete(die.group)
    const idx = placedDies.value.findIndex((d) => d.uid === die.uid)
    if (idx >= 0) placedDies.value.splice(idx, 1)
    shot.group.remove(die.group)
    die.group.destroy()
    setLastRejectReason('Shot 扩展后与棋盘元素重叠或越界')
    return null
  }
  refreshShotLayout(shot)
  return die
}

// 阵列填充(CAD/半导体核心操作):用模板尺寸把 Shot 内所有空槽位填满,返回成功数
export function fillShotWithTemplate(shot: Shot, preset: Preset): number {
  let n = 0
  for (let j = 0; j < shot.rows; j++) {
    for (let i = 0; i < shot.cols; i++) {
      if (placeInShot(shot, preset, i, j)) n++
    }
  }
  return n
}

export function removeDie(die: PlacedDie) {
  if (die.shot) {
    die.shot.dies = die.shot.dies.filter((d) => d.uid !== die.uid)
    die.shot.group.remove(die.group)
    refreshShotLayout(die.shot)
  } else {
    dieLayer?.remove(die.group)
  }
  die.group.destroy()
  dieMap.delete(die.group)
  placedDies.value = placedDies.value.filter((d) => d !== die)
  if (selected.value === die) selected.value = null
}

// ---------- Shot ----------

export function buildShotGrid(shot: Shot) {
  // 槽位区域棋盘格填充,便于查看阵列结构
  shot.fillLayer.destroy()
  shot.fillLayer = new Group({ hittable: false })
  for (let j = 0; j < shot.rows; j++) {
    for (let i = 0; i < shot.cols; i++) {
      const o = slotOrigin(shot, i, j)
      const span = slotSpan(shot, i, j)
      shot.fillLayer.add(
        new Rect({
          x: o.x,
          y: o.y,
          width: span.w,
          height: span.h,
          fill: (i + j) % 2 === 0 ? palette().fillA : palette().fillB,
        }),
      )
    }
  }
  shot.group.add(shot.fillLayer, 1)

  shot.gridLayer.destroy()
  shot.gridLayer = new Group({ hittable: false })
  const cell = params.cellSizeUm
  const lw = Math.max(cell * 0.007, 0.1)
  const gridColor = palette().shotGrid
  const innerX = shot.padX
  const innerY = shot.padY
  const innerW = shotW(shot) - shot.padX * 2
  const innerH = shotH(shot) - shot.padY * 2

  // 内边距范围边界(亮蓝虚线框)
  shot.gridLayer.add(
    new Rect({
      x: innerX,
      y: innerY,
      width: innerW,
      height: innerH,
      stroke: palette().shotBoundary,
      strokeWidth: lw,
      dashPattern: [cell * 0.12, cell * 0.08],
    }),
  )
  // 槽位网格线:每个单元格左右/上下两条边线(die 边缘),间隙带留空
  for (let i = 0; i < shot.cols; i++) {
    const x1 = slotOriginX(shot, i)
    const x2 = x1 + colWidth(shot, i)
    shot.gridLayer.add(new Rect({ x: x1 - lw / 2, y: innerY, width: lw, height: innerH, fill: gridColor }))
    shot.gridLayer.add(new Rect({ x: x2 - lw / 2, y: innerY, width: lw, height: innerH, fill: gridColor }))
  }
  for (let j = 0; j < shot.rows; j++) {
    const y1 = slotOriginY(shot, j)
    const y2 = y1 + rowHeight(shot, j)
    shot.gridLayer.add(new Rect({ x: innerX, y: y1 - lw / 2, width: innerW, height: lw, fill: gridColor }))
    shot.gridLayer.add(new Rect({ x: innerX, y: y2 - lw / 2, width: innerW, height: lw, fill: gridColor }))
  }
  // 网格封口线:最后一列/行的右/下边界,与内边距边界重合(等分视觉)
  const closeX = slotOriginX(shot, shot.cols - 1) + colWidth(shot, shot.cols - 1)
  const closeY = slotOriginY(shot, shot.rows - 1) + rowHeight(shot, shot.rows - 1)
  if (closeX <= innerX + innerW + 1e-6) {
    shot.gridLayer.add(
      new Rect({ x: closeX - lw / 2, y: innerY, width: lw, height: innerH, fill: gridColor }),
    )
  }
  if (closeY <= innerY + innerH + 1e-6) {
    shot.gridLayer.add(
      new Rect({ x: innerX, y: closeY - lw / 2, width: innerW, height: lw, fill: gridColor }),
    )
  }
  // 层级:body → fillLayer → gridLayer → die
  shot.group.add(shot.gridLayer, 2)
}

function updateShotBody(shot: Shot) {
  shot.body.width = shotW(shot)
  shot.body.height = shotH(shot)
}

export function createShot(x: number, y: number): Shot {
  const cell = params.cellSizeUm
  const group = new Group({ x, y, editable: true })
  const shot: Shot = {
    uid: setUidCounter(uidCounter + 1),
    group,
    body: new Rect({
      x: 0,
      y: 0,
      width: 0,
      height: 0,
      fill: palette().shotFill,
      stroke: palette().shotStroke,
      strokeWidth: Math.max(cell * 0.007, 0.1),
      dashPattern: [cell * 0.14, cell * 0.1],
    }),
    fillLayer: new Group({ hittable: false }),
    gridLayer: new Group({ hittable: false }),
    cols: 2,
    rows: 2,
    // 行业默认:刀切划片道典型 80μm(来源:IMAPS/行业资料 80–100μm)
    gapX: 80,
    gapY: 80,
    padX: 80,
    padY: 80,
    showDims: true,
    showOuter: false,
    refField: 'none',
    dies: [],
  }
  group.add(shot.body)
  group.add(shot.gridLayer)
  updateShotBody(shot)
  buildShotGrid(shot)
  group.on('pointerenter', () => {
    hoverHighlight(shotRect(shot))
    setStatus(
      `Shot#${shot.uid} · ${shot.cols}×${shot.rows} 槽位 · 外形 ${fmtUm(shotW(shot))}×${fmtUm(shotH(shot))} · 外圈 scribe ${fmtUm(shot.padX)}/${fmtUm(shot.padY)} · 内圈 scribe ${fmtUm(shot.gapX)}/${fmtUm(shot.gapY)}`,
    )
  })
  group.on('pointerleave', () => hoverHighlight(null))
  // Figma 式隔离:双击 Shot 只显示它,其余淡化(再双击或 Esc 退出)
  group.on('double_tap', () => {
    isolateShot(isolatedShotUid.value === shot.uid ? null : shot)
  })
  app?.tree.add(group)
  shots.value.push(shot)
  shotMap.set(group, shot)
  buildAllShotDims()
  focusShot(shot)
  return shot
}

// 视图聚焦到指定 Shot(创建后调用,方便继续配置 die)
export function focusShot(shot: Shot) {
  if (!app) return
  const rect = shotRect(shot)
  const { w, h } = viewSize()
  const pad = 60
  const s = Math.max(0.02, Math.min((w - pad * 2) / rect.w, (h - pad * 2) / rect.h, 4))
  app.tree.zoomLayer.scale = s
  app.tree.zoomLayer.x = w / 2 - (rect.x + rect.w / 2) * s
  app.tree.zoomLayer.y = h / 2 - (rect.y + rect.h / 2) * s
  drawRuler()
}

export function removeShot(shot: Shot) {
  if (isolatedShotUid.value === shot.uid) isolateShot(null)
  for (const die of [...shot.dies]) {
    dieMap.delete(die.group)
    placedDies.value = placedDies.value.filter((d) => d !== die)
  }
  shot.dies = []
  app?.tree.remove(shot.group)
  shot.group.destroy()
  shotMap.delete(shot.group)
  shots.value = shots.value.filter((s) => s !== shot)
  if (selectedShot.value === shot) selectedShot.value = null
}

// Figma 式隔离:只高亮被隔离的 Shot,其余淡化到 15% 透明度
export function isolateShot(shot: Shot | null) {
  isolatedShotUid.value = shot ? shot.uid : null
  for (const s of shots.value) {
    s.group.opacity = !shot || s.uid === shot.uid ? 1 : 0.15
  }
  buildAllShotDims()
  drawRuler()
  setStatus(shot ? `已隔离 Shot#${shot.uid}(双击它或按 Esc 退出)` : '已退出隔离,显示全部 Shot')
}

// 校验 Shot 内部 die 在新几何下是否都合法(不越内边距、互不重叠)
export function shotContentValid(shot: Shot): boolean {
  const rects = layoutRects(shot)
  if (!rects) return false
  const list = [...rects.values()]
  for (let a = 0; a < list.length; a++) {
    for (let b = a + 1; b < list.length; b++) {
      if (rectsOverlap(list[a], list[b])) return false
    }
  }
  return true
}

export function refreshShotLayout(shot: Shot) {
  updateShotBody(shot)
  buildShotGrid(shot)
  for (const die of shot.dies) applyGeometry(die)
  buildAllShotDims()
}

// 修改 Shot 参数:先校验新尺寸不与外部元素重叠,再重排内部 die
export function applyShotParams(
  shot: Shot,
  patch: Partial<Pick<Shot, 'cols' | 'rows' | 'gapX' | 'gapY' | 'padX' | 'padY'>>,
  onReject?: () => void,
): boolean {
  const old = {
    cols: shot.cols,
    rows: shot.rows,
    gapX: shot.gapX,
    gapY: shot.gapY,
    padX: shot.padX,
    padY: shot.padY,
  }
  Object.assign(shot, patch)

  const restore = () => {
    Object.assign(shot, { cols: old.cols, rows: old.rows, gapX: old.gapX, gapY: old.gapY, padX: old.padX, padY: old.padY })
  }

  const rect = shotRect(shot)
  if (!canPlaceRect(rect.x, rect.y, rect.w, rect.h, shot)) {
    restore()
    setStatus('Shot 新尺寸与画布上其他元素重叠或越界,已还原')
    onReject?.()
    return false
  }

  updateShotBody(shot)
  buildShotGrid(shot)

  // 重排内部 die:优先保持原槽位,放不下则找第一个合法槽位,再不行移除
  const dies = [...shot.dies]
  shot.dies = []
  let removed = 0
  for (const die of dies) {
    const { i, j } = die.slot ?? { i: 0, j: 0 }
    if (canPlaceInShot(shot, i, j, die.wUm, die.hUm, die)) {
      die.slot = { i, j }
      shot.dies.push(die)
      applyGeometry(die)
      continue
    }
    let placed = false
    for (let r = 0; r < shot.rows && !placed; r++) {
      for (let c = 0; c < shot.cols && !placed; c++) {
        if (canPlaceInShot(shot, c, r, die.wUm, die.hUm, die)) {
          die.slot = { i: c, j: r }
          shot.dies.push(die)
          applyGeometry(die)
          placed = true
        }
      }
    }
    if (!placed) {
      die.group.destroy()
      dieMap.delete(die.group)
      placedDies.value = placedDies.value.filter((d) => d !== die)
      removed++
    }
  }
  buildAllShotDims()
  if (removed) setStatus(`Shot 参数调整完成,${removed} 个放不下的 Die 已移除`)
  return true
}

export function findShotAt(x: number, y: number): Shot | null {
  for (const s of shots.value) {
    const r = shotRect(s)
    if (x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h) return s
  }
  return null
}

// 在棋盘上扫描一个能放下 w×h 的位置(格对齐)
export function scanFreeRect(w: number, h: number): { x: number; y: number } | null {
  const cell = params.cellSizeUm
  const colsMax = 64
  const rowsMax = 64
  for (let row = 0; row < rowsMax; row++) {
    for (let col = 0; col < colsMax; col++) {
      const x = col * cell
      const y = row * cell
      if (canPlaceRect(x, y, w, h)) return { x, y }
    }
  }
  return null
}

// ---------- 选中 / 删除 ----------

export function editorDies(): PlacedDie[] {
  const targets = editorTargets()
  const dies: PlacedDie[] = []
  for (const t of targets) {
    const die = dieMap.get(t as UIGroup)
    if (die && !dies.includes(die)) dies.push(die)
  }
  return dies
}

export function clearAll() {
  ;[...shots.value].forEach(removeShot)
  ;[...placedDies.value].forEach(removeDie)
  app?.editor.cancel()
  selected.value = null
  selectedShot.value = null
  updateSelectionMarker()
  rebuildPane()
  setStatus('已清空布局')
}

export function deleteSelected() {
  if (!app) return
  const targets = editorTargets()
  let removed = 0
  let removedShots = 0
  for (const t of targets) {
    const shot = shotMap.get(t as UIGroup)
    if (shot) {
      const n = shot.dies.length
      removeShot(shot)
      removedShots++
      removed += n
      continue
    }
    const die = dieMap.get(t as UIGroup)
    if (die) {
      removeDie(die)
      removed++
    }
  }
  if (removed || removedShots) {
    app.editor.cancel()
    selected.value = null
    selectedShot.value = null
    updateSelectionMarker()
    setStatus(`已删除 ${removedShots} 个 Shot(含 ${removed} 个 Die)`)
  }
  rebuildPane()
}

export function updateSelectedRefs() {
  const targets = editorTargets()
  selected.value = null
  selectedShot.value = null
  for (const t of targets) {
    const die = dieMap.get(t as UIGroup)
    if (die) selected.value = die
    const shot = shotMap.get(t as UIGroup)
    if (shot) selectedShot.value = shot
  }
  rebuildPane()
  drawRuler()
}

// ---------- 吸附 ----------


// 提交尺寸变更(校验,失败还原),返回是否成功
export function commitDieResize(die: PlacedDie, newW: number, newH: number): boolean {
  const s = die.shot
  if (!s) return false
  const oldW = die.wUm
  const oldH = die.hUm
  die.wUm = newW
  die.hUm = newH
  const slot = die.slot ?? { i: 0, j: 0 }
  if (canPlaceInShot(s, slot.i, slot.j, newW, newH, die)) {
    refreshShotLayout(s)
    app?.editor.update()
    setStatus(`尺寸已调整为 ${fmtUm(newW)} × ${fmtUm(newH)}`)
    return true
  }
  die.wUm = oldW
  die.hUm = oldH
  applyGeometry(die)
  setStatus('调整后放不下(与棋盘元素重叠),已还原')
  return false
}

// ---------- 吸附 ----------

export function snapAfterEditorMove() {
  if (!app) return
  const targets = editorTargets()
  let moved = 0
  for (const t of targets) {
    const shot = shotMap.get(t as UIGroup)
    if (shot) {
      // Shot 尺寸由内部 die 推导,控制点缩放一律还原
      if (Math.abs(num(shot.group.scaleX, 1) - 1) > 1e-6 || Math.abs(num(shot.group.scaleY, 1) - 1) > 1e-6) {
        shot.group.scaleX = 1
        shot.group.scaleY = 1
        setStatus('Shot 尺寸由内部 Die 推导,不可直接缩放(请调整 Die 尺寸)')
        updateSelectedRefs()
        updateSelectionMarker()
        return
      }
      const cell = params.cellSizeUm
      const nx = Math.round(num(shot.group.x) / cell) * cell
      const ny = Math.round(num(shot.group.y) / cell) * cell
      if (canPlaceRect(nx, ny, shotW(shot), shotH(shot), shot)) {
        shot.group.x = nx
        shot.group.y = ny
        buildAllShotDims()
        moved++
      } else {
        setStatus('Shot 目标位置越界或重叠,已还原')
        updateSelectedRefs()
        updateSelectionMarker()
        return
      }
      continue
    }
    const die = dieMap.get(t as UIGroup)
    if (!die) continue
    if (die.shot) {
      const s = die.shot
      // resize 路径:编辑器控制点缩放 → 换算回实际尺寸(3 位小数),走派生模型
      const gsx = num(die.group.scaleX, 1)
      const gsy = num(die.group.scaleY, 1)
      if (Math.abs(gsx - 1) > 0.005 || Math.abs(gsy - 1) > 0.005) {
        const newW = Math.max(0.001, round3(die.wUm * gsx))
        const newH = Math.max(0.001, round3(die.hUm * gsy))
        die.group.scaleX = 1
        die.group.scaleY = 1
        commitDieResize(die, newW, newH)
        moved++
        continue
      }
      // shot 内部:吸附到最近槽位
      const { i, j } = nearestSlot(s, num(die.group.x), num(die.group.y))
      if (canPlaceInShot(s, i, j, die.wUm, die.hUm, die)) {
        die.slot = { i, j }
        applyGeometry(die)
        buildAllShotDims()
        moved++
      } else {
        applyGeometry(die)
        setStatus('槽位已被占用或空间不足,已还原')
        updateSelectedRefs()
        updateSelectionMarker()
        return
      }
    } else {
      const cell = params.cellSizeUm
      const col = Math.round(num(die.group.x) / cell)
      const row = Math.round(num(die.group.y) / cell)
      if (canPlaceBoardDie(col, row, die.wUm, die.hUm, die)) {
        die.col = col
        die.row = row
        applyGeometry(die)
        moved++
      } else {
        applyGeometry(die)
        setStatus('目标位置越界或重叠,已还原')
        updateSelectedRefs()
        updateSelectionMarker()
        return
      }
    }
  }
  if (moved) setStatus(`已吸附对齐(${moved} 个)`)
  updateSelectedRefs()
  updateSelectionMarker()
}

// 复制选中 die(同 shot 找第一个空槽,保留实际尺寸)
export function duplicateSelected(): number {
  let n = 0
  for (const die of editorDies()) {
    if (!die.shot) continue
    const s = die.shot
    const preset: Preset = { ...die.preset, wUm: die.wUm, hUm: die.hUm }
    let placed: PlacedDie | null = null
    for (let j = 0; j < s.rows && !placed; j++) {
      for (let i = 0; i < s.cols && !placed; i++) {
        placed = placeInShot(s, preset, i, j)
      }
    }
    if (placed) n++
  }
  return n
}

export function copySelected() {
  setClipboard(
    editorDies().map((d) => ({
      preset: { ...d.preset, wUm: d.wUm, hUm: d.hUm },
      wUm: d.wUm,
      hUm: d.hUm,
    })),
  )
  if (clipboard.length) setStatus(`已复制 ${clipboard.length} 个 Die`)
}

export function pasteClipboard(): number {
  let n = 0
  for (const item of clipboard) {
    let placed: PlacedDie | null = null
    for (const shot of shots.value) {
      for (let j = 0; j < shot.rows && !placed; j++) {
        for (let i = 0; i < shot.cols && !placed; i++) {
          placed = placeInShot(shot, item.preset, i, j)
        }
      }
      if (placed) break
    }
    if (placed) n++
  }
  return n
}

// 拖动实时读数(CAD 状态行)
export function onEditorMove(e: EditorMoveEvent) {
  const dx = num((e as unknown as { moveX?: number }).moveX)
  const dy = num((e as unknown as { moveY?: number }).moveY)
  setStatus(`移动 ΔX ${fmtUm(dx)} · ΔY ${fmtUm(dy)}(松手自动吸附)`)
}

// 右键上下文菜单
export function onCanvasContextMenu(e: MouseEvent) {
  e.preventDefault()
  if (!app) return
  const t = (app.editor.hoverTarget ?? app.editor.target) as unknown
  const die = dieMap.get(t as UIGroup) ?? null
  const shot = shotMap.get(t as UIGroup) ?? null
  if (!die && !shot) {
    ctxMenu.value = { x: e.clientX, y: e.clientY, die: null, shot: null }
    return
  }
  if (die) app.editor.select(die.group)
  else if (shot) app.editor.select(shot.group)
  ctxMenu.value = { x: e.clientX, y: e.clientY, die, shot }
}

export function closeCtxMenu() {
  if (ctxMenu.value) ctxMenu.value = null
}

export function ctxAction(action: 'duplicate' | 'fillShot' | 'saveTpl' | 'delete' | 'selectAll' | 'cancel') {
  const m = ctxMenu.value
  ctxMenu.value = null
  if (!app) return
  if (action === 'duplicate') {
    const n = duplicateSelected()
    setStatus(n ? `已复制 ${n} 个 Die 到空槽位` : '没有空槽位')
  } else if (action === 'fillShot' && m?.die) {
    const d = m.die
    if (!d.shot) {
      setStatus('该 Die 不在 Shot 内,无法填充')
      return
    }
    const preset: Preset = { ...d.preset, wUm: d.wUm, hUm: d.hUm }
    const n = fillShotWithTemplate(d.shot, preset)
    setStatus(n ? `已用 ${preset.name} 填充 ${n} 个空槽位` : '没有可填充的空槽位')
  } else if (action === 'saveTpl' && m?.die) {
    const d = m.die
    const tpl: Preset = {
      id: `custom-${Date.now()}`,
      name: d.preset.name,
      wUm: round3(d.wUm),
      hUm: round3(d.hUm),
      color: d.preset.color,
    }
    dieLibrary.value.push(tpl)
    saveLibrary()
    setStatus(`已保存模板 ${tpl.name}(${fmtUm(tpl.wUm)} × ${fmtUm(tpl.hUm)})`)
  } else if (action === 'delete') {
    deleteSelected()
  } else if (action === 'selectAll') {
    app.editor.select([...dieMap.keys(), ...shotMap.keys()])
  } else if (action === 'cancel') {
    app.editor.cancel()
  }
}

export function onKeyDown(e: KeyboardEvent) {
  if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return
  const mod = e.ctrlKey || e.metaKey
  if (e.key === 'Delete' || e.key === 'Backspace') {
    deleteSelected()
  } else if (mod && (e.key === 'd' || e.key === 'D')) {
    e.preventDefault()
    const n = duplicateSelected()
    setStatus(n ? `已复制 ${n} 个 Die 到空槽位` : '没有可复制的 Die 或没有空槽位')
  } else if (mod && (e.key === 'c' || e.key === 'C')) {
    copySelected()
  } else if (mod && (e.key === 'v' || e.key === 'V')) {
    e.preventDefault()
    const n = pasteClipboard()
    setStatus(n ? `已粘贴 ${n} 个 Die` : '剪贴板为空或没有空槽位')
  } else if (mod && (e.key === 'a' || e.key === 'A')) {
    e.preventDefault()
    if (app) app.editor.select([...dieMap.keys(), ...shotMap.keys()])
  } else if (e.key === 'Escape') {
    ctxMenu.value = null
    // 优先退出 Shot 隔离,再取消编辑器选择
    if (isolatedShotUid.value !== null) isolateShot(null)
    else app?.editor.cancel()
  }
}

export function onKeyUp(e: KeyboardEvent) {
  if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return
  if (e.key.startsWith('Arrow')) {
    snapAfterEditorMove()
  }
}


// ---------- Die 手柄 resize(DOM 覆盖层驱动;leafer 内置 resize 已关闭) ----------

let handleDrag: {
  die: PlacedDie
  corner: string
  startW: number
  startH: number
  startClientX: number
  startClientY: number
} | null = null

export function startHandleResize(die: PlacedDie, corner: string, clientX: number, clientY: number) {
  handleDrag = { die, corner, startW: die.wUm, startH: die.hUm, startClientX: clientX, startClientY: clientY }
  window.addEventListener('pointermove', onHandleMove)
  window.addEventListener('pointerup', onHandleUp, { once: true })
}

function handleDelta(e: PointerEvent): { newW: number; newH: number } | null {
  const h = handleDrag
  if (!h || !app) return null
  const s = num(app.tree.zoomLayer.scale, 1)
  const dxUm = (e.clientX - h.startClientX) / s
  const dyUm = (e.clientY - h.startClientY) / s
  const signW = h.corner.includes('right') ? 1 : -1
  const signH = h.corner.includes('bottom') ? 1 : -1
  return {
    newW: Math.max(0.001, round3(h.startW + signW * dxUm)),
    newH: Math.max(0.001, round3(h.startH + signH * dyUm)),
  }
}

function onHandleMove(e: PointerEvent) {
  const h = handleDrag
  const d = handleDelta(e)
  if (!h || !d) return
  h.die.group.scaleX = d.newW / h.startW
  h.die.group.scaleY = d.newH / h.startH
  setStatus(`resize → ${fmtUm(d.newW)} × ${fmtUm(d.newH)}`)
}

function onHandleUp(e: PointerEvent) {
  window.removeEventListener('pointermove', onHandleMove)
  const h = handleDrag
  const d = handleDelta(e)
  handleDrag = null
  if (!h || !d) return
  h.die.group.scaleX = 1
  h.die.group.scaleY = 1
  commitDieResize(h.die, d.newW, d.newH)
}
