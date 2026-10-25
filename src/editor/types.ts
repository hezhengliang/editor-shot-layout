import type { Group, Rect, Text } from 'leafer-editor'

export type UIRect = InstanceType<typeof Rect>
export type UIText = InstanceType<typeof Text>
export type UIGroup = InstanceType<typeof Group>

export interface Preset {
  id: string
  name: string
  // 模板尺寸(μm,3 位小数),与棋盘 cell 解耦
  wUm: number
  hUm: number
  color: string
}

export interface ShotSlot {
  i: number
  j: number
}

export interface Shot {
  uid: number
  group: UIGroup
  body: UIRect
  fillLayer: UIGroup
  gridLayer: UIGroup
  cols: number
  rows: number
  gapX: number
  gapY: number
  padX: number
  padY: number
  // 是否展示间距尺寸标注(属性面板开关控制)
  showDims: boolean
  // 是否展示外圈 scribe 与容器尺寸标注(默认关闭)
  showOuter: boolean
  // 参考曝光场(none/scanner/stepper/highna),用于适配校验
  refField: string
  dies: PlacedDie[]
}

// 行业标准曝光场尺寸(μm):Scanner 26×33mm / Stepper 22×22mm / High-NA 半场 26×16.5mm
export const REF_FIELDS: Record<string, { w: number; h: number; label: string }> = {
  scanner: { w: 26000, h: 33000, label: 'Scanner 26×33mm' },
  stepper: { w: 22000, h: 22000, label: 'Stepper 22×22mm' },
  highna: { w: 26000, h: 16500, label: 'High-NA 26×16.5mm' },
}

export interface PlacedDie {
  uid: number
  preset: Preset
  col: number
  row: number
  // 实际尺寸(μm,3 位小数),放置后可在属性面板单独修改
  wUm: number
  hUm: number
  group: UIGroup
  rect: UIRect
  label: UIText | null
  shot: Shot | null
  slot: ShotSlot | null
}

export interface LRect {
  x: number
  y: number
  w: number
  h: number
}

export const SHOT_CARD = { id: 'shot', name: 'Shot 容器' }
