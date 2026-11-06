// 应用状态与核心操作(放置/校验/填充/参数应用),与 web 版 ops.ts 语义对齐

use crate::geometry::*;
use crate::model::*;
use egui::{Color32, Pos2, Vec2};

pub const RULER: f32 = 26.0;

#[derive(Clone, Copy, PartialEq)]
pub enum AppView {
    Editor,
    Bitmap,
}

pub struct DieLayoutApp {
    pub presets: Vec<Preset>,
    pub dies: Vec<Die>,
    pub shots: Vec<Shot>,
    pub next_id: u64,

    pub selected: Selection,
    pub isolated: Option<ShotId>,
    pub right_tab: RightTab,
    pub theme_dark: bool,
    pub status: String,

    // 视图:screen = offset + world * zoom(世界原点钉在标尺左上角,不允许负坐标)
    pub zoom: f32,
    pub offset: Vec2,

    pub drag: Option<DragState>,      // 侧栏卡片拖拽
    pub canvas_drag: Option<CanvasDrag>,
    pub pointer_world: Option<(f64, f64)>, // 画布内指针世界坐标(μm,f64 保精度)
    pub hover: Selection,             // 悬停高亮
    pub tool: ToolMode,               // 画布工具(选择/测量标尺)
    pub measure_rulers: Vec<MeasureRuler>, // 测量标尺(KLayout 式)
    pub guides: Vec<Guide>,           // Shot 拖拽对齐参考线(当帧有效)
    pub canvas_rect: egui::Rect,      // 画布可视区域(屏幕坐标,每帧刷新;绘制裁剪用)
    pub needs_fit: bool,              // 首帧自动居中内容
    pub frame_count: u64,             // 帧计数(调试用)
    pub view: AppView,                // 顶层视图:布局编辑器 / BitMap
    pub nav_collapsed: bool,          // 左侧导航折叠
    pub bitmap: Option<crate::bitmap::BitmapView>, // 懒加载(首次切换时创建,内部会尝试连本地服务)
}

