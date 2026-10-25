<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { App, Group, Rect, MoveEvent, ZoomEvent, EditorMoveEvent } from 'leafer-editor'
import { theme, toggleTheme, palette } from '../theme'
import { SHOT_CARD } from '../editor/types'
import type { UIGroup } from '../editor/types'
import {
  params,
  containerRef,
  viewHostRef,
  placedDies,
  shots,
  zoomPct,
  ctxMenu,
  dragState,
  dropHint,
  paneRef,
  paneLeftRef,
  pane,
  paneLeft,
  app,
  dieLayer,
  markerLayer,
  previewLayer,
  shotDimsLayer,
  resizeObserver,
  lastRejectReason,
  dieMap,
  shotMap,
  setApp,
  setDieLayer,
  setMarkerLayer,
  setPreviewLayer,
  setShotDimsLayer,
  setResizeObserver,
  setPane,
  setPaneLeft,
  setLastRejectReason,
  setLastMouse,
  num,
  fmtUm,
  setStatus,
  resizeHandles,
  selected,
} from '../editor/state'
import { dieLibrary, presetName, presetColor, ghostSize } from '../editor/library'
import { slotOrigin, layoutRects, nearestSlot, shotW, shotH } from '../editor/geometry'
import {
  canPlaceRect,
  canPlaceInShot,
  placeInShot,
  createShot,
  findShotAt,
  scanFreeRect,
  refreshShotLayout,
  snapAfterEditorMove,
  startHandleResize,
  fillShotWithTemplate,
  isolateShot,
  onCanvasContextMenu,
  closeCtxMenu,
  ctxAction,
  onKeyDown,
  onKeyUp,
  onEditorMove,
} from '../editor/ops'
import { drawRuler, applyCanvasTheme, fitView, zoomBy } from '../editor/draw'
import { rebuildPane, buildLeftPane } from '../editor/panes'

// 模板 string ref(pane 容器,由 panes.ts 读取)
void paneRef
void paneLeftRef

// ---------- 拖放 ----------

function isShotCard(cardId: string): boolean {
  return cardId === SHOT_CARD.id
}

function onPresetPointerDown(e: PointerEvent, cardId: string) {
  if (e.button !== 0) return
  e.preventDefault()
  dragState.value = {
    cardId,
    x: e.clientX,
    y: e.clientY,
    startX: e.clientX,
    startY: e.clientY,
    moved: false,
  }
  window.addEventListener('pointermove', onPresetDragMove)
  window.addEventListener('pointerup', onPresetDragEnd, { once: true })
  window.addEventListener('pointercancel', onPresetDragCancel, { once: true })
}

function onPresetDragMove(e: PointerEvent) {
  const state = dragState.value
  if (!state) return
  state.x = e.clientX
  state.y = e.clientY
  if (Math.hypot(e.clientX - state.startX, e.clientY - state.startY) > 5) {
    state.moved = true
  }
}

function endPresetDrag() {
  window.removeEventListener('pointermove', onPresetDragMove)
  window.removeEventListener('pointercancel', onPresetDragCancel)
  const state = dragState.value
  dragState.value = null
  return state
}

function onPresetDragCancel() {
  endPresetDrag()
  clearDropHint()
}

function clearDropHint() {
  dropHint.value = null
  dropLabel.value = ''
  if (previewLayer) previewLayer.removeAll(true)
}

// CAD 风格拖放浮签:跟随拖拽,实时显示落点槽位/拒绝原因
const dropLabel = ref('')

