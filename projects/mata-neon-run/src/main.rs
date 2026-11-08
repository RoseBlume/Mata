//! MATA // NEON RUN — software 3D shooter on MataWin.
//! Native pointer capture (Windows/X11), dynamic internal resolution,
//! full-resolution HUD, and reusable mata-3d depth-tested mesh rendering.
mod campaign;
mod game_gpu;
mod environment;
mod enemy_motion;
mod enemy_ai;
mod enemy_rig;
mod hud;
mod parallel;
mod upscale;
mod focus;
mod math;
mod mouse;
mod render;
mod sound;
mod world;
mod weapons;
mod visibility;
#[cfg(target_os="windows")] use mata_3d::intel_gpu::D3d11Gpu;
use mata_3d::gpu_backend::GpuPreference;

use mata_win::{Framebuffer, Window, events::{Key,WindowEvent}};
use std::time::{Duration,Instant};
use world::{Mode,World};
use sound::{Audio,SoundCue};
use parallel::{RenderPool,AutoThreads};

fn down(win:&Window,ch:char)->bool{
    win.is_key_down(Key::Char(ch))||win.is_key_down(Key::Char(ch.to_ascii_uppercase()))
}

const WINDOW_TITLE:&str="MATA // OUTPOST ZERO - THE ECHO DIRECTIVE";

