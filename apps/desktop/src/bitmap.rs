// BitMap 视图(egui 移植):纹理上传式视口 shading、层级/WL-BL、分析面板
// 数据:优先本地 FastAPI 服务 /bits,失败回退内置场调制生成(与服务端同公式)
use egui::{
    Align2, Color32, ColorImage, ComboBox, DragValue, Pos2, Rect, RichText, Sense, Stroke, TextureHandle,
    TextureOptions, Ui, Vec2,
};

pub const ROWS: usize = 2048;
pub const COLS: usize = 2048;
const RULER: f32 = 26.0;
const BIT: f64 = 1.0; // 1μm/bit

#[derive(Clone, Copy, PartialEq)]
pub enum Shading {
    Bin,
    Yield,
    Defect,
}

struct HierLv {
    name: &'static str,
    short: &'static str,
    span: usize,
    color: Color32,
    lw: f32,
}

const HIER: [HierLv; 3] = [
    HierLv { name: "Bank", short: "B", span: 1024, color: Color32::from_rgb(0x38, 0xbd, 0xf8), lw: 2.5 },
    HierLv { name: "Array", short: "A", span: 512, color: Color32::from_rgb(0x60, 0xa5, 0xfa), lw: 1.8 },
    HierLv { name: "Block", short: "K", span: 256, color: Color32::from_rgb(0x3b, 0x82, 0xf6), lw: 1.2 },
];

const BIN_COLORS: [Color32; 5] = [
    Color32::from_rgb(0x22, 0xc5, 0x5e),
    Color32::from_rgb(0xef, 0x44, 0x44),
    Color32::from_rgb(0xea, 0xb3, 0x08),
    Color32::from_rgb(0x3b, 0x82, 0xf6),
    Color32::from_rgb(0x1e, 0x29, 0x3b),
];
const BIN_LABELS: [&str; 5] = ["PASS", "FAIL", "WARN", "SKIP", "NULL"];

fn is_fail(b: u8) -> bool {
    b == 1 || b == 2
}

/// 与服务端同公式的场调制数据(seed 固定,FAIL 沿低频场聚集)
fn gen_data() -> Vec<u8> {
    let mut state: u64 = 0x9e3779b97f4a7c15;
    let mut rng = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state as f32 / u64::MAX as f32
    };
    let mut data = vec![0u8; ROWS * COLS];
    for y in 0..ROWS {
        for x in 0..COLS {
            let f = (x as f64 / 170.0).sin() * (y as f64 / 220.0).sin() + 0.6 * ((x + y) as f64 / 97.0).sin();
            let field = ((f + 1.6) / 3.2).powi(2);
            let r = rng();
            let fail_p = 0.06 + 0.6 * field as f32;
            let i = y * COLS + x;
            if r < fail_p {
                data[i] = 1
            } else if r < fail_p + 0.05 {
                data[i] = 2
            } else if r >= 0.93 {
                data[i] = 3
            } else if r >= 0.89 {
                data[i] = 4
            }
        }
    }
    data
}

fn fetch_remote_bits() -> Option<Vec<u8>> {
    let resp = ureq::get("http://localhost:8000/bits").timeout(std::time::Duration::from_secs(3)).call();
    match resp {
        Ok(r) => {
            let mut buf = Vec::new();
            if r.into_reader().read_to_end(&mut buf).is_ok() && buf.len() == ROWS * COLS {
                return Some(buf);
            }
            None
        }
        Err(_) => None,
    }
}

enum TbAction {
    ZoomOut,
    ZoomIn,
    Fit,
}

pub struct BitmapView {
    pub data: Vec<u8>,
    pub source: &'static str,
    mips: Vec<Vec<u8>>,
    fail_mips: Vec<Vec<u32>>,
    warn_mips: Vec<Vec<u32>>,
    // 分析统计(数据加载时单次全扫)
    bin_counts: [u32; 5],
    row_fail: Vec<u32>,
    col_fail: Vec<u32>,
    row_fail_rows: usize,
    col_fail_cols: usize,
    clustered: u32,
    isolated: u32,
    // 视图状态
    pub shading: Shading,
    pub soft_contrast: bool,
    pub bin_filter: [bool; 5],
    pub hier_vis: [bool; 3],
    pub show_wlbl: bool,
    pub zoom: f32,
    pub offset: Vec2,
    texture: Option<TextureHandle>,
    tex_key: (i32, usize, usize, usize, usize),
    hover: Option<(usize, usize, u8)>,
    goto_row: i64,
    goto_col: i64,
    pending_fit: bool,
    pending_goto: Option<(usize, usize)>,
}

