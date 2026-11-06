// 入口:eframe 原生窗口,加载中文字体(macOS),装配三栏布局

mod app;
mod bitmap;
mod canvas;
mod geometry;
mod model;
mod panels;

use app::{AppView, DieLayoutApp};
use model::{Selection, ToolMode};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1500.0, 950.0]).with_min_inner_size([1100.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DieLayoutEditor (egui)",
        options,
        Box::new(|cc| Ok(Box::new(DieLayoutApp::new(cc)))),
    )
}

impl eframe::App for DieLayoutApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count += 1;
        // egui-winit 会按系统外观回写主题,每帧强制重刷应用主题(gpui-kit 风格)
        crate::app::apply_theme(ctx, self.theme_dark);

        // 调试截图:设 EGUI_SHOT=/path.png 时第 90 帧自动截图退出;或按 F10 手动截图
        let shot_path = std::env::var("EGUI_SHOT").ok();
        if shot_path.is_some() && self.frame_count == 90 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        ctx.input(|i| {
            if i.key_down(egui::Key::F10) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            for ev in &i.events {
                if let egui::Event::Screenshot { image, .. } = ev {
                    let path = shot_path.clone().unwrap_or_else(|| "/tmp/egui-capture.png".into());
                    let rgba = image::RgbaImage::from_raw(
                        image.width() as u32,
                        image.height() as u32,
                        image.pixels.iter().flat_map(|p| [p.r(), p.g(), p.b(), p.a()]).collect(),
                    );
                    if let Some(img) = rgba {
                        if img.save(&path).is_ok() {
                            eprintln!("screenshot saved: {}", path);
                        }
                    }
                    if std::env::var("EGUI_SHOT").is_ok() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        });

        // 顶栏:标题 + 状态 + 主题切换(gpui-kit 风格:panel 底 + 底部分隔线)
        egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::NONE
                    .fill(ctx.style().visuals.faint_bg_color) // title_bar.background #171717
                    .stroke(egui::Stroke::new(1.0_f32, ctx.style().visuals.window_stroke.color))
                    .inner_margin(egui::Margin::symmetric(12, 7)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Shot 布局编辑器").strong());
                    ui.label(egui::RichText::new(&self.status).small().color(ctx.style().visuals.hyperlink_color));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("空格抓手 · R 标尺 · 双击 Shot 隔离 · Esc 取消 · Delete 删除")
                                .small()
                                .weak(),
                        );
                    });
                });
            });

        // —— 左侧导航:布局编辑器 / Bit Map,折叠/展开,主题切换 ——
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(if self.nav_collapsed { 48.0 } else { 160.0 })
            .show(ctx, |ui| {
                ui.add_space(10.0);
                // 品牌
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("▦").size(18.0).color(egui::Color32::from_rgb(0x3b, 0x82, 0xf6)));
                    if !self.nav_collapsed {
                        ui.label(egui::RichText::new("Shot Layout").size(13.0).strong());
                    }
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                // 菜单项
                let items = [
                    (AppView::Editor, "▦", "布局编辑器"),
                    (AppView::Bitmap, "▩", "Bit Map"),
                ];
                for (v, icon, label) in items {
                    let active = self.view == v;
                    let btn = egui::Button::new(egui::RichText::new(if self.nav_collapsed { icon.to_string() } else { format!("{}  {}", icon, label) }).size(13.0))
                        .frame(true)
                        .fill(if active { ui.visuals().widgets.inactive.weak_bg_fill } else { egui::Color32::TRANSPARENT })
                        .min_size(egui::vec2(ui.available_width(), 30.0));
                    let r = ui.add(btn);
                    if r.clicked() {
                        self.view = v;
                    }
                    if self.nav_collapsed {
                        r.on_hover_text(label);
                    }
                }
                // 底部:主题 + 折叠
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(if self.theme_dark { "☀" } else { "☾" }).clicked() {
                            self.theme_dark = !self.theme_dark;
                            crate::app::apply_theme(ctx, self.theme_dark);
                            self.set_status(if self.theme_dark { "已切换深色主题" } else { "已切换浅色主题" });
                        }
                        if ui.button(if self.nav_collapsed { "»" } else { "«" }).clicked() {
                            self.nav_collapsed = !self.nav_collapsed;
                        }
                        if !self.nav_collapsed {
                            ui.label(egui::RichText::new("light / dark").size(10.0).weak());
                        }
                    });
                });
            });

        if self.view == AppView::Bitmap {
            if self.bitmap.is_none() {
                self.bitmap = Some(bitmap::BitmapView::new())
            }
            self.bitmap.as_mut().unwrap().ui(ctx);
            return;
        }

        // —— 底部状态栏(KLayout/Virtuoso 式):指针世界坐标 / 缩放 / 选中对象 / 工具 ——
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::NONE
                    .fill(ctx.style().visuals.faint_bg_color)
                    .stroke(egui::Stroke::new(1.0_f32, ctx.style().visuals.window_stroke.color))
                    .inner_margin(egui::Margin::symmetric(12, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let weak = ui.visuals().weak_text_color();
                    let accent = ctx.style().visuals.hyperlink_color;
                    // 指针世界坐标(μm,3 位小数)
                    match self.pointer_world {
                        Some((wx, wy)) => {
                            ui.label(egui::RichText::new(format!("X {}", crate::geometry::fmt_num(wx))).size(11.0).color(weak));
                            ui.label(egui::RichText::new(format!("Y {}", crate::geometry::fmt_num(wy))).size(11.0).color(weak));
                        }
                        None => {
                            ui.label(egui::RichText::new("X —").size(11.0).color(weak));
                            ui.label(egui::RichText::new("Y —").size(11.0).color(weak));
                        }
                    }
                    ui.separator();
                    ui.label(egui::RichText::new(format!("{}%", (self.zoom * 100.0).round())).size(11.0).color(weak));
                    ui.separator();
                    // 选中对象
                    let sel = self.describe_selection();
                    ui.label(egui::RichText::new(&sel).size(11.0).color(if self.selected == Selection::None { weak } else { accent }));
                    // 右侧:元素统计 + 当前工具
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let tool_txt = match self.tool {
                            ToolMode::Ruler => "标尺模式(R/Esc 退出)",
                            ToolMode::Select => "选择模式",
                        };
                        ui.label(egui::RichText::new(tool_txt).size(11.0).color(if self.tool == ToolMode::Ruler { accent } else { weak }));
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!("Shot ×{} · Die ×{}", self.shots.len(), self.dies.len()))
                                .size(11.0)
                                .color(weak),
                        );
                    });
                });
            });

        egui::SidePanel::left("left_panel")
            .resizable(false)
            .exact_width(240.0)
            .frame(
                egui::Frame::NONE
                    .fill(ctx.style().visuals.window_fill)
                    .stroke(egui::Stroke::new(1.0_f32, ctx.style().visuals.window_stroke.color))
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    panels::left_panel(ui, self);
                });
            });

        egui::SidePanel::right("right_panel")
            .resizable(false)
            .exact_width(240.0)
            .frame(
                egui::Frame::NONE
                    .fill(ctx.style().visuals.window_fill)
                    .stroke(egui::Stroke::new(1.0_f32, ctx.style().visuals.window_stroke.color))
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| {
                panels::right_panel(ui, self);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(ctx.style().visuals.window_fill))
            .show(ctx, |ui| {
                canvas::canvas(ui, self);
            });

        // 拖拽 ghost(CAD 浮签):在窗口级图层绘制,不依赖悬停区域
        if let Some(d) = &self.drag {
            if d.started {
                let layer = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drag_ghost")));
                let p = d.screen;
                let (label, size, color) = match d.card {
                    model::DragCard::ShotCard => ("Shot".to_string(), egui::vec2(44.0, 44.0), egui::Color32::from_rgb(0x38, 0xbd, 0xf8)),
                    model::DragCard::Preset(i) => {
                        let p = self.presets.get(i);
                        (
                            p.map(|p| p.name.clone()).unwrap_or_default(),
                            egui::vec2(60.0, 40.0),
                            p.map(|p| p.color).unwrap_or(egui::Color32::GRAY),
                        )
                    }
                };
                let rect = egui::Rect::from_min_size(p + egui::vec2(12.0, 12.0), size);
                layer.rect_filled(rect, 3, color.linear_multiply(0.75));
                layer.text(rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(11.0), egui::Color32::WHITE);
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
