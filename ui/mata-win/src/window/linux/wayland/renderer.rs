//! CPU Wayland wl_shm presenter: mapped triple buffering with per-slot damage history.
use std::sync::{atomic::{AtomicBool, Ordering},Arc};
use wayland_client::{protocol::{wl_buffer,wl_shm,wl_surface},QueueHandle};
use crate::{window::{DirtyRect,damage::{self,DamageHistory}}, buffer::Framebuffer};
use super::WaylandState;
use super::super::shared_memory::MappedMemory;
const BUFFER_COUNT: usize = 3;

struct BufferSlot {
    // Drop mapping only after compositor has emitted wl_buffer.release.
    memory: MappedMemory,
    buffer: wl_buffer::WlBuffer,
    busy: Arc<AtomicBool>,
    last_sequence: u64,
}
struct BufferGeneration {
    size: (u32,u32),
    slots: Vec<BufferSlot>,
    next_slot: usize,
}
impl BufferGeneration {
    fn new(shm: &wl_shm::WlShm, qh: &QueueHandle<WaylandState>, size: (u32,u32)) -> Self {
        use std::os::fd::AsFd;
        let stride = size.0.checked_mul(4).expect("Wayland stride overflow");
        let length = (stride as usize).checked_mul(size.1 as usize).expect("Wayland mapping overflow");
        let pool_length = i32::try_from(length).expect("Wayland shared buffer too large");
        let mut slots = Vec::with_capacity(BUFFER_COUNT);
        for _ in 0..BUFFER_COUNT {
            let memory = MappedMemory::new(length).expect("Wayland shared-memory mmap failed");
            let pool = shm.create_pool(memory.file.as_fd(), pool_length, qh, ());
            let busy = Arc::new(AtomicBool::new(false));
            let buffer = pool.create_buffer(0, size.0 as i32, size.1 as i32,
                stride as i32, wl_shm::Format::Xrgb8888, qh, busy.clone());
            pool.destroy();
            slots.push(BufferSlot { memory, buffer, busy, last_sequence: 0 });
        }
        Self { size, slots, next_slot: 0 }
    }
    fn all_released(&self) -> bool { self.slots.iter().all(|s| !s.busy.load(Ordering::Acquire)) }
    fn free_slot(&mut self) -> Option<usize> {
        for n in 0..self.slots.len() {
            let i = (self.next_slot+n) % self.slots.len();
            if !self.slots[i].busy.load(Ordering::Acquire) {
                self.next_slot = (i+1) % self.slots.len();
                return Some(i);
            }
        }
        None
    }
}

