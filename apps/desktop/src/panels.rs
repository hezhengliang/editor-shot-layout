// 左/右侧面板:gpui-kit / shadcn 风格 —— Card 容器(border + rounded-lg + 统一间距)
// 表单:label 在上、控件在下(h-32px);按钮:primary / secondary / outline / destructive 四态

use crate::app::DieLayoutApp;
use crate::geometry::*;
use crate::model::*;
use egui::{Align2, Color32, RichText, Stroke, Ui};

fn zc(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Card 容器(shadcn Card:border + 背景 + rounded-lg,标题为 card header)
fn card(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    let frame = egui::Frame::NONE
        .fill(ui.visuals().window_fill)
        .stroke(Stroke::new(1.0_f32, ui.visuals().window_stroke.color))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 12));
    // Frame 默认收缩包裹内容;set_min_width 强制满宽,避免窄内容卡片(如"画布统计")变窄
    let w = ui.available_width();
    frame.show(ui, |ui| {
        ui.set_min_width(w - 24.0); // 减左右内边距 12×2
        ui.label(RichText::new(title).size(13.0).strong());
        ui.add_space(8.0);
        add(ui);
    });
    ui.add_space(12.0);
}

/// 表单字段标签(shadcn Label:sm、muted)
fn flabel(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(11.5).color(ui.visuals().weak_text_color()));
}

/// 控件标准高度
const CTRL_H: f32 = 32.0;

/// 按钮变体(委托 app::kit_button 自绘,文案精确居中)
#[derive(Clone, Copy, PartialEq)]
enum Btn {
    Outline,
    Destructive,
}

fn styled_button(ui: &mut Ui, text: &str, kind: Btn) -> egui::Response {
    styled_button_w(ui, text, kind, None)
}

/// width = Some(w) 时按指定宽度(通栏铺满用);None 为内容宽
fn styled_button_w(ui: &mut Ui, text: &str, kind: Btn, width: Option<f32>) -> egui::Response {
    let style = match kind {
        Btn::Outline => crate::app::KitBtn::Outline,
        Btn::Destructive => crate::app::KitBtn::Destructive,
    };
    crate::app::kit_button_w(ui, text, style, width)
}

