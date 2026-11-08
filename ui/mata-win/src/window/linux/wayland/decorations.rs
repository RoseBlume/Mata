//! Standard-library-only Wayland client-side titlebar fallback.
//! The 34-pixel strip is outside the caller's content coordinate system.

use crate::{buffer::Framebuffer, window::DirtyRect};

pub(super) const HEIGHT: u32 = 34;
const CONTROL_WIDTH: i32 = 44;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum Action { None, Move, Minimize, Maximize, Close }

pub(super) fn hit_test(x: i32, y: i32, width: u32, minimize: bool, maximize: bool) -> Action {
    if x < 0 || y < 0 || y >= HEIGHT as i32 || x >= width as i32 {return Action::None;}
    let right = width as i32;
    if x >= right - CONTROL_WIDTH { Action::Close }
    else if x >= right - CONTROL_WIDTH * 2 {
        if maximize { Action::Maximize } else { Action::None }
    }
    else if x >= right - CONTROL_WIDTH * 3 {
        if minimize { Action::Minimize } else { Action::None }
    }
    else { Action::Move }
}

fn pixel(frame: &mut Framebuffer, x: i32, y: i32, color: u32) {
    if x >= 0 && y >= 0 && (x as u32) < frame.width && (y as u32) < frame.height {
        frame.pixels[y as usize * frame.width as usize + x as usize] = color;
    }
}
fn line(frame: &mut Framebuffer, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: u32) {
    let dx=(x1-x0).abs(); let sx=if x0<x1 {1} else {-1};
    let dy=-(y1-y0).abs(); let sy=if y0<y1 {1} else {-1};
    let mut err=dx+dy;
    loop {
        pixel(frame,x0,y0,color);
        if x0==x1 && y0==y1 { break; }
        let e=err*2;
        if e>=dy { err+=dy; x0+=sx; }
        if e<=dx { err+=dx; y0+=sy; }
    }
}

fn glyph(ch: char) -> [u8;7] {
    // Five-column bitmap font; unknown glyphs are rendered as a small dot.
    match ch.to_ascii_uppercase() {
        'A'=>[14,17,17,31,17,17,17], 'B'=>[30,17,17,30,17,17,30],
        'C'=>[14,17,16,16,16,17,14], 'D'=>[30,17,17,17,17,17,30],
        'E'=>[31,16,16,30,16,16,31], 'F'=>[31,16,16,30,16,16,16],
        'G'=>[14,17,16,23,17,17,14], 'H'=>[17,17,17,31,17,17,17],
        'I'=>[31,4,4,4,4,4,31], 'J'=>[7,2,2,2,18,18,12],
        'K'=>[17,18,20,24,20,18,17], 'L'=>[16,16,16,16,16,16,31],
        'M'=>[17,27,21,21,17,17,17], 'N'=>[17,25,21,19,17,17,17],
        'O'=>[14,17,17,17,17,17,14], 'P'=>[30,17,17,30,16,16,16],
        'Q'=>[14,17,17,17,21,18,13], 'R'=>[30,17,17,30,20,18,17],
        'S'=>[15,16,16,14,1,1,30], 'T'=>[31,4,4,4,4,4,4],
        'U'=>[17,17,17,17,17,17,14], 'V'=>[17,17,17,17,17,10,4],
        'W'=>[17,17,17,21,21,21,10], 'X'=>[17,17,10,4,10,17,17],
        'Y'=>[17,17,10,4,4,4,4], 'Z'=>[31,1,2,4,8,16,31],
        '0'=>[14,17,19,21,25,17,14], '1'=>[4,12,4,4,4,4,14],
        '2'=>[14,17,1,2,4,8,31], '3'=>[30,1,1,14,1,1,30],
        '4'=>[2,6,10,18,31,2,2], '5'=>[31,16,30,1,1,17,14],
        '6'=>[6,8,16,30,17,17,14], '7'=>[31,1,2,4,4,4,4],
        '8'=>[14,17,17,14,17,17,14], '9'=>[14,17,17,15,1,2,12],
        '-'=>[0,0,0,31,0,0,0], '_'=>[0,0,0,0,0,0,31],
        '.'=>[0,0,0,0,0,12,12], ':'=>[0,12,12,0,12,12,0],
        ' '=>[0;7], _=>[0,0,0,4,0,0,0],
    }
}