function updateDropHint(pageX: number, pageY: number) {
  const state = dragState.value
  if (!state || !state.moved) return
  const cell = params.cellSizeUm

  if (isShotCard(state.cardId)) {
    const probe = createShotProbe()
    const x = Math.round(pageX / cell) * cell
    const y = Math.round(pageY / cell) * cell
    const valid = canPlaceRect(x, y, probe.w, probe.h)
    dropHint.value = { x, y, w: probe.w, h: probe.h, valid }
    dropLabel.value = valid ? '创建 Shot 于此' : '位置被占或越界'
  } else {
    const preset = dieLibrary.value.find((p) => p.id === state.cardId)
    if (!preset) return
    const shot = findShotAt(pageX, pageY)
    if (shot) {
      const { i, j } = nearestSlot(shot, pageX - num(shot.group.x), pageY - num(shot.group.y))
      const box = layoutRects(shot, {
        slot: { i, j },
        wUm: preset.wUm,
        hUm: preset.hUm,
      })?.get('candidate')
      const origin = slotOrigin(shot, i, j)
      const valid = canPlaceInShot(shot, i, j, preset.wUm, preset.hUm)
      dropHint.value = {
        x: num(shot.group.x) + (box?.x ?? origin.x),
        y: num(shot.group.y) + (box?.y ?? origin.y),
        w: box?.w ?? preset.wUm,
        h: box?.h ?? preset.hUm,
        valid,
      }
      dropLabel.value = valid ? `槽位 (${i}, ${j}) · 松手放置` : `✕ ${lastRejectReason || '无法放置'}`
    } else {
      // die 只能放入 shot 内,棋盘区域预览无效(红色)
      const col = Math.floor(pageX / cell)
      const row = Math.floor(pageY / cell)
      dropHint.value = {
        x: col * cell,
        y: row * cell,
        w: preset.wUm,
        h: preset.hUm,
        valid: false,
      }
      dropLabel.value = '✕ Die 只能放入 Shot 内'
    }
  }
  renderDropHint()
}

function createShotProbe(): { w: number; h: number } {
  // 空 shot:列宽=cell,2×2 槽位,gap=0 → pad×2 + 2×cell
  const cell = params.cellSizeUm
  const pad = cell / 2
  return { w: pad * 2 + cell * 2, h: pad * 2 + cell * 2 }
}

function renderDropHint() {
  if (!app) return
  if (!previewLayer) {
    setPreviewLayer(new Group({ hittable: false }))
    app!.tree.add(previewLayer!)
  }
  previewLayer!.removeAll(true)
  const hint = dropHint.value
  if (!hint) return
  previewLayer!.add(
    new Rect({
      x: hint.x,
      y: hint.y,
      width: hint.w,
      height: hint.h,
      fill: hint.valid ? 'rgba(74,222,128,0.25)' : 'rgba(248,113,113,0.25)',
      stroke: hint.valid ? '#4ade80' : '#f87171',
      strokeWidth: Math.max(hint.w / 300, 1),
    }),
  )
}

function onPresetDragEnd(e: PointerEvent) {
  const state = endPresetDrag()
  clearDropHint()
  if (!state) return

  if (!state.moved) {
    autoPlaceCard(state.cardId)
    return
  }

  const container = containerRef.value
  if (!app || !container) return
  const rect = container.getBoundingClientRect()
  const inside =
    e.clientX >= rect.left &&
    e.clientX <= rect.right &&
    e.clientY >= rect.top &&
    e.clientY <= rect.bottom
  if (!inside) {
    setStatus('未放入画布,已取消')
    return
  }

  const page = app.tree.getPagePointByClient({ clientX: e.clientX, clientY: e.clientY })
  const cell = params.cellSizeUm

  if (isShotCard(state.cardId)) {
    const probe = createShotProbe()
    const x = Math.round(page.x / cell) * cell
    const y = Math.round(page.y / cell) * cell
    if (canPlaceRect(x, y, probe.w, probe.h)) {
      const shot = createShot(x, y)
      setStatus(`已创建 Shot#${shot.uid}(${shot.cols}×${shot.rows} 槽位)`)
    } else {
      // 落点被占,尝试自动找位
      const free = scanFreeRect(probe.w, probe.h)
      if (free) {
        const shot = createShot(free.x, free.y)
        setStatus(`落点被占,Shot#${shot.uid} 已放到最近空位`)
      } else {
        setStatus('棋盘上没有能放下 Shot 的空位')
      }
    }
    return
  }

  const preset = dieLibrary.value.find((p) => p.id === state.cardId)
  if (!preset) return

  const shot = findShotAt(page.x, page.y)
  if (shot) {
    const { i, j } = nearestSlot(shot, page.x - num(shot.group.x), page.y - num(shot.group.y))
    if (placeInShot(shot, preset, i, j)) {
      setStatus(`已把 ${preset.name} 放入 Shot#${shot.uid} 槽位 (${i}, ${j})`)
    } else {
      setStatus(`无法放入:${lastRejectReason || '空间不足'}(一个槽位只能放一个 Die)`)
    }
    return
  }

  setLastRejectReason('')
  setStatus('Die 只能放入 Shot 槽位,请先拖入一个 Shot 容器(Die 不能直接放在棋盘)')
}

