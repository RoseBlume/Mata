//! Source-only regression cases. These are intentionally not run by the build
//! process; run `cargo test` after integrating mata-fonts locally.
use mata_docx::{glyph_variations::{GlyphOutline,OutlinePoint},truetype_hinting::{self,HintLimits}};

fn hinted_outline(program:&[u8],x:f32,y:f32)->GlyphOutline{
    let outline=GlyphOutline{points:vec![OutlinePoint{x,y,on_curve:true}],contours:vec![0]};
    let mut head=vec![0;54];head[50..52].copy_from_slice(&1u16.to_be_bytes());
    let mut glyf=vec![0;10];glyf[0..2].copy_from_slice(&1u16.to_be_bytes());
    glyf.extend_from_slice(&[0,0]);
    glyf.extend_from_slice(&(program.len() as u16).to_be_bytes());
    glyf.extend_from_slice(program);
    let mut loca=vec![0;8];loca[4..8].copy_from_slice(&(glyf.len() as u32).to_be_bytes());
    truetype_hinting::hint_outline(&outline,&head,&loca,&glyf,0,None,None,None,None,
        10.0,1000,None,HintLimits::default()).expect("hinting simple synthetic glyph")
}

#[test]
fn move_direct_absolute_point_to_grid(){
    // PUSHB 0; MDAP[round]
    let glyph=hinted_outline(&[0xB0,0,0x2F],60.,0.);
    assert!((glyph.points[0].x-100.).abs()<0.01);
}
#[test]
fn arithmetic_and_control_flow_are_stack_bounded(){
    // PUSHB 0; PUSHB 0; DIV should reject a zero divisor.
    let outline=GlyphOutline{points:vec![],contours:vec![]};
    let mut head=vec![0;54];head[50..52].copy_from_slice(&1u16.to_be_bytes());
    let mut glyf=vec![0;10];glyf.extend_from_slice(&[0,0,0,5,0xB1,0,0,0x62,0x21]);
    let mut loca=vec![0;8];loca[4..8].copy_from_slice(&(glyf.len() as u32).to_be_bytes());
    assert!(truetype_hinting::hint_outline(&outline,&head,&loca,&glyf,0,None,None,None,None,
        12.,1000,None,HintLimits::default()).is_err());
}

#[test]
fn flip_point_updates_outline_flags() {
    let glyph=hinted_outline(&[0xB0,0,0x80],120.,0.);
    assert!(!glyph.points[0].on_curve);
}
#[test]
fn flip_range_changes_outline_flags() {
    let glyph=hinted_outline(&[0xB1,0,0,0x82],120.,0.);
    assert!(!glyph.points[0].on_curve);
    let glyph=hinted_outline(&[0xB1,0,0,0x81],120.,0.);
    assert!(glyph.points[0].on_curve);
}
