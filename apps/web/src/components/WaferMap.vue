<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { App, Group, Rect, Text, Canvas as LeaferCanvas } from 'leafer-editor'

export interface DieInfo {
  bin: number
  x: number
  y: number
}

interface WaferData {
  diameter: number
  dieWidth: number
  dieHeight: number
  dies: DieInfo[]
}

const containerRef = ref<HTMLDivElement | null>(null)
const tooltipRef = ref<HTMLDivElement | null>(null)
let app: App | null = null
let highlight: Rect | null = null
let label: Text | null = null
let waferGroup: Group | null = null

const emit = defineEmits<{
  (e: 'select-die', die: DieInfo): void
}>()

const VIEW = 820
const CENTER = VIEW / 2

const binColors: Record<number, string> = {
  0: '#4ade80', // pass - green
  1: '#f87171', // fail - red
  2: '#facc15', // warn - yellow
  3: '#60a5fa', // skip - blue
}

const defaultColor = '#94a3b8'

function generateMockData(): WaferData {
  const diameter = 800
  const dieWidth = 8
  const dieHeight = 8
  const gap = 1
  const radius = diameter / 2
  const dies: DieInfo[] = []
  const bins = [0, 0, 0, 0, 0, 1, 1, 2, 3]

  const cols = Math.ceil(diameter / (dieWidth + gap))
  const rows = Math.ceil(diameter / (dieHeight + gap))

  for (let row = -rows; row <= rows; row++) {
    for (let col = -cols; col <= cols; col++) {
      const cx = col * (dieWidth + gap)
      const cy = row * (dieHeight + gap)
      const dist = Math.sqrt(cx * cx + cy * cy)
      const margin = Math.sqrt((dieWidth / 2) ** 2 + (dieHeight / 2) ** 2)
      if (dist + margin <= radius) {
        dies.push({
          x: col,
          y: row,
          bin: bins[Math.floor(Math.random() * bins.length)],
        })
      }
    }
  }

  return { diameter, dieWidth, dieHeight, dies }
}

function dieRect(data: WaferData, col: number, row: number) {
  const halfW = data.dieWidth / 2
  const halfH = data.dieHeight / 2
  const gap = 1
  return {
    x: col * (data.dieWidth + gap) - halfW,
    y: row * (data.dieHeight + gap) - halfH,
  }
}