function autoPlaceCard(cardId: string) {
  if (isShotCard(cardId)) {
    const probe = createShotProbe()
    const free = scanFreeRect(probe.w, probe.h)
    if (free) {
      const shot = createShot(free.x, free.y)
      setStatus(`已创建 Shot#${shot.uid}(${shot.cols}×${shot.rows} 槽位)`)
    } else {
      setStatus('棋盘上没有能放下 Shot 的空位')
    }
    return
  }
  const preset = dieLibrary.value.find((p) => p.id === cardId)
  if (!preset) return
  // die 只能放入 shot:依次在已有 shot 中找第一个空槽位
  const wUm = preset.wUm
  const hUm = preset.hUm
  for (const shot of shots.value) {
    for (let j = 0; j < shot.rows; j++) {
      for (let i = 0; i < shot.cols; i++) {
        if (canPlaceInShot(shot, i, j, wUm, hUm)) {
          placeInShot(shot, preset, i, j)
          setStatus(`已把 ${preset.name} 放入 Shot#${shot.uid} 槽位 (${i}, ${j})`)
          return
        }
      }
    }
  }
  setStatus(`没有可用的 Shot 槽位(Die 只能放入 Shot 内,请先创建 Shot)`)
}

function onResizeHandleDown(corner: string, e: PointerEvent) {
  const die = selected.value
  if (!die) return
  e.stopPropagation()
  e.preventDefault()
  startHandleResize(die, corner, e.clientX, e.clientY)
}

watch(theme, () => {
  applyCanvasTheme()
  setStatus(theme.value === 'dark' ? '已切换深色主题' : '已切换浅色主题')
})

watch(
  () => params.cellSizeUm,
  () => {
    for (const shot of [...shots.value]) refreshShotLayout(shot)
    drawRuler()
    setStatus('默认 Die 尺寸已更新,Shot 网格已重排')
  },
)

// 把运行时异常显示到状态栏,便于定位问题
function onWindowError(e: ErrorEvent) {
  setStatus(`⚠ 错误: ${e.message}`)
}

function onCanvasPointerMove(e: PointerEvent) {
  const rect = containerRef.value?.getBoundingClientRect()
  if (!rect) return
  setLastMouse(e.clientX - rect.left, e.clientY - rect.top)
  if (dragState.value?.moved && app) {
    const page = app.tree.getPagePointByClient({ clientX: e.clientX, clientY: e.clientY })
    updateDropHint(page.x, page.y)
  }
  drawRuler()
}

function onCanvasPointerLeave() {
  setLastMouse(-1, -1)
  drawRuler()
}

