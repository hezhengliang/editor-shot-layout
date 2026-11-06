// BitMap 常量与纯函数(不依赖 Vue / leafer)

export const ROWS = 2048
export const COLS = 2048
export const BIT_PITCH = 1
export const GRID_W = COLS * BIT_PITCH
export const GRID_H = ROWS * BIT_PITCH
export const SHEET_PAD = 1024

export const binColors: Record<number, string> = {
  0: '#22c55e', // pass - green
  1: '#ef4444', // fail - red (defect)
  2: '#eab308', // warn - yellow
  3: '#3b82f6', // skip - blue
  4: '#1e293b', // null - dark slate
}

export const binLabels: Record<number, string> = {
  0: 'PASS',
  1: 'FAIL',
  2: 'WARN',
  3: 'SKIP',
  4: 'NULL',
}

export const isFailBit = (b: number) => b === 1 || b === 2

// —— 存储层级(Die → Bank → Array → Block → Bit)——
// 2048² die:Bank 2×2(1024²)→ Array 2×2(512²)→ Block 2×2(256²,对齐标尺 256 块)
export const HIERARCHY = [
  { name: 'Bank', short: 'B', span: 1024, color: '#38bdf8', lw: 2.5 },
  { name: 'Array', short: 'A', span: 512, color: '#60a5fa', lw: 1.8 },
  { name: 'Block', short: 'K', span: 256, color: '#3b82f6', lw: 1.2 },
]

/** bit 所在层级路径,如 B1 · A2 · K3 */
export function bitPath(r: number, c: number): string {
  return HIERARCHY.map((lv) => {
    const n = COLS / lv.span
    const j = Math.floor(r / lv.span)
    const i = Math.floor(c / lv.span)
    return `${lv.short}${j * n + i}`
  }).join(' · ')
}

// 着色模式:bin=多数 bin 硬色;yield=块内不良率 绿→红 渐变(经典良率 shading);
// defect=失效类型分离合成(FAIL 红色阶 + WARN 黄色阶叠加,缺陷类型分布直观)
export type ShadingMode = 'bin' | 'yield' | 'defect'

export type YieldStop = [number, [number, number, number]]

export const yieldStops: YieldStop[] = [
  [0.0, [0x16, 0xa3, 0x4a]],
  [0.15, [0x84, 0xcc, 0x16]],
  [0.35, [0xea, 0xb3, 0x08]],
  [0.6, [0xf9, 0x73, 0x16]],
  [1.0, [0xdc, 0x26, 0x26]],
]

export function yieldColor(r: number): [number, number, number] {
  const t = Math.min(1, Math.max(0, r))
  for (let i = 1; i < yieldStops.length; i++) {
    if (t <= yieldStops[i][0]) {
      const [t0, c0] = yieldStops[i - 1]
      const [t1, c1] = yieldStops[i]
      const f = (t - t0) / (t1 - t0)
      return [
        Math.round(c0[0] + (c1[0] - c0[0]) * f),
        Math.round(c0[1] + (c1[1] - c0[1]) * f),
        Math.round(c0[2] + (c1[2] - c0[2]) * f),
      ]
    }
  }
  return yieldStops[yieldStops.length - 1][1]
}

/** die 坐标(与 WaferMap.vue 的 DieInfo 同构;本模块不依赖 ../WaferMap.vue) */
export interface DieCoord {
  x: number
  y: number
  bin: number
}

/** BitMap 视图 Props */
export interface DieBits {
  die: DieCoord
}

// —— 失效分析结果类型(data.ts 的 computed 返回值) ——
export interface BinStat {
  bin: number
  count: number
  pct: number
}

export interface PatternStat {
  rowFailRows: number
  colFailCols: number
  clustered: number
  isolated: number
}

export interface TopItem {
  i: number
  n: number
}

export interface DefectItem {
  r: number
  c: number
  bin: number
}

export interface WaferDiesInfo {
  list: { x: number; y: number }[]
  diameter: number
}