// ---------------- 左侧面板 ----------------
pub fn left_panel(ui: &mut Ui, app: &mut DieLayoutApp) {
    // —— Die 模板管理 ——
    card(ui, "Die 模板管理", |ui| {
        let mut name: String = ui.ctx().data_mut(|d| d.get_temp(egui::Id::new("tpl_name")).unwrap_or_else(|| "Die 2×2".into()));
        let mut w: f64 = ui.ctx().data_mut(|d| d.get_temp(egui::Id::new("tpl_w")).unwrap_or(1000.0));
        let mut h: f64 = ui.ctx().data_mut(|d| d.get_temp(egui::Id::new("tpl_h")).unwrap_or(1000.0));
        let mut color: Color32 = ui.ctx().data_mut(|d| d.get_temp(egui::Id::new("tpl_c")).unwrap_or(Color32::from_rgb(0x4a, 0xde, 0x80)));

        flabel(ui, "名称");
        if ui
            .add_sized(
                [ui.available_width(), CTRL_H],
                egui::TextEdit::singleline(&mut name).vertical_align(egui::Align::Center),
            )
            .changed()
        {
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("tpl_name"), name.clone()));
        }
        ui.add_space(6.0);
        ui.columns(2, |cols| {
            flabel(&mut cols[0], "宽 W (μm)");
            { let w_ = cols[0].available_width(); drag_centered(&mut cols[0], w_, &mut w, 10.0, 3); }
            flabel(&mut cols[1], "高 H (μm)");
            { let w_ = cols[1].available_width(); drag_centered(&mut cols[1], w_, &mut h, 10.0, 3); }
        });
        ui.ctx().data_mut(|d| {
            d.insert_temp(egui::Id::new("tpl_w"), w);
            d.insert_temp(egui::Id::new("tpl_h"), h);
        });
        ui.add_space(6.0);
        flabel(ui, "颜色");
        ui.color_edit_button_srgba(&mut color);
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("tpl_c"), color));
        ui.add_space(8.0);
        if crate::app::primary_button(ui, "＋ 添加 Die 模板").clicked() {
            app.presets.push(Preset {
                id: format!("custom-{}", app.next_id),
                name: name.trim().to_string(),
                w: (w * 1000.0).round() / 1000.0,
                h: (h * 1000.0).round() / 1000.0,
                color,
            });
            app.set_status(format!("已添加模板 {}({} × {})", name, fmt_um(w), fmt_um(h)));
        }
        if app.presets.len() > 1 {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            let mut del = app.presets.len() - 1;
            flabel(ui, "删除模板");
            {
                let names: Vec<String> = app.presets.iter().map(|p| p.name.clone()).collect();
                let cur = names.get(del).cloned().unwrap_or_default();
                if let Some(i) = crate::app::kit_select(ui, "del_tpl", &cur, &names) {
                    del = i;
                }
            }
            ui.add_space(6.0);
            if styled_button(ui, "删除所选模板", Btn::Destructive).clicked() {
                if app.presets.len() <= 1 {
                    app.set_status("至少保留一个 Die 模板");
                } else {
                    let removed = app.presets.remove(del);
                    app.set_status(format!("已删除模板 {}", removed.name));
                }
            }
        }
    });

    // —— 小 Die 库 ——
    card(ui, "小 Die 库", |ui| {
        let mut to_place: Option<usize> = None;
        let mut drag_start: Option<usize> = None;
        for (i, p) in app.presets.iter().enumerate() {
            let row = ui.horizontal(|ui| {
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), CTRL_H), egui::Sense::click_and_drag());
                let visuals = ui.visuals().clone();
                let bg = if resp.hovered() { visuals.widgets.inactive.weak_bg_fill } else { Color32::TRANSPARENT };
                ui.painter().rect_filled(rect, 6, bg);
                ui.painter().rect_filled(egui::Rect::from_min_size(rect.min + egui::vec2(10.0, 10.0), egui::vec2(13.0, 13.0)), 3, p.color);
                ui.painter().text(
                    rect.min + egui::vec2(32.0, CTRL_H / 2.0),
                    Align2::LEFT_CENTER,
                    &p.name,
                    egui::FontId::proportional(13.0),
                    visuals.text_color(),
                );
                resp
            });
            let resp = row.inner;
            if resp.hovered() {
                resp.ctx.set_cursor_icon(egui::CursorIcon::Grab);
            }
            if resp.drag_started() {
                drag_start = Some(i);
            }
            if resp.double_clicked() {
                to_place = Some(i);
            }
            resp.on_hover_text(format!("{} · {} × {} · 拖入 Shot / 双击自动放置", p.name, fmt_um(p.w), fmt_um(p.h)));
        }
        if let Some(i) = drag_start {
            let pos = ui.ctx().input(|inp| inp.pointer.latest_pos()).unwrap_or_else(|| ui.min_rect().center());
            app.drag = Some(DragState { card: DragCard::Preset(i), screen: pos, started: true });
        }
        if let Some(i) = to_place {
            app.auto_place_die(i);
        }
    });

    // —— Shot 容器 ——
    card(ui, "Shot 容器", |ui| {
        let row = ui.horizontal(|ui| {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 48.0), egui::Sense::click_and_drag());
            let visuals = ui.visuals().clone();
            let border = if resp.hovered() { zc(0x3b, 0x82, 0xf6) } else { visuals.window_stroke.color };
            ui.painter().rect_filled(rect, 8, visuals.extreme_bg_color);
            // 虚线边框(圆角矩形,四段绘制)
            let stroke = Stroke::new(1.0_f32, border);
            let d = 6.0;
            let g = 4.0;
            let pts = |a: egui::Pos2, b: egui::Pos2| [a, b];
            ui.painter().extend(egui::Shape::dashed_line(&pts(rect.min, egui::Pos2::new(rect.max.x, rect.min.y)), stroke, d, g));
            ui.painter().extend(egui::Shape::dashed_line(&pts(egui::Pos2::new(rect.max.x, rect.min.y), rect.max), stroke, d, g));
            ui.painter().extend(egui::Shape::dashed_line(&pts(rect.max, egui::Pos2::new(rect.min.x, rect.max.y)), stroke, d, g));
            ui.painter().extend(egui::Shape::dashed_line(&pts(egui::Pos2::new(rect.min.x, rect.max.y), rect.min), stroke, d, g));
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, "▦  Shot 容器(拖入 / 双击)", egui::FontId::proportional(13.0), visuals.text_color());
            resp
        });
        let resp = row.inner;
        if resp.hovered() {
            resp.ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
        if resp.drag_started() {
            let pos = resp.ctx.input(|inp| inp.pointer.latest_pos()).unwrap_or(resp.rect.center());
            app.drag = Some(DragState { card: DragCard::ShotCard, screen: pos, started: true });
        }
        if resp.double_clicked() {
            app.auto_place_shot();
        }
        resp.on_hover_text("Shot(曝光场)容器:拖入画布创建,默认 2×2 槽位、scribe 80μm");
    });
}

