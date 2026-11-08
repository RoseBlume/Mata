//! Bounded, lazy worker pool for independent scanline bands.
//!
//! At one lane the main thread rasterizes directly, with **zero** worker
//! threads. Above one lane the scene is transformed/clipped/shaded **once**
//! into a shared projected-triangle stream, then lightweight workers fill
//! disjoint bands. No worker loads a copy of the 4K models or clones World.
//! Idle threads block on channels; pixel and depth storage are disjoint.
use crate::{render::Renderer, world::World};
use mata_win::Framebuffer;
use mata_3d::{GpuFrame,SoftwareFrame,Rasterizer};
use std::{sync::{Arc,mpsc::{self,Sender,Receiver}},thread::{self,JoinHandle},time::{Duration,Instant}};

struct Task {generation:u64,scene:Arc<SoftwareFrame>,width:u32,
    total_height:u32,start:u32,height:u32,buffer:Vec<u32>}
struct Complete {generation:u64,scene:Arc<SoftwareFrame>,start:u32,
    height:u32,buffer:Vec<u32>,pixels:u32}
struct Worker {sender:Sender<Task>,receiver:Receiver<Complete>,thread:Option<JoinHandle<()>>,buffer:Vec<u32>}

impl Worker {
    fn spawn(index:usize)->Self {
        let (tx,rx)=mpsc::channel::<Task>();
        let (done_tx,done_rx)=mpsc::channel::<Complete>();
        let handle=thread::Builder::new().name(format!("neon-render-{}",index+1))
            .spawn(move || {
                let mut raster=Rasterizer::new();
                let mut fb=Framebuffer::from_color(1,1,0xff091827);
                while let Ok(task)=rx.recv() {
                    fb.width=task.width;
                    fb.height=task.height;
                    fb.pixels=task.buffer;
                    fb.pixels.resize(task.width as usize*task.height as usize,0xff091827);
                    Renderer::paint_sky(&mut fb,task.total_height as usize,task.start as usize);
                    raster.draw_software_frame_band(&mut fb,&task.scene,
                        task.total_height as usize,task.start as usize);
                    if done_tx.send(Complete{generation:task.generation,scene:task.scene,
                        start:task.start,height:task.height,buffer:std::mem::take(&mut fb.pixels),
                        pixels:raster.stats.pixels}).is_err(){break;}
                }
            }).expect("could not spawn an arena renderer");
        Self{sender:tx,receiver:done_rx,thread:Some(handle),buffer:Vec::new()}
    }
    fn shutdown(mut self){
        // Sender drop ends the worker's blocking receive loop.
        drop(self.sender);
        if let Some(thread)=self.thread.take(){let _=thread.join();}
    }
}

pub struct RenderPool {
    workers:Vec<Worker>,
    single:Renderer,
    pub active:usize,
    pub maximum:usize,
    generation:u64,
    pub last_stats:(u32,u32),
    pub last_capture:Duration,
    pub last_raster:Duration,
}
impl RenderPool {
    pub fn new()->Self {
        let logical=std::thread::available_parallelism().map_or(1,|n|n.get());
        Self {workers:Vec::new(),single:Renderer::new(),active:1,
            maximum:logical.saturating_sub(1).max(1),generation:0,last_stats:(0,0),
            last_capture:Duration::ZERO,last_raster:Duration::ZERO}
    }
    pub fn set_active(&mut self,requested:usize){
        let active=requested.clamp(1,self.maximum);
        if active==self.active{return;}
        if active>1 {
            while self.workers.len()<active {
                let i=self.workers.len();
                self.workers.push(Worker::spawn(i));
            }
        }
        while self.workers.len()>active {
            if let Some(worker)=self.workers.pop(){worker.shutdown();}
        }
        if active==1 {
            while let Some(worker)=self.workers.pop(){worker.shutdown();}
        }
        self.active=active;
    }
    /// GPU rendering is single command submission; do not duplicate the
    /// entire high-poly scene for each software band worker.
    pub fn render_gpu(&mut self,target:&mut Framebuffer,world:&World)->GpuFrame{
        let frame=self.single.render_gpu(target,world);
        self.last_stats=self.single.stats();
        frame
    }
    pub fn recycle_gpu_frame(&mut self,frame:GpuFrame){
        self.single.recycle_gpu_frame(frame);
    }
    pub fn render(&mut self,target:&mut Framebuffer,world:&World){
        let w=target.width;let h=target.height;
        if w==0||h==0 {return;}
        if self.active<=1 || h<self.active as u32 {
            let started=Instant::now();
            self.single.render_scene(target,world);
            self.last_stats=self.single.stats();
            self.last_capture=Duration::ZERO;
            self.last_raster=started.elapsed();
            return;
        }
        self.generation+=1;
        let generation=self.generation;
        // This is the *only* model animation, transform, light, near-clip and
        // projection pass. Arc clones retain only a tiny refcount per worker.
        let capture_start=Instant::now();
        let mut captured=self.single.render_software_capture(target,world);
        captured.prepare_bands(h as usize,self.active);
        self.last_capture=capture_start.elapsed();
        let raster_start=Instant::now();
        let scene=Arc::new(captured);
        self.last_stats=(scene.len().min(u32::MAX as usize) as u32,0);
        // Worker threads may terminate unexpectedly.  Channels report this
        // without unwinding the main thread, so an entire frame can be
        // rerendered serially rather than exiting the game.
        let mut worker_failed=false;
        for (i,worker) in self.workers.iter_mut().take(self.active).enumerate(){
            let start=h*i as u32/self.active as u32;
            let end=h*(i as u32+1)/self.active as u32;
            if worker.sender.send(Task{generation,scene:Arc::clone(&scene),width:w,total_height:h,
                start,height:end-start,buffer:std::mem::take(&mut worker.buffer)}).is_err() {
                worker_failed=true;
                break;
            }
        }
        if !worker_failed {
            let stride=w as usize;
            for worker in self.workers.iter_mut().take(self.active) {
                let Ok(done)=worker.receiver.recv() else {
                    worker_failed=true;
                    break;
                };
                if done.generation!=generation {
                    worker_failed=true;
                    break;
                }
                let dest=done.start as usize*stride..(done.start+done.height) as usize*stride;
                target.pixels[dest].copy_from_slice(&done.buffer);
                self.last_stats.1=self.last_stats.1.saturating_add(done.pixels);
                worker.buffer=done.buffer; // Recycle pixel allocation on next frame.
                drop(done.scene);
            }
        }
        if worker_failed {
            eprintln!("[Neon Run CPU] raster worker failed; switching to serial rendering");
            // Shutdown joins any outstanding tasks; after that, render the
            // entire target, overwriting potentially incomplete bands.
            self.set_active(1);
            drop(scene);
            self.render(target,world);
            return;
        }
        // All worker-owned scene pointers returned with their completions.
        // Recover the projected triangle buffer for reuse in the next frame.
        if let Ok(owned)=Arc::try_unwrap(scene){self.single.recycle_software_frame(owned);}
        self.last_raster=raster_start.elapsed();

    }
}
impl Drop for RenderPool {
    fn drop(&mut self){
        while let Some(worker)=self.workers.pop(){worker.shutdown();}
    }
}