onMounted(() => {
  const host = containerRef.value
  if (!host) return

  app = new App({
    view: host,
    width: VIEW,
    height: VIEW,
    fill: '#0f172a',
    tree: { type: 'design' },
    wheel: { preventDefault: true },
    pointer: { preventDefaultMenu: true },
  })

  const data = generateMockData()
  waferGroup = new Group({ x: CENTER, y: CENTER })
  app.tree.add(waferGroup)

  // 晶圆体 + 缺口:静态内容一次性画进 Canvas 元素(数千 die 不建叶子节点)
  // 画布放在 waferGroup 内 (-CENTER,-CENTER),内部以 CENTER 为圆心绘制 → 世界坐标不变
  const sheet = new LeaferCanvas({ x: -CENTER, y: -CENTER, width: VIEW, height: VIEW, hittable: false })
  waferGroup.add(sheet)
  {
    const ctx = sheet.context
    // 晶圆体
    ctx.fillStyle = '#334155'
    ctx.beginPath()
    ctx.arc(CENTER, CENTER, data.diameter / 2, 0, Math.PI * 2)
    ctx.fill()
    ctx.lineWidth = 2
    ctx.strokeStyle = '#94a3b8'
    ctx.stroke()
    // die 阵列
    for (const die of data.dies) {
      const p = dieRect(data, die.x, die.y)
      ctx.fillStyle = binColors[die.bin] ?? defaultColor
      ctx.fillRect(CENTER + p.x, CENTER + p.y, data.dieWidth, data.dieHeight)
    }
    // 缺口(底部 V 形,先清底再描边)
    const notchWidth = 14
    const notchDepth = 10
    const notchY = CENTER + data.diameter / 2
    ctx.fillStyle = '#0f172a'
    ctx.beginPath()
    ctx.moveTo(CENTER - notchWidth / 2, notchY + 6)
    ctx.lineTo(CENTER - notchWidth / 2, notchY)
    ctx.lineTo(CENTER, notchY - notchDepth)
    ctx.lineTo(CENTER + notchWidth / 2, notchY)
    ctx.lineTo(CENTER + notchWidth / 2, notchY + 6)
    ctx.closePath()
    ctx.fill()
    ctx.strokeStyle = '#94a3b8'
    ctx.stroke()
    sheet.paint()
  }

  // hover 高亮 + 标签
  highlight = new Rect({ visible: false, stroke: '#ffffff', strokeWidth: 1 })
  waferGroup.add(highlight)
  label = new Text({ text: '', fill: '#ffffff', fontSize: 12, visible: false })
  waferGroup.add(label)

  function findDieAt(clientX: number, clientY: number): DieInfo | null {
    if (!app) return null
    const rect = host!.getBoundingClientRect()
    const px = clientX - rect.left - CENTER
    const py = clientY - rect.top - CENTER
    const halfW = data.dieWidth / 2
    const halfH = data.dieHeight / 2
    const gap = 1
    const col = Math.round((px + halfW) / (data.dieWidth + gap))
    const row = Math.round((py + halfH) / (data.dieHeight + gap))
    return data.dies.find((d) => d.x === col && d.y === row) ?? null
  }

  const onMove = (e: PointerEvent) => {
    if (!app || !highlight || !label) return
    const die = findDieAt(e.clientX, e.clientY)
    if (die) {
      const p = dieRect(data, die.x, die.y)
      highlight.set({ x: p.x, y: p.y, width: data.dieWidth, height: data.dieHeight, visible: true })
      label.text = `Die(${die.x},${die.y}) bin=${die.bin}`
      label.x = p.x + data.dieWidth + 4
      label.y = p.y
      label.visible = true
      if (tooltipRef.value) {
        tooltipRef.value.style.left = `${e.clientX + 12}px`
        tooltipRef.value.style.top = `${e.clientY + 12}px`
        tooltipRef.value.textContent = `Die(${die.x}, ${die.y}) — Bin ${die.bin}`
        tooltipRef.value.style.display = 'block'
      }
    } else {
      highlight.visible = false
      label.visible = false
      if (tooltipRef.value) tooltipRef.value.style.display = 'none'
    }
  }
  const onClick = (e: PointerEvent) => {
    const die = findDieAt(e.clientX, e.clientY)
    if (die) emit('select-die', die)
  }
  const onLeave = () => {
    if (highlight) highlight.visible = false
    if (label) label.visible = false
    if (tooltipRef.value) tooltipRef.value.style.display = 'none'
  }

  host.addEventListener('pointermove', onMove)
  host.addEventListener('click', onClick)
  host.addEventListener('pointerleave', onLeave)

  // dev 调试钩子(仅开发环境)
  if (import.meta.env.DEV) {
    ;(window as unknown as Record<string, unknown>).__wm = {
      dieAt: (col: number, row: number) => data.dies.find((d) => d.x === col && d.y === row) ?? null,
      count: () => data.dies.length,
    }
  }

  onUnmounted(() => {
    host.removeEventListener('pointermove', onMove)
    host.removeEventListener('click', onClick)
    host.removeEventListener('pointerleave', onLeave)
    if (app) {
      app.destroy()
      app = null
    }
    waferGroup = null
    highlight = null
    label = null
  })
})
</script>

<template>
  <div class="relative inline-block">
    <div ref="containerRef" class="block overflow-hidden rounded-lg shadow-[0_4px_24px_rgba(0,0,0,0.3)]" />
    <div
      ref="tooltipRef"
      class="pointer-events-none fixed z-10 hidden rounded border border-slate-600 bg-slate-900/90 px-2.5 py-1.5 text-xs text-white"
    />
  </div>
</template>
