// 画布:无限网格 + 标尺 + Shot/Die 绘制 + 尺寸标注 + 交互(平移/缩放/拖放/点选/双击隔离)

use crate::app::{DieLayoutApp, RULER};
use crate::geometry::*;
use crate::model::*;
use egui::{Align2, Color32, CornerRadius, Key, Modifiers, PointerButton, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};

const NICE_STEPS: &[f64] = &[
    0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0,
    200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0, 50000.0, 100000.0,
];

fn nice_step(zoom: f32, target_px: f32) -> f64 {
    for &s in NICE_STEPS {
        if s * zoom as f64 >= target_px as f64 {
            return s;
        }
    }
    NICE_STEPS[NICE_STEPS.len() - 1]
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas_bg: Color32,
    pub ruler_bg: Color32,
    pub ruler_line: Color32,
    pub ruler_text: Color32,
    pub ruler_minor: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub shot_fill: Color32,
    pub shot_stroke: Color32,
    pub shot_grid: Color32,
    pub shot_boundary: Color32,
    pub fill_a: Color32,
    pub fill_b: Color32,
    pub die_stroke: Color32,
    pub dim_gray: Color32,
    pub pill_bg: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        // gpui-kit(shadcn zinc dark):zinc-950 底,blue-500 强调
        Palette {
            canvas_bg: Color32::from_rgb(0x09, 0x09, 0x0b),
            ruler_bg: Color32::from_rgb(0x09, 0x09, 0x0b),
            ruler_line: Color32::from_rgb(0x27, 0x27, 0x2a),
            ruler_text: Color32::from_rgb(0xa1, 0xa1, 0xaa),
            ruler_minor: Color32::from_rgb(0x52, 0x52, 0x5b),
            grid_minor: Color32::from_rgba_unmultiplied(255, 255, 255, 8),
            grid_major: Color32::from_rgba_unmultiplied(255, 255, 255, 20),
            shot_fill: Color32::from_rgba_unmultiplied(59, 130, 246, 14),
            shot_stroke: Color32::from_rgb(0x2f, 0x6b, 0xff),
            shot_grid: Color32::from_rgba_unmultiplied(59, 130, 246, 110),
            shot_boundary: Color32::from_rgba_unmultiplied(59, 130, 246, 140),
            fill_a: Color32::from_rgba_unmultiplied(59, 130, 246, 20),
            fill_b: Color32::from_rgba_unmultiplied(255, 255, 255, 8),
            die_stroke: Color32::from_rgb(0x0a, 0x0a, 0x0a),
            dim_gray: Color32::from_rgb(0xa3, 0xa3, 0xa3),
            pill_bg: Color32::from_rgba_unmultiplied(10, 10, 10, 220),
        }
    } else {
        // shadcn zinc light:zinc-50 底,blue-600 强调
        Palette {
            canvas_bg: Color32::from_rgb(0xff, 0xff, 0xff),
            ruler_bg: Color32::from_rgb(0xff, 0xff, 0xff),
            ruler_line: Color32::from_rgb(0xe4, 0xe4, 0xe7),
            ruler_text: Color32::from_rgb(0x71, 0x71, 0x7a),
            ruler_minor: Color32::from_rgb(0xa1, 0xa1, 0xaa),
            grid_minor: Color32::from_rgba_unmultiplied(0, 0, 0, 10),
            grid_major: Color32::from_rgba_unmultiplied(0, 0, 0, 24),
            shot_fill: Color32::from_rgba_unmultiplied(37, 99, 235, 12),
            shot_stroke: Color32::from_rgb(0x25, 0x63, 0xeb),
            shot_grid: Color32::from_rgba_unmultiplied(37, 99, 235, 100),
            shot_boundary: Color32::from_rgba_unmultiplied(37, 99, 235, 128),
            fill_a: Color32::from_rgba_unmultiplied(37, 99, 235, 22),
            fill_b: Color32::from_rgba_unmultiplied(0, 0, 0, 16),
            die_stroke: Color32::from_rgb(0xe5, 0xe5, 0xe5),
            dim_gray: Color32::from_rgb(0x73, 0x73, 0x73),
            pill_bg: Color32::from_rgba_unmultiplied(255, 255, 255, 235),
        }
    }
}

/// 命中测试:先 die(后画的在上),再 shot
fn hit_test(app: &DieLayoutApp, wx: f64, wy: f64) -> Selection {
    for s in self_shots_rev(app) {
        let sr = shot_rect(s, &app.dies);
        if sr.contains(wx, wy) {
            for &did in s.dies.iter().rev() {
                if let Some(d) = app.dies.iter().find(|d| d.id == did) {
                    if die_rect(s, &app.dies, d).contains(wx, wy) {
                        return Selection::Die(d.id);
                    }
                }
            }
            return Selection::Shot(s.id);
        }
    }
    Selection::None
}

fn self_shots_rev(app: &DieLayoutApp) -> Vec<&Shot> {
    app.shots.iter().rev().collect()
}

