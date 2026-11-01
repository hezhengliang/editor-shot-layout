// 几何推导:Shot 外形(W×H)与网格几何全部固定 —— 单元格均匀分布:
//   cellW = (W - 2·padX - (cols-1)·gapX) / cols   (街宽 gapX 固定,网格不随 die 变化)
//   cellH = (H - 2·padY - (rows-1)·gapY) / rows
// die 在单元格内水平垂直居中,允许略超出格子吃街宽(以实际矩形不重叠为校验);
// 相邻 die 的标注距离 = 实测矩形边缘距离,随 die 尺寸自动变化。
// 效果:网格稳定;调整 Die 大小 → 标注自动重算

use crate::model::{Die, DieId, Shot, WRect, CELL};

fn die_at<'a>(shot: &Shot, dies: &'a [Die], i: usize, j: usize) -> Option<&'a Die> {
    shot.dies
        .iter()
        .filter_map(|&id| dies.iter().find(|d| d.id == id))
        .find(|d| d.slot == (i, j))
}

/// 单元格宽(固定网格,与 die 无关)
pub fn cell_w(shot: &Shot) -> f64 {
    let n = shot.cols.max(1) as f64;
    ((shot.w - shot.pad_x * 2.0 - (shot.cols as f64 - 1.0) * shot.gap_x) / n).max(0.0)
}

/// 单元格高
pub fn cell_h(shot: &Shot) -> f64 {
    let n = shot.rows.max(1) as f64;
    ((shot.h - shot.pad_y * 2.0 - (shot.rows as f64 - 1.0) * shot.gap_y) / n).max(0.0)
}

/// 槽位宽 = 单元格宽(网格固定,die 居中其中,可略超出吃街宽)
pub fn slot_width(shot: &Shot, _dies: &[Die], _i: usize, _j: usize) -> f64 {
    cell_w(shot)
}

pub fn slot_height(shot: &Shot, _dies: &[Die], _i: usize, _j: usize) -> f64 {
    cell_h(shot)
}

pub fn shot_w(shot: &Shot, _dies: &[Die]) -> f64 {
    shot.w
}

pub fn shot_h(shot: &Shot, _dies: &[Die]) -> f64 {
    shot.h
}

/// 行 j 的水平街宽 = gapX(固定;die 间标注距离由实际矩形测量,见 measured_gap_*)
pub fn row_gap(shot: &Shot, _dies: &[Die], _j: usize) -> f64 {
    shot.gap_x.max(0.0)
}

/// 列 i 的垂直街宽 = gapY(固定)
pub fn col_gap(shot: &Shot, _dies: &[Die], _i: usize) -> f64 {
    shot.gap_y.max(0.0)
}

/// 槽位 (i,j) 左下角原点(shot 局部)
pub fn slot_origin(shot: &Shot, dies: &[Die], i: usize, j: usize) -> (f64, f64) {
    let mut x = shot.pad_x;
    for k in 0..i {
        x += slot_width(shot, dies, k, j) + row_gap(shot, dies, j);
    }
    let mut y = shot.pad_y;
    for k in 0..j {
        y += slot_height(shot, dies, i, k) + col_gap(shot, dies, i);
    }
    (x, y)
}



/// 行 j 的槽位带高(该行最高槽位)
pub fn row_height(shot: &Shot, dies: &[Die], j: usize) -> f64 {
    (0..shot.cols)
        .map(|i| slot_height(shot, dies, i, j))
        .fold(CELL, f64::max)
}

pub fn col_width(shot: &Shot, dies: &[Die], i: usize) -> f64 {
    (0..shot.rows)
        .map(|j| slot_width(shot, dies, i, j))
        .fold(0.0f64, f64::max)
}

pub fn shot_rect(shot: &Shot, dies: &[Die]) -> WRect {
    WRect {
        x: shot.x,
        y: shot.y,
        w: shot_w(shot, dies),
        h: shot_h(shot, dies),
    }
}

/// die 在 shot 内的内容矩形(槽位内水平垂直居中)
pub fn die_rect(shot: &Shot, dies: &[Die], die: &Die) -> WRect {
    let (i, j) = die.slot;
    let (ox, oy) = slot_origin(shot, dies, i, j);
    let cw = slot_width(shot, dies, i, j);
    let ch = slot_height(shot, dies, i, j);
    WRect {
        x: shot.x + ox + (cw - die.w) / 2.0,
        y: shot.y + oy + (ch - die.h) / 2.0,
        w: die.w,
        h: die.h,
    }
}