onMounted(() => {
  const container = containerRef.value
  const viewHost = viewHostRef.value
  if (!container || !viewHost) return

  // leafer 必须挂在显式尺寸的独立宿主上:外层 flex 容器会导致编辑器控制点拾取失效(逐项对照实验证实)
  setApp(new App({
    view: viewHost,
    editor: {
      // 内置 resize 控制点在本应用环境下拾取失效且无法根治,已关闭;
      // 改用手柄(DOM 覆盖层,样式对齐 leafer 原生控制点)驱动同一套 resize 派生逻辑
      resizeable: false,
      rotateable: false,
      skewable: false,
      flipable: false,
    },
    tree: { type: 'design' },
    fill: palette().canvasBg,
    wheel: { preventDefault: true },
    touch: { preventDefault: true },
    pointer: { preventDefaultMenu: true },
    zoom: { min: 0.02, max: 64 },
  }))

  setDieLayer(new Group())
  app!.tree.add(dieLayer!)
  setPreviewLayer(new Group({ hittable: false }))
  app!.tree.add(previewLayer!)
  setShotDimsLayer(new Group({ hittable: false }))
  app!.tree.add(shotDimsLayer!)
  setMarkerLayer(new Group({ hittable: false }))
  app!.tree.add(markerLayer!)

  fitView()
  rebuildPane()

  buildLeftPane()

  app!.tree.on(MoveEvent.MOVE, drawRuler)
  app!.tree.on(ZoomEvent.ZOOM, drawRuler)

  container.addEventListener('pointerup', snapAfterEditorMove)
  container.addEventListener('pointermove', onCanvasPointerMove)
  container.addEventListener('pointerleave', onCanvasPointerLeave)
  container.addEventListener('contextmenu', onCanvasContextMenu)
  window.addEventListener('pointerdown', closeCtxMenu)
  ;(app!.editor as unknown as { on(t: string, f: (e: EditorMoveEvent) => void): void }).on(
    EditorMoveEvent.MOVE,
    onEditorMove,
  )
  window.addEventListener('keydown', onKeyDown)
  window.addEventListener('keyup', onKeyUp)
  window.addEventListener('error', onWindowError)

  setResizeObserver(new ResizeObserver(() => {
    if (!app) return
    app.resize({ width: viewHost.clientWidth, height: viewHost.clientHeight })
    fitView()
  }))
  resizeObserver!.observe(viewHost)
})

