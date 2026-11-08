//! Platform-independent document editing, pagination and metadata.
//! This crate uses Rust's standard library exclusively. It creates no windows,
//! loads no native libraries, and performs no OS-specific FFI.
#![forbid(unsafe_code)]
use std::{cell::RefCell, io, ops::Range, rc::Rc};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CharacterStyle {
    pub bold: bool, pub italic: bool, pub underline: bool, pub strike: bool,
    pub size: u16, pub face: String, pub color: u32, pub highlight: Option<u32>,
    pub script: i8,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledRange { pub range: Range<usize>, pub style: CharacterStyle }
#[derive(Debug, Clone)]
struct Snapshot { text: String, cursor: usize, anchor: usize, styles: Vec<StyledRange> }
/// UTF-8-safe editable document, with selection, formatting, and bounded history.
#[derive(Debug, Clone)]
pub struct DocEdit {
    text: String, cursor: usize, anchor: usize, styles: Vec<StyledRange>,
    typing_style: CharacterStyle, undo: Vec<Snapshot>, redo: Vec<Snapshot>,
    dirty: bool, clipboard: String, paragraph_alignment: u16,
}
impl Default for DocEdit { fn default()->Self {Self::new()} }
impl DocEdit {
    pub fn new()->Self {Self {text:String::new(),cursor:0,anchor:0,styles:Vec::new(),
        typing_style:CharacterStyle {size:12,face:"Aptos".into(),color:0xff202020,..CharacterStyle::default()},
        undo:Vec::new(),redo:Vec::new(),dirty:false,clipboard:String::new(),paragraph_alignment:1}}
    pub fn text(&self)->&str {&self.text}
    pub fn caret(&self)->usize {self.cursor}
    pub fn styles(&self)->&[StyledRange] {&self.styles}
    pub fn selection(&self)->Range<usize>{self.cursor.min(self.anchor)..self.cursor.max(self.anchor)}
    pub fn take_changed(&mut self)->bool {std::mem::take(&mut self.dirty)}
    fn snapshot(&self)->Snapshot {Snapshot{text:self.text.clone(),cursor:self.cursor,anchor:self.anchor,styles:self.styles.clone()}}
    fn checkpoint(&mut self){if self.undo.len()==100{self.undo.remove(0);}self.undo.push(self.snapshot());self.redo.clear();}
    fn restore(&mut self,s:Snapshot){self.text=s.text;self.cursor=s.cursor;self.anchor=s.anchor;self.styles=s.styles;self.dirty=true;}
    fn boundary(&self,pos:usize)->usize {let mut p=pos.min(self.text.len());while !self.text.is_char_boundary(p){p-=1;}p}
    pub fn set_text(&mut self,text:&str){if self.text==text{return;}self.checkpoint();self.text=text.into();self.cursor=self.text.len();self.anchor=self.cursor;self.styles.clear();self.dirty=true;}
    pub fn new_doc(&mut self){self.set_text("");self.styles.clear();}
    pub fn set_caret(&mut self,pos:usize){self.cursor=self.boundary(pos);self.anchor=self.cursor;}
    pub fn set_selection(&mut self,start:usize,end:usize){self.anchor=self.boundary(start);self.cursor=self.boundary(end);}
    pub fn select_all(&mut self){self.anchor=0;self.cursor=self.text.len();}
    pub fn move_left(&mut self,extend:bool){let p=self.text[..self.cursor].char_indices().next_back().map(|(p,_)|p).unwrap_or(0);self.cursor=p;if !extend{self.anchor=p;}}
    pub fn move_right(&mut self,extend:bool){let p=self.text[self.cursor..].chars().next().map(|c|self.cursor+c.len_utf8()).unwrap_or(self.cursor);self.cursor=p;if !extend{self.anchor=p;}}
    pub fn move_line(&mut self,down:bool,extend:bool){let before=&self.text[..self.cursor];let col=before.rsplit('\n').next().unwrap_or("").chars().count();let p=if down{let tail=&self.text[self.cursor..];tail.find('\n').map(|i|self.cursor+i+1).unwrap_or(self.text.len())} else{let start=before.rfind('\n').unwrap_or(0);if start==0{0}else{self.text[..start].rfind('\n').map(|i|i+1).unwrap_or(0)}};let offset=self.text[p..].chars().take_while(|c|*c!='\n').take(col).map(char::len_utf8).sum::<usize>();self.cursor=(p+offset).min(self.text.len());if !extend{self.anchor=self.cursor;}}
    fn replace(&mut self,range:Range<usize>,value:&str){self.checkpoint();let removed=range.end-range.start;self.text.replace_range(range.clone(),value);let delta=value.len() as isize-removed as isize;
        self.styles.retain_mut(|r| {if r.range.end<=range.start{return true;}if r.range.start>=range.end{r.range.start=r.range.start.saturating_add_signed(delta);r.range.end=r.range.end.saturating_add_signed(delta);return true;}false});
        self.cursor=range.start+value.len();self.anchor=self.cursor;if !value.is_empty(){self.styles.push(StyledRange{range:range.start..self.cursor,style:self.typing_style.clone()});}self.dirty=true;
    }
    pub fn replace_selection(&mut self,value:&str){let range=self.selection();self.replace(range,value);}
    pub fn insert_char(&mut self,ch:char){if !ch.is_control()||ch=='\n'||ch=='\t'{self.replace_selection(&ch.to_string());}}
    pub fn backspace(&mut self){let range=self.selection();if range.start!=range.end{self.replace(range,"");return;}if self.cursor>0{let start=self.text[..self.cursor].char_indices().next_back().map(|(p,_)|p).unwrap_or(0);self.replace(start..self.cursor,"");}}
    pub fn delete_forward(&mut self){let range=self.selection();if range.start!=range.end{self.replace(range,"");return;}if self.cursor<self.text.len(){let end=self.cursor+self.text[self.cursor..].chars().next().unwrap().len_utf8();self.replace(self.cursor..end,"");}}
    pub fn copy(&mut self){let range=self.selection();self.clipboard=self.text[range].to_string();}
    pub fn cut(&mut self){self.copy();self.replace_selection("");}
    pub fn paste(&mut self){let data=self.clipboard.clone();self.replace_selection(&data);}
    pub fn undo(&mut self){if let Some(s)=self.undo.pop(){self.redo.push(self.snapshot());self.restore(s);}}
    pub fn redo(&mut self){if let Some(s)=self.redo.pop(){self.undo.push(self.snapshot());self.restore(s);}}
    pub fn toggle(&mut self,flag:u32){let mut s=self.typing_style.clone();match flag{1=>s.bold=!s.bold,2=>s.italic=!s.italic,4=>s.underline=!s.underline,8=>s.strike=!s.strike,_=>return}self.set_style(s);}
    fn set_style(&mut self,s:CharacterStyle){self.typing_style=s.clone();let range=self.selection();if range.start!=range.end{self.checkpoint();self.styles.push(StyledRange{range,style:s});self.dirty=true;}}
    pub fn set_size(&mut self,pt:i32){let mut s=self.typing_style.clone();s.size=pt.clamp(6,144) as u16;self.set_style(s);}
    pub fn adjust_size(&mut self,delta:i32){self.set_size(self.typing_style.size as i32+delta);}
    pub fn set_face(&mut self,name:&str){let mut s=self.typing_style.clone();s.face=name.into();self.set_style(s);}
    pub fn set_color(&mut self,rgb:u32,background:bool){let mut s=self.typing_style.clone();if background{s.highlight=Some(rgb);}else{s.color=rgb;}self.set_style(s);}
    pub fn underline_style(&mut self,_kind:u8){let mut s=self.typing_style.clone();s.underline=true;self.set_style(s);}
    pub fn script(&mut self,super_:bool){let mut s=self.typing_style.clone();s.script=if super_{1}else{-1};self.set_style(s);}
    pub fn normal_script(&mut self){let mut s=self.typing_style.clone();s.script=0;self.set_style(s);}
    pub fn align(&mut self,alignment:u16){self.paragraph_alignment=alignment.clamp(1,4);self.dirty=true;}
    pub fn indent(&mut self,dir:i32){if dir>0{self.replace_selection("    ");}else if self.cursor>=4&&self.text[..self.cursor].ends_with("    "){self.replace(self.cursor-4..self.cursor,"");}}
    pub fn paragraph_spacing(&mut self,_twips:i32){} // Paragraph layout policy is held by the page renderer.
    pub fn list(&mut self,numbering:u16,level:u16){if numbering!=0 {self.replace_selection(&format!("{} ",if numbering==1{"•".into()}else{format!("{}.",level.max(1))}));}}
    pub fn apply_style(&mut self,style:&str){match style{"Heading 1"=>{self.set_size(22);self.toggle(1)},"Heading 2"=>{self.set_size(17);self.toggle(1)},"Title"=>self.set_size(30),"Subtitle"=>self.set_size(16),_=>self.set_size(12)}}
    pub fn format_range(&mut self,start:usize,end:usize,bold:bool,italic:bool,underline:bool,strike:bool,size:Option<u16>,color:Option<u32>){let range=self.boundary(start)..self.boundary(end);if range.start>=range.end{return;}let mut style=self.typing_style.clone();style.bold=bold;style.italic=italic;style.underline=underline;style.strike=strike;if let Some(pt)=size{style.size=pt;}if let Some(c)=color{style.color=c;}self.styles.push(StyledRange{range,style});}
    pub fn insert_link(&mut self,title:&str,_url:&str){self.replace_selection(title);}
    pub fn insert_cover(&mut self){self.replace_selection("Document Title\nSubtitle\n\n");}
    pub fn insert_page_break(&mut self){self.replace_selection("\u{000C}");}
}
/// Portable paper geometry, in twips; no native rendering dependency.
#[derive(Clone,Copy,Debug,PartialEq,Eq)] pub enum PaperSize{Letter,A4}
impl PaperSize{pub fn size_twips(self)->(u32,u32){match self{Self::Letter=>(12240,15840),Self::A4=>(11906,16838)}}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum DrawTool{Pen,Line,Rectangle,Ellipse,Eraser}
impl DrawTool{fn tag(self)->u8{match self{Self::Pen=>0,Self::Line=>1,Self::Rectangle=>2,Self::Ellipse=>3,Self::Eraser=>4}}fn parse(i:u8)->io::Result<Self>{match i{0=>Ok(Self::Pen),1=>Ok(Self::Line),2=>Ok(Self::Rectangle),3=>Ok(Self::Ellipse),4=>Ok(Self::Eraser),_=>Err(io::Error::new(io::ErrorKind::InvalidData,"unknown tool"))}}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)] pub struct Point{pub x:i32,pub y:i32}
#[derive(Clone,Debug)]pub struct Stroke{pub page:u32,pub tool:DrawTool,pub color:u32,pub width:u16,pub points:Vec<Point>}
#[derive(Clone,Debug)]pub struct PageLayout{pub start:usize,pub end:usize}
/// Portable pagination, persistent headers/footers, and vector annotations.
pub struct PagedDocument{editor:Rc<RefCell<DocEdit>>,paper:PaperSize,header:String,footer:String,numbers:bool,tool:Option<DrawTool>,ink:u32,width:u16,strokes:Vec<Stroke>,revision:u64,pages:Vec<PageLayout>}
impl PagedDocument{
    pub fn new(editor:Rc<RefCell<DocEdit>>)->Self{let mut s=Self{editor,paper:PaperSize::Letter,header:String::new(),footer:String::new(),numbers:true,tool:None,ink:0x205fbf,width:3,strokes:Vec::new(),revision:0,pages:Vec::new()};s.refresh();s}
    pub fn refresh(&mut self){self.pages.clear();let input=self.editor.borrow();let chars=if self.paper==PaperSize::Letter{90}else{88};let rows=if self.paper==PaperSize::Letter{54}else{58};let mut from=0;let mut count=0;let mut last=0;for (i,ch) in input.text().char_indices(){if ch=='\n'||count>=chars{if ch=='\n'{count=0;}else{count=1;}last+=1;if last>=rows{self.pages.push(PageLayout{start:from,end:i});from=i;last=0;}}else{count+=1;}}self.pages.push(PageLayout{start:from,end:input.text().len()});}
    pub fn pages(&self)->&[PageLayout]{&self.pages}
    pub fn page_count(&self)->usize{self.pages.len().max(1)}
    pub fn revision(&self)->u64{self.revision}
    pub fn paper(&self)->PaperSize{self.paper}
    pub fn set_paper(&mut self,p:PaperSize){if self.paper!=p{self.paper=p;self.revision+=1;self.pages.clear();self.refresh();}}
    pub fn header(&self)->&str{&self.header}
    pub fn footer(&self)->&str{&self.footer}
    pub fn set_header(&mut self,v:&str){self.header=v.into();self.revision+=1;}
    pub fn set_footer(&mut self,v:&str){self.footer=v.into();self.revision+=1;}
    pub fn toggle_page_numbers(&mut self)->bool{self.numbers=!self.numbers;self.revision+=1;self.numbers}
    pub fn page_numbers(&self)->bool{self.numbers}
    pub fn set_draw_tool(&mut self,v:Option<DrawTool>){self.tool=v;}
    pub fn draw_tool(&self)->Option<DrawTool>{self.tool}
    pub fn set_ink_color(&mut self,c:u32){self.ink=c;}
    pub fn set_ink_width(&mut self,w:u16){self.width=w.clamp(1,20);}
    pub fn strokes(&self)->&[Stroke]{&self.strokes}
    pub fn add_stroke(&mut self,page:u32,points:Vec<Point>){if let Some(tool)=self.tool{if points.len()>1_000_000{return;}
        if tool==DrawTool::Eraser {
            if let Some(mark)=points.first(){
                if let Some(i)=self.strokes.iter().rposition(|s|s.page==page&&s.points.iter().any(|p|(p.x-mark.x).abs()<24&&(p.y-mark.y).abs()<24)) {
                    self.strokes.remove(i);self.revision+=1;
                }
            }
        }else{self.strokes.push(Stroke{page,tool,color:self.ink,width:self.width,points});self.revision+=1;}
    }}
    pub fn clear_drawings(&mut self){self.strokes.clear();self.revision+=1;}
    pub fn undo_drawing(&mut self){if self.strokes.pop().is_some(){self.revision+=1;}}
    pub fn reset(&mut self){self.paper=PaperSize::Letter;self.header.clear();self.footer.clear();self.numbers=true;self.strokes.clear();self.revision+=1;self.pages.clear();self.refresh();}
    pub fn export_metadata(&self)->io::Result<Vec<u8>>{let mut b=b"MATAPG02".to_vec();b.push(u8::from(self.paper==PaperSize::A4));b.push(u8::from(self.numbers));fn u32b(b:&mut Vec<u8>,v:usize)->io::Result<()> {let n=u32::try_from(v).map_err(|_|io::Error::new(io::ErrorKind::InvalidData,"too large"))?;b.extend(n.to_le_bytes());Ok(())}for s in [&self.header,&self.footer]{u32b(&mut b,s.len())?;b.extend(s.as_bytes());}u32b(&mut b,self.strokes.len())?;for s in &self.strokes{b.extend(s.page.to_le_bytes());b.push(s.tool.tag());b.extend(s.color.to_le_bytes());b.extend(s.width.to_le_bytes());u32b(&mut b,s.points.len())?;for p in &s.points{b.extend(p.x.to_le_bytes());b.extend(p.y.to_le_bytes());}}Ok(b)}
    pub fn import_metadata(&mut self,bytes:&[u8])->io::Result<()>{if bytes.len()>128*1024*1024{return Err(io::Error::new(io::ErrorKind::InvalidData,"metadata too large"));}let mut p=0;fn take<'a>(b:&'a[u8],p:&mut usize,n:usize)->io::Result<&'a[u8]>{let end=p.checked_add(n).filter(|end|*end<=b.len()).ok_or(io::Error::from(io::ErrorKind::UnexpectedEof))?;let s=&b[*p..end];*p=end;Ok(s)}fn u32r(b:&[u8],p:&mut usize)->io::Result<u32>{Ok(u32::from_le_bytes(take(b,p,4)?.try_into().unwrap()))}fn strr(b:&[u8],p:&mut usize)->io::Result<String>{let n=u32r(b,p)? as usize;if n>65536{return Err(io::Error::new(io::ErrorKind::InvalidData,"string too long"))}String::from_utf8(take(b,p,n)?.to_vec()).map_err(|e|io::Error::new(io::ErrorKind::InvalidData,e))}if take(bytes,&mut p,8)?!=b"MATAPG02"{return Err(io::Error::new(io::ErrorKind::InvalidData,"invalid metadata"))}let paper=if take(bytes,&mut p,1)?[0]!=0{PaperSize::A4}else{PaperSize::Letter};let numbers=take(bytes,&mut p,1)?[0]!=0;let header=strr(bytes,&mut p)?;let footer=strr(bytes,&mut p)?;let n=u32r(bytes,&mut p)? as usize;if n>100000{return Err(io::Error::new(io::ErrorKind::InvalidData,"too many strokes"));}let mut strokes=Vec::new();for _ in 0..n{let page=u32r(bytes,&mut p)?;let tool=DrawTool::parse(take(bytes,&mut p,1)?[0])?;let color=u32r(bytes,&mut p)?;let width=u16::from_le_bytes(take(bytes,&mut p,2)?.try_into().unwrap());let m=u32r(bytes,&mut p)? as usize;if m>1000000{return Err(io::Error::new(io::ErrorKind::InvalidData,"too many points"));}let mut points=Vec::with_capacity(m);for _ in 0..m{points.push(Point{x:i32::from_le_bytes(take(bytes,&mut p,4)?.try_into().unwrap()),y:i32::from_le_bytes(take(bytes,&mut p,4)?.try_into().unwrap())});}strokes.push(Stroke{page,tool,color,width,points});}self.paper=paper;self.numbers=numbers;self.header=header;self.footer=footer;self.strokes=strokes;self.revision+=1;self.pages.clear();self.refresh();Ok(())}
}
#[cfg(test)] mod tests {use super::*;
    #[test]fn unicode_edit_undo_redo(){let mut d=DocEdit::new();d.insert_char('é');d.insert_char('🦀');d.backspace();assert_eq!(d.text(),"é");d.undo();assert_eq!(d.text(),"é🦀");d.redo();assert_eq!(d.text(),"é");}
    #[test]fn selection_boundaries(){let mut d=DocEdit::new();d.set_text("a🦀b");d.set_selection(1,5);d.replace_selection("ok");assert_eq!(d.text(),"aokb");}
    #[test]fn metadata_roundtrip(){let model=Rc::new(RefCell::new(DocEdit::new()));let mut a=PagedDocument::new(model.clone());a.set_header("Résumé");a.set_paper(PaperSize::A4);a.set_draw_tool(Some(DrawTool::Pen));a.add_stroke(0,vec![Point{x:2,y:4},Point{x:7,y:9}]);let b=a.export_metadata().unwrap();let mut c=PagedDocument::new(model);c.import_metadata(&b).unwrap();assert_eq!(c.header(),"Résumé");assert_eq!(c.paper(),PaperSize::A4);assert_eq!(c.strokes().len(),1);}
    #[test]fn truncated_metadata_is_error(){let mut p=PagedDocument::new(Rc::new(RefCell::new(DocEdit::new())));assert!(p.import_metadata(b"MATAPG02\x01").is_err());}
}
