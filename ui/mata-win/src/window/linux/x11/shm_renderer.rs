//! Optional MIT-SHM path for local X11 servers using x11rb's existing `shm` feature.
//! This module is enabled by the crate feature `x11-shm` (see dependency notes).
use std::{cell::Cell, os::fd::OwnedFd};
use x11rb::{connection::Connection,protocol::shm::ConnectionExt as _,
    rust_connection::RustConnection,protocol::xproto::{Window,Gcontext}};
use crate::window::{DirtyRect,damage::{self,DamageHistory}};
use super::super::shared_memory::MappedMemory;

struct Slot {
    mem: MappedMemory,
    segment: u32,
    pending_completions: Cell<u32>,
    last_sequence: u64,
}
pub(super) struct ShmBackend {
    size: (u32,u32),
    slots: Vec<Slot>,
    next: usize,
    history: DamageHistory,
}
impl ShmBackend {
    pub(super) fn new(conn: &RustConnection, size:(u32,u32)) -> Option<Self> {
        if size.0==0 || size.1==0 || size.0>i16::MAX as u32 || size.1>i16::MAX as u32 {return None;}
        // Query first; remote X11 servers typically reject local fd attachment.
        conn.shm_query_version().ok()?.reply().ok()?;
        let len=(size.0 as usize).checked_mul(size.1 as usize)?.checked_mul(4)?;
        if len>u32::MAX as usize {return None;}
        let mut result=Self {size,slots:Vec::new(),next:0,history:DamageHistory::new()};
        for _ in 0..3 {
            let mem=match MappedMemory::new(len) {Ok(m)=>m,Err(_)=>{result.shutdown(conn);return None;}};
            let segment=match conn.generate_id() {Ok(id)=>id,Err(_)=>{result.shutdown(conn);return None;}};
            let fd: OwnedFd=match mem.file.try_clone() {Ok(file)=>file.into(),Err(_)=>{result.shutdown(conn);return None;}};
            let Ok(cookie)=conn.shm_attach_fd(segment,fd,false) else {result.shutdown(conn);return None;};
            if cookie.check().is_err() {result.shutdown(conn);return None;}
            result.slots.push(Slot {mem,segment,pending_completions:Cell::new(0),last_sequence:0});
        }
        Some(result)
    }
    pub(super) fn size(&self)->(u32,u32) {self.size}
    pub(super) fn idle(&self)->bool {self.slots.iter().all(|s|s.pending_completions.get()==0)}
    pub(super) fn release(&self, seg:u32) {
        if let Some(s)=self.slots.iter().find(|s|s.segment==seg) {
            s.pending_completions.set(s.pending_completions.get().saturating_sub(1));
        }
    }
    pub(super) fn invalidate(&mut self) {
        self.history=DamageHistory::new();
        for s in &mut self.slots {s.last_sequence=0;}
    }
    /// Returns false when all slots are owned by the X server, so caller falls
    /// back to the core PutImage path without blocking the main event loop.
    pub(super) fn present(&mut self,conn:&RustConnection,win:Window,gc:Gcontext,depth:u8,
        src:&[u32],dirty:&[DirtyRect])->bool {
        let len=self.slots.len();
        let Some(i)=(0..len).map(|j|(self.next+j)%len)
            .find(|&j| self.slots[j].pending_completions.get()==0) else {return false};
        self.next=(i+1)%len;
        let seq=self.history.record(dirty);
        let slot=&mut self.slots[i];
        let repairs=self.history.since(slot.last_sequence,self.size);
        let sw=self.size.0 as usize;
        let bytes=slot.mem.bytes_mut();
        let (prefix,pixels,suffix)=unsafe {bytes.align_to_mut::<u32>()};
        debug_assert!(prefix.is_empty() && suffix.is_empty());
        for r in repairs {
            for y in r.y as usize..(r.y+r.height) as usize {
                let left=y*sw+r.x as usize;
                let right=left+r.width as usize;
                pixels[left..right].copy_from_slice(&src[left..right]);
            }
        }
        slot.last_sequence=seq;
        std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
        let mut sent=0u32;
        for r in dirty {
            // Source is the full mapped image, destination is the matching window rect.
            if conn.shm_put_image(win,gc,self.size.0 as u16,self.size.1 as u16,
                r.x as u16,r.y as u16,r.width as u16,r.height as u16,
                r.x as i16,r.y as i16,depth,2,true,slot.segment,0).is_ok() {
                sent+=1;
            }
        }
        slot.pending_completions.set(sent);
        let _=conn.flush();
        sent==dirty.len() as u32
    }
    pub(super) fn shutdown(&mut self,conn:&RustConnection) {
        for s in self.slots.drain(..) {let _=conn.shm_detach(s.segment);}
        let _=conn.flush();
    }
}