pub fn canvas(ui: &mut Ui, app: &mut DieLayoutApp) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let canvas = response.rect;
    app.canvas_rect = canvas;
    let pal = palette(app.theme_dark);
    painter.rect_filled(canvas, CornerRadius::ZERO, pal.canvas_bg);

    // 首帧自动居中已有内容
    if app.needs_fit {
        app.needs_fit = false;
        app.fit_view(canvas);
    }

    let pointer = response.hover_pos().or(response.interact_pointer_pos());
    let pointer_in = pointer.map(|p| canvas.contains(p)).unwrap_or(false);
    app.pointer_world = if pointer_in { Some(app.s2w(pointer.unwrap())) } else { None };

    // ---------- 输入:捏合 / Ctrl|Cmd+滚轮 = 缩放(指针锚点),普通滚轮 = 平移 ----------
    let (scroll, zoom_mod, pinch) = ui.ctx().input(|i| (i.smooth_scroll_delta, i.modifiers.ctrl || i.modifiers.command, i.zoom_delta()));
    if pointer_in && (pinch - 1.0).abs() > 1e-3 {
        // 触摸板双指捏合(macOS 原生)
        let p = pointer.unwrap();
        let (wx, wy) = app.s2w(p);
        let new_zoom = (app.zoom * pinch).clamp(0.02, 64.0);
        app.offset.x = p.x - wx as f32 * new_zoom;
        app.offset.y = p.y - wy as f32 * new_zoom;
        app.zoom = new_zoom;
        app.clamp_view();
    }
    if pointer_in && zoom_mod && scroll.y != 0.0 {
        let p = pointer.unwrap();
        let (wx, wy) = app.s2w(p);
        let factor = (scroll.y as f64 * -0.002).exp() as f32;
        let new_zoom = (app.zoom * factor).clamp(0.02, 64.0);
        // 保持指针下的世界点不动
        app.offset.x = p.x - wx as f32 * new_zoom;
        app.offset.y = p.y - wy as f32 * new_zoom;
        app.zoom = new_zoom;
        app.clamp_view();
    } else if pointer_in && !zoom_mod && scroll != Vec2::ZERO {
        app.offset -= scroll;
        app.clamp_view();
    }
    // 键盘:文本/数字编辑中(wants_keyboard_input)不响应画布快捷键,
    // 否则在属性面板按 Backspace/Delete 改数字会触发删除选中元素
    let editing = ui.ctx().wants_keyboard_input();
    // Figma 式空格临时抓手
    let space = !editing && ui.ctx().input(|i| i.key_down(Key::Space));
    ui.ctx().input_mut(|i| {
        if !editing && (i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace)) {
            app.delete_selected();
        }
        // R 切换测量标尺工具(KLayout 式)
        if !editing && i.consume_key(Modifiers::NONE, Key::R) {
            app.tool = if app.tool == ToolMode::Ruler { ToolMode::Select } else { ToolMode::Ruler };
            app.canvas_drag = None;
            app.set_status(if app.tool == ToolMode::Ruler { "标尺模式:拖拽测量,右键删标尺,Esc/R 退出" } else { "选择模式" });
        }
        if i.consume_key(Modifiers::NONE, Key::Escape) && !editing {
            if matches!(app.canvas_drag, Some(CanvasDrag::Ruler { .. })) {
                app.canvas_drag = None;
            } else if app.tool == ToolMode::Ruler {
                app.tool = ToolMode::Select;
                app.set_status("选择模式");
            } else if app.isolated.is_some() {
                app.isolated = None;
                app.set_status("已退出隔离,显示全部 Shot");
            } else {
                app.selected = Selection::None;
            }
        }
    });

    // ---------- 侧栏拖放 ----------
    if let Some(d) = &mut app.drag {
        if let Some(p) = ui.ctx().input(|i| i.pointer.latest_pos()) {
            d.screen = p;
        }
        let released = ui.ctx().input(|i| !i.pointer.primary_down());
        if released {
            let drag = app.drag.take().unwrap();
            if canvas.contains(drag.screen) {
                let (wx, wy) = app.s2w(drag.screen);
                match drag.card {
                    DragCard::ShotCard => {
                        let (pw, ph) = app.shot_probe();
                        let (x, y) = ((wx / CELL).round() * CELL, (wy / CELL).round() * CELL);
                        if app.can_place_rect(x, y, pw, ph, None) {
                            app.create_shot(x, y);
                        } else if let Some((fx, fy)) = app.scan_free(pw, ph) {
                            app.create_shot(fx, fy);
                            app.set_status("落点被占,Shot 已放到最近空位");
                        } else {
                            app.set_status("画布上没有能放下 Shot 的空位");
                        }
                    }
                    DragCard::Preset(idx) => {
                        let hit = hit_test(app, wx, wy);
                        match shot_at(app, &hit) {
                            Some(sid) => {
                                let shot = app.shots.iter().find(|s| s.id == sid).unwrap().clone();
                                let (i, j) = nearest_slot(&shot, &app.dies, wx - shot.x, wy - shot.y);
                                if let Some(did) = app.place_die(sid, idx, i, j) {
                                    app.selected = Selection::Die(did);
                                    app.set_status(format!("已放入 Shot#{} 槽位 ({}, {})", sid.0, i, j));
                                }
                            }
                            None => app.set_status("Die 只能放入 Shot 槽位,请先拖入一个 Shot"),
                        }
                    }
                }
            } else {
                app.set_status("未放入画布,已取消");
            }
        }
    }

    // ---------- 画布内手势 ----------
    let world_at = |app: &DieLayoutApp, p: Pos2| app.s2w(p);
    if response.double_clicked() {
        if let Some(p) = response.hover_pos() {
            let (wx, wy) = world_at(app, p);
            if let Selection::Shot(sid) = hit_test(app, wx, wy) {
                app.toggle_isolate(sid);
            }
        }
    }
    if response.drag_started_by(PointerButton::Primary) {
        if let Some(p) = response.interact_pointer_pos() {
            if space {
                // 空格临时抓手(Figma 手感)
                app.canvas_drag = Some(CanvasDrag::Pan { start: p, start_offset: app.offset });
            } else if app.tool == ToolMode::Ruler {
                let (wx, wy) = world_at(app, p);
                app.canvas_drag = Some(CanvasDrag::Ruler { start: (wx, wy) });
            } else {
            // 手柄优先:命中选中 die 的角手柄 → resize
            if let Some(corner) = handle_at(app, p) {
                if let Selection::Die(id) = app.selected {
                    if let Some((_, die)) = find_die(app, id) {
                        let (wx, wy) = world_at(app, p);
                        app.canvas_drag = Some(CanvasDrag::ResizeDie {
                            id,
                            corner,
                            start_wx: wx,
                            start_wy: wy,
                            start_w: die.w,
                            start_h: die.h,
                        });
                    }
                }
            }
            if app.canvas_drag.is_some() {
                // 已命中手柄,跳过后续手势判定
            } else {
            let (wx, wy) = world_at(app, p);
            match hit_test(app, wx, wy) {
                Selection::Die(id) => {
                    app.canvas_drag = Some(CanvasDrag::MoveDie { id });
                    app.selected = Selection::Die(id);
                }
                Selection::Shot(id) => {
                    let s = app.shots.iter().find(|s| s.id == id).unwrap();
                    app.canvas_drag = Some(CanvasDrag::MoveShot { id, start: (s.x, s.y) });
                    app.selected = Selection::Shot(id);
                }
                Selection::None => {
                    app.canvas_drag = Some(CanvasDrag::Pan { start: p, start_offset: app.offset });
                }
            }
            }
            }
        }
    }
    if let Some(mode) = app.canvas_drag {
        match mode {
            CanvasDrag::Pan { start, start_offset } => {
                if let Some(p) = response.interact_pointer_pos() {
                    app.offset = start_offset + (p - start);
                    app.clamp_view();
                }
            }
            CanvasDrag::MoveShot { id, start } => {
                if response.interact_pointer_pos().is_some() {
                    let d = response.drag_delta();
                    let (dx, dy) = (d.x as f64 / app.zoom as f64, d.y as f64 / app.zoom as f64);
                    // Figma 智能参考线:与其他 Shot 边缘/中线、世界 0 轴对齐
                    let (ndx, ndy, guides) = snap_shot_delta(app, id, dx, dy);
                    if let Some(s) = app.shots.iter_mut().find(|s| s.id == id) {
                        s.x = start.0 + ndx;
                        s.y = start.1 + ndy;
                    }
                    app.guides = guides;
                }
            }
            CanvasDrag::MoveDie { .. } => {}
            CanvasDrag::Ruler { .. } => {
                // 预览在绘制层处理(需要 painter 与指针)
            }
            CanvasDrag::ResizeDie { id, corner, start_wx, start_wy, start_w, start_h } => {
                if let Some(p) = response.interact_pointer_pos() {
                    let (wx, wy) = world_at(app, p);
                    let nw = ((start_w + corner.sign_w() * (wx - start_wx)).max(0.001) * 1000.0).round() / 1000.0;
                    let nh = ((start_h + corner.sign_h() * (wy - start_wy)).max(0.001) * 1000.0).round() / 1000.0;
                    if app.resize_die_live(id, nw, nh) {
                        // 实时生效:跟手,标注/布局每帧自动重算
                    } else if let Some((shot, die)) = find_die(app, id) {
                        // 放不下:红色预览框提示上限,保持当前(最后一次合法)尺寸
                        let (i, j) = die.slot;
                        let (ox, oy) = slot_origin(shot, &app.dies, i, j);
                        let cw = slot_width(shot, &app.dies, i, j);
                        let ch = slot_height(shot, &app.dies, i, j);
                        let cx = shot.x + ox + cw / 2.0;
                        let cy = shot.y + oy + ch / 2.0;
                        let prev = wr_to_screen(app, &WRect { x: cx - nw / 2.0, y: cy - nh / 2.0, w: nw, h: nh });
                        painter_preview(&painter, prev, Color32::from_rgb(0xef, 0x44, 0x44));
                        app.set_status(format!("resize → {} × {}(超出 Shot 容量,已停在最后一次合法尺寸)", fmt_um(nw), fmt_um(nh)));
                    }
                }
            }
        }
        if response.drag_stopped() {
            match mode {
                CanvasDrag::Pan { .. } => {}
                CanvasDrag::MoveShot { id, start } => {
                    let cur = app.shots.iter().find(|s| s.id == id).map(|s| (s.x, s.y));
                    if let Some((cx, cy)) = cur {
                        let (w, h) = {
                            let s = app.shots.iter().find(|s| s.id == id).unwrap();
                            let r = shot_rect(s, &app.dies);
                            (r.w, r.h)
                        };
                        if !app.guides.is_empty() {
                            // 已被智能参考线吸附:直接用实时位置
                            if app.can_place_rect(cx, cy, w, h, Some(id)) {
                                app.set_status("已对齐参考线");
                            } else {
                                if let Some(s) = app.shots.iter_mut().find(|s| s.id == id) {
                                    s.x = start.0;
                                    s.y = start.1;
                                }
                                app.set_status("对齐后位置重叠,已还原");
                            }
                            app.guides.clear();
                        } else {
                            let nx = (cx / CELL).round() * CELL;
                            let ny = (cy / CELL).round() * CELL;
                            if app.can_place_rect(nx, ny, w, h, Some(id)) {
                                if let Some(s) = app.shots.iter_mut().find(|s| s.id == id) {
                                    s.x = nx;
                                    s.y = ny;
                                }
                                app.set_status("已吸附对齐");
                            } else {
                                if let Some(s) = app.shots.iter_mut().find(|s| s.id == id) {
                                    s.x = start.0;
                                    s.y = start.1;
                                }
                                app.set_status("目标位置越界或重叠,已还原");
                            }
                        }
                    }
                }
                CanvasDrag::MoveDie { id } => {
                    if let Some(p) = response.interact_pointer_pos() {
                        let (wx, wy) = world_at(app, p);
                        let Some(die) = app.dies.iter().find(|d| d.id == id).cloned() else { app.canvas_drag = None; return };
                        let shot = app.shots.iter().find(|s| s.id == die.shot).unwrap().clone();
                        let (i, j) = nearest_slot(&shot, &app.dies, wx - shot.x, wy - shot.y);
                        if app.can_place_in_shot(shot.id, i, j, die.w, die.h, Some(id)) {
                            if let Some(d) = app.dies.iter_mut().find(|d| d.id == id) {
                                d.slot = (i, j);
                            }
                            app.set_status(format!("已移动到槽位 ({}, {})", i, j));
                        } else {
                            app.set_status("槽位已被占用或空间不足,已还原");
                        }
                    }
                }
                CanvasDrag::ResizeDie { .. } => {
                    // 拖动中已实时应用合法尺寸,松手无需再提交
                }
                CanvasDrag::Ruler { start } => {
                    if let Some(p) = response.interact_pointer_pos() {
                        let (wx, wy) = world_at(app, p);
                        let r = MeasureRuler { a: start, b: (wx, wy) };
                        if (r.len() * app.zoom as f64) >= 2.0 {
                            app.measure_rulers.push(r);
                            app.set_status(format!("标尺 #{}: {}", app.measure_rulers.len(), fmt_um(r.len())));
                        }
                    }
                }
            }
            app.canvas_drag = None;
        }
    }
    let mut toolbar_clicked = false;
    if response.clicked() {
        if let Some(p) = response.hover_pos() {
            // 工具栏按钮:缩小 / 放大 / 自适应 / 标尺工具
            let bar = toolbar_rect(canvas);
            if bar.contains(p) {
                toolbar_clicked = true;
                let rel = p.x - bar.left();
                if rel < 26.0 {
                    self_zoom_at(app, canvas.center(), 1.0 / 1.25);
                } else if rel < 78.0 {
                    self_zoom_at(app, canvas.center(), 1.25);
                } else if rel < 104.0 {
                    app.fit_view(canvas);
                } else if app.tool == ToolMode::Ruler {
                    app.tool = ToolMode::Select;
                    app.set_status("选择模式");
                } else {
                    app.tool = ToolMode::Ruler;
                    app.set_status("标尺模式:拖拽测量,右键删标尺,Esc/R 退出");
                }
            }
        }
    }
    // 右键删除测量标尺(命中线段 6px 内)
    if response.secondary_clicked() {
        if let Some(p) = response.hover_pos() {
            if let Some(idx) = ruler_at(app, p) {
                app.measure_rulers.remove(idx);
                app.set_status(format!("已删除标尺(剩余 {})", app.measure_rulers.len()));
            }
        }
    }
    if response.clicked() && !toolbar_clicked && app.tool == ToolMode::Select && !space {
        if let Some(p) = response.hover_pos() {
            let (wx, wy) = world_at(app, p);
            app.selected = hit_test(app, wx, wy);
            app.canvas_drag = None;
        }
    }

    // 光标:空格抓手 > 标尺十字 > resize 手柄
    if let Some(p) = pointer {
        if pointer_in {
            if space {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            } else if app.tool == ToolMode::Ruler {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            } else if let Some(c) = handle_at(app, p) {
                ui.ctx().set_cursor_icon(c.cursor());
            }
        }
    }

    // 悬停高亮
    app.hover = match pointer {
        Some(p) if pointer_in => {
            let (wx, wy) = world_at(app, p);
            hit_test(app, wx, wy)
        }
        _ => Selection::None,
    };

    // ---------- 绘制 ----------
    if !matches!(app.canvas_drag, Some(CanvasDrag::MoveShot { .. })) {
        app.guides.clear();
    }
        draw_grid(&painter, app, canvas, pal);
    draw_shots(&painter, app, pal);
    draw_guides(&painter, app);
    draw_measure_rulers(&painter, app, pal);
    // 标尺拖拽实时预览
    if let Some(CanvasDrag::Ruler { start }) = app.canvas_drag {
        if pointer_in {
            if let Some(p) = pointer {
                let (wx, wy) = app.s2w(p);
                let r = MeasureRuler { a: start, b: (wx, wy) };
                draw_ruler_shape(&painter, app, &r, pal, true);
            }
        }
    }
    // 拖动 Die 预览:ghost 跟随指针 + 目标槽位高亮(绿=可放置,红=占用/不足)
    if let Some(CanvasDrag::MoveDie { id }) = app.canvas_drag {
        if pointer_in {
            if let Some(p) = pointer {
                if let Some((shot, die)) = find_die(app, id) {
                    let (wx, wy) = app.s2w(p);
                    let g = wr_to_screen(app, &WRect { x: wx - die.w / 2.0, y: wy - die.h / 2.0, w: die.w, h: die.h });
                    painter.add(Shape::rect_filled(g, CornerRadius::same(2), die.color.linear_multiply(0.6)));
                    painter.add(Shape::rect_stroke(g, CornerRadius::same(2), Stroke::new(1.0_f32, pal.die_stroke), StrokeKind::Inside));
                    let (i, j) = nearest_slot(shot, &app.dies, wx - shot.x, wy - shot.y);
                    let ok = app.can_place_in_shot(shot.id, i, j, die.w, die.h, Some(id));
                    let cw = slot_width(shot, &app.dies, i, j);
                    let rh = slot_height(shot, &app.dies, i, j);
                    let (ox, oy) = slot_origin(shot, &app.dies, i, j);
                    let r = wr_to_screen(app, &WRect { x: shot.x + ox, y: shot.y + oy, w: cw, h: rh });
                    let c = if ok { Color32::from_rgb(0x22, 0xc5, 0x5e) } else { Color32::from_rgb(0xef, 0x44, 0x44) };
                    painter.add(Shape::rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.5_f32, c), StrokeKind::Inside));
                }
            }
        }
    }
    draw_drop_hint(&painter, app, pointer, canvas, pal);
    draw_selection_badge(&painter, app, canvas, pal);
    draw_rulers(&painter, app, canvas, pal, pointer, pointer_in);
    draw_toolbar(&painter, app, canvas);
}

