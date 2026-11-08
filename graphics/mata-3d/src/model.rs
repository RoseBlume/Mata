//! Multi-material Wavefront models for the Mata software rasterizer.
//!
//! OBJ uses `v`, `f`, and `usemtl` (triangles, quads and polygons). The
//! corresponding MTL uses `newmtl`, `Kd` and optional `Ke`. Geometry is split
//! into compact indexed submeshes by material, so a model can combine opaque
//! armour with emissive glass, lights and weapon coils. No runtime I/O occurs.
use std::collections::HashMap;
use mata_win::Framebuffer;
use crate::{Material,Mesh,Rasterizer,Transform,Vec3};

#[derive(Debug,Clone)]
pub struct ModelPart {
    pub name: String,
    pub mesh: Mesh,
    pub material: Material,
}

#[derive(Debug,Clone,Default)]
pub struct Model {
    pub parts: Vec<ModelPart>,
}

#[derive(Clone,Copy)]
struct PaletteEntry { diffuse:u32, emissive:bool }

fn color_from_rgb(components:&[&str], line:usize)->Result<u32,String> {
    if components.len()<3 {return Err(format!("line {line}: expected three color components"));}
    let mut rgb=[0u32;3];
    for (dst,v) in rgb.iter_mut().zip(components.iter().take(3)) {
        let parsed=v.parse::<f32>().map_err(|_|format!("line {line}: invalid MTL colour value"))?;
        if !parsed.is_finite() {return Err(format!("line {line}: nonfinite MTL colour value"));}
        *dst=(parsed.clamp(0.0,1.0)*255.0).round() as u32;
    }
    Ok(0xff000000|(rgb[0]<<16)|(rgb[1]<<8)|rgb[2])
}
fn read_palette(mtl:&str)->Result<HashMap<String,PaletteEntry>,String> {
    let mut palette=HashMap::new();
    let mut name:Option<String>=None;
    let mut current=PaletteEntry{diffuse:0xff99aebe,emissive:false};
    for (index,line) in mtl.lines().enumerate() {
        let mut tokens=line.split_whitespace();
        let Some(kind)=tokens.next() else {continue};
        match kind {
            "newmtl"=>{
                if let Some(previous)=name.take(){palette.insert(previous,current);}
                name=Some(tokens.next().ok_or_else(||format!("line {}: missing material name",index+1))?.to_string());
                current=PaletteEntry{diffuse:0xff99aebe,emissive:false};
            }
            "Kd" if name.is_some()=>{current.diffuse=color_from_rgb(&tokens.collect::<Vec<_>>(),index+1)?;}
            "Ke" if name.is_some()=>{
                let emission=color_from_rgb(&tokens.collect::<Vec<_>>(),index+1)?;
                current.emissive=(emission&0xffffff)!=0;
            }
            _=>{},
        }
    }
    if let Some(previous)=name {palette.insert(previous,current);}
    Ok(palette)
}
impl Model {
    /// Parse an OBJ + its MTL palette. OBJ normals/UV coordinates are ignored:
    /// the software renderer already computes flat lighting from triangle faces.
    pub fn from_obj_mtl(obj:&str,mtl:&str)->Result<Self,String> {
        let palette=read_palette(mtl)?;
        let mut vertices=Vec::<Vec3>::new();
        let mut groups=Vec::<(String,Vec<[u32;3]>)>::new();
        let mut slots=HashMap::<String,usize>::new();
        let mut material=String::from("default");
        for (line_index,line) in obj.lines().enumerate(){
            let mut words=line.split_whitespace();
            let Some(kind)=words.next() else {continue};
            match kind {
                "v"=>{
                    let mut values=[0.0f32;3];
                    for x in &mut values {
                        *x=words.next().ok_or_else(||format!("line {}: short vertex",line_index+1))?
                            .parse().map_err(|_|format!("line {}: bad vertex",line_index+1))?;
                        if !x.is_finite(){return Err(format!("line {}: nonfinite vertex",line_index+1));}
                    }
                    vertices.push(Vec3::new(values[0],values[1],values[2]));
                }
                "usemtl"=>{
                    material=words.next().ok_or_else(||format!("line {}: missing material name",line_index+1))?.to_owned();
                }
                "f"=>{
                    let mut face=Vec::<u32>::new();
                    for vertex in words.take_while(|w|!w.starts_with('#')) {
                        let raw=vertex.split('/').next().unwrap_or("");
                        let n=raw.parse::<i64>().map_err(|_|format!("line {}: bad face index",line_index+1))?;
                        let normalized=if n>0{n-1}else if n<0{vertices.len() as i64+n}else{-1};
                        if normalized<0||normalized>=vertices.len() as i64 {return Err(format!("line {}: out-of-bounds vertex",line_index+1));}
                        face.push(normalized as u32);
                    }
                    if face.len()<3{return Err(format!("line {}: face needs 3 vertices",line_index+1));}
                    let slot=*slots.entry(material.clone()).or_insert_with(||{
                        let id=groups.len();groups.push((material.clone(),Vec::new()));id
                    });
                    for i in 1..face.len()-1{groups[slot].1.push([face[0],face[i],face[i+1]]);}
                }
                _=>{},
            }
        }
        let mut parts=Vec::with_capacity(groups.len());
        for (name,triangles) in groups {
            let mut index_map=HashMap::<u32,u32>::new();
            let mut part_verts=Vec::new();
            let mut local_tris=Vec::with_capacity(triangles.len());
            for tri in triangles {
                let mut converted=[0u32;3];
                for (dst,global) in converted.iter_mut().zip(tri) {
                    *dst=*index_map.entry(global).or_insert_with(||{
                        let local=part_verts.len() as u32;
                        part_verts.push(vertices[global as usize]);local
                    });
                }
                local_tris.push(converted);
            }
            let swatch=palette.get(&name).copied().unwrap_or(PaletteEntry{diffuse:0xff99aebe,emissive:false});
            parts.push(ModelPart{name,mesh:Mesh::new(part_verts,local_tris)?,
                material:Material::two_sided(swatch.diffuse,swatch.emissive)});
        }
        if parts.is_empty() {return Err("OBJ model has no polygon faces".into());}
        Ok(Self{parts})
    }
    #[inline]pub fn triangle_count(&self)->usize{
        self.parts.iter().map(|p|p.mesh.triangles.len()).sum()
    }
    /// World-space mesh with part-wise material shading; hit flash overrides
    /// are shared between parts without altering the underlying model.
    pub fn draw_world(&self,raster:&mut Rasterizer,fb:&mut Framebuffer,transform:Transform,override_material:Option<Material>){
        for part in &self.parts {
            raster.mesh(fb,&part.mesh,transform,override_material.unwrap_or(part.material));
        }
    }
    /// Camera-space mesh for a first-person weapon. This ignores the world
    /// camera matrix and uses the same per-pixel depth test as normal meshes.
    pub fn draw_view(&self,raster:&mut Rasterizer,fb:&mut Framebuffer,transform:Transform){
        for part in &self.parts {raster.mesh_view(fb,&part.mesh,transform,part.material);}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn import_part_colors_quad_and_negative_indices() {
        let obj="v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nusemtl hull\nf 1 2 3 4\nusemtl lamp\nf -4 -3 -2\n";
        let mtl="newmtl hull\nKd 0.2 0.4 0.6\nnewmtl lamp\nKd 1 0.5 0\nKe 0.1 0 0\n";
        let model=Model::from_obj_mtl(obj,mtl).unwrap();
        assert_eq!(model.parts.len(),2);
        assert_eq!(model.triangle_count(),3);
        assert_eq!(model.parts[0].material.color,0xff336699);
        assert!(model.parts[1].material.emissive);
    }
    #[test] fn invalid_obj_is_not_silently_accepted(){
        assert!(Model::from_obj_mtl("v 0 0 0\nf 1 2 3","").is_err());
        assert!(Model::from_obj_mtl("v 0 0 0","").is_err());
    }
}
