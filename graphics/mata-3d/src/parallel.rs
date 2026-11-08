//! Optional parallel software rendering for dense scientific scenes.
//!
//! The scene is transformed, lit, clipped and projected exactly once using
//! `Rasterizer`'s existing software command capture. Independent workers own
//! nonoverlapping row bands and depth buffers; their allocations are reused.
//! At one lane there is no capture, thread creation, or extra framebuffer copy.
use crate::{Camera,Rasterizer};
use mata_win::Framebuffer;

struct BandWorker {raster:Rasterizer, pixels:Framebuffer}
impl BandWorker {
    fn new()->Self {Self{raster:Rasterizer::new(),pixels:Framebuffer::new(0,0)}}
}

pub struct ParallelSoftware {
    workers:Vec<BandWorker>,active:usize,maximum:usize,
    capturing:bool,last_triangles:usize,slow_frames:u8,fast_frames:u8,
}
impl Default for ParallelSoftware {fn default()->Self{Self::new()}}
impl ParallelSoftware {
    pub fn new()->Self {
        let logical=std::thread::available_parallelism().map_or(1,|v|v.get());
        let maximum=logical.saturating_sub(1).max(1);
        let requested=std::env::var("MATA_RENDER_THREADS").ok()
            .and_then(|value|value.parse::<usize>().ok()).unwrap_or(1);
        let active=requested.clamp(1,maximum);
        let mut this=Self{workers:Vec::new(),active:1,maximum,
            capturing:false,last_triangles:0,slow_frames:0,fast_frames:0};
        this.set_active_threads(active);
        this
    }
    pub fn active_threads(&self)->usize{self.active}
    pub fn max_threads(&self)->usize{self.maximum}
    pub fn set_active_threads(&mut self,threads:usize){
        self.active=threads.clamp(1,self.maximum);
        while self.workers.len()<self.active {self.workers.push(BandWorker::new());}
    }
    /// Adapt to render time, without oscillating on every fast/slow frame.
    /// The expensive scene is the only thing parallelized; UI stays on its
    /// original thread. Use measurements excluding presentation/VSync.
    pub fn observe_render_ms(&mut self,ms:f32) {
        if !ms.is_finite()||ms<=0.0{return;}
        if ms>32.0 && self.last_triangles>3_000 {
            self.slow_frames=self.slow_frames.saturating_add(1);
            self.fast_frames=0;
            if self.slow_frames>=4 {self.set_active_threads(self.active+1);self.slow_frames=0;}
        }else if ms<16.0 {
            self.fast_frames=self.fast_frames.saturating_add(1);
            self.slow_frames=0;
            if self.fast_frames>=90 {self.set_active_threads(self.active.saturating_sub(1));self.fast_frames=0;}
        }else{self.slow_frames=0;self.fast_frames=0;}
    }
    /// Call before any scene geometry is submitted.
    pub fn begin(&mut self,raster:&mut Rasterizer,target:&Framebuffer,camera:Camera) {
        self.capturing=self.active>1 && target.height>=self.active as u32 && self.last_triangles>3_000;
        raster.set_software_capture(self.capturing);
        raster.begin_frame(target,camera);
    }
    /// Complete an optional captured scene. Background pixels already painted
    /// in `target` are preserved, and every band performs identical depth
    /// testing to the serial renderer.
    pub fn finish(&mut self,raster:&mut Rasterizer,target:&mut Framebuffer) {
        if !self.capturing {
            self.last_triangles=raster.stats.triangles as usize;
            return;
        }
        self.capturing=false;
        let mut frame=raster.take_software_frame();
        self.last_triangles=frame.len();
        let lanes=self.active.min(target.height as usize).max(1);
        let (w,h)=(target.width as usize,target.height as usize);
        if w==0||h==0 {raster.recycle_software_frame(frame);return;}
        frame.prepare_bands(h,lanes);
        for (i,worker) in self.workers[..lanes].iter_mut().enumerate() {
            let start=h*i/lanes;let end=h*(i+1)/lanes;
            worker.pixels.width=target.width;
            worker.pixels.height=(end-start) as u32;
            worker.pixels.pixels.resize((end-start)*w,0);
            worker.pixels.pixels.copy_from_slice(&target.pixels[start*w..end*w]);
        }
        std::thread::scope(|scope| {
            for (i,worker) in self.workers[..lanes].iter_mut().enumerate(){
                let start=h*i/lanes;
                let commands=&frame;
                scope.spawn(move ||{
                    worker.raster.draw_software_frame_band(&mut worker.pixels,commands,h,start);
                });
            }
        });
        for (i,worker) in self.workers[..lanes].iter().enumerate(){
            let start=h*i/lanes;let end=h*(i+1)/lanes;
            target.pixels[start*w..end*w].copy_from_slice(&worker.pixels.pixels);
            raster.stats.pixels=raster.stats.pixels.saturating_add(worker.raster.stats.pixels);
        }
        raster.recycle_software_frame(frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec3;
    #[test]
    fn parallel_matches_serial_with_occlusion() {
        let cam=Camera::new(Vec3::default(),Vec3::new(0.,0.,1.),Vec3::new(1.,0.,0.));
        let mut a=Framebuffer::from_color(80,63,0xff102034);
        let mut b=Framebuffer::from_color(80,63,0xff102034);
        let mut rs=Rasterizer::new();let mut rp=Rasterizer::new();
        let mut one=ParallelSoftware::new();let mut many=ParallelSoftware::new();
        many.maximum=4;many.set_active_threads(3);many.last_triangles=4_000;
        for (software,raster,fb) in [(&mut one,&mut rs,&mut a),(&mut many,&mut rp,&mut b)] {
            software.begin(raster,fb,cam);
            for i in 0..4000 {
                let x=(i%21) as f32*0.07-0.7;
                let z=if i%2==0{2.0}else{3.0};
                raster.triangle_view(fb,[Vec3::new(x,-0.7,z),Vec3::new(x+0.2,0.9,z),Vec3::new(x+0.3,-0.6,z)],
                    0xff0088cc);
            }
            software.finish(raster,fb);
        }
        assert_eq!(a.pixels,b.pixels);
    }
}