// dev 调试钩子(仅开发环境):供端到端测试驱动
if (import.meta.env.DEV) {
  ;(window as unknown as Record<string, unknown>).__dl = {
    autoPlaceShot: () => autoPlaceCard(SHOT_CARD.id),
    autoPlaceDie: () => autoPlaceCard(dieLibrary.value[0].id),
    state: () => ({
      shots: shots.value.length,
      dies: placedDies.value.length,
      shotDies: shots.value.map((s) => ({ uid: s.uid, cols: s.cols, rows: s.rows, n: s.dies.length, w: 0, opacity: num(s.group.opacity, 1) })),
      dieSizes: placedDies.value.map((d) => ({ w: d.wUm, h: d.hUm, slot: d.slot, shotUid: d.shot?.uid })),
      zoom: app ? { s: num(app.tree.zoomLayer.scale), x: num(app.tree.zoomLayer.x), y: num(app.tree.zoomLayer.y) } : null,
      firstDieRect: placedDies.value[0]
        ? (() => {
            const d = placedDies.value[0]
            const g = d.group
            return { x: num(g.x), y: num(g.y), w: num(d.rect.width), h: num(d.rect.height), sx: num(g.scaleX, 1), sy: num(g.scaleY, 1) }
          })()
        : null,
      shotRect: shots.value[0]
        ? (() => {
            const s = shots.value[0]
            return { x: num(s.group.x), y: num(s.group.y), w: shotW(s), h: shotH(s) }
          })()
        : null,

      editorTarget: app?.editor.target ? (dieMap.has(app.editor.target as UIGroup) ? 'die' : shotMap.has(app.editor.target as UIGroup) ? 'shot' : 'other') : null,
      // die 右下角的世界/屏幕坐标(取第一个 die)
      dieHandle: placedDies.value[0]
        ? (() => {
            const d = placedDies.value[0]
            const s = d.shot!
            const wx = num(s.group.x) + num(d.group.x) + num(d.rect.width) * num(d.group.scaleX, 1)
            const wy = num(s.group.y) + num(d.group.y) + num(d.rect.height) * num(d.group.scaleY, 1)
            const client = app!.tree.getClientPointByWorld({ x: wx, y: wy })
            return { wx, wy, cx: client.x, cy: client.y }
          })()
        : null,
    }),
    // 编辑框控制点(世界+屏幕坐标)
    editPoints: () => {
      if (!app) return null
      const eb = (app.editor as unknown as { editBox?: { points?: { x: number; y: number; pointType?: string; name?: string }[] } }).editBox
      if (!eb?.points) return null
      return eb.points.map((pt) => {
        const c = app!.tree.getClientPointByWorld({ x: pt.x, y: pt.y })
        return { x: pt.x, y: pt.y, cx: c.x, cy: c.y, type: pt.pointType ?? pt.name }
      })
    },
    appRef: () => app,
    // 直接触发 pointerup 吸附/resize 处理(绕过编辑器手势,验证换算逻辑)
    snap: () => snapAfterEditorMove(),
    // 阵列填充(用 shot 内首颗 die 的模板/尺寸,无 die 时用模板库第一个)
    fill: () => {
      const s = shots.value[0]
      if (!s) return 0
      const d = s.dies[0]
      const preset = d ? { ...d.preset, wUm: d.wUm, hUm: d.hUm } : dieLibrary.value[0]
      return fillShotWithTemplate(s, preset)
    },
    // 隔离/退出隔离(供 E2E;UI 入口是双击 Shot)
    isolate: (uid: number | null) =>
      isolateShot(uid === null ? null : (shots.value.find((s) => s.uid === uid) ?? null)),
    setDieScale: (sx: number, sy: number) => {
      const d = placedDies.value[0]
      if (d) {
        d.group.scaleX = sx
        d.group.scaleY = sy
      }
    },
  }
}

onUnmounted(() => {
  endPresetDrag()
  window.removeEventListener('keydown', onKeyDown)
  window.removeEventListener('keyup', onKeyUp)
  window.removeEventListener('error', onWindowError)
  window.removeEventListener('pointerdown', closeCtxMenu)
  ;(app?.editor as unknown as { off(t: string, f: (e: EditorMoveEvent) => void): void })?.off?.(
    EditorMoveEvent.MOVE,
    onEditorMove,
  )
  pane?.dispose()
  setPane(null)
  paneLeft?.dispose()
  setPaneLeft(null)
  if (app) {
    app.tree.off(MoveEvent.MOVE, drawRuler)
    app.tree.off(ZoomEvent.ZOOM, drawRuler)
  }
  resizeObserver?.disconnect()
  setResizeObserver(null)
  if (app) {
    app.destroy()
    setApp(null)
  }
  dieMap.clear()
  shotMap.clear()
})
</script>