impl DieLayoutApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        apply_text_styles(&cc.egui_ctx);
        let dark = std::env::var("EGUI_DARK").is_ok(); // shadcn blue 默认 light,顶栏可切 dark
        apply_theme(&cc.egui_ctx, dark);
        let mut app = Self {
            presets: vec![
                Preset { id: "d1".into(), name: "Die 1×1".into(), w: 500.0, h: 500.0, color: Color32::from_rgb(0x4a, 0xde, 0x80) },
                Preset { id: "d2".into(), name: "Die 2×2".into(), w: 1000.0, h: 1000.0, color: Color32::from_rgb(0x60, 0xa5, 0xfa) },
            ],
            dies: vec![],
            shots: vec![],
            next_id: 1,
            selected: Selection::None,
            isolated: None,
            right_tab: if std::env::var("EGUI_TAB").ok().as_deref() == Some("info") {
                RightTab::Info
            } else {
                RightTab::Config
            },
            theme_dark: dark,
            status: "从左侧把 Die 拖入 Shot,或点击卡片自动放置".into(),
            zoom: 0.6,
            offset: Vec2::new(RULER + 40.0, RULER + 40.0),
            drag: None,
            canvas_drag: None,
            pointer_world: None,
            hover: Selection::None,
            tool: ToolMode::Select,
            measure_rulers: Vec::new(),
            guides: Vec::new(),
            canvas_rect: egui::Rect::NOTHING,
            needs_fit: true,
            frame_count: 0,
            view: if std::env::var("EGUI_VIEW").ok().as_deref() == Some("bitmap") {
                AppView::Bitmap
            } else {
                AppView::Editor
            },
            nav_collapsed: false,
            bitmap: None,
        };
        // 调试:EGUI_ZOOM=0.15 固定初始缩放(跳过首帧 fit)
        if let Ok(z) = std::env::var("EGUI_ZOOM").and_then(|v| v.parse::<f32>().map_err(|_| std::env::VarError::NotPresent)) {
            app.zoom = z;
            app.needs_fit = false;
        }
        app.auto_place_shot();
        // 调试:EGUI_DEMO=1 填充 2×2 并开标注;EGUI_DEMO=2 再把首颗 die 缩到 300(间距应变大)
        if std::env::var("EGUI_DEMO").is_ok() {
            if let Some(sid) = app.shots.first().map(|s| s.id) {
                let n = app.fill_shot(sid, 0);
                app.set_status(format!("DEMO:已填充 {} 个 die", n));
                if let Some(s) = app.shots.iter_mut().find(|s| s.id == sid) {
                    s.show_dims = true;
                    s.show_outer = true;
                }
                if std::env::var("EGUI_DEMO").ok().as_deref() == Some("2") {
                    if let Some(d) = app.dies.first_mut() {
                        d.w = 300.0;
                        d.h = 300.0;
                    }
                }
                if std::env::var("EGUI_DEMO").ok().as_deref() == Some("3") {
                    if let Some(d) = app.dies.first_mut() {
                        d.w = 333.333;
                        d.h = 456.789;
                    }
                }
                if std::env::var("EGUI_DEMO").ok().as_deref() == Some("4") {
                    if let Some(d) = app.dies.first() {
                        app.selected = Selection::Die(d.id);
                    }
                }
                if std::env::var("EGUI_DEMO").ok().as_deref() == Some("5") {
                    // 第二个 Shot + 测量标尺:验证对齐参考线/标尺绘制
                    if let Some(s2) = app.shots.first().map(|s| {
                        let r = shot_rect(s, &app.dies);
                        (r.x + r.w, r.y)
                    }) {
                        app.create_shot(s2.0, s2.1);
                    }
                    app.measure_rulers.push(crate::model::MeasureRuler { a: (0.0, 0.0), b: (620.0, 460.0) });
                    app.measure_rulers.push(crate::model::MeasureRuler { a: (80.0, 220.0), b: (80.0, 220.0 + 500.0) });
                }
            }
        }
        app
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
    }

    /// 状态栏用:选中对象的人类可读描述
    pub fn describe_selection(&self) -> String {
        match self.selected {
            Selection::Die(id) => {
                if let Some(d) = self.dies.iter().find(|d| d.id == id) {
                    format!("Die #{} · {} × {} · Shot #{} 槽位 ({}, {})", d.id.0, fmt_um(d.w), fmt_um(d.h), d.shot.0, d.slot.0, d.slot.1)
                } else {
                    "Die".into()
                }
            }
            Selection::Shot(id) => {
                if let Some(s) = self.shots.iter().find(|s| s.id == id) {
                    let r = shot_rect(s, &self.dies);
                    format!("Shot #{} · {} × {} · {}×{} 槽", id.0, fmt_um(r.w), fmt_um(r.h), s.cols, s.rows)
                } else {
                    "Shot".into()
                }
            }
            Selection::None => "未选中".into(),
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    // ---------- 视图换算 ----------
    pub fn w2s(&self, x: f64, y: f64) -> Pos2 {
        Pos2::new(
            self.offset.x + x as f32 * self.zoom,
            self.offset.y + y as f32 * self.zoom,
        )
    }
    pub fn s2w(&self, p: Pos2) -> (f64, f64) {
        (
            (p.x - self.offset.x) as f64 / self.zoom as f64,
            (p.y - self.offset.y) as f64 / self.zoom as f64,
        )
    }
    /// 视图钳制:仅钳缩放;视图允许自由平移(元素坐标仍不允许负值,由放置校验保证)
    pub fn clamp_view(&mut self) {
        self.zoom = self.zoom.clamp(0.02, 64.0);
    }

    pub fn fit_view(&mut self, canvas: egui::Rect) {
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut any = false;
        for s in &self.shots {
            let r = shot_rect(s, &self.dies);
            any = true;
            min_x = min_x.min(r.x);
            min_y = min_y.min(r.y);
            max_x = max_x.max(r.x + r.w);
            max_y = max_y.max(r.y + r.h);
        }
        if !any {
            // 空画布:默认视野约 8000μm 见方,世界原点居中
            self.zoom = (canvas.width().min(canvas.height()) / 8000.0).clamp(0.02, 64.0);
            self.offset = Vec2::new(canvas.min.x + canvas.width() / 2.0, canvas.min.y + canvas.height() / 2.0);
            return;
        }
        let pad = 80.0;
        let s = ((canvas.width() - pad * 2.0) / (max_x - min_x) as f32)
            .min((canvas.height() - pad * 2.0) / (max_y - min_y) as f32)
            .min(2.0)
            .max(0.02);
        self.zoom = s;
        // 注意:offset 是绝对屏幕坐标(画布区左上角为 canvas.min,不是 (0,0))
        self.offset = Vec2::new(
            canvas.min.x + (canvas.width() - (max_x - min_x) as f32 * s) / 2.0 - min_x as f32 * s,
            canvas.min.y + (canvas.height() - (max_y - min_y) as f32 * s) / 2.0 - min_y as f32 * s,
        );
        self.clamp_view();
    }

    // ---------- 放置校验 ----------
    pub fn can_place_rect(&self, x: f64, y: f64, w: f64, h: f64, ignore: Option<ShotId>) -> bool {
        if x < 0.0 || y < 0.0 {
            return false;
        }
        let r = WRect { x, y, w, h };
        for s in &self.shots {
            if Some(s.id) == ignore {
                continue;
            }
            if r.overlaps(&shot_rect(s, &self.dies)) {
                return false;
            }
        }
        true
    }

    pub fn can_place_in_shot(&self, shot_id: ShotId, i: usize, j: usize, w: f64, h: f64, ignore: Option<DieId>) -> bool {
        let Some(shot) = self.shots.iter().find(|s| s.id == shot_id) else { return false };
        if i >= shot.cols || j >= shot.rows {
            return false;
        }
        if slot_occupied(shot, i, j, &self.dies, ignore) {
            return false;
        }
        // 用临时 die 验证尺寸推导后不与棋盘其他元素冲突
        let mut dies = self.dies.clone();
        if let Some(ig) = ignore {
            dies.retain(|d| d.id != ig);
        }
        let pseudo = Die {
            id: DieId(u64::MAX),
            name: "p".into(),
            w,
            h,
            color: Color32::GRAY,
            shot: shot_id,
            slot: (i, j),
        };
        dies.push(pseudo);
        let test_shot = shot.clone();
        // 固定外形 + 最小内圈间距:行/列容量校验(自动间距模型下打包互不重叠)
        if !content_fits(&test_shot, &dies) {
            return false;
        }
        // 与画布其他元素不重叠
        let r = shot_rect(&test_shot, &dies);
        self.can_place_rect(r.x, r.y, r.w, r.h, Some(shot_id))
    }

    // ---------- 创建/删除 ----------
    pub fn create_shot(&mut self, x: f64, y: f64) -> ShotId {
        let id = ShotId(self.alloc_id());
        // 固定外形:按空槽占位宽 CELL + 最小内圈间距推导初始 W×H
        let pad = 80.0f64;
        let gap = 80.0f64;
        let w = pad * 2.0 + 2.0 * CELL + gap;
        let h = pad * 2.0 + 2.0 * CELL + gap;
        self.shots.push(Shot {
            id,
            x,
            y,
            w,
            h,
            cols: 2,
            rows: 2,
            gap_x: gap,
            gap_y: gap,
            pad_x: pad,
            pad_y: pad,
            show_dims: true,
            show_outer: false,
            dies: vec![],
        });
        self.selected = Selection::Shot(id);
        self.set_status(format!("已创建 Shot#{}", id.0));
        id
    }

    pub fn place_die(&mut self, shot_id: ShotId, preset_idx: usize, i: usize, j: usize) -> Option<DieId> {
        let p = self.presets.get(preset_idx)?.clone();
        if !self.can_place_in_shot(shot_id, i, j, p.w, p.h, None) {
            self.set_status("该槽位已占用或空间不足(一个槽位只能放一个 Die)");
            return None;
        }
        let id = DieId(self.alloc_id());
        let die = Die {
            id,
            name: p.name.clone(),
            w: p.w,
            h: p.h,
            color: p.color,
            shot: shot_id,
            slot: (i, j),
        };
        self.dies.push(die);
        if let Some(s) = self.shots.iter_mut().find(|s| s.id == shot_id) {
            s.dies.push(id);
        }
        Some(id)
    }

    pub fn remove_die(&mut self, id: DieId) {
        self.dies.retain(|d| d.id != id);
        for s in &mut self.shots {
            s.dies.retain(|&d| d != id);
        }
        if self.selected == Selection::Die(id) {
            self.selected = Selection::None;
        }
    }

    pub fn remove_shot(&mut self, id: ShotId) {
        if let Some(s) = self.shots.iter().find(|s| s.id == id) {
            let ids = s.dies.clone();
            for d in ids {
                self.remove_die(d);
            }
        }
        self.shots.retain(|s| s.id != id);
        if self.selected == Selection::Shot(id) {
            self.selected = Selection::None;
        }
        if self.isolated == Some(id) {
            self.isolated = None;
        }
    }

    pub fn delete_selected(&mut self) {
        match self.selected {
            Selection::Die(id) => {
                self.remove_die(id);
                self.set_status("已删除 Die");
            }
            Selection::Shot(id) => {
                self.remove_shot(id);
                self.set_status("已删除 Shot(含内部 Die)");
            }
            Selection::None => {}
        }
    }

    pub fn clear_all(&mut self) {
        self.dies.clear();
        self.shots.clear();
        self.selected = Selection::None;
        self.isolated = None;
        self.set_status("已清空布局");
    }

    // ---------- 阵列填充 / 预设 ----------
    pub fn fill_shot(&mut self, shot_id: ShotId, preset_idx: usize) -> usize {
        let Some(shot) = self.shots.iter().find(|s| s.id == shot_id) else { return 0 };
        let (cols, rows) = (shot.cols, shot.rows);
        let mut n = 0;
        for j in 0..rows {
            for i in 0..cols {
                if self.place_die(shot_id, preset_idx, i, j).is_some() {
                    n += 1;
                }
            }
        }
        n
    }


    pub fn apply_shot_params(
        &mut self,
        id: ShotId,
        cols: usize,
        rows: usize,
        pad_x: f64,
        pad_y: f64,
        gap_x: f64,
        gap_y: f64,
        w: f64,
        h: f64,
    ) -> bool {
        let Some(idx) = self.shots.iter().position(|s| s.id == id) else { return false };
        let old = self.shots[idx].clone();
        self.shots[idx].cols = cols;
        self.shots[idx].rows = rows;
        self.shots[idx].pad_x = pad_x;
        self.shots[idx].pad_y = pad_y;
        self.shots[idx].gap_x = gap_x.max(0.0);
        self.shots[idx].gap_y = gap_y.max(0.0);
        self.shots[idx].w = w.max(1.0);
        self.shots[idx].h = h.max(1.0);
        // 校验:不越界、不与其他 shot 重叠、固定外形下内容(最小间距约束)放得下
        let r = shot_rect(&self.shots[idx], &self.dies);
        if !self.can_place_rect(r.x, r.y, r.w, r.h, Some(id)) || !content_fits(&self.shots[idx], &self.dies) {
            self.shots[idx] = old;
            self.set_status("Shot 新参数无效:与画布元素重叠/越界,或内容在最小间距下放不下,已还原");
            return false;
        }
        let shot = self.shots[idx].clone();
        let mut removed = 0;
        for k in (0..shot.dies.len()).rev() {
            let die_id = shot.dies[k];
            let Some(die) = self.dies.iter().find(|d| d.id == die_id).cloned() else { continue };
            let (i, j) = die.slot;
            let fits = i < cols && j < rows && !slot_occupied(&shot, i, j, &self.dies, Some(die_id));
            if !fits {
                self.remove_die(die_id);
                removed += 1;
            }
        }
        if removed > 0 {
            self.set_status(format!("Shot 参数已更新,{} 个放不下的 Die 已移除", removed));
        } else {
            self.set_status("Shot 参数已更新");
        }
        true
    }

    /// 手柄拖动中的实时 resize:校验通过立即改尺寸(跟手);失败返回 false 由画布画红色预览
    pub fn resize_die_live(&mut self, id: DieId, w: f64, h: f64) -> bool {
        let Some(die) = self.dies.iter().find(|d| d.id == id).cloned() else { return false };
        let nw = (w * 1000.0).round() / 1000.0;
        let nh = (h * 1000.0).round() / 1000.0;
        if !self.can_place_in_shot(die.shot, die.slot.0, die.slot.1, nw, nh, Some(id)) {
            return false;
        }
        if let Some(d) = self.dies.iter_mut().find(|d| d.id == id) {
            d.w = nw;
            d.h = nh;
        }
        self.set_status(format!("resize → {} × {}", fmt_um(nw), fmt_um(nh)));
        true
    }


    // ---------- 隔离 ----------
    pub fn toggle_isolate(&mut self, id: ShotId) {
        if self.isolated == Some(id) {
            self.isolated = None;
            self.set_status("已退出隔离,显示全部 Shot");
        } else {
            self.isolated = Some(id);
            self.set_status(format!("已隔离 Shot#{}(双击它或按 Esc 退出)", id.0));
        }
    }

    // ---------- 点击自动放置 ----------
    pub fn auto_place_shot(&mut self) {
        let probe = self.shot_probe();
        let free = self.scan_free(probe.0, probe.1);
        if let Some((x, y)) = free {
            self.create_shot(x, y);
            self.set_status("已创建 Shot(2×2 槽位)");
        } else {
            self.set_status("画布上没有能放下 Shot 的空位");
        }
    }

    pub fn auto_place_die(&mut self, preset_idx: usize) {
        for si in 0..self.shots.len() {
            let (cols, rows, id) = {
                let s = &self.shots[si];
                (s.cols, s.rows, s.id)
            };
            for j in 0..rows {
                for i in 0..cols {
                    if self.can_place_in_shot(id, i, j, self.presets[preset_idx].w, self.presets[preset_idx].h, None) {
                        if let Some(did) = self.place_die(id, preset_idx, i, j) {
                            self.selected = Selection::Die(did);
                            return;
                        }
                    }
                }
            }
        }
        self.set_status("没有可用的 Shot 槽位(请先创建 Shot)");
    }

    pub fn shot_probe(&self) -> (f64, f64) {
        let pad = CELL / 2.0;
        (pad * 2.0 + CELL * 2.0, pad * 2.0 + CELL * 2.0)
    }

    pub fn scan_free(&self, w: f64, h: f64) -> Option<(f64, f64)> {
        for row in 0..64 {
            for col in 0..64 {
                let (x, y) = (col as f64 * CELL, row as f64 * CELL);
                if self.can_place_rect(x, y, w, h, None) {
                    return Some((x, y));
                }
            }
        }
        None
    }
}

/// 整体字号下调一档(贴近 web 版 11~12px 的紧凑面板密度)
pub fn apply_text_styles(ctx: &egui::Context) {
    use egui::{FontFamily, FontId, TextStyle};
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(15.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(10.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(11.0, FontFamily::Monospace)),
    ]
    .into();
    ctx.set_style(style);
}

/// shadcn blue 主题:zinc 基底 + blue 强调(light primary #2563eb / dark #3b82f6)
pub fn apply_theme(ctx: &egui::Context, dark: bool) {
    let z = |r: u8, g: u8, b: u8| egui::Color32::from_rgb(r, g, b);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let r6 = egui::CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.corner_radius = r6;
    w.inactive.corner_radius = r6;
    w.hovered.corner_radius = r6;
    w.active.corner_radius = r6;
    w.open.corner_radius = r6;
    if dark {
        v.window_fill = z(0x09, 0x09, 0x0b); // zinc-950
        v.extreme_bg_color = z(0x09, 0x09, 0x0b); // input 底
        v.faint_bg_color = z(0x18, 0x18, 0x1b); // zinc-900 card
        v.window_stroke = egui::Stroke::new(1.0_f32, z(0x27, 0x27, 0x2a)); // zinc-800
        v.widgets.noninteractive.bg_fill = z(0x09, 0x09, 0x0b);
        v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, z(0xa1, 0xa1, 0xaa)); // zinc-400
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, z(0x3f, 0x3f, 0x46)); // input border
        v.widgets.inactive.weak_bg_fill = z(0x27, 0x27, 0x2a); // zinc-800
        v.widgets.inactive.bg_fill = z(0x27, 0x27, 0x2a);
        v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, z(0xfa, 0xfa, 0xfa));
        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, z(0x27, 0x27, 0x2a));
        v.widgets.inactive.expansion = 0.0;
        v.widgets.hovered.weak_bg_fill = z(0x2a, 0x2a, 0x2e);
        v.widgets.hovered.bg_fill = z(0x2a, 0x2a, 0x2e);
        v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, z(0xfa, 0xfa, 0xfa));
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, z(0x27, 0x27, 0x2a));
        v.widgets.hovered.expansion = 0.0;
        v.widgets.active.weak_bg_fill = z(0x2e, 0x2e, 0x33);
        v.widgets.active.bg_fill = z(0x2e, 0x2e, 0x33);
        v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, z(0xfa, 0xfa, 0xfa));
        v.widgets.open.bg_fill = z(0x27, 0x27, 0x2a);
        v.selection.bg_fill = z(0x3b, 0x82, 0xf6).linear_multiply(0.35); // blue-500
        v.selection.stroke = egui::Stroke::new(2.0_f32, z(0x3b, 0x82, 0xf6)); // 兼作 input focus ring
        v.hyperlink_color = z(0x60, 0xa5, 0xfa); // blue-400
    } else {
        v.window_fill = z(0xff, 0xff, 0xff);
        v.extreme_bg_color = z(0xff, 0xff, 0xff);
        v.faint_bg_color = z(0xf4, 0xf4, 0xf5); // zinc-100
        v.window_stroke = egui::Stroke::new(1.0_f32, z(0xe4, 0xe4, 0xe7)); // zinc-200
        v.widgets.noninteractive.bg_fill = z(0xff, 0xff, 0xff);
        v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, z(0x71, 0x71, 0x7a)); // zinc-500
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, z(0xe4, 0xe4, 0xe7));
        v.widgets.inactive.weak_bg_fill = z(0xf4, 0xf4, 0xf5);
        v.widgets.inactive.bg_fill = z(0xf4, 0xf4, 0xf5);
        v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, z(0x18, 0x18, 0x1b));
        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, z(0xe4, 0xe4, 0xe7));
        v.widgets.inactive.expansion = 0.0;
        v.widgets.hovered.weak_bg_fill = z(0xe4, 0xe4, 0xe7);
        v.widgets.hovered.bg_fill = z(0xe4, 0xe4, 0xe7);
        v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, z(0x18, 0x18, 0x1b));
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, z(0xe4, 0xe4, 0xe7));
        v.widgets.hovered.expansion = 0.0;
        v.widgets.active.weak_bg_fill = z(0xd4, 0xd4, 0xd8);
        v.widgets.active.bg_fill = z(0xd4, 0xd4, 0xd8);
        v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, z(0x18, 0x18, 0x1b));
        v.widgets.open.bg_fill = z(0xf4, 0xf4, 0xf5);
        v.selection.bg_fill = z(0x25, 0x63, 0xeb).linear_multiply(0.18); // blue-600
        v.selection.stroke = egui::Stroke::new(2.0_f32, z(0x25, 0x63, 0xeb)); // 兼作 input focus ring
        v.hyperlink_color = z(0x25, 0x63, 0xeb);
    }
    ctx.set_visuals(v);
}