/// Copies only the changed application pixels below the titlebar. The caller
/// supplies content-coordinate damage; the result uses surface coordinates.
pub(super) fn update_content(dst: &mut Framebuffer, content: &Framebuffer,
    dirty: &[DirtyRect]) -> Vec<DirtyRect> {
    let mut mapped = Vec::with_capacity(dirty.len());
    for region in dirty {
        let Some(region)=region.clipped(content.width,content.height) else {continue};
        let x=region.x as usize;
        let w=region.width as usize;
        for row in region.y as usize..(region.y+region.height) as usize {
            let from=row*content.width as usize+x;
            let to=(row+HEIGHT as usize)*dst.width as usize+x;
            dst.pixels[to..to+w].copy_from_slice(&content.pixels[from..from+w]);
        }
        mapped.push(DirtyRect::new(region.x,region.y+HEIGHT,
            region.width,region.height));
    }
    mapped
}

/// Redraws only the titlebar. The caller maintains content in the same buffer.
pub(super) fn draw_title(dst: &mut Framebuffer,
    title: &str, maximized: bool, can_maximize: bool, can_minimize: bool) {
    let width=dst.width as usize;
    if width==0 || dst.height < HEIGHT {return;}
    for row in dst.pixels.chunks_exact_mut(width).take(HEIGHT as usize).enumerate() {
        let (y,row)=row;
        row.fill(if y==HEIGHT as usize-1 {0xff353945} else {0xff242832});
    }
    let title_chars=title.chars().take(((dst.width.saturating_sub(160))/6) as usize);
    for (index,ch) in title_chars.enumerate() {
        let gx=12+(index as i32)*6;
        for (gy,bits) in glyph(ch).iter().copied().enumerate() {
            for bit in 0..5 {
                if bits & (1 << (4-bit)) != 0 {
                    pixel(dst,gx+bit,13+gy as i32,0xffe4e7ed);
                }
            }
        }
    }
    let right=dst.width as i32;
    let centers=[right-110,right-66,right-22];
    if can_minimize {line(dst,centers[0]-6,22,centers[0]+6,22,0xffe4e7ed);}
    if can_maximize {
        let x=centers[1];
        if maximized {
            for dy in 0..9 {pixel(dst,x-4,12+dy,0xffe4e7ed);pixel(dst,x+4,12+dy,0xffe4e7ed);}
            line(dst,x-4,12,x+4,12,0xffe4e7ed);
            line(dst,x-4,20,x+4,20,0xffe4e7ed);
        } else {
            line(dst,x-6,12,x+6,12,0xffe4e7ed);
            line(dst,x-6,21,x+6,21,0xffe4e7ed);
            line(dst,x-6,12,x-6,21,0xffe4e7ed);
            line(dst,x+6,12,x+6,21,0xffe4e7ed);
        }
    }
    let x=centers[2];
    line(dst,x-5,12,x+5,22,0xffffd8d8);
    line(dst,x-5,22,x+5,12,0xffffd8d8);
}

/// Full content and titlebar repaint, used for tests and initial mapping.
#[allow(dead_code)]
pub(super) fn draw(dst: &mut Framebuffer, content: &Framebuffer,
    title: &str, maximized: bool, can_maximize: bool, can_minimize: bool) {
    update_content(dst,content,&[DirtyRect::new(0,0,content.width,content.height)]);
    draw_title(dst,title,maximized,can_maximize,can_minimize);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hit_testing_controls_and_client() {
        assert_eq!(hit_test(10,10,600,true,true),Action::Move);
        assert_eq!(hit_test(480,10,600,true,true),Action::Minimize);
        assert_eq!(hit_test(525,10,600,true,true),Action::Maximize);
        assert_eq!(hit_test(585,10,600,true,true),Action::Close);
        assert_eq!(hit_test(585,HEIGHT as i32,600,true,true),Action::None);
        assert_eq!(hit_test(525,10,600,true,false),Action::None);
    }
    #[test]
    fn decorations_never_overwrite_document_pixels() {
        let content=Framebuffer::from_color(400,50,0xffaabbcc);
        let mut dst=Framebuffer::new(400,50+HEIGHT);
        draw(&mut dst,&content,"Mata Document",false,true,true);
        assert_eq!(&dst.pixels[(HEIGHT*400) as usize..],content.as_slice());
    }
}
