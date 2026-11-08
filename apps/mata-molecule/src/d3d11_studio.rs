//! Optional native Direct3D 11 presentation of the molecular 3D viewport.
//!
//! MataWin continues to paint the rest of the Studio UI as a CPU framebuffer.
//! `capture_ui_underlay` runs after the 3D canvas gradient is painted and before
//! labels, toolbar, atom selection badges and floating inspector are drawn.
//! Those later pixels become an alpha HUD over the clipped GPU geometry.
//! Other tabs and all non-Windows builds continue using MataWin normally.
use mata_3d::{GpuFrame,gpu_backend::GpuPreference,intel_gpu::D3d11Gpu};
use mata_win::Framebuffer;
use crate::draw::Rect;
use std::ffi::c_void;

pub struct Layer {
    gpu:D3d11Gpu,
    hud:Framebuffer,
    underlay:Vec<u32>,
    clip:Option<Rect>,
}
impl Layer {
    pub fn new(hwnd:*mut c_void,w:u32,h:u32)->Result<Self,String>{
        let preference=std::env::var("MATA_GPU_BACKEND")
            .ok().map(|x|GpuPreference::parse(&x)).transpose()?
            .unwrap_or(GpuPreference::Auto);
        Self::with_preference(hwnd,w,h,preference)
    }
    pub fn with_preference(hwnd:*mut c_void,w:u32,h:u32,preference:GpuPreference)->Result<Self,String>{
        if preference==GpuPreference::Software{return Err("software renderer selected".into());}
        let shaders=mata_3d::shader_dispatch::D3d11Shaders::new()?;
        let gpu=D3d11Gpu::from_hwnd_with_preference(hwnd,w,h,&shaders,preference)?;
        Ok(Self {gpu,hud:Framebuffer::new(0,0),underlay:Vec::new(),clip:None})
    }
    pub fn name(&self)->&str{&self.gpu.adapter_name}
    pub fn clear_frame(&mut self){self.clip=None;}
    pub fn has_capture(&self)->bool{self.clip.is_some()}
    /// Snapshot only the viewport, not the complete application's framebuffer.
    pub fn capture_ui_underlay(&mut self,fb:&Framebuffer,scene_rect:Rect){
        let Some(clip)=clipped(scene_rect,fb.width,fb.height) else {
            self.clip=None;return;
        };
        self.underlay.clear();
        self.underlay.reserve(clip.w as usize*clip.h as usize);
        for y in clip.y as usize..(clip.y+clip.h) as usize {
            let start=y*fb.width as usize+clip.x as usize;
            self.underlay.extend_from_slice(&fb.pixels[start..start+clip.w as usize]);
        }
        self.clip=Some(clip);
    }
    /// The base texture is the completed UI framebuffer. The GPU rasterizer
    /// overlays only `clip`, while the HUD restores any foreground UI drawn
    /// after the viewport snapshot (labels, selection and floating controls).
    pub fn present(&mut self,completed_ui:&Framebuffer,scene:&GpuFrame)->Result<(),String>{
        let clip=self.clip.ok_or("missing molecule UI viewport snapshot")?;
        let expected=clip.w as usize*clip.h as usize;
        if self.underlay.len()!=expected{return Err("mismatched viewport snapshot".into());}
        if self.hud.width!=completed_ui.width||self.hud.height!=completed_ui.height{
            self.hud.resize_and_clear(completed_ui.width,completed_ui.height,0);
        }else{self.hud.pixels.fill(0);}
        // A changed pixel is an opaque post-scene control pixel. Unchanged
        // pixels remain transparent, preserving the newly rendered GPU atoms.
        // This also handles multiple UI overlays without manually duplicating
        // the code that draws them.
        for y in 0..clip.h as usize {
            let start=(clip.y as usize+y)*completed_ui.width as usize+clip.x as usize;
            let before=y*clip.w as usize;
            for x in 0..clip.w as usize {
                let pixel=completed_ui.pixels[start+x];
                if pixel!=self.underlay[before+x] {
                    self.hud.pixels[start+x]=pixel|0xff00_0000;
                }
            }
        }
        self.gpu.present_geometry_clipped(completed_ui,scene,&self.hud,
            (clip.x,clip.y,clip.w,clip.h))
    }
}
fn clipped(r:Rect,w:u32,h:u32)->Option<Rect>{
    let left=i64::from(r.x).clamp(0,i64::from(w));
    let top=i64::from(r.y).clamp(0,i64::from(h));
    let right=(i64::from(r.x)+i64::from(r.w.max(0))).clamp(0,i64::from(w));
    let bottom=(i64::from(r.y)+i64::from(r.h.max(0))).clamp(0,i64::from(h));
    (right>left && bottom>top).then(||Rect::new(left as i32,top as i32,(right-left) as i32,(bottom-top) as i32))
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn clip_never_exceeds_window(){
        assert_eq!((clipped(Rect::new(-20,-3,40,35),100,100).unwrap().w,
                    clipped(Rect::new(-20,-3,40,35),100,100).unwrap().h),(20,32));
        assert!(clipped(Rect::new(150,150,30,40),100,100).is_none());
    }
}