/// 以指定屏幕锚点缩放(工具栏按钮用)
fn self_zoom_at(app: &mut DieLayoutApp, anchor: Pos2, factor: f32) {
    let (wx, wy) = app.s2w(anchor);
    let new_zoom = (app.zoom * factor).clamp(0.02, 64.0);
    app.offset.x = anchor.x - wx as f32 * new_zoom;
    app.offset.y = anchor.y - wy as f32 * new_zoom;
    app.zoom = new_zoom;
    app.clamp_view();
}

/// 选中 Die 的四角 resize 手柄(屏幕坐标,10px 白底蓝边,参考 web 版样式)
const HANDLE_PX: f32 = 10.0;

fn resize_handle_rects(app: &DieLayoutApp) -> Vec<(ResizeCorner, Rect)> {
    let mut out = Vec::new();
    if let Selection::Die(id) = app.selected {
        if let Some((shot, die)) = find_die(app, id) {
            let r = die_rect(shot, &app.dies, die);
            let p1 = app.w2s(r.x, r.y);
            let p2 = app.w2s(r.x + r.w, r.y + r.h);
            let corners = [
                (ResizeCorner::TopLeft, Pos2::new(p1.x, p1.y)),
                (ResizeCorner::TopRight, Pos2::new(p2.x, p1.y)),
                (ResizeCorner::BottomLeft, Pos2::new(p1.x, p2.y)),
                (ResizeCorner::BottomRight, Pos2::new(p2.x, p2.y)),
            ];
            for (c, p) in corners {
                out.push((c, Rect::from_center_size(p, Vec2::splat(HANDLE_PX))));
            }
        }
    }
    out
}

/// 命中手柄(含 6px 容差)
fn handle_at(app: &DieLayoutApp, p: Pos2) -> Option<ResizeCorner> {
    resize_handle_rects(app)
        .into_iter()
        .find(|(_, r)| r.expand(3.0).contains(p))
        .map(|(c, _)| c)
}

