//! Native software paint layer for catalog widgets.
//! Interactive widgets are updated by WidgetExt::pointer_activate in WidgetWindow.
//! Draws directly into the host framebuffer; no separate bitmap allocation.
use crate::Framebuffer;
use crate::widgets::Rect;
use super::base::PaintedWidget;

const W: usize = 320;
const H: usize = 174;
const BG: u32 = 0xff141e30;
const SURFACE: u32 = 0xff223048;
const SURFACE2: u32 = 0xff2c405b;
const BORDER: u32 = 0xff4a617a;
const WHITE: u32 = 0xffe9f4ff;
const MUTED: u32 = 0xffa7c0d5;
const GREEN: u32 = 0xff3bd6ac;
const BLUE: u32 = 0xff59a9fa;
const GOLD: u32 = 0xfff5bb6e;
const PURPLE: u32 = 0xffac91fa;

struct Canvas<'a> {
    fb: &'a mut Framebuffer,
    bounds: Rect,
    opacity: f32,
    background: u32,
    foreground: u32,
}
impl Canvas<'_> {
    fn rect(&mut self,x:i32,y:i32,w:i32,h:i32,c:u32) {
        let c=if c==BG {self.background} else if c==WHITE {self.foreground} else {c};
        if w <= 0 || h <= 0 || self.bounds.width == 0 || self.bounds.height == 0 {return;}
        let (bx,by,bw,bh)=(self.bounds.x as i64,self.bounds.y as i64,self.bounds.width as i64,self.bounds.height as i64);
        let x0=bx+(x as i64*bw/W as i64);
        let y0=by+(y as i64*bh/H as i64);
        let x1=bx+((x+w) as i64*bw/W as i64);
        let y1=by+((y+h) as i64*bh/H as i64);
        let left=x0.max(bx).max(0).min(self.fb.width as i64);
        let top=y0.max(by).max(0).min(self.fb.height as i64);
        let right=x1.min(bx+bw).min(self.fb.width as i64).max(left);
        let bottom=y1.min(by+bh).min(self.fb.height as i64).max(top);
        let a=(((c>>24)&255) as f32*self.opacity).round().clamp(0.0,255.0) as u32;
        if a == 0 {return;}
        let rgb=c&0xffffff;
        let opaque=0xff000000|rgb;
        let inv=255-a;
        for yy in top as usize..bottom as usize {for xx in left as usize..right as usize {
            let cell=&mut self.fb.pixels[yy*self.fb.width as usize+xx];
            if a==255 {*cell=opaque;} else {
                let dst=*cell;
                let r=(((rgb>>16)&255)*a+((dst>>16)&255)*inv+127)/255;
                let g=(((rgb>>8)&255)*a+((dst>>8)&255)*inv+127)/255;
                let b=((rgb&255)*a+(dst&255)*inv+127)/255;
                *cell=0xff000000|(r<<16)|(g<<8)|b;
            }
        }}
    }
    fn outline(&mut self,x:i32,y:i32,w:i32,h:i32,c:u32) {
        self.rect(x,y,w,2,c);self.rect(x,y+h-2,w,2,c);
        self.rect(x,y,2,h,c);self.rect(x+w-2,y,2,h,c);
    }
    fn circle(&mut self,x:i32,y:i32,r:i32,c:u32) {
        for yy in (-r)..=r {for xx in (-r)..=r {if xx*xx+yy*yy <= r*r { self.rect(x+xx,y+yy,1,1,c); }}}
    }
    fn line(&mut self,x:i32,y:i32,x2:i32,y2:i32,c:u32) {
        let dx=(x2-x).abs();let dy=-(y2-y).abs();
        let sx=if x<x2{1}else{-1};let sy=if y<y2{1}else{-1};let mut err=dx+dy;
        let (mut px,mut py)=(x,y);
        loop {self.rect(px,py,2,2,c);if px==x2&&py==y2{break;}
            let e=err*2;if e>=dy {err+=dy;px+=sx;}if e<=dx {err+=dx;py+=sy;}}
    }
    fn label(&mut self,x:i32,y:i32,s:&str,color:u32,scale:i32) {
        let mut xp=x;
        for ch in s.chars().take(44) {
            if ch=='\n' {continue}
            let glyph=font(ch.to_ascii_uppercase());
            for (row,bits) in glyph.iter().enumerate(){for col in 0..5{
                if (*bits & (1u8<<(4-col))) !=0 {self.rect(xp+col*scale,y+row as i32*scale,scale,scale,color);}
            }}xp+=6*scale;
            if xp>(W as i32-9){break;}
        }
    }
    fn panel(&mut self,x:i32,y:i32,w:i32,h:i32){self.rect(x,y,w,h,SURFACE);self.outline(x,y,w,h,BORDER);}
    fn button(&mut self,x:i32,y:i32,w:i32,s:&str,color:u32){self.rect(x,y,w,24,color);self.label(x+8,y+9,s,BG,1);}
    fn field(&mut self,x:i32,y:i32,w:i32,s:&str){self.rect(x,y,w,26,BG);self.outline(x,y,w,26,BORDER);self.label(x+9,y+9,s,MUTED,1);}
    fn row(&mut self,y:i32,n:i32,color:u32){self.rect(43,y,228,25,SURFACE2);self.rect(50,y+8,9,9,color);self.label(68,y+9,&format!("ITEM {n}"),WHITE,1);}
    fn small_window(&mut self,caption:&str){self.panel(46,29,226,120);self.rect(46,29,226,23,SURFACE2);self.circle(61,40,4,GOLD);self.circle(75,40,4,GREEN);self.label(91,37,caption,WHITE,1);}
}