impl BitmapView {
    pub fn new() -> Self {
        let (data, source) = match fetch_remote_bits() {
            Some(d) => (d, "fastapi"),
            None => (gen_data(), "embedded"),
        };
        let mut v = BitmapView {
            data,
            source,
            mips: vec![],
            fail_mips: vec![],
            warn_mips: vec![],
            bin_counts: [0; 5],
            row_fail: vec![],
            col_fail: vec![],
            row_fail_rows: 0,
            col_fail_cols: 0,
            clustered: 0,
            isolated: 0,
            shading: Shading::Yield,
            soft_contrast: true,
            bin_filter: [false, true, false, false, true],
            hier_vis: [true, true, true],
            show_wlbl: false,
            zoom: 0.4,
            offset: Vec2::new(RULER + 40.0, RULER + 40.0),
            texture: None,
            tex_key: (-1, 0, 0, 0, 0),
            hover: None,
            goto_row: 0,
            goto_col: 0,
            pending_fit: true,
            pending_goto: None,
        };
        v.rebuild();
        v
    }

    fn rebuild(&mut self) {
        self.mips.clear();
        self.fail_mips.clear();
        self.warn_mips.clear();
        self.mips.push(self.data.clone());
        self.fail_mips.push(self.data.iter().map(|&b| (b == 1) as u32).collect());
        self.warn_mips.push(self.data.iter().map(|&b| (b == 2) as u32).collect());
        while self.mips.last().unwrap().len() > 1 {
            let cur = self.mips.last().unwrap();
            let fc = self.fail_mips.last().unwrap();
            let wc = self.warn_mips.last().unwrap();
            let n = ((cur.len() as f64).sqrt() as usize) / 2;
            let mut next = vec![0u8; n * n];
            let mut fnext = vec![0u32; n * n];
            let mut wnext = vec![0u32; n * n];
            for y in 0..n {
                for x in 0..n {
                    let mut counts = [0u8; 5];
                    for dy in 0..2 {
                        for dx in 0..2 {
                            counts[cur[(y * 2 + dy) * n * 2 + x * 2 + dx] as usize] += 1;
                        }
                    }
                    let mut best = 0usize;
                    for b in 1..5 {
                        if counts[b] > counts[best] {
                            best = b;
                        }
                    }
                    next[y * n + x] = best as u8;
                    fnext[y * n + x] = fc[(y * 2) * n * 2 + x * 2]
                        + fc[(y * 2) * n * 2 + x * 2 + 1]
                        + fc[(y * 2 + 1) * n * 2 + x * 2]
                        + fc[(y * 2 + 1) * n * 2 + x * 2 + 1];
                    wnext[y * n + x] = wc[(y * 2) * n * 2 + x * 2]
                        + wc[(y * 2) * n * 2 + x * 2 + 1]
                        + wc[(y * 2 + 1) * n * 2 + x * 2]
                        + wc[(y * 2 + 1) * n * 2 + x * 2 + 1];
                }
            }
            self.mips.push(next);
            self.fail_mips.push(fnext);
            self.warn_mips.push(wnext);
        }
        self.bin_counts = [0; 5];
        self.row_fail = vec![0; ROWS];
        self.col_fail = vec![0; COLS];
        for r in 0..ROWS {
            let base = r * COLS;
            for c in 0..COLS {
                let b = self.data[base + c];
                self.bin_counts[b as usize] += 1;
                if is_fail(b) {
                    self.row_fail[r] += 1;
                    self.col_fail[c] += 1;
                }
            }
        }
        self.row_fail_rows = self.row_fail.iter().filter(|&&f| f as f64 >= COLS as f64 * 0.7).count();
        self.col_fail_cols = self.col_fail.iter().filter(|&&f| f as f64 >= ROWS as f64 * 0.7).count();
        self.clustered = 0;
        self.isolated = 0;
        for r in 0..ROWS {
            for c in 0..COLS {
                if !is_fail(self.data[r * COLS + c]) {
                    continue;
                }
                let adj = (c + 1 < COLS && is_fail(self.data[r * COLS + c + 1]))
                    || (r + 1 < ROWS && is_fail(self.data[(r + 1) * COLS + c]))
                    || (c > 0 && is_fail(self.data[r * COLS + c - 1]))
                    || (r > 0 && is_fail(self.data[(r - 1) * COLS + c]));
                if adj {
                    self.clustered += 1;
                } else {
                    self.isolated += 1;
                }
            }
        }
        self.texture = None;
        self.tex_key.0 = -1;
    }