<template>
  <div class="die-layout absolute inset-0 flex flex-col bg-bg font-[Arial,sans-serif]" :data-theme="theme">
    <header class="flex items-center gap-4 border-b border-input px-3 py-2 bg-panel text-fg">
      <span class="whitespace-nowrap font-bold">Leafer Editor</span>
      <!-- <span class="status overflow-hidden text-sm text-sky-400 text-ellipsis whitespace-nowrap">{{ status }}</span> -->
      <button
        class="ml-1 h-7 w-7 cursor-pointer rounded-md border border-iborder bg-input text-sm leading-none text-fg hover:bg-btnhover"
        :title="theme === 'dark' ? '切换浅色主题' : '切换深色主题'"
        @click="toggleTheme"
      >
        {{ theme === 'dark' ? '☀' : '☾' }}
      </button>
    </header>
    <div class="flex flex-1 overflow-hidden">
      <aside class="sidebar left w-60 shrink-0 overflow-y-auto border-r border-input bg-panel p-4 text-fg">
        <div ref="paneLeftRef" class="pane-host w-full" />
        <div class="mb-6">
          <h3 class="m-0 mb-3 border-b border-iborder pb-1.5 text-[11px] font-semibold tracking-wide text-[var(--tp-label-foreground-color)] uppercase">小 Die 库(拖入 Shot / 点击放置)</h3>
          <div
            v-for="t in dieLibrary"
            :key="t.id"
            class="mb-1 flex cursor-grab items-center gap-2 rounded border border-[var(--tp-groove-foreground-color)] bg-[var(--tp-input-background-color)] px-2 py-[3px] text-[11px] text-fg select-none hover:border-sky-400 active:cursor-grabbing"
            :title="`${t.name} · ${fmtUm(t.wUm)} × ${fmtUm(t.hUm)} · 颜色 ${t.color}`"
            @pointerdown="onPresetPointerDown($event, t.id)"
          >
            <span class="h-3 w-3 shrink-0 rounded-[2px]" :style="{ background: t.color }" />
            <span class="overflow-hidden text-ellipsis whitespace-nowrap">{{ t.name }}</span>
          </div>
        </div>

        <div class="mb-6">
          <h3 class="m-0 mb-3 border-b border-iborder pb-1.5 text-[11px] font-semibold tracking-wide text-[var(--tp-label-foreground-color)] uppercase">Shot 容器</h3>
          <div
            class="mb-2 flex cursor-grab items-center gap-2.5 rounded border border-sky-400! border-dashed! bg-[var(--tp-input-background-color)] p-2 text-[11px] select-none hover:border-sky-400 active:cursor-grabbing"
            title="Shot(曝光场)容器:拖入画布创建,视图自动聚焦;默认 2×2 槽位、scribe 80μm;槽位数/内圈/外圈 scribe/参考曝光场在属性面板配置"
            @pointerdown="onPresetPointerDown($event, SHOT_CARD.id)"
          >
            <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded border border-dashed border-sky-400 text-[22px] text-sky-400">▦</div>
            <div class="flex min-w-0 flex-col gap-0.5">
              <span class="text-xs">Shot 容器</span>
            </div>
            <span class="ml-auto text-[10px] text-dim whitespace-nowrap">拖入 / 点击</span>
          </div>
        </div>

      </aside>

      <div
        ref="containerRef"
        class="layout-canvas relative flex-1 overflow-hidden"
        :class="{ 'drop-target': dragState?.moved }"
      >
        <div ref="viewHostRef" class="absolute top-0 left-0 h-full w-full" />

        <div
          v-for="h in resizeHandles"
          :key="h.key"
          class="resize-handle"
          :style="{ left: `${h.x}px`, top: `${h.y}px`, cursor: h.cursor }"
          @pointerdown="onResizeHandleDown(h.key, $event)"
        />
        <div class="absolute right-4 bottom-4 z-[5] flex items-center gap-0.5 rounded-lg border border-iborder bg-panel px-1.5 py-1 shadow-lg">
          <button class="tb-btn" title="缩小" aria-label="缩小" @click="zoomBy(0.8)">
            <svg viewBox="0 0 16 16" width="14" height="14"><path d="M3 8h10" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" /></svg>
          </button>
          <span class="min-w-11 text-center text-[11px] text-dim tabular-nums">{{ zoomPct }}%</span>
          <button class="tb-btn" title="放大" aria-label="放大" @click="zoomBy(1.25)">
            <svg viewBox="0 0 16 16" width="14" height="14"><path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" /></svg>
          </button>
          <span class="mx-1 inline-block h-4 w-px bg-iborder" />
          <button class="tb-btn" title="自适应(居中)" aria-label="自适应(居中)" @click="fitView">
            <svg viewBox="0 0 16 16" width="14" height="14"><path d="M2 6V2h4M10 2h4v4M14 10v4h-4M6 14H2v-4" stroke="currentColor" stroke-width="1.5" fill="none" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </button>
        </div>
      </div>

      <div
        v-if="ctxMenu"
        class="fixed z-[200] flex min-w-[150px] flex-col rounded-lg border border-iborder bg-panel p-1 shadow-xl"
        :style="{ left: `${ctxMenu.x}px`, top: `${ctxMenu.y}px` }"
      >
        <button v-if="ctxMenu.die" class="ctx-item" @click="ctxAction('duplicate')">复制 Die(Ctrl+D)</button>
        <button v-if="ctxMenu.die" class="ctx-item" @click="ctxAction('fillShot')">填充 Shot 空槽(用此 Die)</button>
        <button v-if="ctxMenu.die" class="ctx-item" @click="ctxAction('saveTpl')">保存为模板</button>
        <button v-if="ctxMenu.die || ctxMenu.shot" class="ctx-item danger" @click="ctxAction('delete')">
          删除(Delete)
        </button>
        <span v-if="ctxMenu.die || ctxMenu.shot" class="mx-1.5 my-[3px] h-px bg-iborder" />
        <button class="ctx-item" @click="ctxAction('selectAll')">全选(Ctrl+A)</button>
        <button class="ctx-item" @click="ctxAction('cancel')">取消选择(Esc)</button>
      </div>

      <div
        v-if="dragState && dragState.moved"
        class="pointer-events-none fixed z-[100] flex translate-x-3 translate-y-3 items-center justify-center rounded text-[11px] font-bold text-panel opacity-75"
        :style="{
          left: `${dragState.x}px`,
          top: `${dragState.y}px`,
          width: isShotCard(dragState.cardId) ? '44px' : `${ghostSize(dragState.cardId).w}px`,
          height: isShotCard(dragState.cardId) ? '44px' : `${ghostSize(dragState.cardId).h}px`,
          background: isShotCard(dragState.cardId) ? 'rgba(56,189,248,0.5)' : presetColor(dragState.cardId),
        }"
      >
        <span>{{ isShotCard(dragState.cardId) ? 'Shot' : presetName(dragState.cardId) }}</span>
      </div>
      <div
        v-if="dragState && dragState.moved && dropLabel"
        class="pointer-events-none fixed z-[101] translate-x-4 translate-y-11 whitespace-nowrap rounded border bg-panel px-2 py-0.5 text-[11px] font-semibold shadow-lg"
        :class="dropHint?.valid ? 'border-green-400 text-green-600' : 'border-red-400 text-red-600'"
        :style="{ left: `${dragState.x}px`, top: `${dragState.y}px` }"
      >
        {{ dropLabel }}
      </div>

      <aside class="sidebar right w-60 shrink-0 overflow-y-auto border-l border-input bg-panel p-4 text-fg">
        <div ref="paneRef" class="pane-host w-full" />
      </aside>
    </div>
  </div>
