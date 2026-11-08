//! Real CPU-rasterized rotating 3D mesh using only MataWin for the window.
use std::time::{Duration,Instant};
use mata_win::{Framebuffer,Window,events::{WindowEvent,Key}};
use mata_3d::{Vec3,Mesh,Camera,Instance,SoftwareRenderer};
fn main(){
    let mut win=Window::new("mata-3d / rotating mesh",(900,600));
    let mut fb=Framebuffer::new(900,600);
    let mut renderer=SoftwareRenderer::new();
    let cube=Mesh::cube(0xff71cfee);
    let crystal=Mesh::octahedron(0xffffba55);
    let prism=Mesh::from_obj_str(
        include_str!("../assets/models/energy_cartridge.obj"),0xffb588fa)
        .unwrap_or_else(|_|Mesh::prism(8,0xffb588fa));
    let mut events=Vec::new();let start=Instant::now();
    loop {
        win.poll_events_into(&mut events);
        if events.iter().any(|e|matches!(e,WindowEvent::Close|WindowEvent::KeyPressed(Key::Escape))){break;}
        let (w,h)=win.client_size();if w==0||h==0{std::thread::sleep(Duration::from_millis(30));continue;}
        if fb.width!=w||fb.height!=h{fb.resize_and_clear(w,h,0xff091b30);}
        fb.pixels.fill(0xff081622);
        let t=start.elapsed().as_secs_f32();
        let camera=Camera::raycast(Vec3::new(0.0,0.0,0.7),0.0,0.66);
        renderer.begin_frame(&fb);
        renderer.render(&mut fb,&camera,[
            Instance::new(&cube,Vec3::new(4.0,-1.3,0.7)).rotated(t),
            Instance::new(&crystal,Vec3::new(4.0,0.0,0.7)).rotated(-t*1.2),
            Instance::new(&prism,Vec3::new(4.0,1.3,0.7)).rotated(t*0.6),
        ]);
        win.present(&fb);
        std::thread::sleep(Duration::from_millis(14));
    }
}