fn draw_resize_handles(painter: &egui::Painter, app: &DieLayoutApp) {
    let accent = if app.theme_dark { Color32::from_rgb(0x60, 0xa5, 0xfa) } else { Color32::from_rgb(0x25, 0x63, 0xeb) };
    for (c, r) in resize_handle_rects(app) {
        painter.add(Shape::rect_filled(r, 3, Color32::WHITE));
        painter.add(Shape::rect_stroke(r, 3, Stroke::new(2.0_f32, accent), StrokeKind::Inside));
        if let Some(ptr) = app.pointer_world {
            let _ = ptr;
        }
        let _ = c;
    }
}

fn shot_at(app: &DieLayoutApp, sel: &Selection) -> Option<ShotId> {
    match sel {
        Selection::Shot(s) => Some(*s),
        Selection::Die(d) => app.dies.iter().find(|x| x.id == *d).map(|x| x.shot),
        Selection::None => None,
    }
}

/// 矩形虚线边框(四条边;dashed_line 只接受线段,不能用对角线)
/// 只在与 canvas 相交的子矩形边上生成 dash——高缩放下不为屏幕外部分铺张段数
fn rect_dashed(painter: &egui::Painter, r: Rect, stroke: Stroke, dash: f32, gap: f32, canvas: Rect) {
    let vis = r.intersect(canvas);
    if vis.width() < 2.0 || vis.height() < 2.0 {
        return;
    }
    painter.extend(Shape::dashed_line(&[vis.min, Pos2::new(vis.max.x, vis.min.y)], stroke, dash, gap));
    painter.extend(Shape::dashed_line(&[Pos2::new(vis.max.x, vis.min.y), vis.max], stroke, dash, gap));
    painter.extend(Shape::dashed_line(&[vis.max, Pos2::new(vis.min.x, vis.max.y)], stroke, dash, gap));
    painter.extend(Shape::dashed_line(&[Pos2::new(vis.min.x, vis.max.y), vis.min], stroke, dash, gap));
}

fn draw_grid(painter: &egui::Painter, app: &DieLayoutApp, canvas: Rect, pal: Palette) {
    let (wx0, wy0) = app.s2w(canvas.min);
    let (wx1, wy1) = app.s2w(canvas.max);
    let major = nice_step(app.zoom, 70.0);
    let minor = major / 5.0;
    let lw = 1.0_f32;
    let is_major = |v: f64| ((v / major).round() - v / major).abs() < 1e-9;
    let mut k = (wx0 / minor).floor() as i64;
    while (k as f64) * minor <= wx1 {
        let x = (k as f64) * minor;
        let sx = app.offset.x + x as f32 * app.zoom;
        let c = if is_major(x) { pal.grid_major } else { pal.grid_minor };
        painter.add(Shape::line_segment([Pos2::new(sx, canvas.top()), Pos2::new(sx, canvas.bottom())], Stroke::new(lw, c)));
        k += 1;
    }
    let mut k = (wy0 / minor).floor() as i64;
    while (k as f64) * minor <= wy1 {
        let y = (k as f64) * minor;
        let sy = app.offset.y + y as f32 * app.zoom;
        let c = if is_major(y) { pal.grid_major } else { pal.grid_minor };
        painter.add(Shape::line_segment([Pos2::new(canvas.left(), sy), Pos2::new(canvas.right(), sy)], Stroke::new(lw, c)));
        k += 1;
    }
}

fn wr_to_screen(app: &DieLayoutApp, r: &WRect) -> Rect {
    let p = app.w2s(r.x, r.y);
    Rect::from_min_size(p, Vec2::new(r.w as f32 * app.zoom, r.h as f32 * app.zoom))
}

fn draw_shots(painter: &egui::Painter, app: &DieLayoutApp, pal: Palette) {
    let isolated = app.isolated;
    for shot in &app.shots {
        let faded = isolated.map(|i| i != shot.id).unwrap_or(false);
        let alpha_mul = if faded { 0.15f32 } else { 1.0 };
        let with_alpha = |c: Color32| {
            let mut c = c;
            c = c.linear_multiply(alpha_mul);
            c
        };
        let sr = shot_rect(shot, &app.dies);
        let srect = wr_to_screen(app, &sr);
        // 可视区外跳过:低/高缩放下不为屏幕外 Shot 生成几何(虚线段、槽位、标注)
        if !srect.intersects(app.canvas_rect) {
            continue;
        }

        // 背景 + 棋盘格槽位(槽位几何随内容自动计算)
        painter.add(Shape::rect_filled(srect, CornerRadius::ZERO, with_alpha(pal.shot_fill)));
        for j in 0..shot.rows {
            for i in 0..shot.cols {
                let (ox, oy) = slot_origin(shot, &app.dies, i, j);
                let w = slot_width(shot, &app.dies, i, j);
                let h = slot_height(shot, &app.dies, i, j);
                let r = wr_to_screen(app, &WRect { x: shot.x + ox, y: shot.y + oy, w, h });
                let c = if (i + j) % 2 == 0 { pal.fill_a } else { pal.fill_b };
                painter.add(Shape::rect_filled(r, CornerRadius::ZERO, with_alpha(c)));
            }
        }

        // 槽位网格线(槽位边缘,含 die 边缘)——屏幕固定 1px,高缩放不变成粗带
        for j in 0..shot.rows {
            for i in 0..shot.cols {
                let (ox, oy) = slot_origin(shot, &app.dies, i, j);
                let w = slot_width(shot, &app.dies, i, j);
                let h = slot_height(shot, &app.dies, i, j);
                let r = wr_to_screen(app, &WRect { x: shot.x + ox, y: shot.y + oy, w, h });
                painter.add(Shape::rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0_f32, with_alpha(pal.shot_grid)), StrokeKind::Inside));
            }
        }

        // 内边距边界(虚线矩形)
        let inner = wr_to_screen(app, &WRect { x: shot.x + shot.pad_x, y: shot.y + shot.pad_y, w: sr.w - shot.pad_x * 2.0, h: sr.h - shot.pad_y * 2.0 });
        rect_dashed(painter, inner, Stroke::new(1.0_f32, with_alpha(pal.shot_boundary)), 8.0, 5.0, app.canvas_rect);

        // 外框(虚线矩形,屏幕固定 1.5px)
        rect_dashed(painter, srect, Stroke::new(1.5_f32, with_alpha(pal.shot_stroke)), 10.0, 7.0, app.canvas_rect);

        // dies
        for &did in &shot.dies {
            let Some(die) = app.dies.iter().find(|d| d.id == did) else { continue };
            let dr = die_rect(shot, &app.dies, die);
            let drect = wr_to_screen(app, &dr);
            painter.add(Shape::rect_filled(drect, CornerRadius::same(2), with_alpha(die.color)));
            painter.add(Shape::rect_stroke(drect, CornerRadius::same(2), Stroke::new(2.0_f32, with_alpha(pal.die_stroke)), StrokeKind::Inside));
            if drect.height() >= 16.0 {
                // 屏幕随动的 die 名,字号钳制 10~72px(高缩放防止超大字形拖垮渲染)
                let fs = (drect.height() * 0.22).clamp(10.0, 72.0);
                t(
                    painter,
                    drect.center(),
                    Align2::CENTER_CENTER,
                    die.name.replace("Die ", ""),
                    fs,
                    Color32::from_rgb(0x0f, 0x17, 0x2a),
                );
            }
        }

        // 间距标注(粉):相邻 die 间距
        if shot.show_dims && !faded {
            draw_gap_dims(painter, app, shot, pal);
        }
        if shot.show_outer && !faded {
            draw_outer_dims(painter, app, shot, pal);
        }
    }

    // 选中 / 悬停高亮(最后画)
    let hl_stroke = Stroke::new(1.5_f32, pal.shot_stroke);
    let sel_stroke = Stroke::new(2.0_f32, pal.shot_stroke);
    match app.hover {
        Selection::Die(id) => {
            if let Some((s, d)) = find_die(app, id) {
                painter.add(Shape::rect_stroke(wr_to_screen(app, &die_rect(s, &app.dies, d)), CornerRadius::ZERO, hl_stroke, StrokeKind::Inside));
            }
        }
        Selection::Shot(id) => {
            if let Some(s) = app.shots.iter().find(|s| s.id == id) {
                painter.add(Shape::rect_stroke(wr_to_screen(app, &shot_rect(s, &app.dies)), CornerRadius::ZERO, hl_stroke, StrokeKind::Inside));
            }
        }
        Selection::None => {}
    }
    match app.selected {
        Selection::Die(id) => {
            if let Some((s, d)) = find_die(app, id) {
                painter.add(Shape::rect_stroke(wr_to_screen(app, &die_rect(s, &app.dies, d)), CornerRadius::ZERO, sel_stroke, StrokeKind::Inside));
            }
        }
        Selection::Shot(id) => {
            if let Some(s) = app.shots.iter().find(|s| s.id == id) {
                painter.add(Shape::rect_stroke(wr_to_screen(app, &shot_rect(s, &app.dies)), CornerRadius::ZERO, sel_stroke, StrokeKind::Inside));
            }
        }
        Selection::None => {}
    }

    // 选中 Die 的 resize 手柄(最后画,不被遮挡)
    draw_resize_handles(painter, app);
}

