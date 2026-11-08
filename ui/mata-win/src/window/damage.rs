//! Shared damage handling for CPU framebuffer presentation backends.
use std::collections::VecDeque;
use super::DirtyRect;

pub(super) fn full(size: (u32, u32)) -> DirtyRect {
    DirtyRect::new(0, 0, size.0, size.1)
}

/// Transform clipped source-pixel damage to target-pixel damage for nearest-neighbor sampling.
/// Source endpoints are rounded outward: no target pixel that samples a changed source pixel is lost.
pub(super) fn scaled(rect: DirtyRect, src: (u32, u32), dst: (u32, u32)) -> Option<DirtyRect> {
    let r = rect.clipped(src.0, src.1)?;
    if src == dst {return Some(r);}
    if src.0 == 0 || src.1 == 0 || dst.0 == 0 || dst.1 == 0 { return None; }
    let x0 = (r.x as u64 * dst.0 as u64 / src.0 as u64) as u32;
    let y0 = (r.y as u64 * dst.1 as u64 / src.1 as u64) as u32;
    let x1 = (((r.x + r.width) as u64 * dst.0 as u64 + src.0 as u64 - 1)
        / src.0 as u64).min(dst.0 as u64) as u32;
    let y1 = (((r.y + r.height) as u64 * dst.1 as u64 + src.1 as u64 - 1)
        / src.1 as u64).min(dst.1 as u64) as u32;
    // Expand slightly: nearest neighbor sampling can straddle the exact inverse boundary.
    let x0 = x0.saturating_sub(1);
    let y0 = y0.saturating_sub(1);
    let x1 = x1.saturating_add(1).min(dst.0);
    let y1 = y1.saturating_add(1).min(dst.1);
    (x0 < x1 && y0 < y1).then(|| DirtyRect::new(x0, y0, x1 - x0, y1 - y0))
}

fn joined(a: DirtyRect, b: DirtyRect) -> Option<DirtyRect> {
    let (ax, ay) = (a.x.saturating_add(a.width), a.y.saturating_add(a.height));
    let (bx, by) = (b.x.saturating_add(b.width), b.y.saturating_add(b.height));
    if a.x > bx || b.x > ax || a.y > by || b.y > ay { return None; }
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let right = ax.max(bx);
    let bottom = ay.max(by);
    let merged = (right - x) as u64 * (bottom - y) as u64;
    let separate = a.width as u64 * a.height as u64 + b.width as u64 * b.height as u64;
    (merged <= separate.saturating_mul(2)).then(|| DirtyRect::new(x, y, right - x, bottom - y))
}

pub(super) fn compact(mut rects: Vec<DirtyRect>, size: (u32, u32)) -> Vec<DirtyRect> {
    rects.retain(|r| r.width != 0 && r.height != 0);
    let mut i = 0;
    while i < rects.len() {
        let mut j = i + 1;
        while j < rects.len() {
            if let Some(merged) = joined(rects[i], rects[j]) {
                rects[i] = merged;
                rects.swap_remove(j);
                j = i + 1;
            } else { j += 1; }
        }
        i += 1;
    }
    let area: u64 = rects.iter().map(|r| r.width as u64 * r.height as u64).sum();
    let full_area = size.0 as u64 * size.1 as u64;
    if rects.len() > 24 || (full_area > 0 && area.saturating_mul(4) >= full_area.saturating_mul(3)) {
        vec![full(size)]
    } else { rects }
}

pub(super) fn mapped_damage(rects: &[DirtyRect], src: (u32, u32), dst: (u32, u32)) -> Vec<DirtyRect> {
    compact(rects.iter().filter_map(|r| scaled(*r, src, dst)).collect(), dst)
}

/// Change history allows a recycled Wayland/X11 shared buffer to catch up after other
/// buffers were presented. A missing/expired sequence forces a full refresh.
pub(super) struct DamageHistory {
    sequence: u64,
    recent: VecDeque<(u64, Vec<DirtyRect>)>,
}
impl DamageHistory {
    pub(super) fn new() -> Self { Self { sequence: 0, recent: VecDeque::new() } }
    pub(super) fn record(&mut self, rects: &[DirtyRect]) -> u64 {
        self.sequence += 1;
        self.recent.push_back((self.sequence, rects.to_vec()));
        while self.recent.len() > 48 { self.recent.pop_front(); }
        self.sequence
    }
    pub(super) fn since(&self, last: u64, size: (u32,u32)) -> Vec<DirtyRect> {
        if last == 0 || last > self.sequence ||
            self.recent.front().map_or(true, |(first,_)| last.saturating_add(1) < *first) {
            return vec![full(size)];
        }
        compact(self.recent.iter().filter(|(seq,_)| *seq > last)
            .flat_map(|(_,rs)| rs.iter().copied()).collect(), size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn scaling_damage_is_conservative() {
        let r = scaled(DirtyRect::new(10, 10, 3, 5), (100,100), (200,200)).unwrap();
        assert!(r.x <= 20 && r.y <= 20);
        assert!(r.x + r.width >= 26 && r.y + r.height >= 30);
    }
    #[test] fn history_recovers_stale_buffers() {
        let mut h = DamageHistory::new();
        let a = h.record(&[DirtyRect::new(1,1,1,1)]);
        h.record(&[DirtyRect::new(4,4,1,1)]);
        assert_eq!(h.since(0,(100,100)), vec![full((100,100))]);
        assert!(!h.since(a,(100,100)).is_empty());
    }
    #[test] fn out_of_bounds_ignored() {
        assert!(mapped_damage(&[DirtyRect::new(200,200,10,10)], (100,100),(100,100)).is_empty());
    }
}