</template>


<style scoped>
/* 拖放悬停时画布高亮边框 */
.layout-canvas.drop-target {
  outline: 2px dashed #38bdf8;
  outline-offset: -4px;
}

/* resize 手柄:样式参考 leafer 原生控制点(pointFill #fff / stroke #836DFF / pointSize 10) */
.resize-handle {
  position: absolute;
  width: 10px;
  height: 10px;
  transform: translate(-50%, -50%);
  background: #ffffff;
  border: 2px solid #836dff;
  border-radius: 3px;
  z-index: 6;
  touch-action: none;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
}

.resize-handle:hover {
  background: #836dff;
}

/* 工具栏按钮 */
.tb-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: #cbd5e1;
  cursor: pointer;
}

.tb-btn:hover {
  background: var(--input-border);
  color: var(--btn-text);
}

/* 右键菜单项 */
.ctx-item {
  border: none;
  background: transparent;
  color: var(--text);
  text-align: left;
  padding: 6px 10px;
  font-size: 12px;
  border-radius: 5px;
  cursor: pointer;
}

.ctx-item:hover {
  background: var(--btn);
}

.ctx-item.danger:hover {
  background: #b91c1c;
  color: #fff;
}

/* ---------- Tweakpane(shadcn 风格;无法用具 utility 表达的 :deep 定制保留为普通 CSS) ---------- */
.pane-host {
  width: 100%;
}

