//! Simplified geometry helpers for row/column flex and fixed-column grid layouts.
//!
//! These helpers intentionally stop short of full CSS. They are designed for
//! native Mata applications that want predictable rectangles for controls,
//! cards, and panels without hand-writing every coordinate.

use super::geometry::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Insets {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}
impl Insets {
    pub const fn all(value:u32)->Self { Self { left:value, top:value, right:value, bottom:value } }
    pub const fn symmetric(horizontal:u32, vertical:u32)->Self {
        Self { left:horizontal, right:horizontal, top:vertical, bottom:vertical }
    }
    pub const fn hv(horizontal:u32, vertical:u32)->Self { Self::symmetric(horizontal, vertical) }
    pub const fn new(left:u32, top:u32, right:u32, bottom:u32)->Self {
        Self { left, top, right, bottom }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection { Row, Column }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexItem {
    pub basis: Option<u32>,
    pub grow: f32,
    pub min: u32,
    pub max: Option<u32>,
}
impl FlexItem {
    pub const fn fixed(size:u32)->Self { Self { basis:Some(size), grow:0.0, min:size, max:Some(size) } }
    pub const fn fill() -> Self { Self { basis:None, grow:1.0, min:1, max:None } }
    pub const fn auto(min:u32) -> Self { Self { basis:None, grow:1.0, min, max:None } }
    pub const fn grow(mut self, value:f32)->Self { self.grow=value; self }
    pub const fn min(mut self, value:u32)->Self { self.min=value; self }
    pub const fn max(mut self, value:u32)->Self { self.max=Some(value); self }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexLayout {
    pub container: Rect,
    pub direction: FlexDirection,
    pub gap: u32,
    pub padding: Insets,
    pub justify: JustifyContent,
    pub align: AlignItems,
}
impl FlexLayout {
    pub const fn row(container:Rect)->Self {
        Self { container, direction:FlexDirection::Row, gap:0, padding:Insets::all(0), justify:JustifyContent::Start, align:AlignItems::Start }
    }
    pub const fn column(container:Rect)->Self {
        Self { container, direction:FlexDirection::Column, gap:0, padding:Insets::all(0), justify:JustifyContent::Start, align:AlignItems::Start }
    }
    pub const fn gap(mut self, value:u32)->Self { self.gap=value; self }
    pub const fn padding(mut self, value:Insets)->Self { self.padding=value; self }
    pub const fn justify(mut self, value:JustifyContent)->Self { self.justify=value; self }
    pub const fn align(mut self, value:AlignItems)->Self { self.align=value; self }

    pub fn layout(&self, items:&[FlexItem]) -> Vec<Rect> {
        if items.is_empty() { return Vec::new(); }
        let inner = inset_rect(self.container, self.padding);
        let horizontal = matches!(self.direction, FlexDirection::Row);
        let main_space = if horizontal { inner.width } else { inner.height };
        let cross_space = if horizontal { inner.height } else { inner.width };
        let gap_count = items.len().saturating_sub(1) as u32;
        let natural_gap_total = self.gap.saturating_mul(gap_count);
        let base_sum:u32 = items.iter().map(|item| {
            let mut size = item.basis.unwrap_or(item.min).max(item.min);
            if let Some(max) = item.max { size = size.min(max.max(item.min)); }
            size
        }).sum();
        let mut sizes:Vec<u32> = items.iter().map(|item| {
            let mut size = item.basis.unwrap_or(item.min).max(item.min);
            if let Some(max) = item.max { size = size.min(max.max(item.min)); }
            size
        }).collect();
        let mut free = main_space.saturating_sub(base_sum.saturating_add(natural_gap_total));
        let total_grow:f32 = items.iter().map(|item| item.grow.max(0.0)).sum();
        if free > 0 && total_grow > 0.0 {
            let mut distributed = 0u32;
            for (index, item) in items.iter().enumerate() {
                if item.grow <= 0.0 { continue; }
                let add = if index + 1 == items.len() {
                    free.saturating_sub(distributed)
                } else {
                    ((free as f32) * (item.grow / total_grow)).round() as u32
                };
                sizes[index] = sizes[index].saturating_add(add);
                if let Some(max) = item.max { sizes[index] = sizes[index].min(max); }
                distributed = distributed.saturating_add(add);
            }
            free = main_space.saturating_sub(sizes.iter().sum::<u32>().saturating_add(natural_gap_total));
        }
        let used_sizes:u32 = sizes.iter().sum();
        let base_used = used_sizes.saturating_add(natural_gap_total);
        let extra = main_space.saturating_sub(base_used);
        let (mut cursor, gap) = match self.justify {
            JustifyContent::Start => (0u32, self.gap),
            JustifyContent::Center => (extra / 2, self.gap),
            JustifyContent::End => (extra, self.gap),
            JustifyContent::SpaceBetween if items.len() > 1 => (0, self.gap.saturating_add(extra / gap_count.max(1))),
            JustifyContent::SpaceAround => {
                let slot = extra / items.len() as u32;
                (slot / 2, self.gap.saturating_add(slot))
            }
            JustifyContent::SpaceEvenly => {
                let slot = extra / (items.len() as u32 + 1);
                (slot, self.gap.saturating_add(slot))
            }
            _ => (0, self.gap),
        };
        let mut rects=Vec::with_capacity(items.len());
        for size in sizes {
            let cross_size = match self.align {
                AlignItems::Stretch => cross_space,
                _ => cross_space,
            };
            let cross_offset = match self.align {
                AlignItems::Start | AlignItems::Stretch => 0,
                AlignItems::Center => cross_space.saturating_sub(cross_size) / 2,
                AlignItems::End => cross_space.saturating_sub(cross_size),
            };
            let rect = if horizontal {
                Rect::new(inner.x.saturating_add(cursor as i32), inner.y.saturating_add(cross_offset as i32), size.max(1), cross_size.max(1))
            } else {
                Rect::new(inner.x.saturating_add(cross_offset as i32), inner.y.saturating_add(cursor as i32), cross_size.max(1), size.max(1))
            };
            rects.push(rect);
            cursor = cursor.saturating_add(size).saturating_add(gap);
        }
        rects
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridLayout {
    pub container: Rect,
    pub columns: usize,
    pub rows: usize,
    pub gap_x: u32,
    pub gap_y: u32,
    pub padding: Insets,
}
impl GridLayout {
    pub const fn new(container:Rect, columns:usize, rows:usize)->Self {
        Self { container, columns, rows, gap_x:0, gap_y:0, padding:Insets::all(0) }
    }
    pub const fn columns(container:Rect, columns:usize)->Self {
        Self::new(container, columns, 1)
    }
    pub const fn gap(mut self, x:u32, y:u32)->Self { self.gap_x=x; self.gap_y=y; self }
    pub const fn padding(mut self, value:Insets)->Self { self.padding=value; self }
    pub const fn rows(mut self, value:usize)->Self { self.rows=value; self }
    pub fn auto_rows(mut self, count:usize)->Self {
        self.rows = ((count.max(1) + self.columns.max(1) - 1) / self.columns.max(1)).max(1);
        self
    }
    pub fn cell(&self, column:usize, row:usize) -> Rect {
        self.span(column, row, 1, 1)
    }
    pub fn span(&self, column:usize, row:usize, col_span:usize, row_span:usize) -> Rect {
        let inner = inset_rect(self.container, self.padding);
        let cols = self.columns.max(1);
        let rows = self.rows.max(1);
        let total_gap_x = self.gap_x.saturating_mul(cols.saturating_sub(1) as u32);
        let total_gap_y = self.gap_y.saturating_mul(rows.saturating_sub(1) as u32);
        let cell_w = inner.width.saturating_sub(total_gap_x) / cols as u32;
        let cell_h = inner.height.saturating_sub(total_gap_y) / rows as u32;
        let x = inner.x.saturating_add((column as u32).saturating_mul(cell_w.saturating_add(self.gap_x)) as i32);
        let y = inner.y.saturating_add((row as u32).saturating_mul(cell_h.saturating_add(self.gap_y)) as i32);
        let w = cell_w.saturating_mul(col_span.max(1) as u32)
            .saturating_add(self.gap_x.saturating_mul(col_span.saturating_sub(1) as u32));
        let h = cell_h.saturating_mul(row_span.max(1) as u32)
            .saturating_add(self.gap_y.saturating_mul(row_span.saturating_sub(1) as u32));
        Rect::new(x, y, w.max(1), h.max(1))
    }
    pub fn cells(&self, count:usize) -> Vec<Rect> {
        (0..count).map(|index| self.cell(index % self.columns.max(1), index / self.columns.max(1))).collect()
    }
}

pub fn inset_rect(rect:Rect, padding:Insets) -> Rect {
    let x = rect.x.saturating_add(padding.left as i32);
    let y = rect.y.saturating_add(padding.top as i32);
    let width = rect.width.saturating_sub(padding.left.saturating_add(padding.right)).max(1);
    let height = rect.height.saturating_sub(padding.top.saturating_add(padding.bottom)).max(1);
    Rect::new(x, y, width, height)
}
