//! Windows system-font rasterizer for the dependency-reduced Mata Document build.
//! Renders glyphs into an in-memory DIB and composites the pixels onto Mata's
//! software framebuffer. It never draws UI text into the application's HWND.
//! No font files are distributed and no browser HTML/CSS participates in drawing.

use std::{ffi::c_void, path::Path, ptr};
use crate::Framebuffer;
use super::{Rect, WidgetError, style::{FontStyle, FontVariant, FontWeight, FontStretch}};

type Handle = *mut c_void;
#[repr(C)] struct Size { cx:i32, cy:i32 }
#[repr(C)] struct BitmapInfoHeader {
    size:u32, width:i32, height:i32, planes:u16, bit_count:u16, compression:u32,
    size_image:u32, x_pels_per_meter:i32, y_pels_per_meter:i32, clr_used:u32, clr_important:u32,
}
#[repr(C)] struct RgbQuad { blue:u8, green:u8, red:u8, reserved:u8 }
#[repr(C)] struct BitmapInfo { header:BitmapInfoHeader, colors:[RgbQuad;1] }
#[link(name="gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(dc:Handle)->Handle;
    fn DeleteDC(dc:Handle)->i32;
    fn CreateFontW(height:i32,width:i32,escapement:i32,orientation:i32,weight:i32,
        italic:u32,underline:u32,strikeout:u32,charset:u32,out_precision:u32,
        clip_precision:u32,quality:u32,pitch_and_family:u32,face:*const u16)->Handle;
    fn SelectObject(dc:Handle,object:Handle)->Handle;
    fn DeleteObject(object:Handle)->i32;
    fn GetTextExtentPoint32W(dc:Handle,text:*const u16,len:i32,size:*mut Size)->i32;
    fn CreateDIBSection(dc:Handle,info:*const BitmapInfo,usage:u32,
        bits:*mut *mut c_void,section:Handle,offset:u32)->Handle;
    fn SetBkColor(dc:Handle,color:u32)->u32;
    fn SetTextColor(dc:Handle,color:u32)->u32;
    fn SetBkMode(dc:Handle,mode:i32)->i32;
    fn TextOutW(dc:Handle,x:i32,y:i32,text:*const u16,len:i32)->i32;
    fn GdiFlush()->i32;
}
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(std::iter::once(0)).collect()}
fn weight_value(w:FontWeight)->i32 {
    match w { FontWeight::Thin=>100,FontWeight::ExtraLight=>200,FontWeight::Light=>300,
    FontWeight::Normal=>400,FontWeight::Medium=>500,FontWeight::SemiBold=>600,
    FontWeight::Bold=>700,FontWeight::ExtraBold=>800,FontWeight::Black=>900,
    FontWeight::Numeric(n)=>n as i32 }
}

struct Context { dc:Handle, font:Handle, old:Handle }
impl Context {
    fn new(family:&str,size:u16,style:FontStyle,weight:FontWeight,stretch:FontStretch)->Result<Self,WidgetError> {
        let width = match stretch {FontStretch::UltraCondensed=>-4,FontStretch::ExtraCondensed=>-3,FontStretch::Condensed=>-2,FontStretch::SemiCondensed=>-1,FontStretch::Normal=>0,FontStretch::SemiExpanded=>1,FontStretch::Expanded=>2,FontStretch::ExtraExpanded=>3,FontStretch::UltraExpanded=>4};
        let dc=unsafe{CreateCompatibleDC(ptr::null_mut())};
        if dc.is_null(){return Err(WidgetError("GDI CreateCompatibleDC failed".into()));}
        let font=unsafe{CreateFontW(-(size as i32),width,0,0,weight_value(weight),
            u32::from(style!=FontStyle::Normal),0,0,1,0,0,5,0,wide(family).as_ptr())};
        if font.is_null(){unsafe{DeleteDC(dc);};return Err(WidgetError("GDI CreateFontW failed".into()));}
        let old=unsafe{SelectObject(dc,font)};
        Ok(Self{dc,font,old})
    }
    fn size(&self,s:&str)->(u32,u32){
        let u:Vec<u16>=s.encode_utf16().collect();
        if u.is_empty(){return (0,0)}
        let mut result=Size{cx:0,cy:0};
        unsafe{GetTextExtentPoint32W(self.dc,u.as_ptr(),u.len().min(i32::MAX as usize) as i32,&mut result)};
        (result.cx.max(0) as u32,result.cy.max(0) as u32)
    }
}
impl Drop for Context {fn drop(&mut self){unsafe{SelectObject(self.dc,self.old);DeleteObject(self.font);DeleteDC(self.dc);}}}

