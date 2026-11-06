// BitMap 渲染层:总览纹理、视口 crisp 重绘、后端瓦片、层级边界、WL/BL 十字
import { Group, Rect, Text } from 'leafer-editor'
import type { App, Canvas as LeaferCanvas } from 'leafer-editor'
import type { Ref } from 'vue'
import { RULER_SIZE as RULER } from '../../editor/ruler'
import { BIT_PITCH, COLS, GRID_H, GRID_W, HIERARCHY, ROWS, yieldColor } from './types'
import type { ShadingMode } from './types'
import type { BitmapData } from './data'

export interface BitmapRendererDeps {
  getApp: () => App | null
  getContent: () => Group | null
  getBitmapCanvas: () => LeaferCanvas | null
  data: BitmapData
  tile: { template: Ref<string>; showBounds: Ref<boolean> }
  ui: {
    shadingMode: Ref<ShadingMode>
    softContrast: Ref<boolean>
    showWlbl: Ref<boolean>
    hierVisibility: Ref<{ Bank: boolean; Array: boolean; Block: boolean }>
  }
  /** drawRuler 递归回调(瓦片 onload/onerror 后重绘) */
  requestRedraw: () => void
}

// 视口级清晰渲染:低倍率走 leafer 纹理总览(缩放采样,合理);
// 高倍率(≥6×)在屏幕空间按可见 bit 边界逐格重绘,位格恒清晰不再 shading。
const CRISP_MIN_SCALE = 6
const TILE_PX = 256