/// resize 预览框
fn painter_preview(painter: &egui::Painter, r: Rect, c: Color32) {
    painter.add(Shape::rect_filled(r, CornerRadius::ZERO, c.linear_multiply(0.15)));
    painter.add(Shape::rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.5_f32, c), StrokeKind::Inside));
}

/// 文本绘制便捷函数(屏幕字号)
fn t(painter: &egui::Painter, pos: Pos2, align: Align2, text: impl ToString, size: f32, color: Color32) {
    painter.text(pos, align, text, egui::FontId::proportional(size.max(1.0)), color);
}

fn find_die<'a>(app: &'a DieLayoutApp, id: DieId) -> Option<(&'a Shot, &'a Die)> {
    let d = app.dies.iter().find(|d| d.id == id)?;
    let s = app.shots.iter().find(|s| s.id == d.shot)?;
    Some((s, d))
}

/// die 间距标注(粉色,屏幕固定字号/线宽——低缩放不溢出画布,与 web 版同策略)
fn draw_gap_dims(painter: &egui::Painter, app: &DieLayoutApp, shot: &Shot, pal: Palette) {
    let fs = 9.0;
    let lw = 1.0_f32;
    let tick = 4.0;
    let color = Color32::from_rgb(0xec, 0x48, 0x99);
    let dies: Vec<&Die> = shot.dies.iter().filter_map(|&id| app.dies.iter().find(|d| d.id == id)).collect();
    for a in &dies {
        let ra = die_rect(shot, &app.dies, a);
        // 最近右侧 + 最近下侧(带重叠)
        let mut right: Option<(&&Die, f64)> = None;
        let mut below: Option<(&&Die, f64)> = None;
        for b in &dies {
            if a.id == b.id {
                continue;
            }
            let rb = die_rect(shot, &app.dies, b);
            if ra.y < rb.y + rb.h && ra.y + ra.h > rb.y {
                let gap = rb.x - (ra.x + ra.w);
                if gap > 1e-6 && right.map(|(_, g)| gap < g).unwrap_or(true) {
                    right = Some((b, gap));
                }
            }
            if ra.x < rb.x + rb.w && ra.x + ra.w > rb.x {
                let gap = rb.y - (ra.y + ra.h);
                if gap > 1e-6 && below.map(|(_, g)| gap < g).unwrap_or(true) {
                    below = Some((b, gap));
                }
            }
        }
        if let Some((b, gap)) = right {
            let rb = die_rect(shot, &app.dies, b);
            let y = (ra.y.max(rb.y) + (ra.y + ra.h).min(rb.y + rb.h)) / 2.0;
            let p1 = app.w2s(ra.x + ra.w, y);
            let p2 = app.w2s(rb.x, y);
            painter.add(Shape::line_segment([p1, p2], Stroke::new(lw, color)));
            painter.add(Shape::line_segment([Pos2::new(p1.x, p1.y - tick), Pos2::new(p1.x, p1.y + tick)], Stroke::new(lw, color)));
            painter.add(Shape::line_segment([Pos2::new(p2.x, p2.y - tick), Pos2::new(p2.x, p2.y + tick)], Stroke::new(lw, color)));
            t(painter, Pos2::new((p1.x + p2.x) / 2.0, p1.y - fs), Align2::CENTER_BOTTOM, fmt_um(gap), fs, color);
        }
        if let Some((b, gap)) = below {
            let rb = die_rect(shot, &app.dies, b);
            let x = (ra.x.max(rb.x) + (ra.x + ra.w).min(rb.x + rb.w)) / 2.0;
            let p1 = app.w2s(x, ra.y + ra.h);
            let p2 = app.w2s(x, rb.y);
            painter.add(Shape::line_segment([p1, p2], Stroke::new(lw, color)));
            painter.add(Shape::line_segment([Pos2::new(p1.x - tick, p1.y), Pos2::new(p1.x + tick, p1.y)], Stroke::new(lw, color)));
            painter.add(Shape::line_segment([Pos2::new(p2.x - tick, p2.y), Pos2::new(p2.x + tick, p2.y)], Stroke::new(lw, color)));
            t(painter, Pos2::new(p1.x - fs * 0.9, (p1.y + p2.y) / 2.0), Align2::RIGHT_CENTER, fmt_um(gap), fs, color);
        }
    }
    let _ = pal;
}

/// 外圈 scribe + 容器尺寸标注(灰/蓝,屏幕固定字号)
fn draw_outer_dims(painter: &egui::Painter, app: &DieLayoutApp, shot: &Shot, pal: Palette) {
    let fs = 9.0;
    let lw = 1.0_f32;
    let tick = 4.0;
    let gray = pal.dim_gray;
    let blue = pal.shot_stroke;
    let w = shot_w(shot, &app.dies);
    let h = shot_h(shot, &app.dies);

    // 首/末 die 缘(局部距离)
    let mut first_left = f64::INFINITY;
    let mut first_top = f64::INFINITY;
    let mut last_right = f64::MIN;
    let mut last_bottom = f64::MIN;
    for &id in &shot.dies {
        if let Some(d) = app.dies.iter().find(|d| d.id == id) {
            let r = die_rect_local(shot, &app.dies, d);
            first_left = first_left.min(r.x);
            first_top = first_top.min(r.y);
            last_right = last_right.max(r.x + r.w);
            last_bottom = last_bottom.max(r.y + r.h);
        }
    }
    let outer_xl = if first_left.is_finite() { first_left } else { shot.pad_x };
    let outer_yt = if first_top.is_finite() { first_top } else { shot.pad_y };
    let outer_xr = if last_right.is_finite() { w - last_right } else { shot.pad_x };
    let outer_yb = if last_bottom.is_finite() { h - last_bottom } else { shot.pad_y };

    // X 外圈:shot 垂直中线处,左/右两段
    let oy_x = shot.y + h / 2.0;
    for (from, len, lbl_cx) in [(shot.x, outer_xl, shot.x + outer_xl / 2.0), (shot.x + w - outer_xr, outer_xr, shot.x + w - outer_xr / 2.0)] {
        let p1 = app.w2s(from, oy_x);
        let p2 = app.w2s(from + len, oy_x);
        painter.add(Shape::line_segment([p1, p2], Stroke::new(lw, gray)));
        painter.add(Shape::line_segment([Pos2::new(p1.x, p1.y - tick), Pos2::new(p1.x, p1.y + tick)], Stroke::new(lw, gray)));
        painter.add(Shape::line_segment([Pos2::new(p2.x, p2.y - tick), Pos2::new(p2.x, p2.y + tick)], Stroke::new(lw, gray)));
        t(painter, Pos2::new(app.w2s(lbl_cx, oy_x).x, p1.y - fs * 0.9), Align2::CENTER_BOTTOM, fmt_um(len), fs, gray);
    }
    // Y 外圈:shot 水平中线处,上/下两段
    let ox_y = shot.x + w / 2.0;
    for (from, len) in [(shot.y, outer_yt), (shot.y + h - outer_yb, outer_yb)] {
        let p1 = app.w2s(ox_y, from);
        let p2 = app.w2s(ox_y, from + len);
        painter.add(Shape::line_segment([p1, p2], Stroke::new(lw, gray)));
        painter.add(Shape::line_segment([Pos2::new(p1.x - tick, p1.y), Pos2::new(p1.x + tick, p1.y)], Stroke::new(lw, gray)));
        painter.add(Shape::line_segment([Pos2::new(p2.x - tick, p2.y), Pos2::new(p2.x + tick, p2.y)], Stroke::new(lw, gray)));
        t(painter, Pos2::new(p1.x - fs * 0.9, (p1.y + p2.y) / 2.0), Align2::RIGHT_CENTER, fmt_um(len), fs, gray);
    }

    // 容器尺寸:底边下方 12px(屏幕距离,与缩放无关),蓝色 pill
    let bl = app.w2s(shot.x, shot.y + h);
    let p1 = Pos2::new(bl.x, bl.y + 12.0);
    let p2 = Pos2::new(bl.x + w as f32 * app.zoom, p1.y);
    painter.add(Shape::line_segment([p1, p2], Stroke::new(lw, blue)));
    painter.add(Shape::line_segment([Pos2::new(p1.x, p1.y - tick), Pos2::new(p1.x, p1.y + tick)], Stroke::new(lw, blue)));
    painter.add(Shape::line_segment([Pos2::new(p2.x, p2.y - tick), Pos2::new(p2.x, p2.y + tick)], Stroke::new(lw, blue)));
    let pill_label = format!("Shot {} × {}", fmt_um(w), fmt_um(h));
    let galley = painter.layout(pill_label, egui::FontId::proportional(fs), blue, f32::INFINITY);
    let pill_size = galley.rect.size() + Vec2::new(12.0, 6.0);
    let pill = Rect::from_center_size(Pos2::new((p1.x + p2.x) / 2.0, p1.y + 6.0 + pill_size.y / 2.0 - 3.0), pill_size);
    painter.add(Shape::rect_filled(pill, CornerRadius::same(2), pal.pill_bg));
    painter.galley(pill.min + Vec2::new(6.0, 3.0), galley, blue);
}