// Self-contained 5x7 bitmap font: no dependency or platform font needed for previews.
fn font(ch:char)->[u8;7] {
    match ch {
        'A'=>[14,17,17,31,17,17,17],'B'=>[30,17,17,30,17,17,30],
        'C'=>[14,17,16,16,16,17,14],'D'=>[30,17,17,17,17,17,30],
        'E'=>[31,16,16,30,16,16,31],'F'=>[31,16,16,30,16,16,16],
        'G'=>[14,17,16,23,17,17,15],'H'=>[17,17,17,31,17,17,17],
        'I'=>[31,4,4,4,4,4,31],'J'=>[7,2,2,2,18,18,12],
        'K'=>[17,18,20,24,20,18,17],'L'=>[16,16,16,16,16,16,31],
        'M'=>[17,27,21,21,17,17,17],'N'=>[17,25,21,19,17,17,17],
        'O'=>[14,17,17,17,17,17,14],'P'=>[30,17,17,30,16,16,16],
        'Q'=>[14,17,17,17,21,18,13],'R'=>[30,17,17,30,20,18,17],
        'S'=>[15,16,16,14,1,1,30],'T'=>[31,4,4,4,4,4,4],
        'U'=>[17,17,17,17,17,17,14],'V'=>[17,17,17,17,17,10,4],
        'W'=>[17,17,17,21,21,21,10],'X'=>[17,17,10,4,10,17,17],
        'Y'=>[17,17,10,4,4,4,4],'Z'=>[31,1,2,4,8,16,31],
        '0'=>[14,17,19,21,25,17,14],'1'=>[4,12,4,4,4,4,14],
        '2'=>[14,17,1,2,4,8,31],'3'=>[30,1,1,14,1,1,30],
        '4'=>[2,6,10,18,31,2,2],'5'=>[31,16,16,30,1,1,30],
        '6'=>[14,16,16,30,17,17,14],'7'=>[31,1,2,4,8,8,8],
        '8'=>[14,17,17,14,17,17,14],'9'=>[14,17,17,15,1,1,14],
        ' '=>[0;7],'-'=>[0,0,0,31,0,0,0],'.'=>[0,0,0,0,0,12,12],
        '/'=>[1,1,2,4,8,16,16],':'=>[0,12,12,0,12,12,0],
        '_'=>[0,0,0,0,0,0,31],'>'=>[16,8,4,2,4,8,16],
        '<'=>[1,2,4,8,4,2,1],'+'=>[0,4,4,31,4,4,0],
        '['=>[14,8,8,8,8,8,14],']'=>[14,2,2,2,2,2,14],
        '('=>[2,4,8,8,8,4,2],')'=>[8,4,2,2,2,4,8],
        '*'=>[0,21,14,31,14,21,0],'='=>[0,0,31,0,31,0,0],
        '?'=>[14,17,1,2,4,0,4],'%'=>[17,2,4,8,17,0,0],
        _=>[0,0,0,0,0,0,0]
    }
}