/// 按钮样式(shadcn blue 语义)
#[derive(Clone, Copy, PartialEq)]
pub enum KitBtn {
    /// 强调:blue 蓝底白字(light #2563eb / dark #3b82f6)
    Primary,
    /// 常规:control 底(备用变体)
    #[allow(dead_code)]
    Secondary,
    /// 透明底 + 边框
    Outline,
    /// 危险:red 红底白字(#dc2626)
    Destructive,
}

/// 自绘按钮:h8(32px)/px_2p5/rounded-md/text-sm,文案按 ink 包围盒精确水平垂直居中
pub fn kit_button(ui: &mut egui::Ui, text: &str, style: KitBtn) -> egui::Response {
    kit_button_w(ui, text, style, None)
}

/// width = Some(w) 时按钮铺满指定宽(操作区通栏);None 为内容宽
pub fn kit_button_w(ui: &mut egui::Ui, text: &str, style: KitBtn, width: Option<f32>) -> egui::Response {
    let dark = ui.visuals().dark_mode;
    let font = egui::FontId::proportional(13.0);
    let galley = ui.painter().layout(text.to_owned(), font, egui::Color32::WHITE, f32::INFINITY);
    let (ink_l, ink_t, ink_r, ink_b) = ink_bounds(&galley);
    let w = width.unwrap_or((ink_r - ink_l) + 20.0);
    let h = 32.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());

    let (bg, fg, border, hov) = match (style, dark) {
        (KitBtn::Primary, true) => (
            egui::Color32::from_rgb(0x3b, 0x82, 0xf6), // blue-500
            egui::Color32::from_rgb(0xff, 0xff, 0xff),
            egui::Color32::TRANSPARENT,
            egui::Color32::from_rgb(0x60, 0xa5, 0xfa), // hover blue-400
        ),
        (KitBtn::Primary, false) => (
            egui::Color32::from_rgb(0x25, 0x63, 0xeb), // blue-600
            egui::Color32::from_rgb(0xff, 0xff, 0xff),
            egui::Color32::TRANSPARENT,
            egui::Color32::from_rgb(0x1d, 0x4e, 0xd8), // blue-700
        ),
        (KitBtn::Secondary, true) => (
            egui::Color32::from_rgb(0x27, 0x27, 0x2a),
            egui::Color32::from_rgb(0xfa, 0xfa, 0xfa),
            egui::Color32::from_rgb(0x27, 0x27, 0x2a),
            egui::Color32::from_rgb(0x2a, 0x2a, 0x2e),
        ),
        (KitBtn::Secondary, false) => (
            egui::Color32::from_rgb(0xf4, 0xf4, 0xf5),
            egui::Color32::from_rgb(0x18, 0x18, 0x1b),
            egui::Color32::from_rgb(0xe4, 0xe4, 0xe7),
            egui::Color32::from_rgb(0xe4, 0xe4, 0xe7),
        ),
        (KitBtn::Outline, true) => (
            egui::Color32::TRANSPARENT,
            egui::Color32::from_rgb(0xfa, 0xfa, 0xfa),
            egui::Color32::from_rgb(0x3f, 0x3f, 0x46),
            egui::Color32::from_rgb(0x27, 0x27, 0x2a),
        ),
        (KitBtn::Outline, false) => (
            egui::Color32::TRANSPARENT,
            egui::Color32::from_rgb(0x18, 0x18, 0x1b),
            egui::Color32::from_rgb(0xd4, 0xd4, 0xd8),
            egui::Color32::from_rgb(0xf4, 0xf4, 0xf5),
        ),
        (KitBtn::Destructive, _) => (
            egui::Color32::from_rgb(0xdc, 0x26, 0x26), // red-600
            egui::Color32::from_rgb(0xff, 0xff, 0xff),
            egui::Color32::TRANSPARENT,
            egui::Color32::from_rgb(0xb9, 0x1c, 0x1c), // red-700
        ),
    };
    let fill = if resp.hovered() || resp.is_pointer_button_down_on() { hov } else { bg };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6, fill);
    if border != egui::Color32::TRANSPARENT {
        painter.rect_stroke(rect, 6, egui::Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
    }
    let cx = rect.center().x - (ink_l + ink_r) / 2.0;
    let cy = rect.center().y - (ink_t + ink_b) / 2.0;
    painter.galley_with_override_text_color(egui::Pos2::new(cx, cy), galley, fg);
    resp
}