pub(super) struct WaylandRenderer {
    shm: wl_shm::WlShm,
    qh: QueueHandle<WaylandState>,
    framebuffer_size: (u32,u32),
    target_size: (u32,u32),
    x_map: Vec<usize>,
    y_map: Vec<usize>,
    current: Option<BufferGeneration>,
    retired: Vec<BufferGeneration>,
    history: DamageHistory,
    pending: Vec<DirtyRect>,
    frame_ready: Arc<AtomicBool>,
    needs_full: bool,
}
impl WaylandRenderer {
    pub(super) fn new(shm: wl_shm::WlShm, qh: QueueHandle<WaylandState>,
        framebuffer_size: (u32,u32), target_size: (u32,u32)) -> Self {
        let mut this = Self { shm, qh, framebuffer_size, target_size,
            x_map: Vec::new(), y_map: Vec::new(), current: None,
            retired: Vec::new(), history: DamageHistory::new(), pending: Vec::new(),
            frame_ready: Arc::new(AtomicBool::new(true)), needs_full: true };
        this.rebuild_scale_maps();
        this
    }
    pub(super) fn frame_ready(&self) -> bool { self.frame_ready.load(Ordering::Acquire) }
    pub(super) fn resize_framebuffer(&mut self, size: (u32,u32)) {
        if self.framebuffer_size != size {
            self.framebuffer_size=size;
            self.needs_full=true;
            self.rebuild_scale_maps();
            self.history = DamageHistory::new();
            if let Some(g) = self.current.as_mut() { for slot in &mut g.slots { slot.last_sequence=0; } }
        }
    }
    pub(super) fn resize_target(&mut self, size: (u32,u32)) {
        if self.target_size != size {
            self.target_size=size;
            self.needs_full=true;
            self.rebuild_scale_maps();
            if let Some(g) = self.current.take() { self.retired.push(g); }
            self.history = DamageHistory::new();
            self.pending.clear();
            self.collect_retired();
        }
    }
    fn rebuild_scale_maps(&mut self) {
        self.x_map.clear(); self.y_map.clear();
        let ((sw,sh),(dw,dh)) = (self.framebuffer_size,self.target_size);
        if sw==0 || sh==0 || dw==0 || dh==0 || (sw==dw && sh==dh) {return;}
        self.x_map.extend((0..dw as usize).map(|x| x*sw as usize/dw as usize));
        self.y_map.extend((0..dh as usize).map(|y| y*sh as usize/dh as usize));
    }
    fn collect_retired(&mut self) { self.retired.retain(|g| !g.all_released()); }
    fn ensure_generation(&mut self) {
        if self.current.is_none() && self.target_size.0!=0 && self.target_size.1!=0 {
            self.current=Some(BufferGeneration::new(&self.shm,&self.qh,self.target_size));
        }
    }
    /// `dirty` is expressed in source framebuffer pixels. Full present should pass a full rect.
    pub(super) fn present_dirty(&mut self, surface: &wl_surface::WlSurface,
        framebuffer: &Framebuffer, dirty: &[DirtyRect]) {
        let src_size=self.framebuffer_size;
        let dst_size=self.target_size;
        if src_size.0==0 || src_size.1==0 || dst_size.0==0 || dst_size.1==0 {return;}
        let mut mapped=if self.needs_full {
            vec![damage::full(dst_size)]
        } else {damage::mapped_damage(dirty,src_size,dst_size)};
        if mapped.is_empty() && self.pending.is_empty() {return;}
        mapped.extend(self.pending.drain(..));
        mapped=damage::compact(mapped,dst_size);
        self.collect_retired();
        self.ensure_generation();
        let slot_index=match self.current.as_mut().and_then(BufferGeneration::free_slot) {
            Some(i)=>i,
            None=>{self.pending=mapped;return;}
        };
        // Record changes only when they are actually submitted. A recycled slot may
        // be 1-3 commits behind, so reconstruct all missed damage before reuse.
        let sequence=self.history.record(&mapped);
        let slot = &mut self.current.as_mut().unwrap().slots[slot_index];
        let repairs=self.history.since(slot.last_sequence,dst_size);
        let bytes=slot.memory.bytes_mut();
        let (prefix,target,suffix)=unsafe {bytes.align_to_mut::<u32>()};
        debug_assert!(prefix.is_empty() && suffix.is_empty());
        let src=framebuffer.as_slice();
        update_pixels(src,src_size,target,dst_size,&repairs,&self.x_map,&self.y_map);
        slot.last_sequence=sequence;
        self.needs_full=false;
        // wl_buffer.release is the fence protecting this mapping from writes.
        std::sync::atomic::fence(Ordering::Release);
        slot.busy.store(true,Ordering::Release);
        surface.attach(Some(&slot.buffer),0,0);
        for r in &repairs {
            surface.damage_buffer(r.x as i32,r.y as i32,r.width as i32,r.height as i32);
        }
        // One outstanding frame callback is used as an advisory pacing signal.
        // Interactive presentation is not dropped while the compositor is busy.
        if self.frame_ready.swap(false,Ordering::AcqRel) {
            surface.frame(&self.qh,self.frame_ready.clone());
        }
        surface.commit();
    }
    pub(super) fn shutdown(&mut self) {
        self.current=None;
        self.retired.clear();
        self.pending.clear();
    }
}

fn update_pixels(src: &[u32], sw: (u32,u32), dst: &mut [u32], dw: (u32,u32),
    rects: &[DirtyRect], xmap: &[usize], ymap: &[usize]) {
    let (sw,tw) = (sw.0 as usize,dw.0 as usize);
    let exact = xmap.is_empty() && ymap.is_empty();
    for r in rects {
        let x0=r.x as usize; let x1=(r.x+r.width) as usize;
        for y in r.y as usize..(r.y+r.height) as usize {
            let sy=if exact {y} else {ymap[y]};
            let dst_line=&mut dst[y*tw+x0..y*tw+x1];
            if exact || sw==tw {
                dst_line.copy_from_slice(&src[sy*sw+x0..sy*sw+x1]);
            } else {
                for (offset,pixel) in dst_line.iter_mut().enumerate() {
                    *pixel=src[sy*sw+xmap[x0+offset]];
                }
            }
        }
    }
}
