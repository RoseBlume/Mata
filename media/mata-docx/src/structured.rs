//! Editable DOCX object model. All package processing is safe std-only Rust.
//! Tables, images, notes and sections
//! remain distinct objects, and can be edited before serializing back to OOXML.
use super::{Run,xml,zip};
use std::{collections::BTreeMap, io::{self,ErrorKind}};

fn bad(message: &str) -> io::Error { io::Error::new(ErrorKind::InvalidData, message) }
const WORD_NS:&str="http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const REL_NS:&str="http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DRAW_NS:&str="http://schemas.openxmlformats.org/drawingml/2006/main";
const PIC_NS:&str="http://schemas.openxmlformats.org/drawingml/2006/picture";
const WP_NS:&str="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const PACKAGE_REL:&str="http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_REL:&str="http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    Text(Run), Image(String), Hyperlink { url: String, runs: Vec<Run> },
    Bookmark { id: u32, name: String },
    NoteReference { id: u32, endnote: bool },
    Field { instruction: String, display: String },
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichParagraph { pub runs: Vec<Inline>, pub alignment: Option<String>, pub style: Option<String> }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell { pub blocks: Vec<Block>, pub width_twips: Option<u32>, pub column_span: u16, pub vertical_merge: Option<String> }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableRow { pub cells: Vec<Cell> }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table { pub columns_twips: Vec<u32>, pub rows: Vec<TableRow> }
#[derive(Clone, Debug, PartialEq)]
pub enum Block { Paragraph(RichParagraph), Table(Table), SectionBreak(Section) }
impl Default for Block { fn default()->Self {Self::Paragraph(RichParagraph::default())} }
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub width_twips: u32, pub height_twips: u32,
    pub margin_top: u32, pub margin_bottom: u32,
    pub margin_left: u32, pub margin_right: u32,
    pub header_twips: u32, pub footer_twips: u32,
    pub landscape: bool, pub columns: u16,
    pub header: String, pub footer: String,
}
impl Default for Section {
    fn default()->Self {Self{width_twips:12240,height_twips:15840,margin_top:1440,margin_bottom:1440,
        margin_left:1440,margin_right:1440,header_twips:720,footer_twips:720,landscape:false,columns:1,
        header:String::new(),footer:String::new()}}
}
#[derive(Clone, Debug, PartialEq)]
pub struct Image { pub id:String,pub bytes:Vec<u8>,pub mime:String,pub width_emu:u64,pub height_emu:u64,pub alt:String }
#[derive(Clone, Debug, PartialEq)]
pub struct Note { pub id:u32,pub endnote:bool,pub blocks:Vec<Block> }
#[derive(Clone, Debug, Default)]
pub struct RichDocument {
    pub blocks:Vec<Block>,pub final_section:Section,
    pub images:BTreeMap<String,Image>,pub notes:Vec<Note>,pub warnings:Vec<String>,
    /// Parts not rewritten by the structural serializer are retained, such as
    /// styles, theme and font tables. Imported macros and external links are never executed.
    pub original_parts:BTreeMap<String,Vec<u8>>,
}
impl RichDocument {
    pub fn new()->Self {Self {blocks:vec![Block::default()],..Self::default()}}
    pub fn insert_table(&mut self,at:usize,rows:usize,columns:usize)->io::Result<usize>{
        if rows==0||columns==0||rows>4096||columns>128{return Err(bad("table dimensions must be 1..4096 rows and 1..128 columns"));}
        let cell=||Cell {blocks:vec![Block::default()],column_span:1,..Cell::default()};
        let table=Table {columns_twips:vec![2400;columns],rows:(0..rows).map(|_|TableRow{cells:(0..columns).map(|_|cell()).collect()}).collect()};
        let idx=at.min(self.blocks.len());self.blocks.insert(idx,Block::Table(table));Ok(idx)
    }
    pub fn set_cell_text(&mut self,table_block:usize,row:usize,column:usize,text:impl Into<String>)->io::Result<()> {
        let Some(Block::Table(t))=self.blocks.get_mut(table_block) else{return Err(bad("not a table block"));};
        let cell=t.rows.get_mut(row).and_then(|r|r.cells.get_mut(column)).ok_or_else(||bad("table cell out of bounds"))?;
        cell.blocks=vec![Block::Paragraph(RichParagraph{runs:vec![Inline::Text(Run{text:text.into(),..Run::default()})],..RichParagraph::default()})];Ok(())
    }
    /// Merge this cell with the following `span - 1` cells on the same row.
    /// Covered cells are removed from the XML row to maintain a valid tblGrid.
    pub fn set_cell_span(&mut self,table_block:usize,row:usize,column:usize,span:u16)->io::Result<()> {
        let Some(Block::Table(t))=self.blocks.get_mut(table_block) else{return Err(bad("not a table block"));};
        let cells=&mut t.rows.get_mut(row).ok_or_else(||bad("table row out of bounds"))?.cells;
        let count=span as usize;
        if count==0 || count>128 || column.checked_add(count).is_none_or(|last|last>cells.len()){
            return Err(bad("column span exceeds table row"));
        }
        let old_span=cells[column].column_span;
        if old_span!=1{return Err(bad("cannot merge a cell that is already merged"));}
        let covered:Vec<Cell>=cells.drain(column+1..column+count).collect();
        for merged in covered{cells[column].blocks.extend(merged.blocks);}
        cells[column].column_span=span;Ok(())
    }
    /// Merge a vertical run of cells. The first is a restart; all subsequent
    /// cells become continuation cells in the matching table column.
    pub fn merge_vertical(&mut self,table_block:usize,row:usize,column:usize,count:usize)->io::Result<()> {
        let Some(Block::Table(t))=self.blocks.get_mut(table_block) else{return Err(bad("not a table block"));};
        if count<2 || row.checked_add(count).is_none_or(|end|end>t.rows.len()) {
            return Err(bad("invalid vertical merge length"));
        }
        for r in row..row+count {
            let cell=t.rows[r].cells.get_mut(column).ok_or_else(||bad("column unavailable for vertical merge"))?;
            cell.vertical_merge=Some(if r==row{"restart"}else{"continue"}.into());
        }Ok(())
    }
    pub fn insert_image(&mut self,at:usize,bytes:Vec<u8>,alt:impl Into<String>,width_emu:u64,height_emu:u64)->io::Result<String>{
        if bytes.len()>32*1024*1024{return Err(bad("image larger than 32 MiB"));}
        let mime=image_mime(&bytes).ok_or_else(||bad("supported embedded images: PNG, JPEG, GIF"))?;
        if width_emu==0||height_emu==0{return Err(bad("image dimensions must be positive"));}
        let mut n=self.images.len()+1;while self.images.contains_key(&format!("image{n}")){n+=1;}
        let id=format!("image{n}");self.images.insert(id.clone(),Image{id:id.clone(),bytes,mime:mime.into(),width_emu,height_emu,alt:alt.into()});
        self.blocks.insert(at.min(self.blocks.len()),Block::Paragraph(RichParagraph{runs:vec![Inline::Image(id.clone())],..RichParagraph::default()}));Ok(id)
    }
    pub fn resize_image(&mut self,id:&str,width_emu:u64,height_emu:u64)->io::Result<()> {
        if width_emu==0||height_emu==0{return Err(bad("image dimensions must be positive"));}
        let image=self.images.get_mut(id).ok_or_else(||bad("unknown image"))?;image.width_emu=width_emu;image.height_emu=height_emu;Ok(())
    }
    pub fn insert_section_break(&mut self,at:usize,section:Section){self.blocks.insert(at.min(self.blocks.len()),Block::SectionBreak(section));}
    pub fn insert_hyperlink(&mut self,at:usize,label:&str,url:&str)->io::Result<()> {
        if !(url.starts_with("https://")||url.starts_with("http://")||url.starts_with("mailto:")){return Err(bad("unsupported hyperlink scheme"));}
        self.blocks.insert(at.min(self.blocks.len()),Block::Paragraph(RichParagraph{runs:vec![Inline::Hyperlink{url:url.into(),runs:vec![Run{text:label.into(),..Run::default()}]}],..RichParagraph::default()}));Ok(())
    }
    pub fn insert_bookmark(&mut self,at:usize,name:&str)->io::Result<u32> {
        if name.is_empty()||!name.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'){return Err(bad("bookmark name must be alphanumeric or underscore"));}
        let id=(self.blocks.len() as u32)+1;
        self.blocks.insert(at.min(self.blocks.len()),Block::Paragraph(RichParagraph{runs:vec![Inline::Bookmark{id,name:name.to_owned()}],..RichParagraph::default()}));Ok(id)
    }
    pub fn insert_note(&mut self,at:usize,endnote:bool,text:&str)->u32 {
        let id=self.notes.iter().filter(|n|n.endnote==endnote).map(|n|n.id).max().unwrap_or(0)+1;
        self.notes.push(Note{id,endnote,blocks:vec![Block::Paragraph(RichParagraph{runs:vec![Inline::Text(Run{text:text.to_owned(),..Run::default()})],..RichParagraph::default()})]});
        self.blocks.insert(at.min(self.blocks.len()),Block::Paragraph(RichParagraph{runs:vec![Inline::NoteReference{id,endnote}],..RichParagraph::default()}));id
    }
    pub fn insert_citation(&mut self,at:usize,source_tag:&str,display:&str)->io::Result<()> {
        if source_tag.trim().is_empty(){return Err(bad("citation source tag cannot be empty"));}
        self.blocks.insert(at.min(self.blocks.len()),Block::Paragraph(RichParagraph{runs:vec![Inline::Field{instruction:format!(" CITATION {} ",source_tag.trim()),display:display.into()}],..RichParagraph::default()}));Ok(())
    }
    pub fn insert_table_of_contents(&mut self,at:usize) {
        self.blocks.insert(at.min(self.blocks.len()),Block::Paragraph(RichParagraph{runs:vec![Inline::Field{
            instruction:" TOC \\o \"1-3\" \\h ".into(), display:"Table of Contents (update fields in Word)".into()
        }],..RichParagraph::default()}));
    }
    pub fn insert_bibliography(&mut self,at:usize) {
        self.blocks.insert(at.min(self.blocks.len()),Block::Paragraph(RichParagraph{runs:vec![Inline::Field{
            instruction:" BIBLIOGRAPHY ".into(),display:"Bibliography (update fields in Word)".into()
        }],..RichParagraph::default()}));
    }
    pub fn insert_cross_reference(&mut self,at:usize,bookmark:&str,display:&str)->io::Result<()> {
        if bookmark.trim().is_empty(){return Err(bad("reference bookmark cannot be empty"));}
        self.blocks.insert(at.min(self.blocks.len()),Block::Paragraph(RichParagraph{runs:vec![Inline::Field{instruction:format!(" REF {} ",bookmark.trim()),display:display.into()}],..RichParagraph::default()}));Ok(())
    }
    /// Update a paragraph's text with the smallest changed range, retaining
    /// styling on untouched runs. This avoids flattening mixed-format Word runs.
    fn patch_text_runs(p:&mut RichParagraph,new:&str) {
        let previous=p.runs.iter().filter_map(|r|if let Inline::Text(r)=r{Some(r.text.as_str())}else{None}).collect::<String>();
        if previous==new{return;}
        let old:Vec<char>=previous.chars().collect();
        let next:Vec<char>=new.chars().collect();
        let mut prefix=0usize;
        while prefix<old.len()&&prefix<next.len()&&old[prefix]==next[prefix]{prefix+=1;}
        let mut suffix=0usize;
        while suffix<old.len()-prefix && suffix<next.len()-prefix &&
            old[old.len()-1-suffix]==next[next.len()-1-suffix]{suffix+=1;}
        let mut before=Vec::new();let mut after=Vec::new();let mut cursor=0usize;
        let mut insertion_style=Run::default();
        for item in &p.runs {if let Inline::Text(run)=item {
            let chars:Vec<char>=run.text.chars().collect();
            let lo=cursor;let hi=cursor+chars.len();
            if lo<=prefix && prefix<=hi {insertion_style=run.clone();}
            if lo<prefix {
                let count=(prefix-lo).min(chars.len());
                if count>0 {let mut part=run.clone();part.text=chars[..count].iter().collect();before.push(Inline::Text(part));}
            }
            let right=(old.len()-suffix).max(lo);
            if right<hi {
                let offset=right-lo;let mut part=run.clone();part.text=chars[offset..].iter().collect();
                after.push(Inline::Text(part));
            }
            cursor=hi;
        }}
        let mut result=before;
        let inserted=&next[prefix..next.len()-suffix];
        if !inserted.is_empty(){insertion_style.text=inserted.iter().collect();result.push(Inline::Text(insertion_style));}
        result.extend(after);
        p.runs=result;
    }
    /// A one-line-per-block text projection for the native Unicode editor.
    /// Non-text objects remain in the DOCX model and appear as read-only markers.
    pub fn editor_text(&self)->String {
        self.blocks.iter().map(|block|match block {
            Block::Paragraph(p) => p.runs.iter().map(|item|match item {
                Inline::Text(r)=>r.text.replace('\n',"\u{2028}"),
                Inline::Hyperlink{runs,..}=>runs.iter().map(|r|r.text.as_str()).collect::<String>(),
                Inline::Image(_)=>"[Image]".into(),
                Inline::Bookmark{..}=>"[Bookmark]".into(),
                Inline::NoteReference{..}=>"[Note]".into(),
                Inline::Field{display,..}=>display.clone(),
            }).collect::<String>(),
            Block::Table(t)=>format!("[Table: {} rows]",t.rows.len()),
            Block::SectionBreak(_)=>"[Section break]".into(),
        }).collect::<Vec<_>>().join("\n")
    }
    /// Synchronize changed paragraphs while never converting tables, images,
    /// hyperlinks, fields or other DOCX nodes into plain text.
    /// Changes to protected lines are rejected instead of corrupting the DOCX.
    pub fn apply_editor_text(&mut self,input:&str)->io::Result<bool> {
        let normalized=input.replace("\r\n","\n").replace('\r',"\n");
        let lines:Vec<&str>=normalized.split('\n').collect();
        let before=self.editor_text();
        let existing:Vec<&str>=before.split('\n').collect();
        let simple=self.blocks.iter().all(|block|match block {
            Block::Paragraph(p)=>p.runs.iter().all(|r|matches!(r,Inline::Text(_))),
            _=>false,
        });
        if !simple && lines.len()!=existing.len() {
            return Err(bad("adding/removing lines around DOCX tables, images or references requires a structured edit"));
        }
        if !simple {
            for (i,(block,line)) in self.blocks.iter().zip(lines.iter()).enumerate() {
                let editable=matches!(block,Block::Paragraph(p) if p.runs.iter().all(|r|matches!(r,Inline::Text(_))));
                if !editable && *line != existing[i] {
                    return Err(bad("cannot edit a protected DOCX object directly; use the ribbon controls"));
                }
            }
        }
        if lines.iter().any(|l|l.len()>16*1024*1024) || lines.len()>200_000 {
            return Err(bad("DOCX text exceeds editor limits"));
        }
        let mut changed=false;
        if simple {
            for (i,line) in lines.iter().enumerate() {
                if let Some(Block::Paragraph(p))=self.blocks.get_mut(i) {
                    if existing.get(i).copied()!=Some(*line) {
                        Self::patch_text_runs(p,&line.replace('\u{2028}',"\n"));changed=true;
                    }
                } else {
                    self.blocks.push(Block::Paragraph(RichParagraph{runs:vec![Inline::Text(Run{text:line.replace('\u{2028}',"\n"),..Run::default()})],..RichParagraph::default()}));
                    changed=true;
                }
            }
            if self.blocks.len()>lines.len() {self.blocks.truncate(lines.len());changed=true;}
        } else {
            for (i,line) in lines.iter().enumerate() {
                if existing[i]==*line {continue;}
                if let Block::Paragraph(p)=&mut self.blocks[i] {
                    Self::patch_text_runs(p,&line.replace('\u{2028}',"\n"));changed=true;
                }
            }
        }
        Ok(changed)
    }
    pub fn to_plain_text(&self)->String {let mut out=String::new();blocks_plain(&self.blocks,&mut out);out}
}
fn blocks_plain(blocks:&[Block],out:&mut String) {
    for block in blocks {match block {
        Block::Paragraph(p)=>{for r in &p.runs{match r{Inline::Text(t)=>out.push_str(&t.text),Inline::Hyperlink{runs,..}=>for t in runs{out.push_str(&t.text)},Inline::Field{display,..}=>out.push_str(display),Inline::Image(_)=>out.push_str("[Image]"),Inline::NoteReference{..}=>out.push_str("[Note]"),Inline::Bookmark{..}=>{}}}out.push('\n');},
        Block::Table(t)=>for row in &t.rows{for (i,cell) in row.cells.iter().enumerate(){if i>0{out.push('\t');}blocks_plain(&cell.blocks,out);}out.push('\n');},
        Block::SectionBreak(_)=>out.push('\u{000C}'),
    }}
}
fn image_mime(bytes:&[u8])->Option<&'static str>{if bytes.starts_with(b"\x89PNG\r\n\x1a\n"){Some("image/png")}else if bytes.starts_with(b"\xff\xd8\xff"){Some("image/jpeg")}else if bytes.starts_with(b"GIF87a")||bytes.starts_with(b"GIF89a"){Some("image/gif")}else{None}}
#[derive(Clone,Debug)]enum Child{Node(Element),Text(String)}
#[derive(Clone,Debug)]struct Element{name:String,attrs:BTreeMap<String,String>,children:Vec<Child>}
impl Element {
    fn child(&self,name:&str)->Option<&Element>{self.children.iter().filter_map(|c|match c{Child::Node(e) if e.name==name=>Some(e),_=>None}).next()}
    fn children_named<'a>(&'a self,name:&'a str)->impl Iterator<Item=&'a Element>{self.children.iter().filter_map(move |c|match c{Child::Node(e) if e.name==name=>Some(e),_=>None})}
    fn attr(&self,name:&str)->Option<&str>{self.attrs.get(name).map(String::as_str)}
    fn text(&self)->String{let mut out=String::new();for child in &self.children{match child{Child::Text(t)=>out.push_str(t),Child::Node(e)=>out.push_str(&e.text())}}out}
    fn first_descendant(&self,name:&str)->Option<&Element>{self.children.iter().filter_map(|c|match c{Child::Node(e)=>Some(e),_=>None}).find_map(|e|if e.name==name{Some(e)}else{e.first_descendant(name)})}
}
fn root(bytes:&[u8])->io::Result<Element>{
    let mut stack=Vec::<Element>::new();let mut document=None;
    for token in xml::tokenize(bytes)? {match token {
        xml::Token::Start(name,attrs,self_close)=>{
            let node=Element{name,attrs,children:Vec::new()};
            if self_close {if let Some(parent)=stack.last_mut(){parent.children.push(Child::Node(node));}else{document=Some(node);}}
            else {stack.push(node);if stack.len()>128{return Err(bad("XML nesting is too deep"));}}
        },
        xml::Token::Text(text)=>if let Some(top)=stack.last_mut(){top.children.push(Child::Text(text));},
        xml::Token::End(name)=>{
            let node=stack.pop().ok_or_else(||bad("unexpected XML closing element"))?;
            if node.name!=name{return Err(bad("mismatched XML closing element"));}
            if let Some(top)=stack.last_mut(){top.children.push(Child::Node(node));}else{document=Some(node);}
        }
    }}
    if !stack.is_empty(){return Err(bad("incomplete XML document"));}
    document.ok_or_else(||bad("empty XML document"))
}
fn relationships(xml_bytes:Option<&Vec<u8>>)->io::Result<BTreeMap<String,String>>{
    let mut out=BTreeMap::new();let Some(bytes)=xml_bytes else {return Ok(out)};
    let root_node=root(bytes)?;
    for rel in root_node.children_named("Relationship"){
        if rel.attr("TargetMode")==Some("External"){
            if let (Some(id),Some(target))=(rel.attr("Id"),rel.attr("Target")){
                let safe=target.starts_with("https://")||target.starts_with("http://")||target.starts_with("mailto:");
                out.insert(id.into(),if safe{target.into()}else{format!("external:{target}")});
            }
        }else if let (Some(id),Some(target))=(rel.attr("Id"),rel.attr("Target")){out.insert(id.into(),target.into());}
    }Ok(out)
}
fn read_run(e:&Element)->Run {
    let mut run=Run::default();if let Some(pr)=e.child("rPr"){
        for child in &pr.children {if let Child::Node(el)=child {let active=el.attr("val")!=Some("0") && el.attr("val")!=Some("false");match el.name.as_str(){
            "b"=>run.bold=active,"i"=>run.italic=active,"u"=>run.underline=active,"strike"=>run.strike=active,
            "sz"=>run.size=el.attr("val").and_then(|v|v.parse().ok()),"color"=>run.color=el.attr("val").map(str::to_owned),_=>{}
        }}}
    }
    for child in &e.children {if let Child::Node(el)=child{match el.name.as_str(){
        "t"|"delText"=>run.text.push_str(&el.text()),"tab"=>run.text.push('\t'),"br"|"cr"=>run.text.push('\n'),_=>{}
    }}}
    run
}
fn read_paragraph(e:&Element,relations:&BTreeMap<String,String>,images:&mut BTreeMap<String,Image>)->RichParagraph {
    let mut para=RichParagraph::default();if let Some(ppr)=e.child("pPr"){
        para.alignment=ppr.child("jc").and_then(|n|n.attr("val")).map(str::to_owned);
        para.style=ppr.child("pStyle").and_then(|n|n.attr("val")).map(str::to_owned);
    }
    for c in &e.children{if let Child::Node(n)=c{match n.name.as_str(){
        "r"=>{
            let r=read_run(n);if !r.text.is_empty(){para.runs.push(Inline::Text(r));}
            if let Some(blip)=n.first_descendant("blip") {
                if let Some(target)=blip.attr("embed").and_then(|id|relations.get(id)) {
                    let candidate=target.rsplit('/').next().unwrap_or(target);
                    if let Some((id,_))=images.iter().find(|(_,image)|image.id==candidate) {para.runs.push(Inline::Image(id.clone()));}
                }
            }
            if let Some(foot)=n.first_descendant("footnoteReference").and_then(|e|e.attr("id")).and_then(|s|s.parse().ok()){para.runs.push(Inline::NoteReference{id:foot,endnote:false});}
            if let Some(end)=n.first_descendant("endnoteReference").and_then(|e|e.attr("id")).and_then(|s|s.parse().ok()){para.runs.push(Inline::NoteReference{id:end,endnote:true});}
        },
        "hyperlink"=>{
            let url=n.attr("id").and_then(|id|relations.get(id)).cloned().or_else(||n.attr("anchor").map(|a|format!("#{a}"))).unwrap_or_default();
            let runs=n.children_named("r").map(read_run).collect();para.runs.push(Inline::Hyperlink{url,runs});
        },
        "bookmarkStart"=>if let Some(name)=n.attr("name"){para.runs.push(Inline::Bookmark{id:n.attr("id").and_then(|s|s.parse().ok()).unwrap_or(0),name:name.into()});},
        "fldSimple"=>para.runs.push(Inline::Field{instruction:n.attr("instr").unwrap_or("").into(),display:n.text()}),
        _=>{}
    }}}
    para
}
fn read_table(e:&Element,relations:&BTreeMap<String,String>,images:&mut BTreeMap<String,Image>,depth:usize)->io::Result<Table>{
    if depth>16{return Err(bad("nested tables too deep"));}
    let mut table=Table::default();if let Some(grid)=e.child("tblGrid"){
        table.columns_twips=grid.children_named("gridCol").filter_map(|c|c.attr("w").and_then(|s|s.parse().ok())).collect();
    }
    for r in e.children_named("tr"){
        let mut row=TableRow::default();for c in r.children_named("tc"){
            let mut cell=Cell{column_span:1,..Cell::default()};if let Some(pr)=c.child("tcPr"){
                cell.width_twips=pr.child("tcW").and_then(|p|p.attr("w")).and_then(|s|s.parse().ok());
                cell.column_span=pr.child("gridSpan").and_then(|p|p.attr("val")).and_then(|s|s.parse::<u16>().ok()).unwrap_or(1).max(1);
                cell.vertical_merge=pr.child("vMerge").map(|e|e.attr("val").unwrap_or("continue").into());
            }
            cell.blocks=read_blocks(c,relations,images,depth+1)?;row.cells.push(cell);
        }table.rows.push(row);
    }Ok(table)
}
fn read_blocks(e:&Element,relations:&BTreeMap<String,String>,images:&mut BTreeMap<String,Image>,depth:usize)->io::Result<Vec<Block>>{
    let mut blocks=Vec::new();for c in &e.children{if let Child::Node(n)=c{match n.name.as_str(){
        "p"=>{
            let para=read_paragraph(n,relations,images);blocks.push(Block::Paragraph(para));
            if let Some(sect)=n.child("pPr").and_then(|pr|pr.child("sectPr")){blocks.push(Block::SectionBreak(read_section(sect)));}
        },
        "tbl"=>blocks.push(Block::Table(read_table(n,relations,images,depth)?)),_=>{}
    }}}
    Ok(blocks)
}
fn read_section(e:&Element)->Section{
    let mut s=Section::default();if let Some(size)=e.child("pgSz"){
        s.width_twips=size.attr("w").and_then(|v|v.parse().ok()).unwrap_or(s.width_twips);
        s.height_twips=size.attr("h").and_then(|v|v.parse().ok()).unwrap_or(s.height_twips);
        s.landscape=size.attr("orient")==Some("landscape");
    }
    if let Some(m)=e.child("pgMar"){
        for (name,field) in [("top",&mut s.margin_top),("bottom",&mut s.margin_bottom),("left",&mut s.margin_left),("right",&mut s.margin_right),("header",&mut s.header_twips),("footer",&mut s.footer_twips)]{
            if let Some(n)=m.attr(name).and_then(|s|s.parse::<u32>().ok()){*field=n;}
        }
    }
    s.columns=e.child("cols").and_then(|e|e.attr("num")).and_then(|n|n.parse().ok()).unwrap_or(1);
    s
}
fn part_media_path(target:&str)->Option<String>{
    if target.starts_with("external:")||target.starts_with('/')||target.contains("..")||target.contains('\\'){return None;}
    Some(format!("word/{}",target.strip_prefix("./").unwrap_or(target)))
}
/// Decode structured blocks, embedded media, sections and reference parts.
pub fn decode_rich(bytes:&[u8])->io::Result<RichDocument>{
    let parts=zip::read(bytes)?;
    if !parts.contains_key("[Content_Types].xml")||!parts.contains_key("_rels/.rels"){return Err(bad("not an OOXML document"));}
    let main=parts.get("word/document.xml").ok_or_else(||bad("DOCX document.xml missing"))?;
    let document=root(main)?;
    let rels=relationships(parts.get("word/_rels/document.xml.rels"))?;
    let mut result=RichDocument{original_parts:parts.clone(),..RichDocument::default()};
    for (id,target) in &rels{
        if let Some(path)=part_media_path(target){if let Some(data)=parts.get(&path){
            if let Some(mime)=image_mime(data){let key=path.rsplit('/').next().unwrap_or(&path).to_owned();
                result.images.insert(key.clone(),Image{id:key,bytes:data.clone(),mime:mime.into(),width_emu:1_828_800,height_emu:1_828_800,alt:String::new()});}
        }}
        if target.starts_with("external:")&&!target.starts_with("external:http")&&!target.starts_with("external:mailto:"){
            result.warnings.push(format!("External relationship {id} is not editable"));
        }
    }
    // Resolve drawing dimensions/alt text by traversing the package tree.
    let mut image_refs=Vec::<(String,u64,u64,String)>::new();
    fn collect_image_nodes(e:&Element,relations:&BTreeMap<String,String>,out:&mut Vec<(String,u64,u64,String)>){
        if e.name=="drawing"{
            if let Some(blip)=e.first_descendant("blip"){
                if let Some(t)=blip.attr("embed").and_then(|id|relations.get(id)){
                    let file=t.rsplit('/').next().unwrap_or(t).to_owned();
                    let extent=e.first_descendant("extent");let w=extent.and_then(|x|x.attr("cx")).and_then(|s|s.parse().ok()).unwrap_or(1_828_800);
                    let h=extent.and_then(|x|x.attr("cy")).and_then(|s|s.parse().ok()).unwrap_or(1_828_800);
                    let alt=e.first_descendant("docPr").and_then(|e|e.attr("descr")).unwrap_or("").into();out.push((file,w,h,alt));
                }
            }
        }
        for c in &e.children{if let Child::Node(child)=c{collect_image_nodes(child,relations,out);}}
    }
    collect_image_nodes(&document,&rels,&mut image_refs);
    for (id,w,h,alt) in image_refs{if let Some(image)=result.images.get_mut(&id){image.width_emu=w;image.height_emu=h;image.alt=alt;}}
    let body=document.child("body").ok_or_else(||bad("DOCX body missing"))?;
    result.blocks=read_blocks(body,&rels,&mut result.images,0)?;
    if let Some(sect)=body.child("sectPr"){result.final_section=read_section(sect);}
    // Endnotes and footnotes are independent parts with typed reference identifiers.
    for (part,endnote,container,item) in [("word/footnotes.xml",false,"footnotes","footnote"),("word/endnotes.xml",true,"endnotes","endnote")]{
        if let Some(bytes)=parts.get(part){let note_root=root(bytes)?;
            if note_root.name==container{for note in note_root.children_named(item){
                let Some(id)=note.attr("id").and_then(|s|s.parse::<u32>().ok()) else {continue};
                if note.attr("type").is_some(){continue;}
                result.notes.push(Note{id,endnote,blocks:read_blocks(note,&rels,&mut result.images,0)?});
            }}
        }
    }
    // Header/footer text is kept in the section model for subsequent editing.
    for name in ["word/header1.xml","word/footer1.xml"]{
        if let Some(bytes)=parts.get(name){let value=root(bytes)?.text();
            if name.contains("header"){result.final_section.header=value;}else{result.final_section.footer=value;}
        }
    }
    if result.blocks.is_empty(){result.blocks.push(Block::default());}
    Ok(result)
}