/// primary 按钮
pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    kit_button(ui, text, KitBtn::Primary)
}

/// 文案 ink 包围盒(水平/垂直),自绘控件共用
pub fn ink_bounds(galley: &egui::Galley) -> (f32, f32, f32, f32) {
    let (mut l, mut t, mut r, mut b) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    if let Some(row) = galley.rows.first() {
        for g in &row.glyphs {
            if g.uv_rect.is_nothing() {
                continue;
            }
            let lt = g.pos + g.uv_rect.offset;
            l = l.min(lt.x);
            t = t.min(lt.y);
            r = r.max(lt.x + g.uv_rect.size.x);
            b = b.max(lt.y + g.uv_rect.size.y);
        }
    }
    if l > r {
        l = 0.0;
        r = galley.rect.width();
        t = 0.0;
        b = 13.0;
    }
    (l, t, r, b)
}

/// 自绘 Select(shadcn Select):与 Input 同高同边框,选中项文案 ink 垂直居中,右侧 chevron;
/// 点击弹出 popup 列表,返回变更后的索引
pub fn kit_select(ui: &mut egui::Ui, id_salt: &str, selected: &str, options: &[String]) -> Option<usize> {
    let font = egui::FontId::proportional(13.0);
    let galley = ui.painter().layout(selected.to_owned(), font, egui::Color32::WHITE, f32::INFINITY);
    let (ink_l, ink_t, _ink_r, ink_b) = ink_bounds(&galley);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::click());

    let visuals = ui.visuals().clone();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6, visuals.extreme_bg_color); // 与 input 同底
    let border = if resp.hovered() { visuals.widgets.noninteractive.bg_stroke.color } else { visuals.window_stroke.color };
    painter.rect_stroke(rect, 6, egui::Stroke::new(1.0_f32, border), egui::StrokeKind::Inside);
    // 文案:左内边距 10,垂直按 ink 居中
    let cx = rect.left() + 10.0 - ink_l;
    let cy = rect.center().y - (ink_t + ink_b) / 2.0;
    painter.galley_with_override_text_color(egui::Pos2::new(cx, cy), galley.clone(), visuals.text_color());
    // chevron(右侧,ink 垂直居中)
    let cc = egui::Pos2::new(rect.right() - 16.0, rect.center().y);
    let chev_color = visuals.weak_text_color();
    painter.add(egui::Shape::closed_line(
        vec![
            egui::Pos2::new(cc.x - 4.5, cc.y - 2.0),
            egui::Pos2::new(cc.x + 4.5, cc.y - 2.0),
            egui::Pos2::new(cc.x, cc.y + 3.5),
        ],
        egui::Stroke::new(1.5_f32, chev_color),
    ));

    let popup_id = ui.id().with(("kit_select", id_salt));
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    let mut changed = None;
    egui::containers::popup::popup_below_widget(
        ui,
        popup_id,
        &resp,
        egui::containers::popup::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(rect.width());
            let visuals = ui.visuals().clone();
            for (i, opt) in options.iter().enumerate() {
                let (row, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::click());
                let painter = ui.painter_at(row);
                if resp.hovered() {
                    painter.rect_filled(row, 4, visuals.widgets.inactive.weak_bg_fill); // hover:bg-accent
                }
                // 文案 ink 垂直居中(同按钮/select 的对齐逻辑)
                let g = ui.painter().layout(opt.clone(), egui::FontId::proportional(13.0), egui::Color32::WHITE, f32::INFINITY);
                let (l, t, r, b) = ink_bounds(&g);
                painter.galley_with_override_text_color(
                    egui::Pos2::new(row.left() + 10.0 - l, row.center().y - (t + b) / 2.0),
                    g,
                    visuals.text_color(),
                );
                let _ = r;
                if resp.clicked() {
                    changed = Some(i);
                    ui.memory_mut(|m| m.close_popup());
                }
            }
        },
    );
    changed
}

/// 加载系统中文字体(macOS 苹方/冬青黑)。
/// 关键:把 CJK 字体放在 Proportional 链首位 —— 苹方自带 SF 风格拉丁字形,
/// 中英文/数字全部出自同一字体文件,垂直度量一致(否则 egui 默认拉丁字体与
/// 苹方 ascent/x-height 不同,混排基线对齐后视觉高低不平)。
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fd = egui::FontData::from_owned(bytes);
            fd.index = 0;
            // 苹方 ascent 预留大,egui 按行高盒居中时视觉偏低;整体上移补偿(自绘 ink 居中自动跟随)
            fd.tweak.y_offset_factor = 0.23;
            fonts.font_data.insert("cjk".into(), std::sync::Arc::new(fd));
            fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "cjk".into());
            fonts.families.entry(egui::FontFamily::Monospace).or_default().push("cjk".into());
            ctx.set_fonts(fonts);
            return;
        }
    }
}