// ---------------- 右侧面板 ----------------
pub fn right_panel(ui: &mut Ui, app: &mut DieLayoutApp) {
    // Tabs(segmented):tab_bar 底 #171717,激活 tab 底 #0a0a0a
    {
        let visuals = ui.visuals().clone();
        let (seg, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::hover());
        ui.painter().rect_filled(seg, 8, visuals.faint_bg_color);
        let half = seg.width() / 2.0;
        for (n, (tab, label)) in [(RightTab::Info, "信息"), (RightTab::Config, "配置")].iter().enumerate() {
            let item = egui::Rect::from_min_size(seg.min + egui::vec2(n as f32 * half + 3.0, 3.0), egui::vec2(half - 6.0, seg.height() - 6.0));
            let active = app.right_tab == *tab;
            if active {
                ui.painter().rect_filled(item, 6, visuals.window_fill);
            }
            let color = if active { visuals.text_color() } else { zc(0xd4, 0xd4, 0xd4) };
            ui.painter().text(item.center(), Align2::CENTER_CENTER, label, egui::FontId::proportional(13.0), color);
            let resp = ui.interact(item, ui.id().with(("rtab", n)), egui::Sense::click());
            if resp.clicked() {
                app.right_tab = *tab;
            }
        }
    }
    ui.add_space(12.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        match app.right_tab {
            RightTab::Info => info_tab(ui, app),
            RightTab::Config => config_tab(ui, app),
        }
        // 操作固定在最底部:删除选中通栏,清空/重置半行并排
        card(ui, "操作", |ui| {
            let full = ui.available_width();
            if styled_button_w(ui, "删除选中(Delete)", Btn::Destructive, Some(full)).clicked() {
                app.delete_selected();
            }
            ui.add_space(6.0);
            let ctx = ui.ctx().clone();
            ui.columns(2, |cols| {
                let half = cols[0].available_width();
                if styled_button_w(&mut cols[0], "清空布局", Btn::Outline, Some(half)).clicked() {
                    app.clear_all();
                }
                if styled_button_w(&mut cols[1], "重置视图", Btn::Outline, Some(half)).clicked() {
                    app.fit_view(ctx.screen_rect());
                }
            });
        });
    });
}

fn info_tab(ui: &mut Ui, app: &mut DieLayoutApp) {
    if let Selection::Shot(id) = app.selected {
        if let Some(s) = app.shots.iter().find(|s| s.id == id) {
            card(ui, &format!("Shot#{} 信息", id.0), |ui| {
                let w = shot_w(s, &app.dies);
                let h = shot_h(s, &app.dies);
                kv(ui, "概要", format!("{}×{} 槽位", s.cols, s.rows));
                kv(ui, "内圈 scribe 实际 X / Y", format!("{} / {}(最小 {} / {})", fmt_um(actual_gap_x(s, &app.dies)), fmt_um(actual_gap_y(s, &app.dies)), fmt_um(s.gap_x), fmt_um(s.gap_y)));
                kv(ui, "外圈 scribe pad X / Y", format!("{} / {}", fmt_um(s.pad_x), fmt_um(s.pad_y)));
                kv(ui, "外形 W × H", format!("{} × {}", fmt_um(w), fmt_um(h)));
                kv(ui, "位置 X, Y", format!("({}, {})", fmt_um(s.x), fmt_um(s.y)));
                kv(ui, "场利用率 (Die/Shot)", format!("{:.0}%", utilization(s, &app.dies)));
            });
        }
    }
    if let Selection::Die(id) = app.selected {
        if let Some(d) = app.dies.iter().find(|d| d.id == id) {
            card(ui, &format!("{} 信息", d.name), |ui| {
                kv(ui, "槽位", format!("({},{})", d.slot.0, d.slot.1));
                kv(ui, "尺寸", format!("{} × {}", fmt_um(d.w), fmt_um(d.h)));
            });
        }
    }
    card(ui, "画布统计", |ui| {
        kv(ui, "Die 数量", format!("{}", app.dies.len()));
        kv(ui, "Shot 数量", format!("{}", app.shots.len()));
    });
}