.pane-host :deep(.tp-dfwv) {
  width: 100% !important;
  max-width: 100%;
  border: 1px solid var(--input-border);
}

/* 左侧模块标题左对齐 */
.sidebar.left .pane-host :deep(.tp-rotv_t) {
  text-align: left;
  justify-content: flex-start;
  padding-left: 6px;
}

/* Tweakpane 根/文件夹标题 shadcn 化 + 面板圆角 */
.pane-host :deep(.tp-rotv) {
  border-radius: 10px;
}

.pane-host :deep(.tp-rotv_t) {
  padding: 6px 8px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--tp-label-foreground-color);
  opacity: 1;
}

.pane-host :deep(.tp-fldv_t) {
  font-size: 11px;
  color: var(--tp-container-foreground-color);
}

.pane-host :deep(.tp-fldv_t):hover {
  color: var(--tp-input-foreground-color);
}

.pane-host :deep(.tp-btnv_b) {
  text-align: left;
}

/* 两侧面板统一:标签/描述在上、控件在下的上下结构,更紧凑 */
.sidebar .pane-host :deep(.tp-lblv) {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 1px;
  padding-top: 2px;
  padding-bottom: 2px;
}

.sidebar .pane-host :deep(.tp-lblv_l) {
  flex: none;
  padding-right: 0;
  line-height: 1.25;
}

.sidebar .pane-host :deep(.tp-lblv_v) {
  flex: none;
  width: 100%;
}

.sidebar .pane-host :deep(.tp-btnv_b) {
  padding-top: 3px;
  padding-bottom: 3px;
}

/* Tweakpane 主题:shadcn 风格,light 默认 */
.sidebar {
  --tp-base-background-color: #ffffff;
  --tp-base-shadow-color: rgba(15, 23, 42, 0.08);
  --tp-button-background-color: #f1f5f9;
  --tp-button-background-color-active: #e2e8f0;
  --tp-button-background-color-focus: #e2e8f0;
  --tp-button-background-color-hover: #e2e8f0;
  --tp-button-foreground-color: #0f172a;
  --tp-container-background-color: #f8fafc;
  --tp-container-foreground-color: #0f172a;
  --tp-groove-foreground-color: #e2e8f0;
  --tp-input-background-color: #ffffff;
  --tp-input-background-color-active: #f1f5f9;
  --tp-input-background-color-focus: #f1f5f9;
  --tp-input-background-color-hover: #f8fafc;
  --tp-input-foreground-color: #0f172a;
  --tp-label-foreground-color: #64748b;
  --tp-monitor-background-color: #f1f5f9;
  --tp-monitor-foreground-color: #16a34a;
}

.die-layout[data-theme='dark'] .sidebar {
  --tp-base-background-color: #141f36;
  --tp-base-shadow-color: rgba(0, 0, 0, 0.35);
  --tp-button-background-color: #334155;
  --tp-button-background-color-active: #475569;
  --tp-button-background-color-focus: #475569;
  --tp-button-background-color-hover: #475569;
  --tp-button-foreground-color: #e2e8f0;
  --tp-container-background-color: rgba(148, 163, 184, 0.05);
  --tp-container-foreground-color: #e2e8f0;
  --tp-groove-foreground-color: #334155;
  --tp-input-background-color: #1e293b;
  --tp-input-background-color-active: #334155;
  --tp-input-background-color-focus: #334155;
  --tp-input-background-color-hover: #334155;
  --tp-input-foreground-color: #e2e8f0;
  --tp-label-foreground-color: #94a3b8;
  --tp-monitor-background-color: #1e293b;
  --tp-monitor-foreground-color: #4ade80;
}
</style>
