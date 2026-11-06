import { ref } from 'vue'
import type { Preset } from './types'

const LIBRARY_KEY = 'die-layout-editor.library.v1'

function loadLibrary(): Preset[] {
  try {
    const raw = localStorage.getItem(LIBRARY_KEY)
    if (raw) {
      const arr = JSON.parse(raw)
      if (Array.isArray(arr) && arr.length) {
        return arr.filter(
          (t) => t && typeof t.id === 'string' && typeof t.wUm === 'number' && typeof t.hUm === 'number',
        )
      }
    }
  } catch {
    /* ignore */
  }
  // 默认仅展示一个 die
  return [{ id: 'default-1x1', name: 'Die 1×1', wUm: 500, hUm: 500, color: '#60a5fa' }]
}

export const dieLibrary = ref<Preset[]>(loadLibrary())

export function saveLibrary() {
  try {
    localStorage.setItem(LIBRARY_KEY, JSON.stringify(dieLibrary.value))
  } catch {
    /* ignore */
  }
}

export function presetOf(cardId: string): Preset | undefined {
  return dieLibrary.value.find((p) => p.id === cardId)
}

export function presetName(cardId: string): string {
  return presetOf(cardId)?.name ?? ''
}

export function presetColor(cardId: string): string {
  return presetOf(cardId)?.color ?? '#60a5fa'
}

// 幽灵块尺寸:按模板宽高比缩放到约 44px
export function ghostSize(cardId: string): { w: number; h: number } {
  const t = presetOf(cardId)
  if (!t) return { w: 40, h: 40 }
  const scale = 44 / Math.max(t.wUm, t.hUm)
  return { w: Math.max(14, t.wUm * scale), h: Math.max(14, t.hUm * scale) }
}
