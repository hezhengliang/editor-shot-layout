import { reactive, watchEffect } from 'vue'
import { Pane } from 'tweakpane'
import {
  params,
  placedDies,
  shots,
  selected,
  selectedShot,
  paneRef,
  paneLeftRef,
  pane,
  paneLeft,
  setPane,
  setPaneLeft,
  num,
  round3,
  fmtUm,
  setStatus,
  isolatedShotUid,
} from './state'
import type { PlacedDie, Preset, Shot } from './types'
import {
  scribeInfo,
  derivedPitchesX,
  derivedPitchesY,
  shotW,
  shotH,
  shotUtilization,
  fieldFitText,
} from './geometry'
import {
  deleteSelected,
  clearAll,
  applyShotParams,
  shotContentValid,
  refreshShotLayout,
  canPlaceInShot,
  canPlaceBoardDie,
  canPlaceRect,
  applyGeometry,
  fillShotWithTemplate,
  isolateShot,
} from './ops'
import { buildAllShotDims, updateSelectionMarker, fitView } from './draw'
import { dieLibrary, saveLibrary } from './library'

const mon = reactive({ dies: 0, shots: 0 })
watchEffect(() => {
  mon.dies = placedDies.value.length
  mon.shots = shots.value.length
})

// 左侧 Tweakpane:布局参数 + Die 模板管理
export function buildLeftPane() {
  if (!paneLeftRef.value) return
  paneLeft?.dispose()
  setPaneLeft(new Pane({ container: paneLeftRef.value, title: '布局参数' }))
  const g = paneLeft!.addFolder({ title: '默认 Die 尺寸' })
  g.addBinding(params, 'cellSizeUm', { label: 'cell (μm)', min: 0.001, step: 0.001 })

  const lib = paneLeft!.addFolder({ title: 'Die 模板管理' })
  const tpl = { name: 'Die 2×2', w: 1000, h: 1000, color: '#4ade80' }
  lib.addBinding(tpl, 'name', { label: '名称' })
  lib.addBinding(tpl, 'w', { label: '宽 W (μm)', min: 0.001, step: 0.001 })
  lib.addBinding(tpl, 'h', { label: '高 H (μm)', min: 0.001, step: 0.001 })
  lib.addBinding(tpl, 'color', { label: '颜色' })
  lib.addButton({ title: '＋ 添加 Die 模板' }).on('click', () => {
    const t: Preset = {
      id: `custom-${Date.now()}`,
      name: tpl.name.trim() || 'Die',
      wUm: round3(tpl.w),
      hUm: round3(tpl.h),
      color: tpl.color,
    }
    dieLibrary.value.push(t)
    saveLibrary()
    setStatus(`已添加模板 ${t.name}(${fmtUm(t.wUm)} × ${fmtUm(t.hUm)})并保存到本地`)
    setTimeout(buildLeftPane, 0)
  })
  if (dieLibrary.value.length > 1) {
    const del = { id: dieLibrary.value[dieLibrary.value.length - 1].id }
    const options: Record<string, string> = {}
    for (const t of dieLibrary.value) options[t.id] = t.name
    lib.addBinding(del, 'id', { label: '选择模板', options })
    lib.addButton({ title: '删除所选模板' }).on('click', () => {
      if (dieLibrary.value.length <= 1) {
        setStatus('至少保留一个 Die 模板')
        return
      }
      const idx = dieLibrary.value.findIndex((t) => t.id === del.id)
      if (idx >= 0) {
        const [removed] = dieLibrary.value.splice(idx, 1)
        saveLibrary()
        setStatus(`已删除模板 ${removed.name}`)
        setTimeout(buildLeftPane, 0)
      }
    })
  }
}

// 在 tweakpane 事件回调里不能立即 dispose 面板(事件分发未结束会抛 TpError),延迟一帧重建
export function rebuildPaneSoon() {
  setTimeout(() => rebuildPane(), 0)
}

export function rebuildPane() {
  if (!paneRef.value) return
  pane?.dispose()
  setPane(new Pane({ container: paneRef.value, title: '属性编辑' }))
  const safe = (label: string, fn: () => void) => {
    try {
      fn()
    } catch (err) {
      setStatus(`⚠ 面板区块[${label}]出错: ${(err as Error).message}`)
    }
  }

  const die = selected.value
  const shot = selectedShot.value

  if (shot) {
    safe(`Shot#${shot.uid}`, () => buildShotPane(shot))
  }
  if (die) {
    safe(die.preset.name, () => buildDiePane(die))
  }

  safe('信息', () => {
    const info = pane!.addFolder({ title: '信息' })
    info.addBinding(mon, 'dies', { label: 'Die 数量', readonly: true, interval: 300 })
    info.addBinding(mon, 'shots', { label: 'Shot 数量', readonly: true, interval: 300 })
  })

  safe('操作', () => {
    const ops = pane!.addFolder({ title: '操作' })
    ops.addButton({ title: '删除选中' }).on('click', deleteSelected)
    ops.addButton({ title: '清空布局' }).on('click', clearAll)
    ops.addButton({ title: '重置视图' }).on('click', fitView)
  })
}