fn run(mut window:Window){
    let (w,h)=window.client_size();
    let mut fb=Framebuffer::from_color(w.max(1),h.max(1),0xff0a1422);
    let mut low=Framebuffer::from_color(1,1,0xff0a1422);
    // Runtime hardware detection; no GPU or SDK is required at build time.
    // MATA_GPU_BACKEND: auto | cpu | d3d11 | intel | nvidia | amd.
    // Legacy MATA_INTEL_GPU=off is respected only when the new variable is unset.
    let gpu_request=match std::env::var("MATA_GPU_BACKEND") {
        Ok(s)=>match GpuPreference::parse(&s) {
            Ok(p)=>p,
            Err(e)=>{eprintln!("[Neon Run GPU] {e}; using auto");GpuPreference::Auto}
        },
        Err(_)=>if std::env::var("MATA_INTEL_GPU").ok().is_some_and(|s|
            s.eq_ignore_ascii_case("off")||s=="0") {
            GpuPreference::Software
        }else{GpuPreference::Auto},
    };
    eprintln!("[Neon Run GPU] requested backend: {}",gpu_request.label());
    let cpu_profile=std::env::var("MATA_SOFTWARE_PROFILE").ok()
        .is_some_and(|v|v=="1"||v.eq_ignore_ascii_case("on"));
    #[cfg(target_os="windows")]
    let gpu_profile=std::env::var("MATA_GPU_PROFILE").ok()
        .is_some_and(|v|v=="1"||v.eq_ignore_ascii_case("on"));
    #[cfg(target_os="windows")]
    let shaders=game_gpu::make_shader_program();
    #[cfg(target_os="windows")]
    let mut d3d_gpu=if gpu_request==GpuPreference::Software{None}else {
        match shaders.as_ref().map_err(|e|e.clone()).and_then(|source|
            D3d11Gpu::from_hwnd_with_preference(window.handle(),w.max(1),h.max(1),source,gpu_request)){
            Ok(gpu)=>{
                eprintln!("[Neon Run GPU] selected Direct3D 11 on {} ({}, {} MiB dedicated)",
                    gpu.adapter_name,gpu_backend_vendor(gpu.vendor_id),gpu.dedicated_video_memory/(1024*1024));
                Some(gpu)
            },
            Err(err)=>{eprintln!("[Neon Run GPU] {err}; using software renderer");None}
        }
    };
    #[cfg(not(target_os="windows"))]
    eprintln!("[Neon Run GPU] native GPU 3D backend unavailable on this OS; using software renderer");
    #[cfg(target_os="windows")]
    let mut overlay=Framebuffer::from_color(1,1,0);
    let mut pool=RenderPool::new();
    let mut controller=AutoThreads::new();
    eprintln!("[Neon Run] target: {} fps | 3D render lanes: 1..{} (logical CPU count minus one)",
        controller.target(),pool.maximum);
    let mut world=World::new();
    // Never silently swallow audio initialization failures.
    let audio=match Audio::new(){
        Ok(audio)=>Some(audio),
        Err(err)=>{eprintln!("[Neon Run audio] disabled: {err}");None}
    };
    let mut muted=false;
    let mut pointer=mouse::Mouse::new(&window);
    let mut events=Vec::with_capacity(48);
    let mut last=Instant::now();
    let mut accumulator=0.0f32;
    // F2: automatic (default), fast fixed 50%, full native resolution.
    let mut quality_mode=0u8;
    let mut adaptive_scale=0.55f32;
    let mut frame_counter=0u32;
    let mut redraw=true;
    let mut was_active=false;
    let mut last_idle_paint=Instant::now();
    let mut smoothed_render_ms=16.0f32;
    let mut smoothed_frame_s=1.0f32/60.0;
    let mut upscaler=upscale::Upscaler::new();
    loop {
        let frame_start=Instant::now();
        let dt=frame_start.duration_since(last).as_secs_f32().min(0.10);
        last=frame_start;
        window.poll_events_into(&mut events);
        let mut trigger=false;
        let mut request_dash=false;
        for event in events.iter().copied(){
            match event {
                WindowEvent::Close=>{let _=window.show_cursor();return},
                WindowEvent::KeyPressed(Key::Escape)=>{
                    world.sounds.push(SoundCue::Menu);
                    if world.story.is_some(){world.dismiss_story();continue;}
                    if world.showing_help{world.showing_help=false;continue;}
                    match world.mode {
                        Mode::Playing=>world.mode=Mode::Paused,
                        Mode::Paused=>world.mode=Mode::Playing,
                        _=>{let _=window.show_cursor();return}
                    }
                },
                WindowEvent::KeyPressed(Key::Enter)=>{
                    if world.story.is_some(){world.dismiss_story();}
                    else if world.mode==Mode::Lost{world.retry_mission();}
                    else if world.mode==Mode::Won{world=World::new();}
                    else if world.mode==Mode::Playing{world.interact();}
                },
                WindowEvent::KeyPressed(Key::Char('e')|Key::Char('E'))=>world.interact(),
                WindowEvent::KeyPressed(Key::Char('t')|Key::Char('T'))=>world.open_journal(),
                WindowEvent::KeyPressed(Key::Char('['))=>world.next_record(-1),
                WindowEvent::KeyPressed(Key::Char(']'))=>world.next_record(1),
                WindowEvent::KeyPressed(Key::Space)=>world.jump(),
                WindowEvent::KeyPressed(Key::Char('r')|Key::Char('R'))=>world.reload(),
                WindowEvent::KeyPressed(Key::Char('q')|Key::Char('Q'))=>request_dash=true,
                WindowEvent::KeyPressed(Key::Num(1)|Key::Char('1'))=>world.select(0),
                WindowEvent::KeyPressed(Key::Num(2)|Key::Char('2'))=>world.select(1),
                WindowEvent::KeyPressed(Key::Num(3)|Key::Char('3'))=>world.select(2),
                WindowEvent::KeyPressed(Key::Num(4)|Key::Char('4'))=>world.select(3),
                WindowEvent::KeyPressed(Key::Num(5)|Key::Char('5'))=>world.select(4),
                WindowEvent::Scroll(_,vertical) if vertical!=0=>{
                    let delta=if vertical>0{-1}else{1};
                    world.select((world.weapon as i32+delta).rem_euclid(5) as usize);
                },
                WindowEvent::KeyPressed(Key::F(1))=>{world.showing_help=!world.showing_help;world.sounds.push(SoundCue::Menu);},
                WindowEvent::KeyPressed(Key::F(2))=>quality_mode=(quality_mode+1)%3,
                WindowEvent::KeyPressed(Key::F(3))=>world.show_stats=!world.show_stats,
                WindowEvent::KeyPressed(Key::F(5))=>{
                    #[cfg(target_os="windows")]
                    {
                        if d3d_gpu.is_some(){
                            d3d_gpu=None;
                            eprintln!("[Neon Run GPU] manually switched to software rendering");
                        }else {
                            // An explicit F5 enables GPU even if startup was set to CPU.
                            let request=if gpu_request==GpuPreference::Software{GpuPreference::Auto}else{gpu_request};
                            let (width,height)=cw_prev(&window);
                            d3d_gpu=match shaders.as_ref().map_err(|e|e.clone()).and_then(|source|
                                D3d11Gpu::from_hwnd_with_preference(window.handle(),width,height,source,request)){
                                Ok(value)=>{
                                    eprintln!("[Neon Run GPU] selected {} ({})",value.adapter_name,gpu_backend_vendor(value.vendor_id));
                                    Some(value)
                                },
                                Err(err)=>{eprintln!("[Neon Run GPU] {err}; software fallback");None}
                            };
                        }
                    }
                    #[cfg(not(target_os="windows"))]
                    eprintln!("[Neon Run GPU] no native GPU rasterizer for this OS; software rendering active");
                    redraw=true;
                },
                WindowEvent::KeyPressed(Key::F(4))=>{
                    controller.cycle_target();
                    eprintln!("[Neon Run] frame target: {} fps", controller.target());
                    redraw=true;
                },
                WindowEvent::KeyPressed(Key::Char('m')|Key::Char('M'))=>{
                    muted=!muted;
                    if let Some(sound)=&audio {sound.mute(muted);}
                    eprintln!("[Neon Run audio] {}",if muted{"muted"}else{"unmuted"});
                },
                WindowEvent::KeyPressed(Key::MouseLeft)=>trigger=true,
                _=>{}
            }
        }
        if !events.is_empty(){redraw=true;}
        // Centered reticle is the aim point. Mouse deltas rotate the camera;
        // do NOT move the HUD crosshair around the framebuffer.
        let focused=focus::active(&window);
        let playable=focused && world.mode==Mode::Playing&&!world.showing_help&&world.story.is_none();
        world.set_aim_down_sights(playable && window.is_key_down(Key::MouseRight));
        let (dx,dy)=pointer.sample(&window,playable,&events);
        if playable {
            let aim=world.aim_ease();
            let sensitivity=0.0035+(0.0009-0.0035)*aim;
            world.look(dx*sensitivity,-dy*sensitivity);
            let turn=(window.is_key_down(Key::Right) as i32-window.is_key_down(Key::Left) as i32) as f32;
            let tilt=(window.is_key_down(Key::Up) as i32-window.is_key_down(Key::Down) as i32) as f32;
            world.look(turn*dt*(2.2+(0.65-2.2)*aim),tilt*dt*(1.7+(0.5-1.7)*aim));
            let fw=(down(&window,'w') as i32-down(&window,'s') as i32) as f32;
            let side=(down(&window,'d') as i32-down(&window,'a') as i32) as f32;
            let sprint=window.is_key_down(Key::LeftShift)||window.is_key_down(Key::RightShift);
            let step_before=(world.walk*0.44) as u32;
            world.move_player(fw,side,sprint,request_dash,dt);
            if world.on_ground && (world.walk*0.44) as u32!=step_before {
                world.sounds.push(SoundCue::Step);
            }
            if trigger||window.is_key_down(Key::MouseLeft)||down(&window,'j'){
                world.fire();
            }
        }
        accumulator=(accumulator+dt).min(0.08);
        const STEP:f32=1.0/120.0;
        if focused {
            while accumulator>=STEP {world.update(STEP);accumulator-=STEP;}
        } else {accumulator=0.0;}
        if let Some(sound)=&audio {
            sound.set_ambience(playable);
            for cue in world.sounds.drain(..) {sound.cue(cue);}
        }else{
            // Always drain, even on headless systems, to avoid unbounded growth.
            world.sounds.clear();
        }

        let (cw,ch)=window.client_size();
        if cw==0||ch==0 {std::thread::sleep(Duration::from_millis(80));last=Instant::now();continue}
        if cw!=fb.width||ch!=fb.height {
            fb.resize_and_clear(cw,ch,0xff091827);
            redraw=true;
        }
        // No continuously repeating presents on background/static menus.
        // Let the compositor reuse the last presented buffer, and sleep while
        // monitoring input at 20 Hz. Repaint once on state changes or a rare
        // low-frequency refresh, never continuously in the background.
        let active_scene=focused && !world.showing_help && world.story.is_none() && matches!(world.mode,Mode::Playing|Mode::Between);
        if active_scene!=was_active {
            redraw=true;controller.reset();was_active=active_scene;
            if !active_scene {pool.set_active(1);}
        }
        if !active_scene && !redraw && last_idle_paint.elapsed()<Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(50));
            last=Instant::now();
            continue;
        }
        if !active_scene {last_idle_paint=Instant::now();}
        redraw=false;
        let scale=match quality_mode {
            0=>adaptive_scale, // Auto: scales down when CPU render time is high.
            1=>0.50,           // Fast: constant half-resolution 3D.
            _=>1.0,            // Native: highest quality at full pixels.
        };
        // Quality modes affect pixel resolution only. Even far-away visible
        // enemies retain their full 4K articulated meshes at every setting.
        world.high_quality=true;
        let render_start=Instant::now();
        // Molecule Studio's mata-3d supplies geometry to the Intel iGPU for
        // triangle fill and depth testing. CPU still transforms/lights vertices.
        // GPU draws every scene triangle at native resolution, so the CPU
        // need not run any triangle pixel loop or CPU upscaling pass.
        #[cfg(target_os="windows")]
        let gpu_active=d3d_gpu.is_some();
        #[cfg(not(target_os="windows"))]
        let gpu_active=false;
        // GPU capture uses the single scene encoder. Software worker threads
        // would only duplicate all the high-poly meshes in RAM and contend
        // with the iGPU for shared memory bandwidth; suspend them while GPU is
        // active, and let the auto-tuner manage them only on the CPU path.
        if gpu_active {pool.set_active(1);}
        let render_low=!gpu_active && scale<=0.995;
        #[cfg(target_os="windows")]
        let gpu_scene=if gpu_active {
            Some(pool.render_gpu(&mut fb,&world))
        } else {None};
        if !gpu_active {
            if !render_low {pool.render(&mut fb,&world);}
            else {
                let iw=((cw as f32*scale).round() as u32).max(1);
                let ih=((ch as f32*scale).round() as u32).max(1);
                if low.width!=iw||low.height!=ih {low.resize_and_clear(iw,ih,0xff091827);}
                pool.render(&mut low,&world);
            }
        }
        let scene_time=render_start.elapsed();
        let fps=(1.0/smoothed_frame_s.max(0.0001)).round().min(999.0) as u32;
        let (tris,pixels)=pool.last_stats;
        #[cfg(target_os="windows")]
        let backend_status=d3d_gpu.as_ref().map(|g|format!("D3D11 {} - {}",gpu_backend_vendor(g.vendor_id),g.adapter_name))
            .unwrap_or_else(||"CPU SOFTWARE".to_owned());
        #[cfg(not(target_os="windows"))]
        let backend_status="CPU SOFTWARE".to_owned();
        // The Direct3D 11 backend uploads the native-resolution transparent HUD
        // separately, so expensive CPU scene upscaling can be skipped.
        // CPU fallback is always retained, including after any GPU failure.
        #[cfg(target_os="windows")]
        let mut gpu_presented=false;
        #[cfg(target_os="windows")]
        if let Some(gpu)=d3d_gpu.as_mut(){
            if overlay.width!=cw||overlay.height!=ch{overlay.resize_and_clear(cw,ch,0);}
            else {overlay.pixels.fill(0);}
            hud::draw(&mut overlay,&world,tris,pixels,fps,
                100,audio.is_some(),muted,
                pool.active,pool.maximum,controller.target(),&backend_status);
            let scene=gpu_scene.as_ref().expect("GPU command capture was not prepared");
            let submit_start=Instant::now();
            match gpu.present_geometry_procedural(scene,&overlay){
                Ok(())=>{
                    gpu_presented=true;
                    if gpu_profile && frame_counter%120==0 {
                        eprintln!("[Neon Run GPU] {} | capture {:.2} ms | GPU submission+present {:.2} ms | {:.2} MiB geometry | {:.2} MiB HUD | {} triangles",
                            gpu.adapter_name,scene_time.as_secs_f64()*1000.0,
                            submit_start.elapsed().as_secs_f64()*1000.0,
                            (scene.vertices.len()*std::mem::size_of::<mata_3d::GpuVertex>()) as f64/(1024.0*1024.0),
                            (overlay.pixels.len()*4) as f64/(1024.0*1024.0),tris);
                    }
                },
                Err(err)=>{
                    eprintln!("[Neon Run GPU] {err}; switching to software fallback");
                    d3d_gpu=None;
                    // GPU capture deliberately did not fill the CPU sky.
                    // Rerender all pixels before switching to software present.
                    pool.render(&mut fb,&world);
                }
            }
        }
        #[cfg(not(target_os="windows"))]
        let gpu_presented=false;
        if !gpu_presented{
            if render_low {
                upscaler.render(&low,&mut fb);
            }
            // Reflect a GPU presentation failure immediately in the HUD.
            #[cfg(target_os="windows")]
            let fallback_status=if d3d_gpu.is_none(){"CPU SOFTWARE"}else{&backend_status};
            #[cfg(not(target_os="windows"))]
            let fallback_status=backend_status.as_str();
            hud::draw(&mut fb,&world,tris,pixels,fps,
                (scale*100.0).round() as u32,audio.is_some(),muted,
                pool.active,pool.maximum,controller.target(),fallback_status);
            if !window.present_exact(&fb){window.present(&fb);}
        }
        // Recycle the large native-resolution triangle stream without a new
        // allocation on the next frame. Do this even after a GPU failure.
        #[cfg(target_os="windows")]
        if let Some(frame)=gpu_scene {pool.recycle_gpu_frame(frame);}
        if cpu_profile && !gpu_active && frame_counter%120==0 {
            eprintln!("[Neon Run CPU] {}x{} | {} lanes | capture {:.2} ms | band raster {:.2} ms | total scene {:.2} ms | {} projected triangles | {} painted pixels",
                if render_low{low.width}else{fb.width},
                if render_low{low.height}else{fb.height},pool.active,
                pool.last_capture.as_secs_f64()*1000.0,
                pool.last_raster.as_secs_f64()*1000.0,
                scene_time.as_secs_f64()*1000.0,tris,pixels);
        }
        let render_ms=scene_time.as_secs_f32()*1000.0;
        smoothed_render_ms=smoothed_render_ms*0.92+render_ms*0.08;
        frame_counter+=1;
        if active_scene && frame_counter%40==0 && quality_mode==0 {
            let ms_budget=controller.frame_budget().as_secs_f32()*1000.0;
            if smoothed_render_ms>ms_budget*0.85 {adaptive_scale=(adaptive_scale-0.07).max(0.40);}
            else if smoothed_render_ms<ms_budget*0.50 {adaptive_scale=(adaptive_scale+0.045).min(0.85);}
        }
        if active_scene {
            // Render-only cost is used for scaling worker lanes: VSync/present
            // latency must not spuriously create more render threads.
            if !gpu_active {
                controller.sample(&mut pool,scene_time,frame_start.elapsed());
            }
        }
        // Read elapsed time only once. Two calls can straddle the frame
        // deadline, making `budget - frame_start.elapsed()` underflow and
        // panic (even though the earlier comparison was true).
        if active_scene {
            let remaining=remaining_frame_budget(controller.frame_budget(),frame_start.elapsed());
            if !remaining.is_zero() {
                std::thread::sleep(remaining);
            }
        }
        if active_scene {
            smoothed_frame_s=smoothed_frame_s*0.93+frame_start.elapsed().as_secs_f32().max(0.001)*0.07;
        }
    }
}
#[cfg(target_os="windows")]
fn cw_prev(window:&Window)->(u32,u32){let (w,h)=window.client_size();(w.max(1),h.max(1))}
#[cfg(target_os="windows")]
fn gpu_backend_vendor(vendor:u32)->&'static str {
    match vendor {0x8086=>"Intel",0x10de=>"NVIDIA",0x1002|0x1022=>"AMD",_=>"Other"}
}
/// Return the time remaining in the target frame without panicking after an
/// overrun.  This pure helper also makes the deadline edge case testable.
fn remaining_frame_budget(budget:Duration,elapsed:Duration)->Duration {
    budget.saturating_sub(elapsed)
}

#[cfg(test)]
mod frame_pacing_tests {
    use super::*;

    #[test]
    fn under_budget_sleeps_only_the_difference() {
        assert_eq!(remaining_frame_budget(Duration::from_millis(16),
            Duration::from_millis(10)),Duration::from_millis(6));
    }

    #[test]
    fn exactly_at_budget_does_not_sleep() {
        assert_eq!(remaining_frame_budget(Duration::from_millis(16),
            Duration::from_millis(16)),Duration::ZERO);
    }

    #[test]
    fn over_budget_does_not_panic_or_sleep() {
        assert_eq!(remaining_frame_budget(Duration::from_millis(16),
            Duration::from_millis(17)),Duration::ZERO);
    }
}

fn main(){run(Window::new(WINDOW_TITLE,(1100,700)));}
