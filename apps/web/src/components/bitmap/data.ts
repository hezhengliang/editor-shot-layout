// BitMap 数据层:bit 数据生成/网络加载、mip 金字塔、bin 过滤、失效分析统计
import { computed, ref } from 'vue'
import { COLS, ROWS, binColors, isFailBit } from './types'
import type {
  BinStat,
  DefectItem,
  PatternStat,
  TopItem,
  WaferDiesInfo,
} from './types'

/** 合成随机 bit 数据:dieBin 权重 0.9,其余各 bin 0.02(权重数组法) */
export function generateBitData(dieBin: number): Uint8Array {
  const total = ROWS * COLS
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

/** 从本地 FastAPI 服务拉原始位数据(/bits,2048² uint8);服务不可达返回 null(回退合成) */
export async function loadBitDataFromServer(base = 'http://localhost:8000'): Promise<Uint8Array | null> {
  try {
    const ctrl = new AbortController()
    const timer = setTimeout(() => ctrl.abort(), 8000)
    const res = await fetch(`${base}/bits`, { mode: 'cors', signal: ctrl.signal })
    clearTimeout(timer)
    if (!res.ok) return null
    const buf = new Uint8Array(await res.arrayBuffer())
    return buf.length === COLS * ROWS ? buf : null
  } catch {
    return null
  }
}

export function useBitmapData(dieBin: number, requestRedraw: () => void) {
  // 位数据:ref 化,挂载后优先从本地 FastAPI 服务拉取(/bits),失败回退合成随机
  const bitDataRef = ref<Uint8Array>(generateBitData(dieBin))
  const dataSource = ref<'image' | 'synthetic'>('synthetic')

  // Mip 金字塔(分级 shading 用):mips[0]=2048² 原始,mips[k] 为 2×2 多数表决降采样,直到 1×1
  // failMips[k] 为同结构的 fail/warn bit 计数金字塔(Yield shading 按块内不良比例着色)
  const mips: Uint8Array[] = []
  const failMips: Uint32Array[] = [] // bin1(FAIL)计数金字塔
  const warnMips: Uint32Array[] = [] // bin2(WARN)计数金字塔(defect 类型分离 shading 用)

  function rebuildMips() {
    mips.length = 0
    failMips.length = 0
    warnMips.length = 0
    let cur = bitDataRef.value
    let failCur = new Uint32Array(COLS * ROWS)
    let warnCur = new Uint32Array(COLS * ROWS)
    for (let i = 0; i < cur.length; i++) {
      failCur[i] = cur[i] === 1 ? 1 : 0
      warnCur[i] = cur[i] === 2 ? 1 : 0
    }
    mips.push(cur)
    failMips.push(failCur)
    warnMips.push(warnCur)
    let w = COLS
    while (w > 1) {
      const half = w / 2
      const next = new Uint8Array(half * half)
      const failNext = new Uint32Array(half * half)
      const warnNext = new Uint32Array(half * half)
      for (let y = 0; y < half; y++) {
        for (let x = 0; x < half; x++) {
          const counts = [0, 0, 0, 0, 0]
          counts[cur[(y * 2) * w + x * 2]]++
          counts[cur[(y * 2) * w + x * 2 + 1]]++
          counts[cur[(y * 2 + 1) * w + x * 2]]++
          counts[cur[(y * 2 + 1) * w + x * 2 + 1]]++
          // 平局取序号小者,保证稳定
          let best = 0
          for (let b = 1; b < 5; b++) if (counts[b] > counts[best]) best = b
          next[y * half + x] = best
          failNext[y * half + x] =
            failCur[(y * 2) * w + x * 2] +
            failCur[(y * 2) * w + x * 2 + 1] +
            failCur[(y * 2 + 1) * w + x * 2] +
            failCur[(y * 2 + 1) * w + x * 2 + 1]
          warnNext[y * half + x] =
            warnCur[(y * 2) * w + x * 2] +
            warnCur[(y * 2) * w + x * 2 + 1] +
            warnCur[(y * 2 + 1) * w + x * 2] +
            warnCur[(y * 2 + 1) * w + x * 2 + 1]
        }
      }
      mips.push(next)
      failMips.push(failNext)
      warnMips.push(warnNext)
      cur = next
      failCur = failNext
      warnCur = warnNext
      w = half
    }
  }
  rebuildMips()

  // —— Bin 过滤(未勾选 bin 调暗至背景,保留空间上下文;业界 bin isolate 的轻量版) ——
  // 默认只渲染 FAIL(红),其余 bin 调暗;缺陷分布最纯粹,需要时勾回
  const binFilter = ref<boolean[]>([false, true, false, false, true])

  // 总览纹理兜底重绘状态:由 renderer 的 maybePaintTexture 读写(前三次兜底 + 过滤脏标记)
  const filterDirty = ref(false)
  const texturePaints = ref(0)

  function onBinToggle(b: number, e: Event) {
    binFilter.value[b] = (e.target as HTMLInputElement).checked
    filterDirty.value = true
    texturePaints.value = Math.min(texturePaints.value, 2)
    requestRedraw()
  }

  function binRgb(bin: number): [number, number, number] {
    const hex = binColors[bin] ?? '#94a3b8'
    let r = parseInt(hex.slice(1, 3), 16)
    let g = parseInt(hex.slice(3, 5), 16)
    let b = parseInt(hex.slice(5, 7), 16)
    if (!binFilter.value[bin]) {
      // 调暗:向图纸背景 #0b1120 混合 88%
      r = Math.round(r * 0.12 + 0x0b * 0.88)
      g = Math.round(g * 0.12 + 0x11 * 0.88)
      b = Math.round(b * 0.12 + 0x20 * 0.88)
    }
    return [r, g, b]
  }

  // —— 行业分析:bin 统计 / 失效模式分类 / WL-BL TOP / 晶圆定位 ——

  /** Bin 统计:各 bin 数量与占比 */
  const binStats = computed<BinStat[]>(() => {
    const counts = [0, 0, 0, 0, 0]
    const d = bitDataRef.value
    for (let i = 0; i < d.length; i++) counts[d[i]]++
    return counts.map((n, b) => ({ bin: b, count: n, pct: (n / d.length) * 100 }))
  })

  /** 失效模式分类:行失效(整行 ≥70% fail)/ 列失效 / 聚簇(相邻 fail)/ 单bit(孤立) */
  const patternStats = computed<PatternStat>(() => {
    const d = bitDataRef.value
    const rowFail = new Uint32Array(ROWS)
    const colFail = new Uint32Array(COLS)
    for (let r = 0; r < ROWS; r++) {
      let f = 0
      const base = r * COLS
      for (let c = 0; c < COLS; c++) if (isFailBit(d[base + c])) f++
      rowFail[r] = f
      for (let c = 0; c < COLS; c++) if (isFailBit(d[base + c])) colFail[c]++
    }
    let rowFailRows = 0
    for (let r = 0; r < ROWS; r++) if (rowFail[r] >= COLS * 0.7) rowFailRows++
    let colFailCols = 0
    for (let c = 0; c < COLS; c++) if (colFail[c] >= ROWS * 0.7) colFailCols++
    let clustered = 0
    let isolated = 0
    for (let r = 0; r < ROWS; r++) {
      const base = r * COLS
      for (let c = 0; c < COLS; c++) {
        if (!isFailBit(d[base + c])) continue
        const adj =
          (c + 1 < COLS && isFailBit(d[base + c + 1])) ||
          (r + 1 < ROWS && isFailBit(d[base + COLS + c])) ||
          (c > 0 && isFailBit(d[base + c - 1])) ||
          (r > 0 && isFailBit(d[base - COLS + c]))
        if (adj) clustered++
        else isolated++
      }
    }
    return { rowFailRows, colFailCols, clustered, isolated }
  })

  // WL(行)/ BL(列)失效 TOP8 直方图 + 点击定位最差字线:直方图缓存(惰性构建)
  const rowFailCache = new Uint32Array(ROWS)
  const colFailCache = new Uint32Array(COLS)
  const failCacheBuilt = ref(false)

  function ensureFailCache() {
    if (failCacheBuilt.value) return
    const d = bitDataRef.value
    rowFailCache.fill(0)
    colFailCache.fill(0)
    for (let r = 0; r < ROWS; r++) {
      const base = r * COLS
      for (let c = 0; c < COLS; c++) {
        if (isFailBit(d[base + c])) {
          rowFailCache[r]++
          colFailCache[c]++
        }
      }
    }
    failCacheBuilt.value = true
  }

  function rowFailCount(r: number) {
    ensureFailCache()
    return rowFailCache[r]
  }
  function colFailCount(c: number) {
    ensureFailCache()
    return colFailCache[c]
  }

  function topN(n: number, get: (i: number) => number): TopItem[] {
    ensureFailCache()
    const idx = Array.from({ length: n }, (_, i) => i)
    idx.sort((a, b) => get(b) - get(a))
    return idx.slice(0, 8).map((i) => ({ i, n: get(i) }))
  }

  const wlTop = computed<TopItem[]>(() => topN(ROWS, (r) => rowFailCount(r)))
  const blTop = computed<TopItem[]>(() => topN(COLS, (c) => colFailCount(c)))

  /** 数据替换后调用:失效直方图缓存作废 */
  function invalidateAnalysis() {
    failCacheBuilt.value = false
  }

  const defects = computed<DefectItem[]>(() => {
    // 侧栏只列代表性缺陷(点击定位用),总数看 Fail Patterns / Bin Summary
    const list: DefectItem[] = []
    const target = 20
    let attempts = 0
    while (list.length < target && attempts < target * 20) {
      attempts++
      const idx = Math.floor(Math.random() * bitDataRef.value.length)
      const bin = bitDataRef.value[idx]
      if (bin === 1 || bin === 2) {
        const r = Math.floor(idx / COLS)
        const c = idx % COLS
        list.push({ r, c, bin })
      }
    }
    return list
  })

  const defectTotal = computed(() => {
    let count = 0
    const sampleSize = 1000000
    for (let i = 0; i < sampleSize; i++) {
      const idx = Math.floor(Math.random() * bitDataRef.value.length)
      const bin = bitDataRef.value[idx]
      if (bin === 1 || bin === 2) count++
    }
    return Math.round((count / sampleSize) * bitDataRef.value.length)
  })

  /** 晶圆定位:与 WaferMap 同几何公式重建 die 阵列(diameter 800 / pitch 9 / 内切圆) */
  const waferDies = computed<WaferDiesInfo>(() => {
    const diameter = 800
    const pitch = 9
    const radius = diameter / 2
    const margin = Math.sqrt(4 ** 2 + 4 ** 2)
    const list: { x: number; y: number }[] = []
    for (let row = -50; row <= 50; row++) {
      for (let col = -50; col <= 50; col++) {
        const cx = col * pitch
        const cy = row * pitch
        if (Math.sqrt(cx * cx + cy * cy) + margin <= radius) list.push({ x: col, y: row })
      }
    }
    return { list, diameter }
  })

  return {
    bitDataRef,
    dataSource,
    mips,
    failMips,
    warnMips,
    rebuildMips,
    generateBitData,
    loadBitDataFromServer,
    binFilter,
    onBinToggle,
    filterDirty,
    texturePaints,
    binRgb,
    rowFailCache,
    colFailCache,
    failCacheBuilt,
    ensureFailCache,
    rowFailCount,
    colFailCount,
    topN,
    wlTop,
    blTop,
    invalidateAnalysis,
    binStats,
    patternStats,
    defects,
    defectTotal,
    waferDies,
  }
}

export type BitmapData = ReturnType<typeof useBitmapData>