/// 侧栏拖拽中的落点提示
fn draw_drop_hint(painter: &egui::Painter, app: &DieLayoutApp, pointer: Option<Pos2>, canvas: Rect, pal: Palette) {
    let Some(drag) = app.drag else { return };
    if !drag.started {
        return;
    }
    let Some(p) = pointer else { return };
    if !canvas.contains(p) {
        return;
    }
    let (wx, wy) = app.s2w(p);
    match drag.card {
        DragCard::ShotCard => {
            let (pw, ph) = app.shot_probe();
            let x = (wx / CELL).round() * CELL;
            let y = (wy / CELL).round() * CELL;
            let ok = app.can_place_rect(x, y, pw, ph, None);
            let r = wr_to_screen(app, &WRect { x, y, w: pw, h: ph });
            let c = if ok { Color32::from_rgb(0x4a, 0xde, 0x80) } else { Color32::from_rgb(0xf8, 0x71, 0x71) };
            painter.add(Shape::rect_filled(r, CornerRadius::ZERO, c.linear_multiply(0.25)));
            painter.add(Shape::rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.5_f32, c), StrokeKind::Inside));
            t(painter, Pos2::new(p.x + 16.0, p.y + 16.0), Align2::LEFT_TOP, if ok { "创建 Shot 于此" } else { "位置被占或越界" }, 11.0, c);
        }
        DragCard::Preset(idx) => {
            let Some(preset) = app.presets.get(idx) else { return };
            let hit = hit_test(app, wx, wy);
            let sid = shot_at_pub(app, &hit);
            if let Some(sid) = sid {
                let shot = app.shots.iter().find(|s| s.id == sid).unwrap().clone();
                let (i, j) = nearest_slot(&shot, &app.dies, wx - shot.x, wy - shot.y);
                let ok = app.can_place_in_shot(sid, i, j, preset.w, preset.h, None);
                let cw = col_width(&shot, &app.dies, i);
                let rh = row_height(&shot, &app.dies, j);
                let (ox, oy) = slot_origin(&shot, &app.dies, i, j);
                let r = wr_to_screen(app, &WRect { x: shot.x + ox, y: shot.y + oy, w: cw, h: rh });
                let c = if ok { Color32::from_rgb(0x4a, 0xde, 0x80) } else { Color32::from_rgb(0xf8, 0x71, 0x71) };
                painter.add(Shape::rect_filled(r, CornerRadius::ZERO, c.linear_multiply(0.25)));
                painter.add(Shape::rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.5_f32, c), StrokeKind::Inside));
                let label = if ok { format!("槽位 ({}, {}) · 松手放置", i, j) } else { "✕ 槽位已占用或空间不足".to_string() };
                t(painter, Pos2::new(p.x + 16.0, p.y + 16.0), Align2::LEFT_TOP, label, 11.0, c);
            } else {
                let r = wr_to_screen(app, &WRect { x: (wx / CELL).floor() * CELL, y: (wy / CELL).floor() * CELL, w: preset.w, h: preset.h });
                let c = Color32::from_rgb(0xf8, 0x71, 0x71);
                painter.add(Shape::rect_filled(r, CornerRadius::ZERO, c.linear_multiply(0.2)));
                painter.add(Shape::rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.5_f32, c), StrokeKind::Inside));
                t(painter, Pos2::new(p.x + 16.0, p.y + 16.0), Align2::LEFT_TOP, "✕ Die 只能放入 Shot 内", 11.0, c);
            }
        }
    }
    let _ = pal;
}

pub fn shot_at_pub(app: &DieLayoutApp, sel: &Selection) -> Option<ShotId> {
    shot_at(app, sel)
}