export function createBitmapRenderer(deps: BitmapRendererDeps) {
  const { data, tile, ui } = deps

  // 视口 crisp 重绘的 mip 离屏像素缓冲(按可见区尺寸复用)
  let mipOff: HTMLCanvasElement | null = null
  let mipOffCtx: CanvasRenderingContext2D | null = null

  // —— 后端瓦片模式:后端已渲染 shading 金字塔时,前端按视野拉瓦片(类地图切片) ——
  // URL 模板含 {z}/{x}/{y};z 为聚合层级(2^z bit/像素),瓦片 256×256 像素
  const tileCache = new Map<string, HTMLImageElement | 'loading'>()
  // 瓦片服务不可达时自动回退本地渲染(客户端单独跑场景)
  let tileOk = 0
  let tileFail = 0

  let hierGroup: Group | null = null
  let wlblGroup: Group | null = null

  // 柔和对比:对比例开平方,稀疏缺陷的动态范围更好
  const scaleRatio = (r: number) =>
    ui.softContrast.value ? Math.sqrt(Math.min(1, Math.max(0, r))) : Math.min(1, Math.max(0, r))

  /** 2048×2048 bit → 离屏 canvas 像素,再一次性贴到 leafer Canvas 元素(平移缩放只重绘纹理,不重建) */
  function paintTexture(canvas: LeaferCanvas) {
    const off = document.createElement('canvas')
    off.width = COLS
    off.height = ROWS
    const ctx = off.getContext('2d')!
    const imageData = ctx.createImageData(COLS, ROWS)
    const pixels = imageData.data
    let rgb: [number, number, number] = [0, 0, 0]
    const dataArr = data.bitDataRef.value
    for (let i = 0; i < dataArr.length; i++) {
      rgb = data.binRgb(dataArr[i])
      pixels[i * 4] = rgb[0]
      pixels[i * 4 + 1] = rgb[1]
      pixels[i * 4 + 2] = rgb[2]
      pixels[i * 4 + 3] = 255
    }
    ctx.putImageData(imageData, 0, 0)
    canvas.context.drawImage(off, 0, 0)
    canvas.paint()
  }

  /** LeaferCanvas 初次布局会清掉过早绘制的内容:前几次重绘各刷一次纹理兜底(含 bin 过滤脏) */
  function maybePaintTexture() {
    const canvas = deps.getBitmapCanvas()
    if (canvas && (data.texturePaints.value < 3 || data.filterDirty.value)) {
      data.filterDirty.value = false
      data.texturePaints.value++
      paintTexture(canvas)
    }
  }

  function tileUrl(z: number, x: number, y: number) {
    return tile.template.value.replace('{z}', String(z)).replace('{x}', String(x)).replace('{y}', String(y))
  }

  /** 瓦片模式视口绘制;返回是否有未到达瓦片(到了会触发重绘) */
  function paintTiles(ctx: CanvasRenderingContext2D, sc: number, ox: number, oy: number, w: number, h: number): void {
    // 屏幕原生层级:目标每瓦片像素 ≈1~2 屏幕 px(z = -log2(sc)),聚合 shading 由服务端在该层级完成;
    // 拉远/平移即可见多瓦片拼接(瓦片在屏 ≈256~512px)
    const z = Math.max(0, Math.min(11, Math.round(-Math.log2(sc))))
    const dim = Math.max(1, COLS >> z) // 该层总像素(边长)
    const pitch = sc * (1 << z) // 单像素边长(px)
    const tileCount = Math.ceil(dim / TILE_PX)
    // 可见瓦片范围(像素坐标 → 瓦片索引)
    const tx0 = Math.max(0, Math.floor((RULER - ox) / pitch / TILE_PX))
    const tx1 = Math.min(tileCount - 1, Math.floor((w - ox) / pitch / TILE_PX))
    const ty0 = Math.max(0, Math.floor((RULER - oy) / pitch / TILE_PX))
    const ty1 = Math.min(tileCount - 1, Math.floor((h - oy) / pitch / TILE_PX))
    // 近原生档位平滑(≤2× 拉伸),高倍放大用最近邻保持 bit 硬边界
    ctx.imageSmoothingEnabled = sc * (1 << z) < 4
    const size = pitch * TILE_PX + 0.6
    for (let ty = ty0; ty <= ty1; ty++) {
      for (let tx = tx0; tx <= tx1; tx++) {
        const key = `${z}/${tx}/${ty}`
        const cached = tileCache.get(key)
        if (cached && cached !== 'loading') {
          const dx = ox + tx * TILE_PX * pitch
          const dy = oy + ty * TILE_PX * pitch
          ctx.drawImage(cached, dx, dy, size, size)
          if (tile.showBounds.value) {
            ctx.strokeStyle = 'rgba(255,255,255,0.35)'
            ctx.lineWidth = 1
            ctx.strokeRect(dx + 0.5, dy + 0.5, size - 1, size - 1)
          }
        } else if (!cached) {
          tileCache.set(key, 'loading')
          const img = new Image()
          img.crossOrigin = 'anonymous'
          img.onload = () => {
            tileOk++
            tileCache.set(key, img)
            deps.requestRedraw() // 到达后重绘贴上
          }
          img.onerror = () => {
            tileCache.delete(key)
            tileFail++
            // 连续失败且无成功 = 服务不可达(客户端单独跑):回退本地渲染,避免黑屏
            if (tileFail >= 3 && tileOk === 0) {
              tile.template.value = ''
              tileCache.clear()
              deps.requestRedraw()
            }
          }
          img.src = tileUrl(z, tx, ty)
        }
      }
    }
  }

  /** 瓦片源/缓存重置:模板变更或清除时调用(等价于原 tileCache.clear() + 计数清零) */
  function resetTiles() {
    tileCache.clear()
    tileOk = 0
    tileFail = 0
  }

  /** 视口级清晰渲染(crisp 覆盖层):瓦片模式走 paintTiles,本地模式按可见区 mip 逐格重绘 */
  function paintCrisp(crispRef: Ref<HTMLCanvasElement | null>) {
    const cv = crispRef.value
    const app = deps.getApp()
    if (!app || !cv) return
    const dpr = window.devicePixelRatio || 1
    const w = app.width ?? 0
    const h = app.height ?? 0
    if (w <= 0 || h <= 0) return
    if (cv.width !== Math.round(w * dpr) || cv.height !== Math.round(h * dpr)) {
      cv.width = Math.round(w * dpr)
      cv.height = Math.round(h * dpr)
    }
    const ctx = cv.getContext('2d')!
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, w, h)
    const layer = app.tree.zoomLayer
    const sc = typeof layer.scale === 'number' ? layer.scale : 1
    const ox = typeof layer.x === 'number' ? layer.x : 0
    const oy = typeof layer.y === 'number' ? layer.y : 0
    // 后端瓦片模式:全缩放档位都由服务端渲染(总览也是瓦片,拉远可见多瓦片拼接)
    if (tile.template.value.trim()) {
      // 瓦片只画进内容区:crisp 画布 z 层在 leafer 之上,不裁剪会盖住标尺带
      ctx.save()
      ctx.beginPath()
      ctx.rect(RULER, RULER, w - RULER, h - RULER)
      ctx.clip()
      paintTiles(ctx, sc, ox, oy, w, h)
      ctx.restore()
      return
    }
    if (sc < CRISP_MIN_SCALE) return
    // 分级 shading:聚合 cell 目标 ≈48px —— 缩放越低聚合度越大(1/2/4/8…bit),画面更整体;
    // 缩放越高越接近逐 bit,≥48× 时完全逐 bit 清晰。
    // 渲染:可见区 mip → 离屏像素缓冲 → 单次 drawImage 放大;
    // 聚合档开平滑插值(经典 wafer-map 渐变 shading),逐 bit 档关闭(锐利)。
    let k = 0
    while (k < data.mips.length - 1 && sc * (1 << (k + 1)) <= 48) k++
    const agg = 1 << k // 聚合边长(bit)
    const mipsData = data.mips[k]
    const fails = data.failMips[k]
    const warns = data.warnMips[k]
    const dim = COLS >> k
    const cellTotal = agg * agg
    const pitch = sc * agg
    // 起始格用 floor(覆盖边缘不完整格),标尺带内的部分由下方源矩形裁剪切除
    const c0 = Math.max(0, Math.floor((RULER - ox) / pitch))
    const c1 = Math.min(dim - 1, Math.ceil((w - ox) / pitch))
    const r0 = Math.max(0, Math.floor((RULER - oy) / pitch))
    const r1 = Math.min(dim - 1, Math.ceil((h - oy) / pitch))
    if (c1 < c0 || r1 < r0) return
    const offW = c1 - c0 + 1
    const offH = r1 - r0 + 1
    // 边缘裁剪:首/末格只画落在标尺带之外的部分(源矩形按覆盖率裁,不整格丢弃)
    let srcX0 = 0
    let srcY0 = 0
    let destX = ox + c0 * pitch
    let destY = oy + r0 * pitch
    if (destX < RULER) {
      srcX0 = (RULER - destX) / pitch
      destX = RULER
    }
    if (destY < RULER) {
      srcY0 = (RULER - destY) / pitch
      destY = RULER
    }
    let srcX1 = offW
    let srcY1 = offH
    let destW = (offW - srcX0) * pitch
    let destH = (offH - srcY0) * pitch
    if (destX + destW > w) {
      srcX1 = srcX0 + (w - destX) / pitch
      destW = w - destX
    }
    if (destY + destH > h) {
      srcY1 = srcY0 + (h - destY) / pitch
      destH = h - destY
    }
    if (destW <= 0 || destH <= 0) return
    if (!mipOff || mipOff.width < offW || mipOff.height < offH) {
      mipOff = document.createElement('canvas')
      mipOff.width = Math.max(64, offW)
      mipOff.height = Math.max(64, offH)
      mipOffCtx = mipOff.getContext('2d')!
    }
    const offCtx = mipOffCtx!
    const img = offCtx.createImageData(offW, offH)
    const px = img.data
    const mode = ui.shadingMode.value
    for (let y = 0; y < offH; y++) {
      const base = (r0 + y) * dim
      for (let x = 0; x < offW; x++) {
        const i = base + c0 + x
        let rgb: [number, number, number]
        if (mode === 'yield' && k > 0) {
          rgb = yieldColor(scaleRatio((fails[i] + warns[i]) / cellTotal))
        } else if (mode === 'defect') {
          // FAIL→红、WARN→黄,按 √ 比例向背景叠加;pass/其他区域保持暗底
          const a1 = scaleRatio(fails[i] / cellTotal)
          const a2 = scaleRatio(warns[i] / cellTotal)
          let r = 0x0b
          let g = 0x11
          let b = 0x20
          r += (0xef - r) * a1
          g += (0x44 - g) * a1
          b += (0x44 - b) * a1
          g += (0xea - g) * a2
          r += (0xb3 - r) * a2 * 0.6
          b += (0x08 - b) * a2
          rgb = [Math.round(Math.min(255, r)), Math.round(Math.min(255, g)), Math.round(Math.min(255, b))]
        } else {
          rgb = data.binRgb(mipsData[i])
        }
        const o = (y * offW + x) * 4
        px[o] = rgb[0]
        px[o + 1] = rgb[1]
        px[o + 2] = rgb[2]
        px[o + 3] = 255
      }
    }
    offCtx.putImageData(img, 0, 0)
    ctx.imageSmoothingEnabled = ui.shadingMode.value === 'bin' ? k > 0 : true
    ctx.drawImage(mipOff, srcX0, srcY0, srcX1 - srcX0, srcY1 - srcY0, destX, destY, destW, destH)
  }

  /** 层级边界覆盖层:按缩放 LOD —— 层级格在屏幕上 ≥100px 才显示该级(线宽/字号屏幕恒定) */
  function drawHierarchy() {
    const app = deps.getApp()
    const content = deps.getContent()
    if (!app || !content) return
    hierGroup?.destroy()
    hierGroup = new Group({ hittable: false })
    content.add(hierGroup)
    const layer = app.tree.zoomLayer
    const vs = typeof layer.scale === 'number' ? layer.scale : 1
    for (const lv of HIERARCHY) {
      if (!ui.hierVisibility.value[lv.name as 'Bank' | 'Array' | 'Block']) continue
      const n = COLS / lv.span
      if (lv.span * vs < 100) continue
      const fontSize = 11 / vs
      const lw = lv.lw / vs
      for (let j = 0; j < n; j++) {
        for (let i = 0; i < n; i++) {
          const x = i * lv.span
          const y = j * lv.span
          hierGroup.add(
            new Rect({
              x,
              y,
              width: lv.span,
              height: lv.span,
              stroke: lv.color,
              strokeWidth: lw,
              dashPattern: lv.span === 256 ? [8 / vs, 6 / vs] : undefined,
            }),
          )
          // Array 级不画编号(格子太密,文字成噪音;Bank/Block 保留)
          if (lv.name !== 'Array') {
            hierGroup.add(
              new Text({
                x: x + 8 / vs,
                y: y + 5 / vs,
                text: `${lv.short}${j * n + i}`,
                fill: lv.color,
                fontSize,
              }),
            )
          }
        }
      }
    }
  }

  /** WL(字线,行)/ BL(位线,列)高亮带 */
  function drawWlbl(r: number | null, c: number | null) {
    const app = deps.getApp()
    const content = deps.getContent()
    wlblGroup?.destroy()
    wlblGroup = null
    if (!app || !content || r === null || c === null || !ui.showWlbl.value) return
    wlblGroup = new Group({ hittable: false })
    content.add(wlblGroup)
    wlblGroup.add(
      new Rect({ x: 0, y: r * BIT_PITCH, width: GRID_W, height: BIT_PITCH, fill: 'rgba(56,189,248,0.28)' }),
    )
    wlblGroup.add(
      new Rect({ x: c * BIT_PITCH, y: 0, width: BIT_PITCH, height: GRID_H, fill: 'rgba(244,114,182,0.28)' }),
    )
  }

  /** 销毁层级/十字覆盖层(onUnmounted 用) */
  function clearOverlays() {
    hierGroup?.destroy()
    hierGroup = null
    wlblGroup?.destroy()
    wlblGroup = null
  }

  return {
    paintTexture,
    maybePaintTexture,
    paintCrisp,
    paintTiles,
    resetTiles,
    drawHierarchy,
    drawWlbl,
    clearOverlays,
  }
}

export type BitmapRenderer = ReturnType<typeof createBitmapRenderer>