/// die 矩形(局部坐标,不含 shot 位移)
pub fn die_rect_local(shot: &Shot, dies: &[Die], die: &Die) -> WRect {
    let r = die_rect(shot, dies, die);
    WRect {
        x: r.x - shot.x,
        y: r.y - shot.y,
        ..r
    }
}

pub fn nearest_slot(shot: &Shot, dies: &[Die], x: f64, y: f64) -> (usize, usize) {
    // 以槽位几何中心找最近(含空槽占位)
    let mut best = (0usize, 0usize);
    let mut best_d = f64::MAX;
    for j in 0..shot.rows {
        for i in 0..shot.cols {
            let (ox, oy) = slot_origin(shot, dies, i, j);
            let cw = slot_width(shot, dies, i, j);
            let ch = slot_height(shot, dies, i, j);
            let cx = ox + cw / 2.0;
            let cy = oy + ch / 2.0;
            let d = (cx - x).powi(2) + (cy - y).powi(2);
            if d < best_d {
                best_d = d;
                best = (i, j);
            }
        }
    }
    best
}

/// 数字格式化:3 位小数,去尾零(不带单位)
pub fn fmt_num(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn fmt_um(v: f64) -> String {
    format!("{} μm", fmt_num(v))
}

/// 槽位占用检查(ignore 为正在拖动的 die)
pub fn slot_occupied(shot: &Shot, i: usize, j: usize, dies: &[Die], ignore: Option<DieId>) -> bool {
    shot.dies.iter().any(|&id| {
        if Some(id) == ignore {
            return false;
        }
        dies.iter().any(|d| d.id == id && d.slot == (i, j))
    })
}

/// 内容是否合法:单元格尺寸为正 + die 实际矩形互不重叠且在外圈内
pub fn content_fits(shot: &Shot, dies: &[Die]) -> bool {
    if cell_w(shot) < 1.0 || cell_h(shot) < 1.0 {
        return false;
    }
    let rects: Vec<WRect> = dies
        .iter()
        .filter(|d| d.shot == shot.id)
        .map(|d| die_rect(shot, dies, d))
        .collect();
    for r in &rects {
        if r.x < shot.x + shot.pad_x - 1e-6
            || r.x + r.w > shot.x + shot.w - shot.pad_x + 1e-6
            || r.y < shot.y + shot.pad_y - 1e-6
            || r.y + r.h > shot.y + shot.h - shot.pad_y + 1e-6
        {
            return false;
        }
    }
    for a in 0..rects.len() {
        for b in a + 1..rects.len() {
            if rects[a].overlaps(&rects[b]) {
                return false;
            }
        }
    }
    true
}

/// 实测相邻 die 边缘距离(标注值,随 die 尺寸自动变化):同行 (i,i+1) 最小值
pub fn measured_gap_x(shot: &Shot, dies: &[Die]) -> f64 {
    let mut m = f64::INFINITY;
    for j in 0..shot.rows {
        for i in 0..shot.cols.saturating_sub(1) {
            if let (Some(a), Some(b)) = (die_at(shot, dies, i, j), die_at(shot, dies, i + 1, j)) {
                let ra = die_rect(shot, dies, a);
                let rb = die_rect(shot, dies, b);
                m = m.min(rb.x - (ra.x + ra.w));
            }
        }
    }
    if m.is_finite() {
        m
    } else {
        shot.gap_x
    }
}

/// 实测同列 (j,j+1) 边缘距离最小值
pub fn measured_gap_y(shot: &Shot, dies: &[Die]) -> f64 {
    let mut m = f64::INFINITY;
    for i in 0..shot.cols {
        for j in 0..shot.rows.saturating_sub(1) {
            if let (Some(a), Some(b)) = (die_at(shot, dies, i, j), die_at(shot, dies, i, j + 1)) {
                let ra = die_rect(shot, dies, a);
                let rb = die_rect(shot, dies, b);
                m = m.min(rb.y - (ra.y + ra.h));
            }
        }
    }
    if m.is_finite() {
        m
    } else {
        shot.gap_y
    }
}

/// 信息展示用:实测最小间距
pub fn actual_gap_x(shot: &Shot, dies: &[Die]) -> f64 {
    measured_gap_x(shot, dies)
}

pub fn actual_gap_y(shot: &Shot, dies: &[Die]) -> f64 {
    measured_gap_y(shot, dies)
}