/// Fits a logical painter viewport without anisotropic scaling.
/// Avoid rounding outside the host bounds on resize.
pub(crate) fn aspect_fit(bounds: Rect, logical_w: u32, logical_h: u32) -> Rect {
    if bounds.width == 0 || bounds.height == 0 || logical_w == 0 || logical_h == 0 {
        return Rect::new(bounds.x, bounds.y, 0, 0);
    }
    let width_limited = (bounds.width as u64) * (logical_h as u64)
        <= (bounds.height as u64) * (logical_w as u64);
    let (width, height) = if width_limited {
        (bounds.width, (((bounds.width as u64)*logical_h as u64)/(logical_w as u64)).max(1) as u32)
    } else {
        ((((bounds.height as u64)*logical_w as u64)/(logical_h as u64)).max(1) as u32, bounds.height)
    };
    let offset_x = ((bounds.width - width) / 2) as i32;
    let offset_y = ((bounds.height - height) / 2) as i32;
    Rect::new(bounds.x.saturating_add(offset_x),bounds.y.saturating_add(offset_y),width,height)
}

pub(crate) fn paint(fb: &mut Framebuffer, bounds: Rect, item: &PaintedWidget, inherited_opacity: f32) {
    if !item.widget.visible() {return;}
    let computed=item.widget.computed_style((fb.width, fb.height),Some(bounds));
    let active=item.state.active.unwrap_or(false);
    let name=item.kind;
    // The old painter mapped 320x174 design coordinates independently onto the
    // full host bounds. On wide or resized windows that stretched text/glyphs
    // horizontally by several times and appeared as a smeared control.
    // Preserve the painter's design aspect ratio and always clip to a logical
    // viewport; a fresh frame is cleared by WidgetWindow before painting.
    let fitted = aspect_fit(bounds, W as u32, H as u32);
    let mut c=Canvas {fb, bounds: fitted, opacity: inherited_opacity*computed.opacity,
        background: computed.background_color.unwrap_or(BG),
        foreground: computed.foreground_color.unwrap_or(WHITE),
    };
    c.rect(0,0,320,174,BG);
    c.rect(0,0,320,24,SURFACE2);
    c.label(10,9,name,PURPLE,1);
    c.rect(8,164,304,1,BORDER);
    let on=if active { GREEN } else {BLUE};
    match name {
        "Button"|"LinkButton"|"MenuButton"|"ColorButton"|"FontButton"|"FileChooserWidget"|"AppChooserButton"|"FontDialogButton"|"ColorDialogButton"|"LockButton"|"AppChooserWidget"|"FontChooserWidget"|"ColorChooserWidget"|"EmojiChooser" => {
            let txt= match name {"LinkButton"=>"VISIT LINK >","MenuButton"=>"OPEN MENU V","LockButton"=>"UNLOCK", "FileChooserWidget"=>"BROWSE FILES", "AppChooserButton"|"AppChooserWidget"=>"CHOOSE APP", "FontButton"|"FontChooserWidget"|"FontDialogButton"=>"ARIAL 14", "EmojiChooser"=>"EMOJI PICKER", "ColorButton"|"ColorChooserWidget"|"ColorDialogButton"=>"PICK COLOR",_=>"CLICK ME"};
            c.button(57,74,205,txt,on);
            if name.contains("Color") { for i in 0..7 {c.rect(63+i*27,112,22,19,[BLUE,GREEN,GOLD,PURPLE,WHITE,0xffef6178,SURFACE2][i as usize]);}}
            if name=="EmojiChooser" {for i in 0..4{c.circle(90+i*47,120,12,[GOLD,PURPLE,GREEN,BLUE][i as usize]);}}
            if name.contains("Font") {c.label(85,119,"Aa Bb Cc",WHITE,2);}
            if name=="LinkButton"{c.rect(57,102,183,1,BLUE);}
        }
        "ToggleButton"|"CheckButton"|"Switch" => {
            c.label(45,62,"ENABLED",WHITE,2);
            if name=="Switch" {c.rect(178,66,61,24,if active { GREEN } else {BORDER});c.circle(if active {225} else {190},78,10,WHITE);}
            else {c.outline(177,65,25,25,BLUE);if active {c.line(182,78,188,84,GREEN);c.line(188,84,199,69,GREEN);}}
        }
        "TextField"|"Entry"|"PasswordEntry"|"SearchEntry"|"EditableLabel"|"TextView"|"Text"|"Label"|"Inscription"|"ShortcutLabel"|"ShortcutsShortcut"|"Statusbar" => {
            if name=="TextView" {c.field(34,44,251,"RUST WIDGET TEXT");c.field(34,77,251,"SECOND LINE");c.field(34,111,251,"CURSOR |");}
            else if name=="Statusbar" {c.rect(22,108,277,29,SURFACE2);c.label(39,118,"READY - 100%",GREEN,1);}
            else if name=="ShortcutLabel"||name=="ShortcutsShortcut" {c.label(33,77,"SAVE AS",WHITE,2);c.button(196,72,91,"CTRL S",PURPLE);}
            else if name=="Label"||name=="Text"||name=="Inscription" {c.label(28,70,"HELLO MATAWIN",WHITE,2);c.rect(30,105,254,2,GREEN);}
            else {c.field(38,70,244,if name=="PasswordEntry"{"**********"}else if name=="SearchEntry"{"SEARCH WIDGETS..."}else{"TYPE HERE..."});}
        }
        "Range"|"Scale"|"Scrollbar"|"ProgressBar"|"LevelBar"|"SpinButton"|"Spinner"|"DropDown"|"ComboBox"|"ComboBoxText"|"ScaleButton"|"VolumeButton"|"Calendar" => {
            if name=="Spinner" {for i in 0..10{let a=i as f64*std::f64::consts::TAU/10.0;let x=160+(a.cos()*28.0) as i32;let y=87+(a.sin()*28.0) as i32;c.circle(x,y,4,if i == (if active {7}else{2}) {GREEN}else{BORDER});}}
            else if name=="Calendar" {c.panel(80,39,160,112);c.label(103,49,"OCTOBER 2026",WHITE,1);for r in 0..4{for col in 0..7 {let n=1+r*7+col;c.label(93+col*20,70+r*17,&format!("{n}"),if item.state.value==Some(n as f64) {GREEN}else{MUTED},1);}}}
            else if name=="DropDown"||name.starts_with("ComboBox") {c.field(59,48,203,item.state.text.as_deref().unwrap_or("SELECT AN ITEM"));c.panel(59,82,203,66);c.label(73,92,"APPLE",WHITE,1);c.label(73,109,"BANANA",GREEN,1);c.label(73,126,"CHERRY",WHITE,1);}
            else if name=="SpinButton" {c.field(89,75,145,if active{"5"}else{"4"});c.button(237,75,35,"+",GREEN);}
            else if name=="ScaleButton"||name=="VolumeButton" {c.label(46,54,"VOLUME",WHITE,2);c.rect(49,102,219,5,BORDER);c.rect(49,102,item.state.value.map(|v|(219.0*v.clamp(0.0,1.0)) as i32).unwrap_or(if active{170}else{103}),5,GREEN);c.circle(item.state.value.map(|v|49+(219.0*v.clamp(0.0,1.0)) as i32).unwrap_or(if active{219}else{152}),104,9,WHITE);}
            else if name=="LevelBar" {for i in 0..10{c.rect(48+i*23,81,17,26,if i < item.state.value.map(|v|v.round().clamp(0.0,10.0) as i32).unwrap_or(if active{8}else{6}) {GREEN}else{SURFACE2});}}
            else {c.rect(42,86,237,9,SURFACE2);c.rect(42,86,item.state.value.map(|v|(237.0*v.clamp(0.0,1.0)) as i32).unwrap_or(if active{193}else{120}),9,on);if name!="ProgressBar"{c.circle(item.state.value.map(|v|42+(237.0*v.clamp(0.0,1.0)) as i32).unwrap_or(if active{234}else{162}),90,10,WHITE);}c.label(133,116,&format!("{}%",(100.0*item.state.value.unwrap_or(if active{0.8}else{0.5}).clamp(0.0,1.0)).round() as i32),WHITE,1);}
        }
        "ListBox"|"ListBoxRow"|"ListView"|"GridView"|"IconView"|"ColumnView"|"TreeView"|"CellView"|"TreeExpander"|"ListBase" => {
            if name=="GridView"||name=="IconView" {for r in 0..2{for col in 0..4 {let x=42+col*61;let y=48+r*51;c.rect(x,y,51,40,if item.state.value==Some((r*4+col) as f64){GREEN}else{SURFACE2});c.label(x+12,y+17,&format!("{}",r*4+col+1),BG,1);}}}
            else if name=="ColumnView"||name=="TreeView"||name=="TreeExpander" {c.rect(43,46,229,23,PURPLE);c.label(52,54,"NAME      SIZE     DATE",BG,1);for r in 0..3{c.rect(43,73+r*25,229,23,SURFACE2);c.label(55+r*5,81+r*25,if r==1{" > FOLDER"}else{"  FILE.TXT"},WHITE,1);}}
            else if let Some(ref list)=item.state.text {
                for (i,txt) in list.split(" / ").take(3).enumerate() {
                    let y=43+i as i32*30;
                    let selected = item.state.value == Some(i as f64);
                    c.rect(43,y,228,25,if selected {BLUE}else{SURFACE2});
                    c.label(56,y+8,txt,if selected {BG}else{WHITE},1);
                }
            } else {c.row(43,1,BLUE);c.row(73,2,GREEN);c.row(103,3,GOLD);}
        }
        "Image"|"Picture"|"SvgWidget"|"DrawingArea"|"GLArea"|"GraphicsOffload"|"DragIcon" => {
            c.panel(54,36,212,119);
            c.circle(209,67,14,GOLD);
            c.line(68,139,135,65,BLUE);c.line(135,65,180,113,BLUE);c.line(180,113,210,89,GREEN);c.line(210,89,250,139,GREEN);
            c.label(110,141,if name=="SvgWidget"{"VECTOR SVG"}else if name=="GLArea"{"GPU DRAW"}else{"IMAGE"},WHITE,1);
        }
        "Audio"|"MediaControls"|"Video" => {
            c.panel(29,42,262,109);
            if name=="Video" {c.rect(39,52,242,61,SURFACE2);c.circle(163,83,22,BLUE);c.label(157,80,">",BG,2);}
            else {for i in 0..44{let k=((i*13+7)%28) as i32;c.rect(45+i*5,76-k/2,2,4+k,GREEN);}}
            c.button(46,121,41,if active{"||"}else{">"},GREEN);
            c.rect(96,132,156,4,BORDER);c.rect(96,132,if active{121}else{54},4,BLUE);
        }
        "WebView"=>{
            c.small_window("WEB BROWSER");c.field(56,56,206,"HTTPS://EXAMPLE.ORG");c.rect(56,89,203,49,SURFACE2);c.label(69,108,"WEB CONTENT",GREEN,2);
        }
        "Dialog"|"MessageDialog"|"AboutDialog"|"AppChooserDialog"|"ColorChooserDialog"|"FileChooserDialog"|"FontChooserDialog"|"PageSetupUnixDialog"|"PrintUnixDialog"|"Assistant"|"ApplicationWindow"|"Window"|"ShortcutsWindow" => {
            c.small_window(name);
            c.label(62,70,if name=="MessageDialog"{"CONFIRM ACTION?"}else if name=="AboutDialog"{"MATAWIN 0.1"}else{"SETTINGS"},WHITE,1);
            c.button(68,104,86,"CANCEL",BORDER);c.button(166,104,87,"OK",GREEN);
        }
        "Box"|"Fixed"|"Grid"|"FlowBox"|"FlowBoxChild"|"Paned"|"CenterBox"|"AspectFrame"|"Overlay"|"Frame"|"Viewport"|"ScrolledWindow"|"Stack"|"StackSidebar"|"StackSwitcher"|"Notebook"|"Revealer"|"Expander"|"ShortcutsGroupContainer"|"ShortcutsSectionContainer"|"ShortcutsGroup"|"ShortcutsSection" => {
            c.panel(32,39,256,112);
            if name=="Grid"||name=="FlowBox"||name=="FlowBoxChild" {for row in 0..2{for col in 0..3{c.rect(44+col*81,51+row*44,72,33,if col==row{GREEN}else{SURFACE2});}}}
            else if name=="Paned" {
                let div=44+(230.0*item.state.value.unwrap_or(0.5).clamp(0.0,1.0)) as i32;
                c.rect(44,52,(div-44).max(0),85,SURFACE2);
                c.rect(div+4,52,(275-div-4).max(0),85,SURFACE2);
                c.rect(div,52,4,85,PURPLE);
            }
            else if name=="Stack"||name=="Notebook"||name=="StackSidebar"||name=="StackSwitcher" {
                let second=item.state.text.as_deref().is_some_and(|t|t.contains("Second")||t.contains("second")||t.contains("2"));
                c.button(43,47,72,"TAB ONE",if second {BORDER}else{GREEN});
                c.button(117,47,72,"TAB TWO",if second {GREEN}else{BORDER});
                c.rect(44,77,230,62,SURFACE2);
                c.label(85,101,if second {"PAGE TWO"}else{"PAGE ONE"},WHITE,1);
            }
            else if name=="Overlay" {c.rect(60,62,186,69,BLUE);c.rect(126,81,121,48,PURPLE);}
            else if name=="ScrolledWindow"||name=="Viewport" {for i in 0..4{c.rect(44,52+i*19,218,12,SURFACE2);}c.rect(266,53,5,80,BORDER);c.rect(266,67,5,31,GREEN);}
            else if name=="Revealer"||name=="Expander" {c.label(44,55,if active{"V DETAILS"}else{"> DETAILS"},GREEN,1);if active {c.rect(44,79,215,46,SURFACE2);c.label(56,97,"HIDDEN CONTENT",WHITE,1);}}
            else {c.rect(43,58,229,33,SURFACE2);c.rect(43,100,110,35,BLUE);c.rect(162,100,110,35,GREEN);}
        }
        "Popover"|"PopoverMenu"|"PopoverMenuBar"|"PopoverBin"|"SearchBar"|"ActionBar"|"HeaderBar" => {
            c.panel(35,41,249,111);c.rect(35,41,249,28,SURFACE2);c.label(49,51,"FILE   EDIT   VIEW",WHITE,1);
            if name=="SearchBar"{c.field(52,91,215,"SEARCH HERE...");}
            else if name=="ActionBar"||name=="HeaderBar"{c.button(53,97,93,"SAVE",GREEN);c.button(151,97,107,"CLOSE",BLUE);}
            else{c.panel(83,73,164,70);c.label(98,83,"NEW",WHITE,1);c.label(98,101,"OPEN",WHITE,1);c.label(98,119,"PREFERENCES",GREEN,1);}
        }
        "Separator" => {c.rect(39,66,242,3,BORDER);c.label(107,88,"SECTION",WHITE,2);c.rect(39,116,242,3,BORDER);}
        "InfoBar"|"WindowControls"|"WindowHandle" => {
            c.small_window("CONTROL SURFACE");c.rect(60,69,199,47,SURFACE2);c.label(70,86,if name=="InfoBar"{"INFO: SAVED"}else{"WINDOW CHROME"},GREEN,1);
        }
        "Widget" => {c.panel(59,49,200,100);c.label(93,90,"BASE WIDGET",GREEN,1);}
        _ => { // Exhaustive safe fallback for any newly introduced catalog widget.
            c.panel(32,43,256,105);c.label(60,74,"WIDGET PREVIEW",WHITE,2);
            c.rect(55,116,210,8,on);
        }
    }
    // State label is painted from the real instance rather than example text.
    if let Some(ref text)=item.state.text {
        c.rect(8,146,304,17,BG);
        c.label(10,151,text,WHITE,1);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{ProgressBar, Switch, WidgetExt};

    #[test]
    fn aspect_fit_keeps_proportions_when_resized() {
        for (w,h) in [(1200,180),(320,174),(520,480),(110,700)] {
            let fit = aspect_fit(Rect::new(11,23,w,h),320,174);
            assert!(fit.width<=w && fit.height<=h);
            assert!(fit.x>=11 && fit.y>=23);
            assert!(((fit.width as i64)*174 - (fit.height as i64)*320).abs()<=320);
        }
    }

    #[test]
    fn paints_selection_state_into_framebuffer() {
        let toggle=Switch::new();
        toggle.set_active(true);
        let crate::widgets::WidgetContent::Painted(node)=toggle.renderable_content().unwrap() else {panic!("paint node expected")};
        assert_eq!(node.state.active,Some(true));
        let mut fb=Framebuffer::from_color(200,100,0xff000000);
        paint(&mut fb,Rect::new(0,0,200,100),&node,1.0);
        assert!(fb.pixels.iter().any(|&p|p!=0xff000000));
    }
    #[test]
    fn native_painter_clips_to_supplied_bounds() {
        let bar=ProgressBar::new();
        bar.set_fraction(0.75);
        let crate::widgets::WidgetContent::Painted(node)=bar.renderable_content().unwrap() else {panic!("paint node expected")};
        assert_eq!(node.state.value,Some(0.75));
        let mut fb=Framebuffer::from_color(48,48,0xff000000);
        paint(&mut fb,Rect::new(-100,-100,140,140),&node,1.0);
        assert_eq!(fb.pixels.len(),48*48);
        assert_eq!(fb.pixels[47*48+47],0xff000000);
    }
}