    fn bin_rgb(&self, b: u8) -> [u8; 3] {
        let c = BIN_COLORS[b as usize];
        if self.bin_filter[b as usize] {
            [c.r(), c.g(), c.b()]
        } else {
            [
                (c.r() as f32 * 0.12 + 0x0b as f32 * 0.88) as u8,
                (c.g() as f32 * 0.12 + 0x11 as f32 * 0.88) as u8,
                (c.b() as f32 * 0.12 + 0x20 as f32 * 0.88) as u8,
            ]
        }
    }

    fn scale_r(&self, r: f32) -> f32 {
        if self.soft_contrast {
            r.max(0.0).min(1.0).sqrt()
        } else {
            r.max(0.0).min(1.0)
        }
    }

    fn fit(&mut self, canvas: Rect) {
        let s = ((canvas.width() - 80.0) / (COLS as f32)).min((canvas.height() - 80.0) / (ROWS as f32));
        self.zoom = s.max(0.02).min(64.0);
        self.offset = Vec2::new(
            canvas.min.x + (canvas.width() - COLS as f32 * self.zoom) / 2.0,
            canvas.min.y + (canvas.height() - ROWS as f32 * self.zoom) / 2.0,
        );
    }

    fn focus_cell(&mut self, canvas: Rect, r: usize, c: usize, s: f32) {
        self.zoom = s.clamp(0.02, 64.0);
        self.offset = Vec2::new(
            canvas.center().x - (c as f32 + 0.5) * self.zoom,
            canvas.center().y - (r as f32 + 0.5) * self.zoom,
        );
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("bm_sidebar")
            .resizable(false)
            .exact_width(240.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    self.panel(ui);
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(0x02, 0x06, 0x17)))
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                self.canvas(ctx, ui, rect, &resp);
            });
    }

    fn canvas(&mut self, ctx: &egui::Context, ui: &mut Ui, rect: Rect, resp: &egui::Response) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgb(0x02, 0x06, 0x17));

        let ptr = resp.hover_pos().or(resp.interact_pointer_pos());
        let in_canvas = ptr.map(|p| rect.contains(p)).unwrap_or(false);

        // 悬停(先算,拖拽判定不依赖它)
        self.hover = None;
        if let Some(p) = ptr {
            if in_canvas {
                let (wx, wy) = self.s2w(p);
                let c = (wx / BIT) as i64;
                let r = (wy / BIT) as i64;
                if r >= 0 && r < ROWS as i64 && c >= 0 && c < COLS as i64 {
                    self.hover = Some((r as usize, c as usize, self.data[r as usize * COLS + c as usize]));
                }
            }
        }

        // 输入:滚轮/双指捏合缩放(指针锚点)/ 拖拽平移 / 双击下钻 / 工具栏
        let (scroll, pinch) = ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        if in_canvas && (pinch - 1.0).abs() > 1e-3 {
            // 触摸板双指捏合(macOS 原生,与编辑器一致)
            let p = ptr.unwrap();
            let (wx, wy) = self.s2w(p);
            let ns = (self.zoom * pinch).clamp(0.02, 64.0);
            self.offset = Vec2::new(p.x - wx as f32 * ns, p.y - wy as f32 * ns);
            self.zoom = ns;
        }
        if in_canvas && scroll.y != 0.0 {
            let p = ptr.unwrap();
            let (wx, wy) = self.s2w(p);
            let ns = (self.zoom * (scroll.y * -0.002).exp() as f32).clamp(0.02, 64.0);
            self.offset = Vec2::new(p.x - wx as f32 * ns, p.y - wy as f32 * ns);
            self.zoom = ns;
        }
        if resp.dragged_by(egui::PointerButton::Primary) {
            self.offset += resp.drag_delta();
        }
        if resp.double_clicked() {
            if let Some(p) = resp.hover_pos() {
                let (wx, wy) = self.s2w(p);
                self.drill(rect, wx, wy);
            }
        }
        if resp.clicked() {
            if let Some(a) = self.toolbar_action(rect, ptr, resp) {
                match a {
                    TbAction::ZoomOut => self.zoom_at(rect, 1.0 / 1.25),
                    TbAction::ZoomIn => self.zoom_at(rect, 1.25),
                    TbAction::Fit => self.fit(rect),
                }
            }
        }
        // 视图钳制(与 web/编辑器同语义):世界原点钉在标尺内角 = 画布最左/最上 + RULER。
        // 内容从角落排布,右侧/下侧留白;任何位置不会出现负刻度。
        let max_x = rect.min.x + RULER;
        let max_y = rect.min.y + RULER;
        if self.offset.x > max_x {
            self.offset.x = max_x;
        }
        if self.offset.y > max_y {
            self.offset.y = max_y;
        }
        if self.pending_fit {
            self.pending_fit = false;
            self.fit(rect);
        }
        if let Some((r, c)) = self.pending_goto.take() {
            self.focus_cell(rect, r.min(ROWS - 1), c.min(COLS - 1), 20.0);
        }

        self.paint_bitmap(ctx, &painter, rect);
        self.paint_hierarchy(&painter, rect);

        if self.show_wlbl {
            if let Some((r, c, _)) = self.hover {
                let band_h = self.zoom.max(2.0);
                let band_w = self.zoom.max(2.0);
                let row_rect = Rect::from_min_size(
                    Pos2::new(rect.left().max(self.w2s(0.0, 0.0).x), self.w2s(0.0, r as f64).y),
                    Vec2::new((COLS as f32) * self.zoom, band_h),
                );
                let col_rect = Rect::from_min_size(
                    Pos2::new(self.w2s(c as f64, 0.0).x, rect.top().max(self.w2s(0.0, 0.0).y)),
                    Vec2::new(band_w, (ROWS as f32) * self.zoom),
                );
                painter.rect_filled(row_rect.intersect(rect), 0.0, Color32::from_rgba_unmultiplied(56, 189, 248, 70));
                painter.rect_filled(col_rect.intersect(rect), 0.0, Color32::from_rgba_unmultiplied(244, 114, 182, 70));
            }
        }

        self.paint_ruler(&painter, rect, ptr, in_canvas);
        self.paint_toolbar(&painter, rect, ptr);
        ctx.request_repaint_after(std::time::Duration::from_millis(90));
    }

    fn zoom_at(&mut self, rect: Rect, factor: f32) {
        let c = rect.center();
        let (wx, wy) = self.s2w(c);
        let ns = (self.zoom * factor).clamp(0.02, 64.0);
        self.offset = Vec2::new(c.x - wx as f32 * ns, c.y - wy as f32 * ns);
        self.zoom = ns;
    }

    fn paint_bitmap(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: Rect) {
        let sc = self.zoom;
        let mut k = 0usize;
        while k + 1 < self.mips.len() && sc * (1u32 << (k + 1)) as f32 <= 48.0 {
            k += 1;
        }
        let agg = 1usize << k;
        let dim = COLS >> k;
        let pitch = sc * agg as f32;
        let (ox, oy) = (self.offset.x, self.offset.y);
        let c0 = ((RULER - ox) / pitch).floor().max(0.0) as usize;
        let c1 = (((rect.right() - ox) / pitch).ceil() as usize).min(dim - 1);
        let r0 = ((RULER - oy) / pitch).floor().max(0.0) as usize;
        let r1 = (((rect.bottom() - oy) / pitch).ceil() as usize).min(dim - 1);
        if c1 < c0 || r1 < r0 {
            return;
        }
        let key = (k as i32, c0, c1, r0, r1);
        if self.tex_key != key {
            self.tex_key = key;
            let off_w = (c1 - c0 + 1).min(1024);
            let off_h = (r1 - r0 + 1).min(1024);
            let step_x = ((c1 - c0 + 1) as f32 / off_w as f32).ceil().max(1.0) as usize;
            let step_y = ((r1 - r0 + 1) as f32 / off_h as f32).ceil().max(1.0) as usize;
            let mut img = ColorImage::new([off_w, off_h], Color32::BLACK);
            let cell_total = (agg * agg) as f32;
            for y in 0..off_h {
                let sy = (r0 + y * step_y).min(dim - 1);
                for x in 0..off_w {
                    let sx = (c0 + x * step_x).min(dim - 1);
                    let i = sy * dim + sx;
                    let rgb: [u8; 3] = match self.shading {
                        Shading::Yield if k > 0 => {
                            let t = self.scale_r((self.fail_mips[k][i] + self.warn_mips[k][i]) as f32 / cell_total);
                            yield_rgb(t)
                        }
                        Shading::Defect => {
                            let a1 = self.scale_r(self.fail_mips[k][i] as f32 / cell_total);
                            let a2 = self.scale_r(self.warn_mips[k][i] as f32 / cell_total);
                            let mut r = 0x0bu8 as f32;
                            let mut g = 0x11u8 as f32;
                            let mut b = 0x20u8 as f32;
                            r += (0xef as f32 - r) * a1;
                            g += (0x44 as f32 - g) * a1;
                            b += (0x44 as f32 - b) * a1;
                            g += (0xea as f32 - g) * a2;
                            r += (0xb3 as f32 - r) * a2 * 0.6;
                            b += (0x08 as f32 - b) * a2;
                            [r as u8, g as u8, b as u8]
                        }
                        _ => self.bin_rgb(self.mips[k][i]),
                    };
                    img[(x, y)] = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                }
            }
            let opts = if self.shading == Shading::Bin && k == 0 {
                TextureOptions::NEAREST
            } else {
                TextureOptions::LINEAR
            };
            match &mut self.texture {
                Some(t) => t.set(img, opts),
                None => self.texture = Some(ctx.load_texture("bitmap", img, opts)),
            }
        }
        let tex = self.texture.as_ref().unwrap();
        let dest_x0 = ox + c0 as f32 * pitch;
        let dest_y0 = oy + r0 as f32 * pitch;
        let dest_w = (c1 - c0 + 1) as f32 * pitch;
        let dest_h = (r1 - r0 + 1) as f32 * pitch;
        let clipped = Rect::from_min_size(Pos2::new(dest_x0, dest_y0), Vec2::new(dest_w, dest_h)).intersect(rect);
        if clipped.width() <= 0.0 || clipped.height() <= 0.0 {
            return;
        }
        let uv = Rect::from_min_max(
            Pos2::new(((clipped.min.x - dest_x0) / dest_w).clamp(0.0, 1.0), ((clipped.min.y - dest_y0) / dest_h).clamp(0.0, 1.0)),
            Pos2::new(((clipped.max.x - dest_x0) / dest_w).clamp(0.0, 1.0), ((clipped.max.y - dest_y0) / dest_h).clamp(0.0, 1.0)),
        );
        painter.image(tex.id(), clipped, uv, Color32::WHITE);
    }

    /** 层级边界:格 ≥100px 才显示该级(线宽/字号屏幕恒定) */
    fn paint_hierarchy(&self, painter: &egui::Painter, rect: Rect) {
        let sc = self.zoom.max(1e-6);
        for (hi, lv) in HIER.iter().enumerate() {
            if !self.hier_vis[hi] || (lv.span as f32 * sc) < 100.0 {
                continue;
            }
            let n = COLS / lv.span;
            let lw = (lv.lw / sc).max(1.0 / sc);
            let fs = 11.0 / sc;
            for j in 0..n {
                for i in 0..n {
                    let r = Rect::from_min_size(
                        self.w2s((i * lv.span) as f64, (j * lv.span) as f64),
                        Vec2::new(lv.span as f32 * sc, lv.span as f32 * sc),
                    );
                    let r = r.intersect(rect);
                    if r.width() <= 0.0 || r.height() <= 0.0 {
                        continue;
                    }
                    painter.rect_stroke(r, 0.0, Stroke::new(lw.min(6.0), lv.color), egui::StrokeKind::Inside);
                    // Array 级不画编号(格子太密,文字成噪音;Bank/Block 保留)
                    if lv.name != "Array" {
                        painter.text(
                            Pos2::new(r.min.x + 8.0 / sc, r.min.y + 5.0 / sc),
                            Align2::LEFT_TOP,
                            format!("{}{}", lv.short, j * n + i),
                            egui::FontId::proportional(fs),
                            lv.color,
                        );
                    }
                }
            }
        }
    }

    fn drill(&mut self, canvas: Rect, wx: f64, wy: f64) {
        if wx < 0.0 || wx >= COLS as f64 || wy < 0.0 || wy >= ROWS as f64 {
            self.fit(canvas);
            return;
        }
        for lv in HIER {
            if lv.span as f32 * self.zoom >= 500.0 {
                continue;
            }
            let i = (wx / lv.span as f64) as usize;
            let j = (wy / lv.span as f64) as usize;
            let s = (canvas.width().min(canvas.height()) * 0.7) / lv.span as f32;
            self.focus_cell(canvas, j * lv.span + lv.span / 2, i * lv.span + lv.span / 2, s);
            return;
        }
        self.fit(canvas);
    }

    fn paint_ruler(&self, painter: &egui::Painter, rect: Rect, ptr: Option<Pos2>, in_canvas: bool) {
        let pal = crate::canvas::palette(true);
        let w = rect.width();
        let h = rect.height();
        let sc = self.zoom;
        painter.rect_filled(Rect::from_min_size(rect.min, Vec2::new(w, RULER)), 0.0, pal.ruler_bg);
        painter.rect_filled(Rect::from_min_size(rect.min, Vec2::new(RULER, h)), 0.0, pal.ruler_bg);
        painter.line_segment([rect.min + Vec2::new(0.0, RULER), rect.min + Vec2::new(w, RULER)], Stroke::new(1.0_f32, pal.ruler_line));
        painter.line_segment([rect.min + Vec2::new(RULER, 0.0), rect.min + Vec2::new(RULER, h)], Stroke::new(1.0_f32, pal.ruler_line));

        let steps = [1.0f64, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 256.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0, 50000.0, 100000.0, 256000.0];
        let mut major = *steps.last().unwrap();
        for s in &steps {
            if *s * sc as f64 >= 70.0 {
                major = *s;
                break;
            }
        }
        let minor = (major / 5.0).max(1.0);
        let trim = |v: f64| format!("{:.3}", v).trim_end_matches('0').trim_end_matches('.').to_string();
        let fmt = |v: f64| {
            let a = v.abs();
            if a >= 1e6 {
                format!("{}M", trim(v / 1e6))
            } else if a >= 1e4 {
                format!("{}k", trim(v / 1e3))
            } else {
                trim(v)
            }
        };
        let is_major = |v: f64| ((v / major).round() - v / major).abs() < 1e-9;

        // 刻度起点按画布左缘的世界坐标算;um<0 不画(负值轴永不出现)
        let wx0 = (rect.min.x - self.offset.x) as f64 / sc as f64;
        let wx1 = (rect.right() - self.offset.x) as f64 / sc as f64;
        let mut um = (wx0.max(0.0) / minor).floor() * minor;
        while um <= wx1 {
            let sx = self.offset.x + um as f32 * sc;
            if um >= 0.0 && sx >= RULER {
                let m = is_major(um);
                let len = if m { 10.0 } else { 5.0 };
                painter.line_segment([Pos2::new(sx, rect.top() + RULER), Pos2::new(sx, rect.top() + RULER - len)], Stroke::new(1.0_f32, if m { pal.ruler_text } else { pal.ruler_minor }));
                if m {
                    painter.text(Pos2::new(sx, rect.top() + RULER - len - 2.0), Align2::CENTER_BOTTOM, fmt(um), egui::FontId::proportional(9.0), pal.ruler_text);
                }
            }
            um += minor;
        }
        let wy0 = (rect.min.y - self.offset.y) as f64 / sc as f64;
        let wy1 = (rect.bottom() - self.offset.y) as f64 / sc as f64;
        let mut um = (wy0.max(0.0) / minor).floor() * minor;
        while um <= wy1 {
            let sy = self.offset.y + um as f32 * sc;
            if um >= 0.0 && sy >= RULER && um != 0.0 {
                let m = is_major(um);
                let len = if m { 10.0 } else { 5.0 };
                painter.line_segment([Pos2::new(rect.left() + RULER, sy), Pos2::new(rect.left() + RULER - len, sy)], Stroke::new(1.0_f32, if m { pal.ruler_text } else { pal.ruler_minor }));
                if m {
                    let label: String = fmt(um).chars().map(|ch| ch.to_string()).collect::<Vec<_>>().join("\n");
                    painter.text(Pos2::new(rect.left() + RULER - len - 5.0, sy), Align2::RIGHT_CENTER, label, egui::FontId::proportional(9.0), pal.ruler_text);
                }
            }
            um += minor;
        }

        if let Some(p) = ptr {
            if in_canvas && p.x >= RULER && p.y >= RULER {
                painter.line_segment([Pos2::new(p.x, rect.top() + RULER), Pos2::new(p.x, rect.bottom())], Stroke::new(1.0_f32, pal.ruler_minor));
                painter.line_segment([Pos2::new(rect.left() + RULER, p.y), Pos2::new(rect.right(), p.y)], Stroke::new(1.0_f32, pal.ruler_minor));
                let (wx, wy) = self.s2w(p);
                let text = match self.hover {
                    Some((r, c, b)) => format!("row:{} col:{} bin:{} ({}) · WL={} BL={}", r, c, b, BIN_LABELS[b as usize], r, c),
                    None => format!("X: {:.3} μm  Y: {:.3} μm  (bit {} μm)", wx, wy, BIT as i32),
                };
                painter.text(Pos2::new(rect.right() - 8.0, rect.bottom() - 8.0), Align2::RIGHT_BOTTOM, text, egui::FontId::proportional(11.0), Color32::from_rgb(0x38, 0xbd, 0xf8));
            }
        }
    }

    fn paint_toolbar(&self, painter: &egui::Painter, rect: Rect, ptr: Option<Pos2>) {
        let bar = Rect::from_min_size(Pos2::new(rect.right() - 120.0, rect.bottom() - 50.0), Vec2::new(104.0, 34.0));
        painter.rect_filled(bar, 8.0, Color32::from_rgb(0x0f, 0x17, 0x2a));
        painter.rect_stroke(bar, 8.0, Stroke::new(1.0_f32, Color32::from_rgb(0x33, 0x41, 0x55)), egui::StrokeKind::Inside);
        painter.text(Pos2::new(bar.left() + 13.0, bar.center().y), Align2::CENTER_CENTER, "−", egui::FontId::proportional(14.0), Color32::from_rgb(0xcb, 0xd5, 0xe1));
        painter.text(Pos2::new(bar.left() + 39.0, bar.center().y), Align2::CENTER_CENTER, format!("{}%", (self.zoom * 100.0).round()), egui::FontId::proportional(11.0), Color32::from_rgb(0x94, 0xa3, 0xb8));
        painter.text(Pos2::new(bar.left() + 65.0, bar.center().y), Align2::CENTER_CENTER, "+", egui::FontId::proportional(14.0), Color32::from_rgb(0xcb, 0xd5, 0xe1));
        painter.line_segment([Pos2::new(bar.left() + 78.0, bar.top() + 5.0), Pos2::new(bar.left() + 78.0, bar.bottom() - 5.0)], Stroke::new(1.0_f32, Color32::from_rgb(0x33, 0x41, 0x55)));
        painter.text(Pos2::new(bar.left() + 91.0, bar.center().y), Align2::CENTER_CENTER, "⛶", egui::FontId::proportional(13.0), Color32::from_rgb(0xcb, 0xd5, 0xe1));
        let _ = ptr;
    }

    fn toolbar_action(&self, rect: Rect, ptr: Option<Pos2>, _resp: &egui::Response) -> Option<TbAction> {
        let bar = Rect::from_min_size(Pos2::new(rect.right() - 120.0, rect.bottom() - 50.0), Vec2::new(104.0, 34.0));
        let p = ptr?;
        if !bar.contains(p) {
            return None;
        }
        if p.x < bar.left() + 26.0 {
            Some(TbAction::ZoomOut)
        } else if p.x < bar.left() + 78.0 {
            Some(TbAction::ZoomIn)
        } else {
            Some(TbAction::Fit)
        }
    }

    fn w2s(&self, x: f64, y: f64) -> Pos2 {
        Pos2::new(self.offset.x + x as f32 * self.zoom, self.offset.y + y as f32 * self.zoom)
    }
    fn s2w(&self, p: Pos2) -> (f64, f64) {
        (
            (p.x - self.offset.x) as f64 / self.zoom as f64,
            (p.y - self.offset.y) as f64 / self.zoom as f64,
        )
    }

    fn panel(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.label(RichText::new(format!("Bit Map · {}", self.source)).size(13.0).strong());
        ui.separator();

        ui.label(RichText::new("Shading").size(11.0).color(ui.visuals().weak_text_color()));
        ComboBox::from_id_salt("bm_shading")
            .selected_text(match self.shading {
                Shading::Bin => "Bin 多数色",
                Shading::Yield => "Yield 良率渐变",
                Shading::Defect => "Defect 类型分离",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.shading, Shading::Bin, "Bin 多数色");
                ui.selectable_value(&mut self.shading, Shading::Yield, "Yield 良率渐变");
                ui.selectable_value(&mut self.shading, Shading::Defect, "Defect 类型分离");
            });
        ui.checkbox(&mut self.soft_contrast, RichText::new("柔和对比(√ 色阶)").size(12.0));

        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("Bin Filter").size(11.0).color(ui.visuals().weak_text_color()));
        ui.horizontal_wrapped(|ui| {
            for b in 0..5 {
                ui.checkbox(&mut self.bin_filter[b], RichText::new(format!("■ {}", BIN_LABELS[b])).size(11.0).color(BIN_COLORS[b]));
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("Bin Summary").size(11.0).color(ui.visuals().weak_text_color()));
        let total = (ROWS * COLS) as f32;
        for b in 0..5 {
            ui.horizontal(|ui| {
                ui.colored_label(BIN_COLORS[b], "■");
                ui.label(RichText::new(BIN_LABELS[b]).size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{:.1}%", self.bin_counts[b] as f32 / total * 100.0)).size(11.0).weak());
                    ui.label(RichText::new(format!("{}", self.bin_counts[b])).size(12.0));
                });
            });
        }

        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("Fail Patterns").size(11.0).color(ui.visuals().weak_text_color()));
        egui::Grid::new("bm_fp").num_columns(2).show(ui, |ui| {
            ui.label(RichText::new("Row Fail").size(11.0).weak());
            ui.label(RichText::new(format!("{} 行", self.row_fail_rows)).size(12.0));
            ui.end_row();
            ui.label(RichText::new("Col Fail").size(11.0).weak());
            ui.label(RichText::new(format!("{} 列", self.col_fail_cols)).size(12.0));
            ui.end_row();
            ui.label(RichText::new("Cluster").size(11.0).weak());
            ui.label(RichText::new(format!("{}", self.clustered)).size(12.0));
            ui.end_row();
            ui.label(RichText::new("Single Bit").size(11.0).weak());
            ui.label(RichText::new(format!("{}", self.isolated)).size(12.0));
            ui.end_row();
        });

        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("Worst WL / BL").size(11.0).color(ui.visuals().weak_text_color()));
        let mut wl_idx: Vec<usize> = (0..ROWS).collect();
        wl_idx.sort_by_key(|&i| std::cmp::Reverse(self.row_fail[i]));
        let mut bl_idx: Vec<usize> = (0..COLS).collect();
        bl_idx.sort_by_key(|&i| std::cmp::Reverse(self.col_fail[i]));
        let wl_max = self.row_fail[wl_idx[0]].max(1);
        let bl_max = self.col_fail[bl_idx[0]].max(1);
        ui.label(RichText::new("Wordline").size(10.0).weak());
        for &i in wl_idx.iter().take(6) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("WL{}", i)).size(11.0).weak());
                let w = (self.row_fail[i] as f32 / wl_max as f32 * 80.0).max(4.0);
                let (r, _) = ui.allocate_exact_size(Vec2::new(w, 8.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, Color32::from_rgb(0x38, 0xbd, 0xf8));
                ui.label(RichText::new(format!("{}", self.row_fail[i])).size(10.0).weak());
            });
        }
        ui.label(RichText::new("Bitline").size(10.0).weak());
        for &i in bl_idx.iter().take(6) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("BL{}", i)).size(11.0).weak());
                let w = (self.col_fail[i] as f32 / bl_max as f32 * 80.0).max(4.0);
                let (r, _) = ui.allocate_exact_size(Vec2::new(w, 8.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, Color32::from_rgb(0xf4, 0x72, 0xb6));
                ui.label(RichText::new(format!("{}", self.col_fail[i])).size(10.0).weak());
            });
        }

        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("Hierarchy").size(11.0).color(ui.visuals().weak_text_color()));
        ui.horizontal(|ui| {
            for (i, lv) in HIER.iter().enumerate() {
                ui.checkbox(&mut self.hier_vis[i], RichText::new(lv.name).size(11.0));
            }
        });
        ui.checkbox(&mut self.show_wlbl, RichText::new("WL/BL 十字").size(11.0));
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut self.goto_row).speed(1).prefix("WL"));
            ui.add(DragValue::new(&mut self.goto_col).speed(1).prefix("BL"));
            if ui.button(RichText::new("跳转").size(12.0)).clicked() {
                self.pending_goto = Some((self.goto_row.max(0) as usize, self.goto_col.max(0) as usize));
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.label(RichText::new("Inspection").size(11.0).color(ui.visuals().weak_text_color()));
        if let Some((r, c, b)) = self.hover {
            ui.label(RichText::new(format!("row={} col={} bin={} ({})", r, c, b, BIN_LABELS[b as usize])).size(12.0));
        } else {
            ui.label(RichText::new("--").size(12.0).weak());
        }
        ui.label(RichText::new("双击下钻 · 滚轮缩放 · 拖拽平移").size(10.0).weak());
    }
}

fn yield_rgb(t: f32) -> [u8; 3] {
    let stops: [(f32, [u8; 3]); 5] = [
        (0.0, [0x16, 0xa3, 0x4a]),
        (0.15, [0x84, 0xcc, 0x16]),
        (0.35, [0xea, 0xb3, 0x08]),
        (0.6, [0xf9, 0x73, 0x16]),
        (1.0, [0xdc, 0x26, 0x26]),
    ];
    for i in 1..stops.len() {
        if t <= stops[i].0 {
            let (t0, c0) = stops[i - 1];
            let (t1, c1) = stops[i];
            let f = (t - t0) / (t1 - t0);
            return [
                (c0[0] as f32 + (c1[0] as f32 - c0[0] as f32) * f) as u8,
                (c0[1] as f32 + (c1[1] as f32 - c0[1] as f32) * f) as u8,
                (c0[2] as f32 + (c1[2] as f32 - c0[2] as f32) * f) as u8,
            ];
        }
    }
    stops[4].1
}

impl Default for BitmapView {
    fn default() -> Self {
        Self::new()
    }
}
