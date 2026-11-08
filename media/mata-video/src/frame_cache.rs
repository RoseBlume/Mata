//! Byte-bounded decoded ARGB cache. Deliberately independent of decoder APIs.
use std::{collections::VecDeque, time::Duration};
use crate::{SeekMode, VideoFrame};

#[derive(Default)]
pub(crate) struct FrameCache {
    entries: VecDeque<VideoFrame>,
    bytes: usize,
}
impl FrameCache {
    pub(crate) fn len(&self) -> usize { self.entries.len() }
    pub(crate) fn bytes(&self) -> usize { self.bytes }
    pub(crate) fn clear(&mut self) { self.entries.clear(); self.bytes = 0; }
    pub(crate) fn trim(&mut self, limit: usize) {
        while self.bytes > limit {
            if let Some(frame) = self.entries.pop_front() {
                self.bytes -= Self::size(&frame);
            } else { self.bytes = 0; break; }
        }
    }
    fn size(frame: &VideoFrame) -> usize { frame.pixels.capacity().saturating_mul(4) }
    pub(crate) fn insert(&mut self, frame: &VideoFrame, limit: usize) {
        let bytes = Self::size(frame);
        if bytes == 0 || bytes > limit { return; }
        // Remove the old entry before reinserting, preserving LRU eviction.
        if let Some(index) = self.entries.iter().position(|f| f.timestamp == frame.timestamp) {
            if let Some(old) = self.entries.remove(index) { self.bytes -= Self::size(&old); }
        }
        // Vec::clone() duplicates decoded pixels; this cache has its own cap.
        let entry = frame.clone();
        let bytes = Self::size(&entry);
        self.bytes += bytes;
        self.entries.push_back(entry);
        self.trim(limit);
    }
    /// A cached, already-decoded frame. Accurate mode uses the next frame
    /// within ~one frame interval, not an arbitrarily distant cached sample.
    pub(crate) fn lookup(&mut self, at: Duration, mode: SeekMode, frame_time: Duration)
        -> Option<VideoFrame>
    {
        let tolerance = match mode {
            SeekMode::Accurate => frame_time,
            SeekMode::Keyframe => Duration::from_millis(250),
        };
        let index = self.entries.iter().enumerate()
            .filter(|(_, f)| {
                if mode == SeekMode::Accurate && f.timestamp < at { return false; }
                f.timestamp.abs_diff(at) <= tolerance
            })
            .min_by_key(|(_, f)| f.timestamp.abs_diff(at))
            .map(|(i, _)| i)?;
        let frame = self.entries.remove(index)?;
        self.bytes -= Self::size(&frame);
        let result = frame.clone();
        self.bytes += Self::size(&frame);
        self.entries.push_back(frame);
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f(ms: u64) -> VideoFrame { VideoFrame { width: 2, height: 2,
        pixels: vec![0; 4], timestamp: Duration::from_millis(ms),
        duration: Some(Duration::from_millis(33)) } }
    #[test]
    fn cap_and_lru() {
        let mut c = FrameCache::default();
        for ms in [0,33,66] { c.insert(&f(ms),32); }
        assert_eq!(c.len(),2);
        assert!(c.lookup(Duration::ZERO,SeekMode::Keyframe,Duration::from_millis(33)).is_none() == false);
        // Newest insertion evicts the least recently used cached frame.
        c.insert(&f(99),32);
        assert!(c.bytes() <= 32);
        assert_eq!(c.len(),2);
    }
    #[test]
    fn accurate_only_chooses_next_close_frame() {
        let mut c = FrameCache::default();
        c.insert(&f(100),256);
        c.insert(&f(133),256);
        assert_eq!(c.lookup(Duration::from_millis(110), SeekMode::Accurate,
            Duration::from_millis(34)).unwrap().timestamp, Duration::from_millis(133));
        assert!(c.lookup(Duration::from_millis(300), SeekMode::Accurate,Duration::from_millis(34)).is_none());
    }
}
