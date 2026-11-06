// 数据模型:与 web 版 (src/editor/types.ts) 语义对齐,单位 μm

#[derive(Clone, Debug)]
pub struct Preset {
    /// 模板标识(保留给持久化/去重用)
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    pub w: f64,
    pub h: f64,
    pub color: egui::Color32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DieId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotId(pub u64);

#[derive(Clone, Debug)]
pub struct Die {
    pub id: DieId,
    pub name: String,
    pub w: f64,
    pub h: f64,
    pub color: egui::Color32,
    pub shot: ShotId,
    pub slot: (usize, usize), // (i 列, j 行)
}

#[derive(Clone, Debug)]
pub struct Shot {
    pub id: ShotId,
    pub x: f64,
    pub y: f64,
    pub w: f64, // 固定外形尺寸(μm):内圈间距由内容自动推导
    pub h: f64,
    pub cols: usize,
    pub rows: usize,
    pub gap_x: f64, // 内圈 scribe 最小间距(实际间距 ≥ 该值,自动计算)
    pub gap_y: f64,
    pub pad_x: f64, // 外圈 scribe
    pub pad_y: f64,
    pub show_dims: bool,
    pub show_outer: bool,
    pub dies: Vec<DieId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Selection {
    None,
    Die(DieId),
    Shot(ShotId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RightTab {
    Info,
    Config,
}

/// 画布工具:选择 / 测量标尺(KLayout 式拖拽建标尺)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolMode {
    Select,
    Ruler,
}

/// 测量标尺(世界坐标 μm)
#[derive(Clone, Copy, Debug)]
pub struct MeasureRuler {
    pub a: (f64, f64),
    pub b: (f64, f64),
}

impl MeasureRuler {
    pub fn len(&self) -> f64 {
        let dx = self.b.0 - self.a.0;
        let dy = self.b.1 - self.a.1;
        (dx * dx + dy * dy).sqrt()
    }
}

/// 拖拽对齐参考线(Figma 智能参考线):axis=true 为竖线(x=at),false 为横线(y=at)
#[derive(Clone, Copy, Debug)]
pub struct Guide {
    pub axis: bool,
    pub at: f64,
    pub from: f64,
    pub to: f64,
}

/// 侧栏拖放来源
#[derive(Clone, Copy, Debug)]
pub enum DragCard {
    ShotCard,
    Preset(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct DragState {
    pub card: DragCard,
    pub screen: egui::Pos2, // 当前指针位置(屏幕)
    pub started: bool,
}

/// 画布内的拖拽手势
#[derive(Clone, Copy, Debug)]
pub enum CanvasDrag {
    Pan { start: egui::Pos2, start_offset: egui::Vec2 },
    MoveShot { id: ShotId, start: (f64, f64) },
    MoveDie { id: DieId },
    /// 手柄 resize:记录起始指针世界坐标与起始尺寸,拖动按角方向增减
    ResizeDie { id: DieId, corner: ResizeCorner, start_wx: f64, start_wy: f64, start_w: f64, start_h: f64 },
    /// 测量标尺:起点世界坐标,终点跟随指针
    Ruler { start: (f64, f64) },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResizeCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeCorner {
    pub fn sign_w(&self) -> f64 {
        matches!(self, ResizeCorner::TopRight | ResizeCorner::BottomRight) as i8 as f64 * 2.0 - 1.0
    }
    pub fn sign_h(&self) -> f64 {
        matches!(self, ResizeCorner::BottomLeft | ResizeCorner::BottomRight) as i8 as f64 * 2.0 - 1.0
    }
    pub fn cursor(&self) -> egui::CursorIcon {
        match self {
            ResizeCorner::TopLeft | ResizeCorner::BottomRight => egui::CursorIcon::ResizeNwSe,
            _ => egui::CursorIcon::ResizeNeSw,
        }
    }
    #[allow(dead_code)]
    pub fn all() -> [ResizeCorner; 4] {
        [ResizeCorner::TopLeft, ResizeCorner::TopRight, ResizeCorner::BottomLeft, ResizeCorner::BottomRight]
    }
}

/// 世界坐标矩形(μm)
#[derive(Clone, Copy, Debug)]
pub struct WRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl WRect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
    pub fn overlaps(&self, o: &WRect) -> bool {
        self.x < o.x + o.w && self.x + self.w > o.x && self.y < o.y + o.h && self.y + self.h > o.y
    }
    #[allow(dead_code)]
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

pub const CELL: f64 = 500.0; // 默认 Die 尺寸(槽位基准)