function buildShotPane(shot: Shot) {
    // 模块一:信息(只读)
    const info = pane!.addFolder({ title: `Shot#${shot.uid} 信息` })
    const sc = scribeInfo(shot)
    const infoProxy = {
      概要: `Shot#${shot.uid} · ${shot.cols}×${shot.rows} 槽位`,
      step: `X ${derivedPitchesX(shot)} / Y ${derivedPitchesY(shot)}`,
      inner: `${sc.ix} / ${sc.iy}(最小 ${fmtUm(shot.gapX)} / ${fmtUm(shot.gapY)})`,
      outer: `${sc.ox} / ${sc.oy}(pad ${fmtUm(shot.padX)} / ${fmtUm(shot.padY)})`,
      size: `外形 ${fmtUm(shotW(shot))} × ${fmtUm(shotH(shot))}`,
      pos: `位置 (${fmtUm(num(shot.group.x))}, ${fmtUm(num(shot.group.y))})`,
      util: `${shotUtilization(shot)}%`,
      field: fieldFitText(shot),
    }
    info.addBinding(infoProxy, '概要', { readonly: true })
    info.addBinding(infoProxy, 'step', { label: '槽位间距(导出)', readonly: true })
    info.addBinding(infoProxy, 'inner', { label: '内圈 scribe 实际 X / Y', readonly: true })
    info.addBinding(infoProxy, 'outer', { label: '外圈 scribe 实际 X / Y', readonly: true })
    info.addBinding(infoProxy, 'size', { label: '外形 W × H', readonly: true })
    info.addBinding(infoProxy, 'pos', { label: '位置 X, Y', readonly: true })
    info.addBinding(infoProxy, 'util', { label: '场利用率 (Die/Shot)', readonly: true })
    info.addBinding(infoProxy, 'field', { label: '曝光场适配', readonly: true })

    // 模块二:配置(可调整)
    const f = pane!.addFolder({ title: 'Shot 配置' })

    // 排布预设(半导体常见阵列):选预设一键设置槽位阵列
    const PRESETS: Record<string, { cols: number; rows: number; label: string }> = {
      p22: { cols: 2, rows: 2, label: '2×2' },
      p44: { cols: 4, rows: 4, label: '4×4 标准' },
      p48: { cols: 4, rows: 8, label: '4×8 存储阵列' },
      p26: { cols: 2, rows: 6, label: '2×6 长条' },
      p64: { cols: 6, rows: 4, label: '6×4' },
    }
    const presetKey = (cols: number, rows: number) =>
      Object.keys(PRESETS).find((k) => PRESETS[k].cols === cols && PRESETS[k].rows === rows) ?? 'custom'
    const presetProxy = { layout: presetKey(shot.cols, shot.rows) }
    const presetOptions: Record<string, string> = { custom: '自定义' }
    for (const k of Object.keys(PRESETS)) presetOptions[k] = PRESETS[k].label
    f.addBinding(presetProxy, 'layout', { label: '排布预设', options: presetOptions }).on('change', () => {
      const p = PRESETS[presetProxy.layout]
      if (!p) return
      const ok = applyShotParams(shot, { cols: p.cols, rows: p.rows })
      setStatus(ok ? `已应用预设 ${p.label}` : '应用预设失败:与画布元素重叠,已还原')
      rebuildPaneSoon()
    })

    // 阵列填充(CAD 风):用 Shot 内首颗 Die 的模板/尺寸填满所有空槽位
    f.addButton({ title: '填充空槽(用 Shot 内首颗 Die)' }).on('click', () => {
      const first = shot.dies[0]
      const preset: Preset = first
        ? { ...first.preset, wUm: first.wUm, hUm: first.hUm }
        : dieLibrary.value[0]
      if (!preset) {
        setStatus('没有可用的 Die 模板')
        return
      }
      const n = fillShotWithTemplate(shot, preset)
      setStatus(n ? `已用 ${preset.name} 填充 ${n} 个空槽位` : '没有可填充的空槽位')
      rebuildPaneSoon()
    })
    f.addButton({
      title: isolatedShotUid.value === shot.uid ? '退出隔离(双击 Shot 亦可)' : '隔离此 Shot(双击 Shot 亦可)',
    }).on('click', () => {
      isolateShot(isolatedShotUid.value === shot.uid ? null : shot)
      rebuildPaneSoon()
    })
    f.addBinding(shot, 'showDims', { label: '显示间距标注' }).on('change', () => {
      buildAllShotDims()
      setStatus(shot.showDims ? `Shot#${shot.uid} 间距标注已显示` : `Shot#${shot.uid} 间距标注已隐藏`)
    })
    f.addBinding(shot, 'showOuter', { label: '显示外圈/容器尺寸' }).on('change', () => {
      buildAllShotDims()
      setStatus(shot.showOuter ? `Shot#${shot.uid} 外圈/容器标注已显示` : `Shot#${shot.uid} 外圈/容器标注已隐藏`)
    })
    // Figma 风格:所有参数即时生效,非法输入自动还原并提示
    const cfgProxy = { cols: shot.cols, rows: shot.rows, padX: shot.padX, padY: shot.padY }
    const lastCfg = { cols: shot.cols, rows: shot.rows, padX: shot.padX, padY: shot.padY }

    const applyShotCfg = (key: 'cols' | 'rows' | 'padX' | 'padY') => {
      const patch =
        key === 'cols'
          ? { cols: Math.round(cfgProxy.cols) }
          : key === 'rows'
            ? { rows: Math.round(cfgProxy.rows) }
            : { [key]: cfgProxy[key] }
      const ok = applyShotParams(shot, patch)
      if (ok) {
        lastCfg[key] = key === 'cols' ? shot.cols : key === 'rows' ? shot.rows : key === 'padX' ? shot.padX : shot.padY
        cfgProxy[key] = lastCfg[key]
        setStatus(`Shot#${shot.uid} ${key} 已更新`)
      } else {
        cfgProxy[key] = lastCfg[key]
        setStatus('参数无效:与画布元素重叠或越界,已还原')
        rebuildPaneSoon()
      }
    }

    f.addBinding(cfgProxy, 'cols', { label: '槽位列数', min: 1, max: 12, step: 1 }).on('change', () => applyShotCfg('cols'))
    f.addBinding(cfgProxy, 'rows', { label: '槽位行数', min: 1, max: 12, step: 1 }).on('change', () => applyShotCfg('rows'))
    f.addBinding(cfgProxy, 'padX', { label: '外圈 scribe padX (μm)', min: 0.001, step: 0.001 }).on('change', () => applyShotCfg('padX'))
    f.addBinding(cfgProxy, 'padY', { label: '外圈 scribe padY (μm)', min: 0.001, step: 0.001 }).on('change', () => applyShotCfg('padY'))

    // 阵列间距 gapX / gapY:相邻 Die 之间的距离,实时生效
    const gapProxy = { gapX: shot.gapX, gapY: shot.gapY }
    const lastGap = { x: shot.gapX, y: shot.gapY }
    f.addBinding(gapProxy, 'gapX', { label: '内圈 scribe 最小 gapX (μm)', min: 0, step: 0.001 }).on('change', () => {
      shot.gapX = Math.max(0, gapProxy.gapX)
      if (
        shotContentValid(shot) &&
        canPlaceRect(num(shot.group.x), num(shot.group.y), shotW(shot), shotH(shot), shot)
      ) {
        lastGap.x = shot.gapX
        refreshShotLayout(shot)
        setStatus(`内圈 scribe gapX 已更新为 ${fmtUm(shot.gapX)}`)
      } else {
        shot.gapX = lastGap.x
        gapProxy.gapX = lastGap.x
        setStatus('inner scribe 过大或网格过小:内容放不下,已还原')
        rebuildPaneSoon()
      }
    })
    f.addBinding(gapProxy, 'gapY', { label: '内圈 scribe 最小 gapY (μm)', min: 0, step: 0.001 }).on('change', () => {
      shot.gapY = Math.max(0, gapProxy.gapY)
      if (
        shotContentValid(shot) &&
        canPlaceRect(num(shot.group.x), num(shot.group.y), shotW(shot), shotH(shot), shot)
      ) {
        lastGap.y = shot.gapY
        refreshShotLayout(shot)
        setStatus(`内圈 scribe gapY 已更新为 ${fmtUm(shot.gapY)}`)
      } else {
        shot.gapY = lastGap.y
        gapProxy.gapY = lastGap.y
        setStatus('inner scribe 过大或网格过小:内容放不下,已还原')
        rebuildPaneSoon()
      }
    })

}