#[derive(Clone,Debug)]
pub struct FontRenderer {family:String}
impl FontRenderer {
    pub fn from_bytes(_bytes:&[u8])->Result<Self,WidgetError>{Err(WidgetError("Use a Windows-installed font by family name".into()))}
    pub fn from_file(_path:impl AsRef<Path>)->Result<Self,WidgetError>{Err(WidgetError("Use a Windows-installed font by family name".into()))}
    pub fn system_default()->Result<Self,WidgetError>{Ok(Self{family:"Segoe UI".into()})}
    pub fn system_family(family:&str,_style:FontStyle,_weight:FontWeight)->Result<Self,WidgetError>{
        Ok(Self{family:if family.trim().is_empty(){"Segoe UI".into()}else{family.into()}})
    }
    pub fn source(&self)->Option<&Path>{None}
    pub fn measure(&mut self,text:&str,pt:u16)->Result<(u32,u32),WidgetError>{
        self.measure_styled(text,pt,FontVariant::Normal,FontWeight::Normal,FontStretch::Normal,None)
    }
    pub fn measure_styled(&mut self,text:&str,pt:u16,variant:FontVariant,weight:FontWeight,stretch:FontStretch,line_height:Option<u32>)->Result<(u32,u32),WidgetError>{
        if pt==0{return Ok((0,0));}
        let ctx=Context::new(&self.family,pt,FontStyle::Normal,weight,stretch)?;
        let mut width=0;let mut height=0;
        for line in text.split('\n') {
            let line=if variant==FontVariant::SmallCaps {line.to_uppercase()} else {line.to_owned()};
            let (w,h)=ctx.size(&line);width=width.max(w);height+=line_height.unwrap_or(h.max(pt as u32));
        }
        Ok((width,height))
    }
    pub fn draw_text(&mut self,target:&mut Framebuffer,text:&str,x:i32,y:i32,pt:u16,color:u32,clip:Option<Rect>)->Result<(),WidgetError>{
        self.draw_text_styled(target,text,x,y,pt,color,clip,FontStyle::Normal,FontVariant::Normal,FontWeight::Normal,FontStretch::Normal,None)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_styled(&mut self,target:&mut Framebuffer,text:&str,x:i32,y:i32,pt:u16,color:u32,clip:Option<Rect>,style:FontStyle,variant:FontVariant,weight:FontWeight,stretch:FontStretch,line_height:Option<u32>)->Result<(),WidgetError>{
        if text.is_empty()||pt==0{return Ok(());} let ctx=Context::new(&self.family,pt,style,weight,stretch)?;
        let clip=clip.unwrap_or(Rect::new(0,0,target.width,target.height));
        let step=line_height.unwrap_or(pt as u32+5).max(1) as i32;
        for (li,raw) in text.split('\n').enumerate(){
            let text=if variant==FontVariant::SmallCaps{raw.to_uppercase()}else{raw.to_owned()};
            let (w,h)=ctx.size(&text);
            let w=w.min(8192);let h=h.min(512);
            if w==0||h==0{continue;}
            let ox=x;let oy=y.saturating_add(step.saturating_mul(li.min(i32::MAX as usize) as i32));
            if oy>=clip.y.saturating_add(clip.height as i32){break;}
            let info=BitmapInfo{header:BitmapInfoHeader{size:std::mem::size_of::<BitmapInfoHeader>() as u32,
                width:w as i32,height:-(h as i32),planes:1,bit_count:32,compression:0,size_image:0,
                x_pels_per_meter:0,y_pels_per_meter:0,clr_used:0,clr_important:0},
                colors:[RgbQuad{blue:0,green:0,red:0,reserved:0}]};
            let mut bits:Handle=ptr::null_mut();
            let bitmap=unsafe{CreateDIBSection(ctx.dc,&info,0,&mut bits,ptr::null_mut(),0)};
            if bitmap.is_null()||bits.is_null(){continue;}
            let previous=unsafe{SelectObject(ctx.dc,bitmap)};
            unsafe {
                ptr::write_bytes(bits,0,(w as usize)*(h as usize)*4);
                SetBkColor(ctx.dc,0);SetTextColor(ctx.dc,0x00ffffff);SetBkMode(ctx.dc,2);
                let u:Vec<u16>=text.encode_utf16().collect();
                TextOutW(ctx.dc,0,0,u.as_ptr(),u.len() as i32);GdiFlush();
                let rgb=std::slice::from_raw_parts(bits.cast::<u32>(),(w as usize)*(h as usize));
                let left=ox.max(clip.x).max(0);let top=oy.max(clip.y).max(0);
                let right=(ox.saturating_add(w as i32)).min(clip.x.saturating_add(clip.width as i32)).min(target.width as i32);
                let bottom=(oy.saturating_add(h as i32)).min(clip.y.saturating_add(clip.height as i32)).min(target.height as i32);
                for yy in top..bottom {for xx in left..right {
                    let src=rgb[((yy-oy) as usize)*w as usize+(xx-ox) as usize];
                    let coverage=((src&255).max((src>>8)&255).max((src>>16)&255)*(color>>24&255)+127)/255;
                    if coverage==0{continue;}
                    let dst=&mut target.pixels[yy as usize*target.width as usize+xx as usize];
                    let inv=255-coverage;
                    let r=(((color>>16&255)*coverage+(*dst>>16&255)*inv+127)/255)&255;
                    let g=(((color>>8&255)*coverage+(*dst>>8&255)*inv+127)/255)&255;
                    let b=(((color&255)*coverage+(*dst&255)*inv+127)/255)&255;
                    *dst=0xff000000|(r<<16)|(g<<8)|b;
                }}
                SelectObject(ctx.dc,previous);DeleteObject(bitmap);
            }
        }
        Ok(())
    }
}