fn utilization(s: &Shot, dies: &[Die]) -> f64 {
    let area: f64 = s.dies.iter().filter_map(|&id| dies.iter().find(|d| d.id == id)).map(|d| d.w * d.h).sum();
    let total = shot_w(s, dies) * shot_h(s, dies);
    if total > 0.0 { area / total * 100.0 } else { 0.0 }
}

/// 信息行:标签在上、值在下(与 web 版 tweakpane 上下结构一致;长值换行不溢出)
fn kv(ui: &mut Ui, k: &str, v: String) {
    ui.label(RichText::new(k).size(11.0).color(ui.visuals().weak_text_color()));
    ui.label(RichText::new(v).size(12.5));
    ui.add_space(4.0);
}

/// 垂直居中布局的 DragValue(内部 TextEdit 跟随 ui.layout().vertical_align)
fn drag_centered(ui: &mut Ui, w: f32, value: &mut f64, speed: f64, decimals: usize) {
    ui.scope_builder(
        egui::UiBuilder::new().layout(egui::Layout::top_down(egui::Align::Center)),
        |ui| {
            ui.add_sized([w, CTRL_H], egui::DragValue::new(value).speed(speed).max_decimals(decimals));
        },
    );
}

fn num_field(ui: &mut Ui, label: &str, value: &mut f64, speed: f64) {
    flabel(ui, label);
    drag_centered(ui, ui.available_width(), value, speed, 3);
    ui.add_space(6.0);
}