/// 标尺数值格式化(去尾零)
fn fmt_ruler(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Y 标尺带宽度:容纳视口内最长标签(动态 26~64px),避免 "100000" 被裁掉前半
fn ruler_band_w(app: &DieLayoutApp, canvas: Rect) -> f32 {
    let (_, wy1) = app.s2w(canvas.left_bottom());
    (fmt_ruler(wy1.max(0.0)).len() as f32 * 6.0 + 20.0).clamp(RULER, 64.0)
}

/// 标尺(屏幕坐标):刻度 + 十字线 + 选中范围高亮(尺寸 pill 收在标尺带内,不压画布)
fn draw_rulers(painter: &egui::Painter, app: &DieLayoutApp, canvas: Rect, pal: Palette, pointer: Option<Pos2>, pointer_in: bool) {
    let w = canvas.width();
    let h = canvas.height();
    let ry = ruler_band_w(app, canvas);
    painter.add(Shape::rect_filled(Rect::from_min_size(canvas.min, Vec2::new(w, RULER)), CornerRadius::ZERO, pal.ruler_bg));
    painter.add(Shape::rect_filled(Rect::from_min_size(canvas.min, Vec2::new(ry, h)), CornerRadius::ZERO, pal.ruler_bg));
    painter.add(Shape::line_segment([Pos2::new(canvas.left(), canvas.top() + RULER), Pos2::new(canvas.right(), canvas.top() + RULER)], Stroke::new(1.0_f32, pal.ruler_line)));
    painter.add(Shape::line_segment([Pos2::new(canvas.left() + ry, canvas.top()), Pos2::new(canvas.left() + ry, canvas.bottom())], Stroke::new(1.0_f32, pal.ruler_line)));

    let major = nice_step(app.zoom, 70.0);
    let minor = major / 5.0;
    let fmt = fmt_ruler;
    // 标签抽稀:按视口最大值的位数估最长标签宽,间距不足时每隔 thin 个 major 显示一个
    let (max_wx, max_wy) = {
        let (ax, ay) = app.s2w(canvas.right_bottom());
        (ax.max(1.0), ay.max(1.0))
    };
    let est_label_w = |v: f64| fmt(v).len() as f32 * 6.0 + 8.0;
    let spacing = (major * app.zoom as f64).max(1.0) as f32;
    let thin_x = (est_label_w(max_wx) / spacing).ceil().max(1.0) as i64;
    let thin_y = (est_label_w(max_wy) / spacing).ceil().max(1.0) as i64;
    let majors_per_minor = (major / minor).round() as i64; // =5
    let (wx0, _) = app.s2w(Pos2::new(canvas.left() + RULER, 0.0));
    let (wx1, _) = app.s2w(Pos2::new(canvas.right(), 0.0));
    let mut k = (wx0 / minor).floor() as i64;
    while (k as f64) * minor <= wx1 {
        let v = (k as f64) * minor;
        if v >= 0.0 {
            let sx = app.offset.x + v as f32 * app.zoom;
            let is_major = ((v / major).round() - v / major).abs() < 1e-9;
            // 抽稀的 major 仍画中等刻度,保持节律感
            let show_label = is_major && k.rem_euclid(thin_x * majors_per_minor) == 0;
            let len = if show_label { 10.0 } else if is_major { 7.0 } else { 5.0 };
            painter.add(Shape::line_segment([Pos2::new(sx, canvas.top() + RULER), Pos2::new(sx, canvas.top() + RULER - len)], Stroke::new(1.0_f32, if is_major { pal.ruler_text } else { pal.ruler_minor })));
            if show_label {
                t(painter, Pos2::new(sx, canvas.top() + RULER - 10.0 - 2.0), Align2::CENTER_BOTTOM, fmt(v), 9.0, pal.ruler_text);
            }
        }
        k += 1;
    }
    let (_, wy0) = app.s2w(Pos2::new(0.0, canvas.top() + RULER));
    let (_, wy1) = app.s2w(Pos2::new(0.0, canvas.bottom()));
    let mut k = (wy0 / minor).floor() as i64;
    while (k as f64) * minor <= wy1 {
        let v = (k as f64) * minor;
        if v >= 0.0 {
            let sy = app.offset.y + v as f32 * app.zoom;
            let is_major = ((v / major).round() - v / major).abs() < 1e-9;
            let show_label = is_major && k.rem_euclid(thin_y * majors_per_minor) == 0;
            let len = if show_label { 10.0 } else if is_major { 7.0 } else { 5.0 };
            painter.add(Shape::line_segment([Pos2::new(canvas.left() + ry, sy), Pos2::new(canvas.left() + ry - len, sy)], Stroke::new(1.0_f32, if is_major { pal.ruler_text } else { pal.ruler_minor })));
            if show_label {
                t(painter, Pos2::new(canvas.left() + ry - 10.0 - 3.0, sy), Align2::RIGHT_CENTER, fmt(v), 9.0, pal.ruler_text);
            }
        }
        k += 1;
    }

    // 十字线(坐标读数收在底部状态栏)
    if pointer_in {
        if let Some(p) = pointer {
            painter.add(Shape::line_segment([Pos2::new(p.x, canvas.top() + RULER), Pos2::new(p.x, canvas.bottom())], Stroke::new(1.0_f32, pal.dim_gray)));
            painter.add(Shape::line_segment([Pos2::new(canvas.left() + ry, p.y), Pos2::new(canvas.right(), p.y)], Stroke::new(1.0_f32, pal.dim_gray)));
            painter.add(Shape::line_segment([Pos2::new(p.x, canvas.top()), Pos2::new(p.x, canvas.top() + RULER)], Stroke::new(2.0_f32, pal.shot_stroke)));
            painter.add(Shape::line_segment([Pos2::new(canvas.left(), p.y), Pos2::new(canvas.left() + ry, p.y)], Stroke::new(2.0_f32, pal.shot_stroke)));
        }
    }

    // 选中范围高亮带 + 尺寸 pill(收在标尺带内)
    if let Some(sel_rect) = selection_bounds(app) {
        let p1 = app.w2s(sel_rect.x, sel_rect.y);
        let p2 = app.w2s(sel_rect.x + sel_rect.w, sel_rect.y + sel_rect.h);
        let hl = Color32::from_rgba_unmultiplied(59, 130, 246, 90);
        if p2.x > canvas.left() + RULER && p1.x < canvas.right() {
            let bx1 = p1.x.max(canvas.left() + RULER);
            let bx2 = p2.x.min(canvas.right());
            painter.add(Shape::rect_filled(Rect::from_min_max(Pos2::new(bx1, canvas.top()), Pos2::new(bx2, canvas.top() + RULER)), CornerRadius::ZERO, hl));
            let cx = (bx1 + bx2) / 2.0;
            let pill = Rect::from_center_size(Pos2::new(cx, canvas.top() + RULER / 2.0), Vec2::new(90.0, RULER - 8.0));
            painter.add(Shape::rect_filled(pill, CornerRadius::same(2), pal.pill_bg));
            t(painter, pill.center(), Align2::CENTER_CENTER, fmt_um(sel_rect.w), 10.0, Color32::from_rgb(0x38, 0xbd, 0xf8));
        }
        if p2.y > canvas.top() + RULER && p1.y < canvas.bottom() {
            let by1 = p1.y.max(canvas.top() + RULER);
            let by2 = p2.y.min(canvas.bottom());
            painter.add(Shape::rect_filled(Rect::from_min_max(Pos2::new(canvas.left(), by1), Pos2::new(canvas.left() + ry, by2)), CornerRadius::ZERO, hl));
            let cy = (by1 + by2) / 2.0;
            let pill = Rect::from_center_size(Pos2::new(canvas.left() + ry / 2.0, cy), Vec2::new(RULER - 8.0, 90.0));
            painter.add(Shape::rect_filled(pill, CornerRadius::same(2), pal.pill_bg));
            // 竖排字符(egui 文本不支持旋转)
            let label: String = fmt_um(sel_rect.h).chars().map(|c| c.to_string()).collect::<Vec<_>>().join("\n");
            t(painter, pill.center(), Align2::CENTER_CENTER, label, 9.0, Color32::from_rgb(0x38, 0xbd, 0xf8));
        }
    }
}

fn selection_bounds(app: &DieLayoutApp) -> Option<WRect> {
    match app.selected {
        Selection::Shot(id) => app.shots.iter().find(|s| s.id == id).map(|s| shot_rect(s, &app.dies)),
        Selection::Die(id) => find_die(app, id).map(|(s, d)| die_rect(s, &app.dies, d)),
        Selection::None => None,
    }
}

/// 右下角工具栏(纯绘制;点击命中在交互层处理)
fn draw_toolbar(painter: &egui::Painter, app: &DieLayoutApp, canvas: Rect) {
    let bar = toolbar_rect(canvas);
    let dark = app.theme_dark;
    let bg = if dark { Color32::from_rgb(0x17, 0x17, 0x17) } else { Color32::WHITE };
    let border = if dark { Color32::from_rgb(0x26, 0x26, 0x26) } else { Color32::from_rgb(0xe5, 0xe5, 0xe5) };
    let fg = if dark { Color32::from_rgb(0xa3, 0xa3, 0xa3) } else { Color32::from_rgb(0x73, 0x73, 0x73) };
    painter.add(Shape::rect_filled(bar, CornerRadius::same(8), bg));
    painter.add(Shape::rect_stroke(bar, CornerRadius::same(8), Stroke::new(1.0_f32, border), StrokeKind::Inside));

    t(painter, Pos2::new(bar.left() + 13.0, bar.center().y), Align2::CENTER_CENTER, "−", 14.0, fg);
    t(painter, Pos2::new(bar.left() + 39.0, bar.center().y), Align2::CENTER_CENTER, format!("{}%", (app.zoom * 100.0).round()), 11.0, fg);
    t(painter, Pos2::new(bar.left() + 65.0, bar.center().y), Align2::CENTER_CENTER, "+", 14.0, fg);
    painter.add(Shape::line_segment([Pos2::new(bar.left() + 78.0, bar.top() + 5.0), Pos2::new(bar.left() + 78.0, bar.bottom() - 5.0)], Stroke::new(1.0_f32, border)));
    t(painter, Pos2::new(bar.left() + 91.0, bar.center().y), Align2::CENTER_CENTER, "⛶", 13.0, fg);
    // 标尺工具开关(激活时 amber 底)
    let rbtn = Rect::from_min_size(Pos2::new(bar.left() + 104.0, bar.top()), Vec2::new(36.0, bar.height()));
    if app.tool == ToolMode::Ruler {
        let amber = if dark { Color32::from_rgb(0xfb, 0xbf, 0x24) } else { Color32::from_rgb(0xf5, 0x9e, 0x0b) };
        painter.add(Shape::rect_filled(rbtn.shrink2(Vec2::new(3.0, 5.0)), CornerRadius::same(5), amber.linear_multiply(0.25)));
        painter.add(Shape::rect_stroke(rbtn.shrink2(Vec2::new(3.0, 5.0)), CornerRadius::same(5), Stroke::new(1.0_f32, amber), StrokeKind::Inside));
        t(painter, rbtn.center(), Align2::CENTER_CENTER, "╱", 13.0, amber);
    } else {
        t(painter, rbtn.center(), Align2::CENTER_CENTER, "╱", 13.0, fg);
    }
}

pub fn toolbar_rect(canvas: Rect) -> Rect {
    Rect::from_min_size(Pos2::new(canvas.right() - 16.0 - 140.0, canvas.bottom() - 16.0 - 34.0), Vec2::new(140.0, 34.0))
}


/// 单轴吸附:候选线(cand, span_from, span_to)vs 自身三边,取 |diff| 最小且在阈值内
fn snap_axis(cands: &[(f64, f64, f64)], mine: [f64; 3], axis: bool, thresh: f64) -> Option<(f64, Guide)> {
    let mut best: Option<(f64, Guide)> = None;
    for &(c, from, to) in cands {
        for &m in &mine {
            let diff = c - m;
            if diff.abs() <= thresh && best.as_ref().map(|(d, _)| diff.abs() < d.abs()).unwrap_or(true) {
                best = Some((diff, Guide { axis, at: c, from, to }));
            }
        }
    }
    best
}

/// Figma 智能参考线:Shot 移动时与其他 Shot 边缘/中线、世界 0 轴对齐(6px 阈值)
fn snap_shot_delta(app: &DieLayoutApp, id: ShotId, dx: f64, dy: f64) -> (f64, f64, Vec<Guide>) {
    let zoom = app.zoom as f64;
    let thresh = 6.0 / zoom;
    let Some(me) = app.shots.iter().find(|s| s.id == id) else { return (dx, dy, Vec::new()) };
    let sr = shot_rect(me, &app.dies);
    let (nx, ny) = (sr.x + dx, sr.y + dy);
    let my_x = [nx, nx + sr.w / 2.0, nx + sr.w];
    let my_y = [ny, ny + sr.h / 2.0, ny + sr.h];

    let mut cands_x: Vec<(f64, f64, f64)> = vec![(0.0, ny, ny + sr.h)];
    let mut cands_y: Vec<(f64, f64, f64)> = vec![(0.0, nx, nx + sr.w)];
    for s in &app.shots {
        if s.id == id || app.isolated.map(|i| i != s.id).unwrap_or(false) {
            continue;
        }
        let r = shot_rect(s, &app.dies);
        cands_x.push((r.x, r.y, r.y + r.h));
        cands_x.push((r.x + r.w / 2.0, r.y, r.y + r.h));
        cands_x.push((r.x + r.w, r.y, r.y + r.h));
        cands_y.push((r.y, r.x, r.x + r.w));
        cands_y.push((r.y + r.h / 2.0, r.x, r.x + r.w));
        cands_y.push((r.y + r.h, r.x, r.x + r.w));
    }
    let bx = snap_axis(&cands_x, my_x, true, thresh);
    let by = snap_axis(&cands_y, my_y, false, thresh);
    let ndx = bx.map(|(d, _)| dx + d).unwrap_or(dx);
    let ndy = by.map(|(d, _)| dy + d).unwrap_or(dy);
    let guides: Vec<Guide> = bx.into_iter().chain(by.into_iter()).map(|(_, g)| g).collect();
    (ndx, ndy, guides)
}

/// 绘制对齐参考线(Figma 品红,贯穿两对象重叠跨度)
fn draw_guides(painter: &egui::Painter, app: &DieLayoutApp) {
    if app.guides.is_empty() {
        return;
    }
    let c = Color32::from_rgb(0xec, 0x48, 0x99);
    for g in &app.guides {
        let (p1, p2) = if g.axis {
            (app.w2s(g.at, g.from), app.w2s(g.at, g.to))
        } else {
            (app.w2s(g.from, g.at), app.w2s(g.to, g.at))
        };
        painter.add(Shape::line_segment([p1, p2], Stroke::new(1.0_f32, c)));
    }
}

/// 点到线段距离(屏幕坐标,右键删标尺用)
fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    let proj = |q: Pos2| (q - a).length();
    if len2 < 1e-6 {
        return proj(p);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// 命中测量标尺(线段 6px 内)
fn ruler_at(app: &DieLayoutApp, p: Pos2) -> Option<usize> {
    app.measure_rulers.iter().position(|r| {
        let a = app.w2s(r.a.0, r.a.1);
        let b = app.w2s(r.b.0, r.b.1);
        seg_dist(p, a, b) < 6.0
    })
}

/// 测量标尺配色:amber(与 blue 选中/pink 间距区分)
fn ruler_colors(dark: bool) -> (Color32, Color32) {
    if dark {
        (Color32::from_rgb(0xfb, 0xbf, 0x24), Color32::from_rgb(0xfd, 0xe6, 0x8a))
    } else {
        (Color32::from_rgb(0xd9, 0x77, 0x06), Color32::from_rgb(0xb4, 0x53, 0x09))
    }
}

fn draw_measure_rulers(painter: &egui::Painter, app: &DieLayoutApp, pal: Palette) {
    for r in &app.measure_rulers {
        draw_ruler_shape(painter, app, r, pal, false);
    }
}

/// 单条测量标尺:端点圆点 + 连线 + 中点 pill 标注;preview=true 用虚线
fn draw_ruler_shape(painter: &egui::Painter, app: &DieLayoutApp, r: &MeasureRuler, pal: Palette, preview: bool) {
    let (line_c, text_c) = ruler_colors(app.theme_dark);
    let a = app.w2s(r.a.0, r.a.1);
    let b = app.w2s(r.b.0, r.b.1);
    if preview {
        painter.extend(Shape::dashed_line(&[a, b], Stroke::new(1.5_f32, line_c), 6.0, 4.0));
    } else {
        painter.add(Shape::line_segment([a, b], Stroke::new(1.5_f32, line_c)));
    }
    for p in [a, b] {
        painter.add(Shape::circle_filled(p, 3.0, line_c));
    }
    // 长度 pill:中点上方,文字用 galley 精确量宽
    let label = fmt_um(r.len());
    let galley = painter.layout(label, egui::FontId::proportional(10.0), text_c, f32::INFINITY);
    let size = galley.rect.size() + Vec2::new(12.0, 6.0);
    let mid = (a + b.to_vec2()) / 2.0;
    let pill = Rect::from_center_size(mid - Vec2::new(0.0, size.y / 2.0 + 6.0), size);
    painter.add(Shape::rect_filled(pill, 3, pal.pill_bg));
    painter.add(Shape::rect_stroke(pill, 3, Stroke::new(1.0_f32, line_c), StrokeKind::Inside));
    painter.galley(pill.min + Vec2::new(6.0, 3.0), galley, text_c);
}

/// 选中对象实测尺寸徽标(Die/Shot 顶部悬浮 pill,跟随拖动/resize 实时更新)
fn draw_selection_badge(painter: &egui::Painter, app: &DieLayoutApp, canvas: Rect, pal: Palette) {
    let (wr, label) = match app.selected {
        Selection::Die(id) => {
            let Some((s, d)) = find_die(app, id) else { return };
            if app.isolated.map(|i| i != s.id).unwrap_or(false) {
                return;
            }
            (die_rect(s, &app.dies, d), format!("{} × {}", fmt_um(d.w), fmt_um(d.h)))
        }
        Selection::Shot(id) => {
            let Some(s) = app.shots.iter().find(|s| s.id == id) else { return };
            if app.isolated.map(|i| i != s.id).unwrap_or(false) {
                return;
            }
            let r = shot_rect(s, &app.dies);
            (r, format!("{} × {}", fmt_um(r.w), fmt_um(r.h)))
        }
        Selection::None => return,
    };
    let r = wr_to_screen(app, &wr);
    let ry = ruler_band_w(app, canvas);
    let galley = painter.layout(label, egui::FontId::proportional(10.0), pal.shot_stroke, f32::INFINITY);
    let size = galley.rect.size() + Vec2::new(12.0, 6.0);
    // 默认放顶边上方,空间不足改放下方;水平方向钳制在画布内
    let mut y = r.top() - size.y - 4.0;
    if y < canvas.top() + RULER + 2.0 {
        y = r.bottom() + 4.0;
    }
    let cx = r.center().x.clamp(canvas.left() + ry + size.x / 2.0 + 2.0, canvas.right() - size.x / 2.0 - 2.0);
    let pill = Rect::from_min_size(Pos2::new(cx - size.x / 2.0, y), size);
    painter.add(Shape::rect_filled(pill, 3, pal.pill_bg));
    painter.add(Shape::rect_stroke(pill, 3, Stroke::new(1.0_f32, pal.shot_stroke), StrokeKind::Inside));
    painter.galley(pill.min + Vec2::new(6.0, 3.0), galley, pal.shot_stroke);
}