struct Writer {
    relationships:Vec<String>,
    media:BTreeMap<String,Vec<u8>>,
    next_rel:u32,next_image:u32,next_bookmark:u32,
}
impl Writer {
    fn new(parts:&BTreeMap<String,Vec<u8>>)->io::Result<Self>{
        let mut relationships_xml=Vec::new();
        if let Some(part)=parts.get("word/_rels/document.xml.rels"){
            let relations=root(part)?;
            for rel in relations.children_named("Relationship"){
                let kind=rel.attr("Type").unwrap_or("").rsplit('/').next().unwrap_or("");
                if matches!(kind,"image"|"hyperlink"|"header"|"footer"|"footnotes"|"endnotes") {continue;}
                if let (Some(id),Some(typ),Some(target))=(rel.attr("Id"),rel.attr("Type"),rel.attr("Target")){
                    relationships_xml.push(format!("<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{}/>",xml::escape(id),xml::escape(typ),xml::escape(target),if rel.attr("TargetMode")==Some("External") {" TargetMode=\"External\""}else{""}));
                }
            }
        }
        if !relationships_xml.iter().any(|s|s.contains("/styles\"")){
            relationships_xml.push(format!("<Relationship Id=\"rId1\" Type=\"{OFFICE_REL}/styles\" Target=\"styles.xml\"/>"));
        }
        Ok(Self{relationships:relationships_xml,media:BTreeMap::new(),next_rel:100_000,next_image:1,next_bookmark:1})
    }
    fn relationship(&mut self,kind:&str,target:&str,external:bool)->String{
        while self.relationships.iter().any(|r|r.contains(&format!("Id=\"rId{}\"",self.next_rel))){self.next_rel+=1;}
        let id=format!("rId{}",self.next_rel);self.next_rel+=1;
        self.relationships.push(format!("<Relationship Id=\"{id}\" Type=\"{OFFICE_REL}/{kind}\" Target=\"{}\"{}/>",xml::escape(target),if external{" TargetMode=\"External\""}else{""}));id
    }
    fn run(&self,b:&mut String,r:&Run){
        if r.text.is_empty(){return;}
        b.push_str("<w:r>");
        if r.bold||r.italic||r.underline||r.strike||r.size.is_some()||r.color.is_some(){
            b.push_str("<w:rPr>");if r.bold{b.push_str("<w:b/>");}if r.italic{b.push_str("<w:i/>");}
            if r.underline{b.push_str("<w:u w:val=\"single\"/>");}if r.strike{b.push_str("<w:strike/>");}
            if let Some(size)=r.size{b.push_str(&format!("<w:sz w:val=\"{size}\"/>"));}
            if let Some(color)=&r.color{b.push_str(&format!("<w:color w:val=\"{}\"/>",xml::escape(color)));}b.push_str("</w:rPr>");
        }
        for (i,line) in r.text.split('\n').enumerate(){if i>0{b.push_str("<w:br/>");}
            for (j,chunk) in line.split('\t').enumerate(){if j>0{b.push_str("<w:tab/>");}if !chunk.is_empty(){b.push_str("<w:t xml:space=\"preserve\">");b.push_str(&xml::escape(chunk));b.push_str("</w:t>");}}
        }b.push_str("</w:r>");
    }
    fn image(&mut self,b:&mut String,id:&str,images:&BTreeMap<String,Image>)->io::Result<()> {
        let image=images.get(id).ok_or_else(||bad("paragraph references missing embedded image"))?;
        let ext=match image.mime.as_str(){"image/png"=>"png","image/jpeg"=>"jpeg","image/gif"=>"gif",_=>return Err(bad("unsupported embedded image format"))};
        let filename=format!("mata_image_{}.{}",self.next_image,ext);let doc_id=self.next_image;self.next_image+=1;
        let relation=self.relationship("image",&format!("media/{filename}"),false);
        self.media.insert(format!("word/media/{filename}"),image.bytes.clone());
        b.push_str(&format!("<w:r><w:drawing><wp:inline xmlns:wp=\"{WP_NS}\" distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{}\" cy=\"{}\"/><wp:docPr id=\"{}\" name=\"Picture {}\" descr=\"{}\"/><a:graphic xmlns:a=\"{DRAW_NS}\"><a:graphicData uri=\"{PIC_NS}\"><pic:pic xmlns:pic=\"{PIC_NS}\"><pic:nvPicPr><pic:cNvPr id=\"{}\" name=\"{}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>",
            image.width_emu,image.height_emu,doc_id,doc_id,xml::escape(&image.alt),doc_id,xml::escape(&filename),relation,image.width_emu,image.height_emu));Ok(())
    }
    fn paragraph(&mut self,b:&mut String,p:&RichParagraph,images:&BTreeMap<String,Image>,section:Option<&Section>)->io::Result<()> {
        b.push_str("<w:p>");
        if p.alignment.is_some()||p.style.is_some()||section.is_some(){b.push_str("<w:pPr>");
            if let Some(style)=&p.style{b.push_str(&format!("<w:pStyle w:val=\"{}\"/>",xml::escape(style)));}
            if let Some(align)=&p.alignment{b.push_str(&format!("<w:jc w:val=\"{}\"/>",xml::escape(align)));}
            if let Some(section)=section{self.section(b,section)?;}b.push_str("</w:pPr>");
        }
        for inline in &p.runs{match inline{
            Inline::Text(run)=>self.run(b,run),Inline::Image(id)=>self.image(b,id,images)?,
            Inline::Hyperlink{url,runs}=>{
                if url.starts_with('#'){b.push_str(&format!("<w:hyperlink w:anchor=\"{}\">",xml::escape(&url[1..])));}
                else{if !(url.starts_with("https://")||url.starts_with("http://")||url.starts_with("mailto:")){return Err(bad("unsafe hyperlink target"));}
                    let rel=self.relationship("hyperlink",url,true);b.push_str(&format!("<w:hyperlink r:id=\"{rel}\">"));}
                for r in runs{self.run(b,r);}b.push_str("</w:hyperlink>");
            },
            Inline::Bookmark{id,name}=>{
                let actual_id=if *id==0{let id=self.next_bookmark;self.next_bookmark+=1;id}else{*id};
                b.push_str(&format!("<w:bookmarkStart w:id=\"{actual_id}\" w:name=\"{}\"/><w:bookmarkEnd w:id=\"{actual_id}\"/>",xml::escape(name)));
            },
            Inline::NoteReference{id,endnote}=>b.push_str(&format!("<w:r><w:{} w:id=\"{id}\"/></w:r>",if *endnote{"endnoteReference"}else{"footnoteReference"})),
            Inline::Field{instruction,display}=>{
                b.push_str(&format!("<w:fldSimple w:instr=\"{}\">",xml::escape(instruction)));
                self.run(b,&Run{text:display.clone(),..Run::default()});b.push_str("</w:fldSimple>");
            },
        }}
        b.push_str("</w:p>");Ok(())
    }
    fn table(&mut self,b:&mut String,t:&Table,images:&BTreeMap<String,Image>)->io::Result<()> {
        b.push_str("<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/><w:tblBorders><w:top w:val=\"single\" w:sz=\"4\"/><w:left w:val=\"single\" w:sz=\"4\"/><w:bottom w:val=\"single\" w:sz=\"4\"/><w:right w:val=\"single\" w:sz=\"4\"/><w:insideH w:val=\"single\" w:sz=\"4\"/><w:insideV w:val=\"single\" w:sz=\"4\"/></w:tblBorders></w:tblPr><w:tblGrid>");
        for width in &t.columns_twips{b.push_str(&format!("<w:gridCol w:w=\"{width}\"/>"));}b.push_str("</w:tblGrid>");
        for row in &t.rows{b.push_str("<w:tr>");for cell in &row.cells{
            b.push_str("<w:tc><w:tcPr>");
            if let Some(width)=cell.width_twips{b.push_str(&format!("<w:tcW w:w=\"{width}\" w:type=\"dxa\"/>"));}
            if cell.column_span>1{b.push_str(&format!("<w:gridSpan w:val=\"{}\"/>",cell.column_span));}
            if let Some(merge)=&cell.vertical_merge{if merge!="restart" && merge!="continue"{return Err(bad("invalid vertical merge"));}b.push_str(&format!("<w:vMerge w:val=\"{merge}\"/>"));}
            b.push_str("</w:tcPr>");self.blocks(b,&cell.blocks,images)?;
            if cell.blocks.is_empty(){b.push_str("<w:p/>");}b.push_str("</w:tc>");
        }b.push_str("</w:tr>");}b.push_str("</w:tbl>");Ok(())
    }
    fn blocks(&mut self,b:&mut String,blocks:&[Block],images:&BTreeMap<String,Image>)->io::Result<()> {
        for block in blocks{match block{
            Block::Paragraph(p)=>self.paragraph(b,p,images,None)?,
            Block::Table(t)=>self.table(b,t,images)?,
            Block::SectionBreak(s)=>self.paragraph(b,&RichParagraph::default(),images,Some(s))?,
        }}Ok(())
    }
    fn section(&mut self,b:&mut String,s:&Section)->io::Result<()> {
        if s.width_twips==0||s.height_twips==0||s.columns==0{return Err(bad("invalid page/section dimensions"));}
        b.push_str("<w:sectPr>");
        if !s.header.is_empty(){let id=self.relationship("header","header1.xml",false);b.push_str(&format!("<w:headerReference w:type=\"default\" r:id=\"{id}\"/>"));}
        if !s.footer.is_empty(){let id=self.relationship("footer","footer1.xml",false);b.push_str(&format!("<w:footerReference w:type=\"default\" r:id=\"{id}\"/>"));}
        b.push_str(&format!("<w:pgSz w:w=\"{}\" w:h=\"{}\"{} />",s.width_twips,s.height_twips,if s.landscape{" w:orient=\"landscape\""}else{""}));
        b.push_str(&format!("<w:pgMar w:top=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:right=\"{}\" w:header=\"{}\" w:footer=\"{}\"/>",s.margin_top,s.margin_bottom,s.margin_left,s.margin_right,s.header_twips,s.footer_twips));
        b.push_str(&format!("<w:cols w:num=\"{}\"/></w:sectPr>",s.columns));Ok(())
    }
    fn notes_part(&mut self,notes:&[Note],endnote:bool,images:&BTreeMap<String,Image>)->io::Result<Vec<u8>>{
        let tag=if endnote{"endnotes"}else{"footnotes"};let child=if endnote{"endnote"}else{"footnote"};
        let mut b=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:{tag} xmlns:w=\"{WORD_NS}\" xmlns:r=\"{REL_NS}\">");
        for n in notes.iter().filter(|n|n.endnote==endnote){b.push_str(&format!("<w:{child} w:id=\"{}\">",n.id));self.blocks(&mut b,&n.blocks,images)?;b.push_str(&format!("</w:{child}>"));}
        b.push_str(&format!("</w:{tag}>"));Ok(b.into_bytes())
    }
    fn header_part(text:&str,header:bool)->Vec<u8>{
        let tag=if header{"hdr"}else{"ftr"};format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:{tag} xmlns:w=\"{WORD_NS}\"><w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:{tag}>",xml::escape(text)).into_bytes()
    }
}
/// Serialize the editable structure to a valid DOCX package. Original styles,
/// themes, settings and unrelated parts are retained; document.xml and all
/// document relationships are regenerated. Unsupported document.xml features
/// cannot be guaranteed to survive structural edits.
pub fn encode_rich(doc:&RichDocument)->io::Result<Vec<u8>>{
    let mut writer=Writer::new(&doc.original_parts)?;let mut body=String::new();writer.blocks(&mut body,&doc.blocks,&doc.images)?;
    writer.section(&mut body,&doc.final_section)?;
    let document=format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document xmlns:w=\"{WORD_NS}\" xmlns:r=\"{REL_NS}\"><w:body>{body}</w:body></w:document>");
    let mut parts=doc.original_parts.clone();
    // Ensure a stand-alone editable model can be saved without an imported package.
    if !parts.contains_key("word/styles.xml"){
        parts.insert("word/styles.xml".into(),format!("<?xml version=\"1.0\"?><w:styles xmlns:w=\"{WORD_NS}\"><w:style w:type=\"paragraph\" w:styleId=\"Normal\" w:default=\"1\"><w:name w:val=\"Normal\"/></w:style></w:styles>").into_bytes());
    }
    if doc.notes.iter().any(|n|!n.endnote){writer.relationship("footnotes","footnotes.xml",false);}
    if doc.notes.iter().any(|n|n.endnote){writer.relationship("endnotes","endnotes.xml",false);}
    // Remove stale known media and write the new relationship graph.
    // Keep unrelated original package parts and media. They may still be used
    // by themes, drawing extensions, charts and retained header variants.
    // Only rewrite the specific parts under our structural ownership.
    parts.retain(|path,_|!matches!(path.as_str(),"word/header1.xml"|"word/footer1.xml"|"word/footnotes.xml"|"word/endnotes.xml"));
    if doc.notes.iter().any(|n|!n.endnote){parts.insert("word/footnotes.xml".into(),writer.notes_part(&doc.notes,false,&doc.images)?);}
    if doc.notes.iter().any(|n|n.endnote){parts.insert("word/endnotes.xml".into(),writer.notes_part(&doc.notes,true,&doc.images)?);}
    for (path,bytes) in &writer.media{parts.insert(path.clone(),bytes.clone());}
    if !doc.final_section.header.is_empty(){parts.insert("word/header1.xml".into(),Writer::header_part(&doc.final_section.header,true));}
    if !doc.final_section.footer.is_empty(){parts.insert("word/footer1.xml".into(),Writer::header_part(&doc.final_section.footer,false));}
    let relationships=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"{PACKAGE_REL}\">{}</Relationships>",writer.relationships.join(""));
    // Preserve original content-type declarations for styles, theme, numbering,
    // embedded metadata and custom XML instead of discarding unrelated parts.
    let mut types=String::from_utf8(parts.get("[Content_Types].xml").cloned().unwrap_or_else(||
        b"<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"></Types>".to_vec()))
        .map_err(|_|bad("invalid package Content_Types UTF-8"))?;
    let close=types.rfind("</Types>").ok_or_else(||bad("malformed OPC content types"))?;
    let mut additions=String::new();
    for (ext,mime) in [("rels","application/vnd.openxmlformats-package.relationships+xml"),
        ("xml","application/xml"),("png","image/png"),("jpeg","image/jpeg"),("gif","image/gif")] {
        if !types.contains(&format!("Extension=\"{ext}\"")){additions.push_str(&format!("<Default Extension=\"{ext}\" ContentType=\"{mime}\"/>"));}
    }
    for (path,mime) in [("/word/document.xml","application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"),
        ("/word/styles.xml","application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"),
        ("/word/footnotes.xml","application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"),
        ("/word/endnotes.xml","application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"),
        ("/customXml/mataLayout.bin","application/octet-stream"),
        ("/word/header1.xml","application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"),
        ("/word/footer1.xml","application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml")] {
        if !types.contains(&format!("PartName=\"{path}\"")){additions.push_str(&format!("<Override PartName=\"{path}\" ContentType=\"{mime}\"/>"));}
    }
    types.insert_str(close,&additions);
    parts.insert("word/document.xml".into(),document.into_bytes());
    parts.insert("word/_rels/document.xml.rels".into(),relationships.into_bytes());
    parts.insert("[Content_Types].xml".into(),types.into_bytes());
    if !parts.contains_key("_rels/.rels") {parts.insert("_rels/.rels".into(),format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"{PACKAGE_REL}\"><Relationship Id=\"rId1\" Type=\"{OFFICE_REL}/officeDocument\" Target=\"word/document.xml\"/></Relationships>").into_bytes());}
    zip::write(&parts)
}

