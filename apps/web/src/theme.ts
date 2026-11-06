import { ref } from 'vue'

export type Theme = 'light' | 'dark'

const KEY = 'die-layout-editor.theme'

export const theme = ref<Theme>(loadTheme())

function loadTheme(): Theme {
  try {
    const t = localStorage.getItem(KEY)
    if (t === 'dark' || t === 'light') return t
  } catch {
    /* ignore */
  }
  return 'light'
}

export function toggleTheme() {
  theme.value = theme.value === 'dark' ? 'light' : 'dark'
  try {
    localStorage.setItem(KEY, theme.value)
  } catch {
    /* ignore */
  }
}

// 画布/标注调色板
export interface CanvasPalette {
  canvasBg: string
  rulerBg: string
  rulerLine: string
  rulerText: string
  rulerMinor: string
  gridMinor: string
  gridMajor: string
  shotFill: string
  shotStroke: string
  shotGrid: string
  shotBoundary: string
  fillA: string
  fillB: string
  dieStroke: string
  dimGray: string
  pillBg: string
}

export const PALETTES: Record<Theme, CanvasPalette> = {
  dark: {
    canvasBg: '#020617',
    rulerBg: '#0f172a',
    rulerLine: '#334155',
    rulerText: '#cbd5e1',
    rulerMinor: '#475569',
    gridMinor: 'rgba(148,163,184,0.05)',
    gridMajor: 'rgba(148,163,184,0.13)',
    shotFill: 'rgba(56,189,248,0.08)',
    shotStroke: '#38bdf8',
    shotGrid: 'rgba(96,165,250,0.45)',
    shotBoundary: 'rgba(56,189,248,0.6)',
    fillA: 'rgba(56,189,248,0.10)',
    fillB: 'rgba(148,163,184,0.08)',
    dieStroke: '#0b1120',
    dimGray: '#94a3b8',
    pillBg: 'rgba(2,6,23,0.85)',
  },
  light: {
    canvasBg: '#e9edf3',
    rulerBg: '#ffffff',
    rulerLine: '#cbd5e1',
    rulerText: '#334155',
    rulerMinor: '#94a3b8',
    gridMinor: 'rgba(100,116,139,0.10)',
    gridMajor: 'rgba(100,116,139,0.22)',
    shotFill: 'rgba(2,132,199,0.05)',
    shotStroke: '#0284c7',
    shotGrid: 'rgba(2,132,199,0.40)',
    shotBoundary: 'rgba(2,132,199,0.50)',
    fillA: 'rgba(2,132,199,0.10)',
    fillB: 'rgba(100,116,139,0.12)',
    dieStroke: '#cbd5e1',
    dimGray: '#64748b',
    pillBg: 'rgba(255,255,255,0.92)',
  },
}

export function palette(): CanvasPalette {
  return PALETTES[theme.value]
}