fn config_tab(ui: &mut Ui, app: &mut DieLayoutApp) {
    if let Selection::Shot(id) = app.selected {
        if let Some(idx) = app.shots.iter().position(|s| s.id == id) {
            card(ui, "Shot 配置", |ui| {
                let mut s = app.shots[idx].clone();
                let presets = [("自定义", 0usize, 0usize), ("2×2", 2, 2), ("4×4 标准", 4, 4), ("4×8 存储阵列", 4, 8), ("2×6 长条", 2, 6), ("6×4", 6, 4)];
                let mut sel = presets.iter().position(|(_, c, r)| *c == s.cols && *r == s.rows).unwrap_or(0);
                flabel(ui, "排布预设");
                {
                    let labels: Vec<String> = presets.iter().map(|(l, _, _)| l.to_string()).collect();
                    if let Some(i) = crate::app::kit_select(ui, "shot_preset", labels[sel].as_str(), &labels) {
                        sel = i;
                    }
                }
                ui.add_space(6.0);
                if sel > 0 && (s.cols != presets[sel].1 || s.rows != presets[sel].2) {
                    let (_, c, r) = presets[sel];
                    app.apply_shot_params(id, c, r, s.pad_x, s.pad_y, s.gap_x, s.gap_y, s.w, s.h);
                    s = app.shots[idx].clone();
                }
                ui.checkbox(&mut s.show_dims, RichText::new("显示间距标注").size(12.5));
                ui.checkbox(&mut s.show_outer, RichText::new("显示外圈/容器尺寸").size(12.5));
                // 标注开关直接落库(s 是克隆,且 apply_shot_params 不接收这两个字段)
                if let Some(real) = app.shots.get_mut(idx) {
                    real.show_dims = s.show_dims;
                    real.show_outer = s.show_outer;
                }
                ui.add_space(6.0);
                ui.columns(2, |cols| {
                    let mut cols_i = s.cols as f64;
                    let mut rows_i = s.rows as f64;
                    flabel(&mut cols[0], "槽位列数");
                    { let w_ = cols[0].available_width(); drag_centered(&mut cols[0], w_, &mut cols_i, 0.2, 0); }
                    flabel(&mut cols[1], "槽位行数");
                    { let w_ = cols[1].available_width(); drag_centered(&mut cols[1], w_, &mut rows_i, 0.2, 0); }
                    s.cols = (cols_i.round() as usize).clamp(1, 12);
                    s.rows = (rows_i.round() as usize).clamp(1, 12);
                });
                ui.add_space(2.0);
                let mut sw = s.w;
                let mut sh = s.h;
                ui.columns(2, |cols| {
                    num_field(&mut cols[0], "外形 W (μm)", &mut sw, 20.0);
                    num_field(&mut cols[1], "外形 H (μm)", &mut sh, 20.0);
                });

                s.w = sw.max(1.0);
                s.h = sh.max(1.0);
                let mut px = s.pad_x;
                let mut py = s.pad_y;
                let mut gx = s.gap_x;
                let mut gy = s.gap_y;
                ui.columns(2, |cols| {
                    num_field(&mut cols[0], "外圈 padX (μm)", &mut px, 5.0);
                    num_field(&mut cols[1], "外圈 padY (μm)", &mut py, 5.0);
                    num_field(&mut cols[0], "内圈最小 gapX (μm)", &mut gx, 2.0);
                    num_field(&mut cols[1], "内圈最小 gapY (μm)", &mut gy, 2.0);
                });
                s.pad_x = px.max(0.001);
                s.pad_y = py.max(0.001);
                s.gap_x = gx.max(0.0);
                s.gap_y = gy.max(0.0);
                app.apply_shot_params(id, s.cols, s.rows, s.pad_x, s.pad_y, s.gap_x, s.gap_y, s.w, s.h);
                ui.add_space(4.0);
                if crate::app::primary_button(ui, "填充空槽(用首颗 Die 模板)").clicked() {
                    let n = app.fill_shot(id, 0);
                    app.set_status(format!("已填充 {} 个空槽位", n));
                }
                ui.add_space(6.0);
                let isolating = app.isolated == Some(id);
                if styled_button(ui, if isolating { "退出隔离(双击 Shot 亦可)" } else { "隔离此 Shot(双击 Shot 亦可)" }, Btn::Outline).clicked() {
                    app.toggle_isolate(id);
                }
            });
        }
    }
    if let Selection::Die(id) = app.selected {
        if let Some(idx) = app.dies.iter().position(|d| d.id == id) {
            card(ui, "Die 配置", |ui| {
                let mut d = app.dies[idx].clone();
                let shot = app.shots.iter().find(|s| s.id == d.shot).cloned();
                if let Some(s) = shot {
                    let mut sx = d.slot.0 as f64;
                    let mut sy = d.slot.1 as f64;
                    ui.columns(2, |cols| {
                        flabel(&mut cols[0], "槽位 X");
                        { let w_ = cols[0].available_width(); drag_centered(&mut cols[0], w_, &mut sx, 0.2, 0); }
                        flabel(&mut cols[1], "槽位 Y");
                        { let w_ = cols[1].available_width(); drag_centered(&mut cols[1], w_, &mut sy, 0.2, 0); }
                    });
                    d.slot.0 = (sx.round() as usize).min(s.cols - 1);
                    d.slot.1 = (sy.round() as usize).min(s.rows - 1);
                    if app.can_place_in_shot(s.id, d.slot.0, d.slot.1, d.w, d.h, Some(id)) {
                        app.dies[idx].slot = d.slot;
                    }
                    ui.add_space(4.0);
                    if crate::app::primary_button(ui, "填充 Shot 空槽(用此 Die 模板)").clicked() {
                        let n = app.fill_shot(s.id, 0);
                        app.set_status(format!("已填充 {} 个空槽位", n));
                    }
                    ui.add_space(6.0);
                }
                let mut dw = d.w;
                let mut dh = d.h;
                ui.columns(2, |cols| {
                    num_field(&mut cols[0], "宽 W (μm)", &mut dw, 5.0);
                    num_field(&mut cols[1], "高 H (μm)", &mut dh, 5.0);
                });
                d.w = dw.max(0.001);
                d.h = dh.max(0.001);
                if let Some(s) = app.shots.iter().find(|s| s.id == d.shot) {
                    if app.can_place_in_shot(s.id, d.slot.0, d.slot.1, d.w, d.h, Some(id)) {
                        app.dies[idx].w = d.w;
                        app.dies[idx].h = d.h;
                    } else {
                        app.set_status("调整后超出 Shot 容量(间距已最小),已保留原尺寸");
                    }
                }
                flabel(ui, "颜色");
                let mut c = d.color;
                if ui.color_edit_button_srgba(&mut c).changed() {
                    app.dies[idx].color = c;
                }
            });
        }
    }
    if app.selected == Selection::None {
        card(ui, "提示", |ui| {
            ui.label(RichText::new("未选中元素:点击画布中的 Shot 或 Die").size(12.0).color(ui.visuals().weak_text_color()));
        });
    }
}