#[cfg(test)]mod rich_tests {
    use super::*;
    #[test]fn typed_model_roundtrip_tables_and_references(){
        let mut d=RichDocument::new();let i=d.insert_table(0,2,3).unwrap();d.set_cell_text(i,1,2,"unicode 北 & <hello>").unwrap();
        d.set_cell_span(i,0,0,2).unwrap();d.insert_bookmark(1,"Example").unwrap();d.insert_hyperlink(2,"Website","https://example.com").unwrap();
        d.insert_note(3,false,"footnote text");d.insert_note(4,true,"endnote text");d.insert_citation(5,"Source2026","Smith, 2026").unwrap();
        let encoded=encode_rich(&d).unwrap();let decoded=decode_rich(&encoded).unwrap();
        assert!(matches!(decoded.blocks[0],Block::Table(_)));
        let Block::Table(t)=&decoded.blocks[0] else {panic!("table lost")};
        assert_eq!(t.rows.len(),2);assert_eq!(t.rows[0].cells[0].column_span,2);
        assert!(decoded.to_plain_text().contains("unicode 北"));
        assert_eq!(decoded.notes.len(),2);assert!(decoded.to_plain_text().contains("Smith, 2026"));
        // Ensure external hyperlink relationships survive a second encode.
        assert!(encode_rich(&decoded).is_ok());
    }
    #[test]fn image_and_layout_roundtrip(){
        // Minimal valid PNG signature is sufficient for package-level media tests.
        let image=b"\x89PNG\r\n\x1a\nminimum-fixture".to_vec();
        let mut d=RichDocument::new();let id=d.insert_image(0,image.clone(),"diagram",2_000_000,3_000_000).unwrap();
        d.resize_image(&id,3_500_000,2_500_000).unwrap();
        d.final_section.landscape=true;d.final_section.width_twips=15840;d.final_section.height_twips=12240;
        d.final_section.margin_left=900;d.final_section.columns=2;
        d.final_section.header="Title".into();d.final_section.footer="Footer".into();
        let zipped=encode_rich(&d).unwrap();let files=zip::read(&zipped).unwrap();
        assert!(files.keys().any(|k|k.starts_with("word/media/image")));
        let reloaded=decode_rich(&zipped).unwrap();assert_eq!(reloaded.final_section.columns,2);
        assert_eq!(reloaded.final_section.header,"Title");assert_eq!(reloaded.final_section.footer,"Footer");
        assert!(reloaded.images.values().any(|v|v.width_emu==3_500_000 && v.bytes==image));
    }
    #[test]fn text_edits_preserve_word_run_formatting(){
        let mut d=RichDocument::new();
        let Block::Paragraph(p)=&mut d.blocks[0] else {unreachable!()};
        p.runs=vec![Inline::Text(Run{text:"Bold".into(),bold:true,..Run::default()}),
            Inline::Text(Run{text:" Normal".into(),..Run::default()})];
        assert!(d.apply_editor_text("Bold inserted Normal").unwrap());
        let Block::Paragraph(p)=&d.blocks[0] else {unreachable!()};
        assert!(matches!(&p.runs[0],Inline::Text(r) if r.bold && r.text=="Bold"));
        assert!(matches!(&p.runs[p.runs.len()-1],Inline::Text(r) if !r.bold && r.text.ends_with("Normal")));
    }
    #[test]fn structured_objects_cannot_be_replaced_by_typing(){
        let mut d=RichDocument::new();d.insert_table(1,2,2).unwrap();
        let original=d.editor_text();
        assert!(d.apply_editor_text(&original.replace("[Table: 2 rows]","deleted")).is_err());
        assert!(matches!(d.blocks[1],Block::Table(_)));
    }
    #[test]fn mata_page_layout_lives_inside_the_docx_package(){
        let mut d=RichDocument::new();
        d.original_parts.insert("customXml/mataLayout.bin".into(),b"MATAPG02\0test".to_vec());
        let bytes=encode_rich(&d).unwrap();
        let parts=zip::read(&bytes).unwrap();
        assert_eq!(parts.get("customXml/mataLayout.bin").unwrap(),b"MATAPG02\0test");
        let xml=std::str::from_utf8(parts.get("[Content_Types].xml").unwrap()).unwrap();
        assert!(xml.contains("/customXml/mataLayout.bin"));
        let roundtrip=decode_rich(&bytes).unwrap();
        assert_eq!(roundtrip.original_parts.get("customXml/mataLayout.bin").unwrap(),b"MATAPG02\0test");
    }
    #[test]fn model_rejects_bad_dimensions_and_urls(){
        let mut d=RichDocument::new();assert!(d.insert_table(0,0,2).is_err());
        assert!(d.insert_image(0,vec![1],"",1,1).is_err());
        assert!(d.insert_hyperlink(0,"bad","javascript:alert(1)").is_err());
    }
}
