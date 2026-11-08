//! Render a 3D box to an ordinary PPM file with no window or GPU.
//! Run: cargo run -p mata-3d --example offscreen_cube --release
use mata_3d::{Camera, Material, Mesh, Rasterizer, Transform, Vec3};
use mata_win::Framebuffer;
use std::{fs::File, io::{BufWriter, Write}};
fn main()->std::io::Result<()> {
    let mut fb=Framebuffer::from_color(640,360,0xff101b2b);
    let mut renderer=Rasterizer::new();
    let camera=Camera::new(Vec3::new(0.0,1.5,-4.0),Vec3::new(0.0,-0.07,1.0),Vec3::new(1.0,0.0,0.0));
    renderer.begin_frame(&fb,camera);
    let mut model=Transform::at_scale(Vec3::new(0.0,1.0,2.0),2.0);
    model.rotation=Vec3::new(0.22,0.5,0.08);
    renderer.mesh(&mut fb,&Mesh::cube(),model,Material::lit(0xff36dcfa));
    let mut file=BufWriter::new(File::create("mata3d-demo.ppm")?);
    writeln!(file,"P6\n{} {}\n255",fb.width,fb.height)?;
    for p in fb.pixels {
        file.write_all(&[((p>>16)&0xff) as u8,((p>>8)&0xff) as u8,(p&0xff) as u8])?;
    }
    eprintln!("wrote mata3d-demo.ppm ({} triangles)",renderer.stats.triangles);
    file.flush()
}