/// Hysteretic target-FPS controller: scale up quickly after consecutive misses,
/// scale down only after a sustained period of spare render time.  This avoids
/// repeatedly creating threads around one transient expensive frame.
pub struct AutoThreads {
    target_fps:u32,
    missed:f32,
    easy:f32,
    last_sample:Instant,
}
impl AutoThreads {
    pub fn new()->Self {let now=Instant::now();Self{target_fps:60,missed:0.0,easy:0.0,last_sample:now}}
    pub fn target(&self)->u32 {self.target_fps}
    pub fn cycle_target(&mut self){
        self.target_fps=match self.target_fps{30=>60,60=>90,90=>120,_=>30};
        self.missed=0.0;self.easy=0.0;
    }
    pub fn frame_budget(&self)->Duration{Duration::from_secs_f32(1.0/self.target_fps as f32)}
    pub fn sample(&mut self,workers:&mut RenderPool,render:Duration,total:Duration){
        let now=Instant::now();
        let delta=now.duration_since(self.last_sample).as_secs_f32().clamp(0.001,0.10);
        self.last_sample=now;
        let budget=self.frame_budget().as_secs_f32();
        let render_s=render.as_secs_f32();
        let total_s=total.as_secs_f32();
        // Rendering must be responsible for missing target to justify threads;
        // waiting for VSync, an audio driver, or frame pacing is not fixed by
        // adding renderer threads.
        let missed=render_s>budget*0.88 && total_s>=budget*0.98;
        let easy=render_s<budget*0.48 && total_s<budget*1.07;
        if missed {self.missed+=delta;self.easy=0.0;}
        else if easy {self.easy+=delta;self.missed=0.0;}
        else {self.missed=(self.missed-delta*0.5).max(0.0);self.easy=0.0;}
        if self.missed>=0.45 && workers.active<workers.maximum {
            workers.set_active(workers.active+1);
            self.missed=0.0;
            self.easy=0.0;
        } else if self.easy>=5.0 && workers.active>1 {
            workers.set_active(workers.active-1);
            self.missed=0.0;
            self.easy=0.0;
        }
    }
    pub fn reset(&mut self) {self.missed=0.0;self.easy=0.0;self.last_sample=Instant::now();}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn worker_limit_leaves_one_logical_cpu_for_main() {
        let pool=RenderPool::new();
        let logical=std::thread::available_parallelism().map_or(1,|n|n.get());
        assert!(pool.maximum<=logical.saturating_sub(1).max(1));
        assert_eq!(pool.active,1);
        assert!(pool.workers.is_empty());
    }
    #[test] fn parallel_bands_match_serial_scene() {
        let mut pool=RenderPool::new();
        let world=World::new();
        let mut serial=Framebuffer::from_color(104,73,0);
        pool.render(&mut serial,&world);
        if pool.maximum<2 {return;}
        pool.set_active(2);
        let mut parallel=Framebuffer::from_color(104,73,0);
        pool.render(&mut parallel,&world);
        assert_eq!(serial.pixels,parallel.pixels);
    }
}
