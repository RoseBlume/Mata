//! Illustrative SARS-CoV-2 virion geometry and provenance-aware component browser.
//!
//! Nothing generated here is an experimental cryo-EM reconstruction. Real density
//! maps and experimentally determined protein structures are loaded separately.
use mata_3d::Vec3;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViralComponent { Envelope, Spike, MembraneProtein, EnvelopeProtein, Nucleocapsid, Rna }

impl ViralComponent {
    pub const ALL: [Self; 6] = [Self::Envelope,Self::Spike,Self::MembraneProtein,Self::EnvelopeProtein,Self::Nucleocapsid,Self::Rna];
    pub fn name(self)->&'static str {match self {
        Self::Envelope=>"Lipid envelope",Self::Spike=>"Spike (S)",Self::MembraneProtein=>"Membrane (M)",
        Self::EnvelopeProtein=>"Envelope (E)",Self::Nucleocapsid=>"Nucleocapsid (N)",Self::Rna=>"RNA genome",
    }}
    pub fn description(self)->&'static str {match self {
        Self::Envelope=>"Host-derived lipid membrane, illustrated as a wire shell",
        Self::Spike=>"Surface glycoprotein; trimers are represented schematically",
        Self::MembraneProtein=>"Abundant viral membrane protein; locations are illustrative",
        Self::EnvelopeProtein=>"Small envelope protein; low-copy schematic placement",
        Self::Nucleocapsid=>"RNA-binding protein surrounding the genome",
        Self::Rna=>"Positive-sense RNA genome, drawn as a stylized coil",
    }}
    pub fn color(self)->u32 {match self {
        Self::Envelope=>0xff42c9e6,Self::Spike=>0xffff819c,Self::MembraneProtein=>0xffffca67,
        Self::EnvelopeProtein=>0xff82e8a1,Self::Nucleocapsid=>0xffbb9aff,Self::Rna=>0xff6ee3b5,
    }}
    pub fn reference(self)->&'static str {match self {
        Self::Spike=>"6VXX (S trimer, experimental PDB)",
        Self::Nucleocapsid=>"6M3M (N N-terminal domain, experimental PDB)",
        Self::Envelope=>"Virion EM density; no atomic coordinates inferred",
        Self::MembraneProtein=>"Load a verified M protein structure separately",
        Self::EnvelopeProtein=>"Load a verified E protein structure separately",
        Self::Rna=>"No whole-genome atomic model is implied",
    }}
    pub fn accepts_protein(self)->bool {matches!(self,Self::Spike|Self::MembraneProtein|Self::EnvelopeProtein|Self::Nucleocapsid)}
}

#[derive(Clone,Debug)]
pub struct VirusParticle {pub component: ViralComponent,pub position:Vec3,pub radius:f32,pub direction:Vec3}

#[derive(Clone,Debug)]
pub struct VirusModel {
    pub name:String,
    /// Illustrative virion radius in nanometres. Not a measurement for a particular virion.
    pub radius_nm:f32,
    pub components:Vec<VirusParticle>,
    pub visible:[bool;6],
    pub cutaway:bool,
}
impl VirusModel {
    pub fn sars_cov_2()->Self {
        let mut components=Vec::new();
        // Golden-angle sphere distributions avoid arbitrary overlap and are fully deterministic.
        for (kind,count,rad,offset) in [
            (ViralComponent::Spike,54usize,11.0f32,0.0f32),
            (ViralComponent::MembraneProtein,98,9.65,0.35),
            (ViralComponent::EnvelopeProtein,12,9.65,0.7),
            (ViralComponent::Nucleocapsid,58,6.0,0.2),
        ] {
            for index in 0..count {
                let t=(index as f32+0.5)/count as f32;
                let y=1.0-2.0*t;
                let r=(1.0-y*y).sqrt();
                let a=index as f32*PI*(3.0-5.0f32.sqrt())+offset*TAU;
                let direction=Vec3::new(a.cos()*r,y,a.sin()*r);
                components.push(VirusParticle {component:kind,position:direction*rad,
                    radius:match kind {ViralComponent::Spike=>1.1,ViralComponent::Nucleocapsid=>0.55,_=>0.40},direction});
            }
        }
        Self{name:"SARS-CoV-2 (illustrative assembly)".into(),radius_nm:50.0,
            components,visible:[true;6],cutaway:true}
    }
    pub fn is_visible(&self,component:ViralComponent)->bool {
        self.visible[Self::component_index(component)]
    }
    pub fn toggle(&mut self,component:ViralComponent){let i=Self::component_index(component);self.visible[i]=!self.visible[i];}
    pub fn component_index(component:ViralComponent)->usize {
        ViralComponent::ALL.iter().position(|&x|x==component).unwrap_or(0)
    }
    pub fn count(&self,component:ViralComponent)->usize {
        self.components.iter().filter(|p|p.component==component).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn every_kind_has_name_and_color(){for kind in ViralComponent::ALL {assert!(!kind.name().is_empty());assert_ne!(kind.color(),0);}}
    #[test]fn virion_is_deterministic_and_toggleable(){let mut a=VirusModel::sars_cov_2();let b=VirusModel::sars_cov_2();assert_eq!(a.count(ViralComponent::Spike),54);assert_eq!(a.count(ViralComponent::Nucleocapsid),58);
        assert_eq!(a.components.len(),b.components.len());assert_eq!(a.components[0].position,b.components[0].position);
        a.toggle(ViralComponent::Spike);assert!(!a.is_visible(ViralComponent::Spike));assert!(a.is_visible(ViralComponent::Envelope));}
    #[test]fn only_protein_components_accept_pdb(){assert!(ViralComponent::Spike.accepts_protein());assert!(!ViralComponent::Rna.accepts_protein());}
    #[test]fn synthetic_mrc_volume_can_be_loaded_for_experimental_mode(){
        let example=std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"),"/../samples/synthetic-cell-density.mrc"));
        let volume=crate::cell::load_volume(example).expect("synthetic MRC must import");
        assert!(volume.dims.iter().all(|&d|d>0));
        assert!(volume.min_max().1>volume.min_max().0);
    }
}