function buildDiePane(die: PlacedDie) {
    const posText = die.shot
      ? `Shot#${die.shot.uid} · 槽位 (${die.slot?.i}, ${die.slot?.j})`
      : `棋盘 (${die.col}, ${die.row})`
    // 模块一:信息(只读)
    const info = pane!.addFolder({ title: `${die.preset.name} 信息` })
    const ip = {
      位置: posText,
      尺寸: `${fmtUm(die.wUm)} × ${fmtUm(die.hUm)}`,
    }
    info.addBinding(ip, '位置', { readonly: true })
    info.addBinding(ip, '尺寸', { readonly: true })

    // 模块二:配置(可调整)
    const f = pane!.addFolder({ title: 'Die 配置' })
    if (die.shot) {
      const s = die.shot
      const proxy = { i: die.slot?.i ?? 0, j: die.slot?.j ?? 0 }
      f.addBinding(proxy, 'i', { label: '槽位 X', min: 0, max: s.cols - 1, step: 1 })
      f.addBinding(proxy, 'j', { label: '槽位 Y', min: 0, max: s.rows - 1, step: 1 })
      f.addButton({ title: '应用位置' }).on('click', () => {
        const i = Math.round(proxy.i)
        const j = Math.round(proxy.j)
        if (canPlaceInShot(s, i, j, die.wUm, die.hUm, die)) {
          die.slot = { i, j }
          applyGeometry(die)
          refreshShotLayout(s)
          setStatus(`已移动到槽位 (${i}, ${j})`)
        } else {
          setStatus('槽位已占用或空间不足')
        }
        rebuildPane()
      })
      // CAD 阵列填充:用当前 Die 的模板+尺寸填满 Shot 所有空槽位
      f.addButton({ title: '填充 Shot 空槽(用此 Die)' }).on('click', () => {
        const preset: Preset = { ...die.preset, wUm: die.wUm, hUm: die.hUm }
        const n = fillShotWithTemplate(s, preset)
        setStatus(n ? `已填充 ${n} 个空槽位` : '没有可填充的空槽位')
        rebuildPaneSoon()
      })
    } else {
      const proxy = { col: die.col, row: die.row }
      f.addBinding(proxy, 'col', { label: '列 X', min: 0, max: params.bigCols - 1, step: 1 })
      f.addBinding(proxy, 'row', { label: '行 Y', min: 0, max: params.bigRows - 1, step: 1 })
      f.addButton({ title: '应用位置' }).on('click', () => {
        const col = Math.round(proxy.col)
        const row = Math.round(proxy.row)
        if (canPlaceBoardDie(col, row, die.wUm, die.hUm, die)) {
          die.col = col
          die.row = row
          applyGeometry(die)
          setStatus(`已移动到 (${col}, ${row})`)
        } else {
          setStatus('目标位置越界或重叠')
        }
        rebuildPane()
      })
    }
    const sizeProxy = { w: die.wUm, h: die.hUm }
    f.addBinding(sizeProxy, 'w', { label: '宽 W (μm)', min: 0.001, step: 0.001 })
    f.addBinding(sizeProxy, 'h', { label: '高 H (μm)', min: 0.001, step: 0.001 })
    f.addButton({ title: '应用尺寸' }).on('click', () => {
      const w = Math.round(sizeProxy.w * 1000) / 1000
      const h = Math.round(sizeProxy.h * 1000) / 1000
      let ok = false
      if (die.shot && die.slot) {
        ok = canPlaceInShot(die.shot, die.slot.i, die.slot.j, w, h, die)
      } else {
        ok = canPlaceBoardDie(die.col, die.row, w, h, die)
      }
      if (ok) {
        die.wUm = w
        die.hUm = h
        applyGeometry(die)
        if (die.shot) refreshShotLayout(die.shot)
        updateSelectionMarker()
        setStatus(`尺寸已更新为 ${fmtUm(w)} × ${fmtUm(h)}`)
      } else {
        setStatus('该尺寸放不下:与相邻元素重叠或超出范围')
      }
      rebuildPaneSoon()
    })
    const colorProxy = { color: die.preset.color }
    f.addBinding(colorProxy, 'color', { label: '颜色' }).on('change', () => {
      die.rect.fill = colorProxy.color
    })

    // 把当前已配置好的 die 保存为模板(含尺寸),持久化到本地
    const saveProxy = { name: `${die.preset.name}` }
    f.addBinding(saveProxy, 'name', { label: '模板名称' })
    f.addButton({ title: '保存为 Die 模板' }).on('click', () => {
      const t: Preset = {
        id: `custom-${Date.now()}`,
        name: saveProxy.name.trim() || die.preset.name,
        wUm: round3(die.wUm),
        hUm: round3(die.hUm),
        color: die.preset.color,
      }
      dieLibrary.value.push(t)
      saveLibrary()
      setStatus(`已保存模板 ${t.name}(${fmtUm(t.wUm)} × ${fmtUm(t.hUm)})到本地库`)
      rebuildPaneSoon()
    })
}
