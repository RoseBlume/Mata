//! Document object model and python-docx-inspired construction APIs.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use crate::xml;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug,Clone,Copy,PartialEq,Eq,Default)]pub enum Alignment {#[default] Left, Center, Right, Justify}
impl Alignment { pub fn as_str(self)->&'static str {match self {Self::Left=>"left",Self::Center=>"center",Self::Right=>"right",Self::Justify=>"both"}} }
#[derive(Debug,Clone,Copy,PartialEq,Eq,Default)]pub enum BreakKind {#[default] Line,Page,Column}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum SectionBreak {NextPage, Continuous, EvenPage, OddPage}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum Protection {ReadOnly,FormsOnly,CommentsOnly}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum StyleType {Paragraph,Character,Table}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum Orientation {Portrait,Landscape}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Default)]pub enum VerticalAlignment {#[default] Top,Center,Bottom}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Default)]pub struct Length(pub i64);
impl Length { pub fn inches(v:f64)->Self{Self((v*914400.0) as i64)} pub fn cm(v:f64)->Self{Self((v*360000.0) as i64)} pub fn pt(v:f64)->Self{Self((v*12700.0) as i64)} pub fn emu(v:i64)->Self{Self(v)} pub fn twips(self)->i64{self.0/635} }
#[derive(Debug,Clone,Default)]pub struct CoreProperties {pub title:String,pub subject:String,pub creator:String,pub keywords:String,pub description:String,pub last_modified_by:String,pub category:String,pub content_status:String,pub language:String,pub identifier:String,pub revision:Option<u32>,pub version:String,pub created:String,pub modified:String}
#[derive(Debug,Clone)]pub struct PageSettings {pub width_twips:u32,pub height_twips:u32,pub margin_top:u32,pub margin_bottom:u32,pub margin_left:u32,pub margin_right:u32,pub orientation:Orientation}
impl Default for PageSettings {fn default()->Self{Self{width_twips:12240,height_twips:15840,margin_top:1440,margin_bottom:1440,margin_left:1440,margin_right:1440,orientation:Orientation::Portrait}}}
/// WordprocessingML section document grid (`w:docGrid`). Line pitch uses
/// twentieths of a point. `char_space` preserves the OOXML integer value;
/// it does not imply full East Asian character-grid composition.
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct DocumentGrid{
    pub kind:String,
    pub line_pitch_twips:Option<u32>,
    pub char_space:Option<i32>,
}
impl Default for DocumentGrid{
    fn default()->Self{Self{kind:"lines".into(),line_pitch_twips:None,char_space:None}}
}
#[derive(Debug,Clone,Default)]pub struct Run {pub text:String,pub bold:bool,pub italic:bool,pub underline:bool,pub strike:bool,pub superscript:bool,pub subscript:bool,pub font_size:Option<f32>,pub font:Option<String>,pub color:Option<String>,pub highlight:Option<String>,pub style:Option<String>,pub hyperlink:Option<String>,pub internal_link:Option<String>,pub break_kind:Option<BreakKind>,
    pub(crate) original_xml:Option<xml::Element>,pub(crate) original_fingerprint:Option<String>}
impl Run {
    /// Fingerprint the modeled run state independently of imported extension XML.
    pub(crate) fn fingerprint(&self)->String {
        format!("{:?}{:?}",(&self.text,self.bold,self.italic,self.underline,self.strike,self.superscript,self.subscript),
            (self.font_size,&self.font,&self.color,&self.highlight,&self.style,&self.break_kind))
    }
pub fn new(text:impl Into<String>)->Self{Self{text:text.into(),..Self::default()}} pub fn bold(&mut self,v:bool)->&mut Self{self.bold=v;self} pub fn italic(&mut self,v:bool)->&mut Self{self.italic=v;self} pub fn underline(&mut self,v:bool)->&mut Self{self.underline=v;self} pub fn strike(&mut self,v:bool)->&mut Self{self.strike=v;self} pub fn font_size(&mut self,v:f32)->&mut Self{self.font_size=Some(v);self} pub fn font(&mut self,v:impl Into<String>)->&mut Self{self.font=Some(v.into());self} pub fn color(&mut self,v:impl Into<String>)->&mut Self{self.color=Some(v.into());self} pub fn highlight(&mut self,v:impl Into<String>)->&mut Self{self.highlight=Some(v.into());self} pub fn style(&mut self,v:impl Into<String>)->&mut Self{self.style=Some(v.into());self} pub fn hyperlink(&mut self,v:impl Into<String>)->&mut Self{self.hyperlink=Some(v.into());self} pub fn internal_link(&mut self,v:impl Into<String>)->&mut Self{self.internal_link=Some(v.into());self} pub fn add_break(&mut self,kind:BreakKind)->&mut Self{self.break_kind=Some(kind);self} }
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum RevisionKind {Insert,Delete,MoveFrom,MoveTo}
#[derive(Debug,Clone,Default)]
pub struct RevisionInfo {pub id:u32,pub author:String,pub date:Option<String>}
#[derive(Debug,Clone,Default)]pub struct ContentBinding {
    pub store_item_id:String,pub xpath:String,pub prefix_mappings:String,
}
#[derive(Debug,Clone,Default)]pub struct StructuredContentControl {
    pub tag:String,pub alias:String,pub id:Option<u32>,pub value:String,
    pub locked:bool,pub rich_text:bool,pub repeating:bool,
    pub binding:Option<ContentBinding>,
    /// Unmodeled extension properties and XML inside the original control.
    pub(crate) original:Option<xml::Element>,
}
#[derive(Debug,Clone)]pub enum Inline { ComplexField{instruction:String,result:String,locked:bool,dirty:bool}, StructuredControl(StructuredContentControl), Revision{kind:RevisionKind,info:RevisionInfo,content:Vec<Inline>}, MathExpr(MathExpr), Run(Run),Bookmark(String),FootnoteRef(u32),EndnoteRef(u32),CommentStart(u32),CommentEnd(u32),Image(usize),Field(String),RawXml(String),Math(String),Shape{width:Length,height:Length,fill:String,text:String},Chart(usize),TrackedInsert{author:String,text:String},TrackedDelete{author:String,text:String},ContentControl{tag:String,value:String},Checkbox{tag:String,checked:bool},Dropdown{tag:String,options:Vec<String>,selected:usize} }
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum LineSpacingRule{Auto,Exact,AtLeast}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum TabAlignment{Left,Center,Right,Decimal}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum TabLeader{None,Dots,Dashes,Line,MiddleDots}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub struct TabStop{
    pub position_twips:i32,pub alignment:TabAlignment,pub leader:TabLeader,
}
#[derive(Debug,Clone,Default)]pub struct Paragraph {pub inlines:Vec<Inline>,pub style:Option<String>,pub alignment:Alignment,pub spacing_before:Option<u32>,pub spacing_after:Option<u32>,pub line_spacing:Option<u32>,pub line_rule:Option<LineSpacingRule>,pub indent_left:Option<i32>,pub indent_right:Option<i32>,pub first_line:Option<i32>,pub keep_with_next:bool,pub keep_together:bool,pub page_break_before:bool,pub widow_control:Option<bool>,pub snap_to_grid:Option<bool>,pub contextual_spacing:bool,pub tab_stops:Vec<TabStop>,pub list:Option<ListInfo>,pub borders:Option<Border>,pub shading:Option<String>}
impl Paragraph {
    pub fn new(text:impl Into<String>)->Self{let mut p=Self::default();let t=text.into();if !t.is_empty(){p.add_run(t);}p}
    pub fn add_run(&mut self,text:impl Into<String>)->&mut Run {self.inlines.push(Inline::Run(Run::new(text)));match self.inlines.last_mut().unwrap(){Inline::Run(r)=>r,_=>unreachable!()}}
    pub fn text(&self)->String {let mut s=String::new();for i in &self.inlines {match i {Inline::Run(r)=>s.push_str(&r.text),Inline::Revision{kind,content,..}=>if matches!(kind,RevisionKind::Insert|RevisionKind::MoveTo){for c in content {if let Inline::Run(r)=c{s.push_str(&r.text);}}},Inline::MathExpr(expr)=>s.push_str(&expr.plain_text()),Inline::ContentControl{value,..}=>s.push_str(value),Inline::StructuredControl(c)=>s.push_str(&c.value),Inline::ComplexField{result,..}=>s.push_str(result),Inline::Math(v)|Inline::TrackedInsert{text:v,..}|Inline::Shape{text:v,..}=>s.push_str(v),Inline::Dropdown{options,selected,..}=>if let Some(v)=options.get(*selected){s.push_str(v)},_=>{}}}s}
    pub fn style(&mut self,v:impl Into<String>)->&mut Self{self.style=Some(v.into());self}
    pub fn alignment(&mut self,v:Alignment)->&mut Self{self.alignment=v;self}
    pub fn add_hyperlink(&mut self,url:impl Into<String>,text:impl Into<String>)->&mut Run{let r=self.add_run(text);r.hyperlink=Some(url.into());r}
    pub fn add_internal_link(&mut self,bookmark:impl Into<String>,text:impl Into<String>)->&mut Run{let r=self.add_run(text);r.internal_link=Some(bookmark.into());r}
    pub fn add_bookmark(&mut self,name:impl Into<String>)->&mut Self{self.inlines.push(Inline::Bookmark(name.into()));self}
    pub fn add_footnote_ref(&mut self,id:u32)->&mut Self{self.inlines.push(Inline::FootnoteRef(id));self}
    pub fn add_endnote_ref(&mut self,id:u32)->&mut Self{self.inlines.push(Inline::EndnoteRef(id));self}
    pub fn comment_range_start(&mut self,id:u32)->&mut Self{self.inlines.push(Inline::CommentStart(id));self}
    pub fn comment_range_end(&mut self,id:u32)->&mut Self{self.inlines.push(Inline::CommentEnd(id));self}
    pub fn add_field(&mut self,code:impl Into<String>)->&mut Self{self.inlines.push(Inline::Field(code.into()));self}
    /// A field with an explicit cached display result. Word may refresh the
    /// result when the document is opened, depending on field settings.
    pub fn add_complex_field(&mut self,instruction:impl Into<String>,result:impl Into<String>)->&mut Self{
        self.inlines.push(Inline::ComplexField{instruction:instruction.into(),result:result.into(),locked:false,dirty:false});self
    }
    pub fn add_structured_control(&mut self,control:StructuredContentControl)->&mut Self{
        self.inlines.push(Inline::StructuredControl(control));self
    }
    pub fn add_text_field(&mut self,name:impl Into<String>,value:impl Into<String>)->&mut Self{self.inlines.push(Inline::ContentControl{tag:name.into(),value:value.into()});self}
    pub fn add_checkbox(&mut self,name:impl Into<String>,checked:bool)->&mut Self{self.inlines.push(Inline::Checkbox{tag:name.into(),checked});self}
    pub fn add_dropdown(&mut self,name:impl Into<String>,options:Vec<String>,selected:usize)->&mut Self{self.inlines.push(Inline::Dropdown{tag:name.into(),options,selected});self}
    pub fn add_raw_xml(&mut self,xml:impl Into<String>)->&mut Self{self.inlines.push(Inline::RawXml(xml.into()));self}
    pub fn add_image(&mut self,index:usize)->&mut Self{self.inlines.push(Inline::Image(index));self}
    pub fn add_chart(&mut self,index:usize)->&mut Self{self.inlines.push(Inline::Chart(index));self}
    pub fn add_math(&mut self,formula:impl Into<String>)->&mut Self{self.inlines.push(Inline::Math(formula.into()));self}
    pub fn add_math_expression(&mut self,expr:MathExpr)->&mut Self{self.inlines.push(Inline::MathExpr(expr));self}
    pub fn add_page_number(&mut self)->&mut Self{self.add_field("PAGE")}
    pub fn add_total_pages(&mut self)->&mut Self{self.add_field("NUMPAGES")}
    pub fn add_cross_reference(&mut self,bookmark:&str)->&mut Self{self.add_field(format!("REF {bookmark} \u{005c}h"))}
    pub fn add_shape(&mut self,width:Length,height:Length,fill:impl Into<String>,text:impl Into<String>)->&mut Self{self.inlines.push(Inline::Shape{width,height,fill:fill.into(),text:text.into()});self}
    pub fn add_revision(&mut self,kind:RevisionKind,info:RevisionInfo,content:Vec<Inline>)->&mut Self {
        self.inlines.push(Inline::Revision{kind,info,content});self
    }
    pub fn add_tracked_insert(&mut self,author:impl Into<String>,text:impl Into<String>)->&mut Self{self.inlines.push(Inline::TrackedInsert{author:author.into(),text:text.into()});self}
    pub fn add_tracked_delete(&mut self,author:impl Into<String>,text:impl Into<String>)->&mut Self{self.inlines.push(Inline::TrackedDelete{author:author.into(),text:text.into()});self}
    pub fn numbering(&mut self,num_id:u32,level:u8)->&mut Self{self.list=Some(ListInfo{num_id,level,bullet:num_id==2});self}
}
#[derive(Debug,Clone,Copy)]pub struct ListInfo{pub num_id:u32,pub level:u8,pub bullet:bool}
/// Structured equation nodes, emitted as native Office Math Markup Language.
#[derive(Debug,Clone,PartialEq)]pub enum MathExpr {
    Text(String), Row(Vec<MathExpr>), Fraction(Box<MathExpr>,Box<MathExpr>),
    Superscript(Box<MathExpr>,Box<MathExpr>),Subscript(Box<MathExpr>,Box<MathExpr>),
    Radical(Box<MathExpr>), Nary {symbol:String,lower:Option<Box<MathExpr>>,upper:Option<Box<MathExpr>>,body:Box<MathExpr>},
    Matrix(Vec<Vec<MathExpr>>), Delimiter{begin:String,end:String,content:Box<MathExpr>},
    Function{function:Box<MathExpr>,argument:Box<MathExpr>},
    SubSup{base:Box<MathExpr>,sub:Box<MathExpr>,sup:Box<MathExpr>},
    Accent{symbol:String,body:Box<MathExpr>},
    EquationArray(Vec<MathExpr>),
    /// Lossless escape hatch for any OMML element not represented by a typed variant.
    /// The caller must provide well-formed Office Math XML.
    RawXml(String),
}
impl MathExpr {pub fn plain_text(&self)->String{match self{
    Self::Text(s)=>s.clone(),Self::Row(items)=>items.iter().map(Self::plain_text).collect(),
    Self::Fraction(a,b)=>format!("({})/({})",a.plain_text(),b.plain_text()),
    Self::Superscript(a,b)=>format!("{}^{}",a.plain_text(),b.plain_text()),
    Self::Subscript(a,b)=>format!("{}_{}",a.plain_text(),b.plain_text()),
    Self::Radical(a)=>format!("√({})",a.plain_text()),
    Self::Nary{symbol,lower,upper,body}=>format!("{}({},{}) {}",symbol,lower.as_ref().map(|v|v.plain_text()).unwrap_or_default(),upper.as_ref().map(|v|v.plain_text()).unwrap_or_default(),body.plain_text()),
    Self::Matrix(rows)=>rows.iter().map(|r|r.iter().map(Self::plain_text).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join("; "),
    Self::Delimiter{begin,end,content}=>format!("{begin}{}{end}",content.plain_text()),
    Self::Function{function,argument}=>format!("{}({})",function.plain_text(),argument.plain_text()),
    Self::SubSup{base,sub,sup}=>format!("{}_{}^{}",base.plain_text(),sub.plain_text(),sup.plain_text()),
    Self::Accent{symbol,body}=>format!("{}{}",body.plain_text(),symbol),
    Self::EquationArray(items)=>items.iter().map(Self::plain_text).collect::<Vec<_>>().join("; "),
    Self::RawXml(raw)=>crate::xml::parse(raw).map(|e|e.text()).unwrap_or_default(),
}}}
/// A section ends after `end_after` top-level document blocks (exclusive).
#[derive(Debug,Clone)]pub struct Section {
    pub end_after:usize,pub break_kind:SectionBreak,pub page:PageSettings,
    pub columns:u16,pub page_number_start:Option<u32>,
    /// Word's ST_NumberFormat (decimal, lowerRoman, upperLetter, etc.).
    pub page_number_format:Option<String>,
    pub column_spacing_twips:u32,pub columns_equal_width:bool,
    pub column_widths_twips:Vec<(u32,u32)>,
    pub mirror_margins:bool,pub gutter_twips:u32,
    pub header_distance_twips:u32,pub footer_distance_twips:u32,
    pub title_page:bool,
    pub doc_grid:Option<DocumentGrid>,
    /// Imported `r:id` values from document.xml.rels, keyed by header/footer type.
    pub header_refs:BTreeMap<String,String>,pub footer_refs:BTreeMap<String,String>,
    /// Optional additional section-property XML from other Office extensions.
    pub extra_properties_xml:Vec<String>,
    pub(crate) original_xml:Option<xml::Element>,
}
impl Section {
    pub fn new(end_after:usize,break_kind:SectionBreak,page:PageSettings)->Self{
        Self{end_after,break_kind,page,columns:1,page_number_start:None,
            page_number_format:None,column_spacing_twips:720,columns_equal_width:true,column_widths_twips:Vec::new(),
            mirror_margins:false,gutter_twips:0,header_distance_twips:720,footer_distance_twips:720,
            title_page:false,doc_grid:None,header_refs:BTreeMap::new(),footer_refs:BTreeMap::new(),
            extra_properties_xml:Vec::new(),original_xml:None}
    }
}
/// Glossary/document building block (Word `docParts`).
#[derive(Debug,Clone)]pub struct BuildingBlock {pub name:String,pub category:String,pub paragraphs:Vec<Paragraph>}
/// Caller-supplied obfuscated OpenType font bytes (`.odttf`). The caller must
/// verify that the font license permits embedding.
#[derive(Debug,Clone)]pub struct EmbeddedFont {pub name:String,pub obfuscated_data:Vec<u8>,pub key_guid:String}
#[derive(Debug,Clone,Default)]pub struct Border {pub color:String,pub size:u32,pub style:String}
/// Independent OOXML table/cell border edges. Unlike the legacy `borders`
/// shortcut this retains mixed edge styles and Word's inside borders.
#[derive(Debug,Clone,Default)]pub struct BorderEdges{
 pub top:Option<Border>,pub bottom:Option<Border>,pub left:Option<Border>,pub right:Option<Border>,
 pub inside_h:Option<Border>,pub inside_v:Option<Border>,
 pub start:Option<Border>,pub end:Option<Border>,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum TableEdge{Top,Bottom,Left,Right}
impl BorderEdges{
 pub fn edge(&self,side:TableEdge)->Option<&Border>{match side{
  TableEdge::Top=>self.top.as_ref(),TableEdge::Bottom=>self.bottom.as_ref(),
  TableEdge::Left=>self.left.as_ref().or(self.start.as_ref()),
  TableEdge::Right=>self.right.as_ref().or(self.end.as_ref())}}
}
/// Word uses a hierarchy of border conflicts. For two explicit adjacent cell
/// borders, prefer the one with greater width and then the heavier style.
/// This function deliberately does not infer physical color from theme names.
pub fn stronger_table_border(a:Option<&Border>,b:Option<&Border>)->Option<Border>{
 let visible=|v:&Border|!matches!(v.style.as_str(),"nil"|"none");
 let rank=|v:&Border|{
  let weight=match v.style.as_str(){"double"=>12,"thick"=>11,"single"=>10,"dotted"=>8,"dashed"=>7,"dotDash"=>6,"dotDotDash"=>5,_=>1};
  (if visible(v){1}else{0},v.size,weight)
 };
 match (a,b){(None,None)=>None,(Some(x),None)=>Some(x.clone()),(None,Some(y))=>Some(y.clone()),
  (Some(x),Some(y))=>Some((if rank(x)>=rank(y){x}else{y}).clone())}
}
#[derive(Debug,Clone,Default)]pub struct Cell {pub paragraphs:Vec<Paragraph>,pub width_twips:Option<u32>,pub colspan:u32,pub vertical_merge:Option<bool>,pub shading:Option<String>,pub valign:VerticalAlignment,pub borders:Option<Border>,pub edge_borders:Option<BorderEdges>,pub text_direction:Option<String>}
impl Cell {pub fn new()->Self{Self{paragraphs:vec![Paragraph::default()],colspan:1,..Self::default()}} pub fn text(&self)->String{self.paragraphs.iter().map(Paragraph::text).collect::<Vec<_>>().join("\n")}pub fn set_text(&mut self,text:impl Into<String>)->&mut Self{self.paragraphs=vec![Paragraph::new(text)];self} pub fn add_paragraph(&mut self,text:impl Into<String>)->&mut Paragraph{self.paragraphs.push(Paragraph::new(text));self.paragraphs.last_mut().unwrap()} pub fn merge_across(&mut self,count:u32)->&mut Self{self.colspan=count.max(1);self}pub fn shade(&mut self,color:impl Into<String>)->&mut Self{self.shading=Some(color.into());self}}
/// Row height semantics from OOXML `w:trHeight/@w:hRule`.
#[derive(Debug,Clone,Copy,PartialEq,Eq,Default)]
pub enum RowHeightRule {#[default] Auto,AtLeast,Exact}
#[derive(Debug,Clone,Default)]pub struct Row {pub cells:Vec<Cell>,pub height_twips:Option<u32>,pub height_rule:RowHeightRule,pub grid_before:u32,pub grid_after:u32,pub is_header:bool,pub cant_split:bool}
impl Row {pub fn cells(&self)->&[Cell]{&self.cells}pub fn cells_mut(&mut self)->&mut [Cell]{&mut self.cells}}
#[derive(Debug,Clone,Default)]pub struct Table {pub rows:Vec<Row>,pub column_widths:Vec<u32>,pub style:Option<String>,pub borders:Option<Border>,pub edge_borders:Option<BorderEdges>,pub width_twips:Option<u32>,pub width_percent:Option<f32>,pub layout_fixed:bool,pub alignment:Alignment,pub indent_twips:Option<i32>,pub cell_margin_twips:Option<u32>}
impl Table {pub fn new(nrows:usize,ncols:usize)->Self{Self{rows:(0..nrows).map(|_|Row{cells:(0..ncols).map(|_|Cell::new()).collect(),..Row::default()}).collect(),column_widths:vec![2160;ncols],..Self::default()}} pub fn rows(&self)->&[Row]{&self.rows}pub fn rows_mut(&mut self)->&mut [Row]{&mut self.rows}pub fn cell(&self,row:usize,col:usize)->Option<&Cell>{self.rows.get(row)?.cells.get(col)}pub fn cell_mut(&mut self,row:usize,col:usize)->Option<&mut Cell>{self.rows.get_mut(row)?.cells.get_mut(col)}pub fn set_column_width(&mut self,index:usize,twips:u32){if let Some(w)=self.column_widths.get_mut(index){*w=twips;}}pub fn add_row(&mut self)->&mut Row{self.rows.push(Row{cells:(0..self.column_widths.len()).map(|_|Cell::new()).collect(),..Row::default()});self.rows.last_mut().unwrap()}}
#[derive(Debug,Clone)]pub enum Block {Paragraph(Paragraph),Table(Table)}
#[derive(Debug,Clone)]pub struct Style {pub id:String,pub name:String,pub kind:StyleType,pub based_on:Option<String>,pub font:Option<String>,pub size:Option<f32>,pub bold:bool,pub italic:bool,pub color:Option<String>,
    /// Explicit true/false values allow a child style to override its parent.
    pub bold_override:Option<bool>,pub italic_override:Option<bool>}
impl Style {pub fn paragraph(id:impl Into<String>,name:impl Into<String>)->Self{Self{id:id.into(),name:name.into(),kind:StyleType::Paragraph,based_on:None,font:None,size:None,bold:false,italic:false,color:None,bold_override:None,italic_override:None}}}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ImageWrap {Square,Tight,Through,TopAndBottom,None}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum RelativePosition {Page,Margin,Column,Paragraph,Line,Character}
impl RelativePosition {pub fn as_str(self)->&'static str {match self{
    Self::Page=>"page",Self::Margin=>"margin",Self::Column=>"column",Self::Paragraph=>"paragraph",Self::Line=>"line",Self::Character=>"character"}}}
#[derive(Debug,Clone)]pub struct FloatingOptions {
    pub horizontal:RelativePosition,pub vertical:RelativePosition,
    pub x_emu:i64,pub y_emu:i64,pub wrap:ImageWrap,
    pub distance_top:i64,pub distance_bottom:i64,pub distance_left:i64,pub distance_right:i64,
    pub behind_text:bool,pub allow_overlap:bool,pub layout_in_cell:bool,
    pub z_order:u32,pub rotation_degrees:f64,
    /// Closed DrawingML wrap outline, in normalized 0..21600 picture space.
    /// The first point is the start; remaining points form the polygon edges.
    pub wrap_polygon:Vec<(i64,i64)>,
}
impl Default for FloatingOptions {fn default()->Self{Self{
    horizontal:RelativePosition::Column,vertical:RelativePosition::Paragraph,x_emu:0,y_emu:0,
    wrap:ImageWrap::Square,distance_top:0,distance_bottom:0,distance_left:114300,distance_right:114300,
    behind_text:false,allow_overlap:true,layout_in_cell:true,z_order:251658240,rotation_degrees:0.0,
    wrap_polygon:Vec::new(),
}}}
#[derive(Debug,Clone)]pub struct Image {pub filename:String,pub bytes:Vec<u8>,pub content_type:String,pub width:Length,pub height:Length,pub floating:bool,
    pub positioning:Option<FloatingOptions>}
impl Image {pub fn float(&mut self,position:FloatingOptions)->&mut Self{self.floating=true;self.positioning=Some(position);self}}

#[derive(Debug,Clone)]pub struct Note{pub id:u32,pub text:String}
#[derive(Debug,Clone)]pub struct Comment{pub id:u32,pub author:String,pub text:String,pub parent:Option<u32>}
/// Cross-version metadata for Word comments and threaded replies.
#[derive(Debug,Clone,Default)]pub struct CommentMetadata {
    pub paragraph_id:Option<String>,pub durable_id:Option<String>,pub resolved:bool,
    pub initials:Option<String>,pub created:Option<String>,
    pub provider_id:Option<String>,pub user_id:Option<String>,
    pub mentions:Vec<String>,
}
#[derive(Debug,Clone)]pub struct CustomXml {pub name:String,pub xml:String}
#[derive(Debug,Clone)]pub struct Chart {pub title:String,pub categories:Vec<String>,pub values:Vec<f64>}
#[derive(Debug,Clone)]pub struct Document {
    pub blocks:Vec<Block>,pub properties:CoreProperties,pub page:PageSettings,pub styles:Vec<Style>,
    pub theme_colors:BTreeMap<String,String>,pub default_run_style:Option<Style>,
    pub header:Vec<Paragraph>,pub footer:Vec<Paragraph>,pub first_header:Vec<Paragraph>,pub first_footer:Vec<Paragraph>,
    pub images:Vec<Image>,pub footnotes:Vec<Note>,pub endnotes:Vec<Note>,pub comments:Vec<Comment>,pub comment_metadata:BTreeMap<u32,CommentMetadata>,pub custom_xml:Vec<CustomXml>,
    pub charts:Vec<Chart>,pub protection:Option<Protection>,pub watermark:Option<String>,pub track_changes:Option<String>,
    pub sections:Vec<Section>,pub final_section:Option<Section>,pub glossary:Vec<BuildingBlock>,pub embedded_fonts:Vec<EmbeddedFont>,
    pub custom_xml_item_props:BTreeMap<String,String>,
    pub package_overrides:BTreeMap<String,Vec<u8>>,pub removed_package_parts:BTreeSet<String>,
    pub(crate) original_parts:BTreeMap<String,Vec<u8>>,
    pub(crate) original_body:Option<crate::xml::Element>,
    pub(crate) original_block_debug:Vec<String>,
    pub(crate) original_snapshot:Option<String>,
    pub(crate) original_components:BTreeMap<String,String>,
}
impl Default for Document {fn default()->Self{Self{blocks:Vec::new(),properties:CoreProperties::default(),page:PageSettings::default(),styles:Vec::new(),theme_colors:BTreeMap::new(),default_run_style:None,header:Vec::new(),footer:Vec::new(),first_header:Vec::new(),first_footer:Vec::new(),images:Vec::new(),footnotes:Vec::new(),endnotes:Vec::new(),comments:Vec::new(),comment_metadata:BTreeMap::new(),custom_xml:Vec::new(),charts:Vec::new(),protection:None,watermark:None,track_changes:None,sections:Vec::new(),final_section:None,glossary:Vec::new(),embedded_fonts:Vec::new(),custom_xml_item_props:BTreeMap::new(),package_overrides:BTreeMap::new(),removed_package_parts:BTreeSet::new(),original_parts:BTreeMap::new(),original_body:None,original_block_debug:Vec::new(),original_snapshot:None,original_components:BTreeMap::new()}}}
impl Document {
    pub fn new()->Self{Self::default()}
    pub fn add_paragraph(&mut self,text:impl Into<String>)->&mut Paragraph{self.blocks.push(Block::Paragraph(Paragraph::new(text)));match self.blocks.last_mut().unwrap(){Block::Paragraph(p)=>p,_=>unreachable!()}}
    pub fn add_table(&mut self,rows:usize,cols:usize)->&mut Table{self.blocks.push(Block::Table(Table::new(rows,cols)));match self.blocks.last_mut().unwrap(){Block::Table(t)=>t,_=>unreachable!()}}
    pub fn paragraphs(&self)->impl Iterator<Item=&Paragraph>{self.blocks.iter().filter_map(|b|match b{Block::Paragraph(p)=>Some(p),_=>None})}
    pub fn paragraphs_mut(&mut self)->impl Iterator<Item=&mut Paragraph>{self.blocks.iter_mut().filter_map(|b|match b{Block::Paragraph(p)=>Some(p),_=>None})}
    pub fn tables(&self)->impl Iterator<Item=&Table>{self.blocks.iter().filter_map(|b|match b{Block::Table(t)=>Some(t),_=>None})}
    pub fn tables_mut(&mut self)->impl Iterator<Item=&mut Table>{self.blocks.iter_mut().filter_map(|b|match b{Block::Table(t)=>Some(t),_=>None})}
    pub fn add_heading(&mut self,title:impl Into<String>,level:u8)->&mut Paragraph{let p=self.add_paragraph(title);p.style(format!("Heading{}",level.clamp(1,9)));p}
    pub fn add_page_break(&mut self)->&mut Paragraph {let p=self.add_paragraph("");p.add_run("").add_break(BreakKind::Page);p}
    pub fn add_image(&mut self,filename:impl Into<String>,bytes:Vec<u8>,width:Length,height:Length)->usize{
        let filename=filename.into();let content_type=match filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str(){"png"=>"image/png","jpg"|"jpeg"=>"image/jpeg","gif"=>"image/gif","bmp"=>"image/bmp","svg"=>"image/svg+xml",_=>"application/octet-stream"}.to_string();
        let ix=self.images.len();self.images.push(Image{filename,bytes,content_type,width,height,floating:false,positioning:None});ix
    }
    pub fn add_image_paragraph(&mut self,index:usize)->&mut Paragraph{let p=self.add_paragraph("");p.add_image(index);p}
    pub fn add_footnote(&mut self,text:impl Into<String>)->u32{let id=self.footnotes.len() as u32+1;self.footnotes.push(Note{id,text:text.into()});id}
    pub fn add_endnote(&mut self,text:impl Into<String>)->u32{let id=self.endnotes.len() as u32+1;self.endnotes.push(Note{id,text:text.into()});id}
    pub fn add_comment(&mut self,author:impl Into<String>,text:impl Into<String>)->u32{let id=self.comments.len() as u32;self.comments.push(Comment{id,author:author.into(),text:text.into(),parent:None});id}
    pub fn set_text_watermark(&mut self,text:impl Into<String>){self.watermark=Some(text.into());}
    pub fn remove_watermark(&mut self){self.watermark=None;}
    pub fn protect(&mut self,mode:Protection){self.protection=Some(mode);}
    pub fn unprotect(&mut self){self.protection=None;}
    pub fn is_protected(&self)->bool{self.protection.is_some()}
    pub fn enable_track_changes(&mut self,author:impl Into<String>){self.track_changes=Some(author.into());}
    pub fn add_style(&mut self,style:Style){self.styles.push(style);}
    /// The last section's settings are stored separately from section breaks.
    pub fn final_section_mut(&mut self)->&mut Section {
        if self.final_section.is_none(){self.final_section=Some(Section::new(self.blocks.len(),SectionBreak::NextPage,self.page.clone()));}
        self.final_section.as_mut().unwrap()
    }
    pub fn add_section_break(&mut self,kind:SectionBreak)->&mut Section {
        self.add_paragraph("");
        self.sections.push(Section::new(self.blocks.len(),kind,self.page.clone()));
        self.sections.last_mut().unwrap()
    }
    pub fn set_comment_metadata(&mut self,id:u32,metadata:CommentMetadata)->bool {
        if self.comments.iter().any(|c|c.id==id){self.comment_metadata.insert(id,metadata);true}else{false}
    }
    pub fn add_comment_reply(&mut self,parent:u32,author:impl Into<String>,text:impl Into<String>)->Option<u32>{
        if !self.comments.iter().any(|c|c.id==parent){return None;}
        let id=self.comments.iter().map(|c|c.id).max().map(|v|v+1).unwrap_or(0);
        self.comments.push(Comment{id,author:author.into(),text:text.into(),parent:Some(parent)});Some(id)
    }
    pub fn add_building_block(&mut self,name:impl Into<String>,category:impl Into<String>,paragraphs:Vec<Paragraph>){
        self.glossary.push(BuildingBlock{name:name.into(),category:category.into(),paragraphs});
    }
    pub fn add_embedded_font(&mut self,font:EmbeddedFont){self.embedded_fonts.push(font);}
    /// Obfuscate a font and embed it only when its OS/2 flags permit this.
    /// The font license must independently allow embedding.
    pub fn embed_font(&mut self,name:impl Into<String>,font:&[u8],guid:&str)->io::Result<()> {
        if !crate::fonts::embedding_allowed(font)? {return Err(io::Error::new(io::ErrorKind::PermissionDenied,"font OS/2 flags forbid embedding"));}
        self.embedded_fonts.push(EmbeddedFont{name:name.into(),obfuscated_data:crate::fonts::obfuscate(font,guid)?,key_guid:guid.to_owned()});Ok(())
    }
    pub fn extract_embedded_font(&self,index:usize)->io::Result<Vec<u8>> {
        let font=self.embedded_fonts.get(index).ok_or_else(||io::Error::new(io::ErrorKind::NotFound,"embedded font index out of range"))?;
        crate::fonts::deobfuscate(&font.obfuscated_data,&font.key_guid)
    }
    pub fn add_custom_xml_item_props(&mut self,item_name:impl Into<String>,xml:impl Into<String>){self.custom_xml_item_props.insert(item_name.into(),xml.into());}
    /// Flatten tracked changes according to the selected revision policy.
    pub fn accept_revisions(&mut self){self.finish_revisions(true);}
    pub fn reject_revisions(&mut self){self.finish_revisions(false);}
    fn finish_revisions(&mut self,accept:bool){
        let update=|p:&mut Paragraph|{
            let mut new_content=Vec::new();
            for inline in p.inlines.drain(..) {
                match inline {
                    Inline::TrackedInsert{text,..}=>if accept {new_content.push(Inline::Run(Run::new(text)));},
                    Inline::TrackedDelete{text,..}=>if !accept {new_content.push(Inline::Run(Run::new(text)));},
                    Inline::Revision{kind,content,..}=>{
                        if accept==matches!(kind,RevisionKind::Insert|RevisionKind::MoveTo){new_content.extend(content);}
                    },
                    other=>new_content.push(other),
                }
            }
            p.inlines=new_content;
        };
        for block in &mut self.blocks{match block{Block::Paragraph(p)=>update(p),Block::Table(t)=>for row in &mut t.rows{for cell in &mut row.cells{for p in &mut cell.paragraphs{update(p);}}}}}
        self.track_changes=None;
    }
    /// Resolve a style through its `based_on` chain, with cycle detection.
    pub fn resolved_style(&self,id:&str)->Option<Style>{
        let mut chain=Vec::new();let mut current=id;
        loop {if chain.contains(&current){return None;}
            let Some(style)=self.styles.iter().find(|v|v.id==current) else{break;};
            chain.push(current);if let Some(parent)=style.based_on.as_deref(){current=parent;}else{break;}
        }
        if chain.is_empty(){return None;}
        let mut result=self.styles.iter().find(|v|v.id==chain[0])?.clone();
        for name in chain.iter().skip(1){if let Some(parent)=self.styles.iter().find(|v|v.id==*name){
            if result.font.is_none(){result.font=parent.font.clone();}
            if result.size.is_none(){result.size=parent.size;}
            if result.color.is_none(){result.color=parent.color.clone();}
            if result.bold_override.is_none(){result.bold_override=parent.bold_override;}
            if result.italic_override.is_none(){result.italic_override=parent.italic_override;}
        }}
        if let Some(default)=&self.default_run_style{
            if result.font.is_none(){result.font=default.font.clone();}
            if result.size.is_none(){result.size=default.size;}
            if result.color.is_none(){result.color=default.color.clone();}
            if result.bold_override.is_none(){result.bold_override=default.bold_override;}
            if result.italic_override.is_none(){result.italic_override=default.italic_override;}
        }
        result.bold=result.bold_override.unwrap_or(false);
        result.italic=result.italic_override.unwrap_or(false);
        if let Some(color)=&result.color {if let Some(key)=color.strip_prefix("theme:"){
            if let Some(hex)=self.theme_colors.get(key){result.color=Some(hex.clone());}
        }}
        Some(result)
    }
    /// Resolve paragraph and run style inheritance, document defaults, and
    /// theme color references. Direct run formatting has precedence.
    pub fn resolved_run(&self,run:&Run,paragraph_style:Option<&str>)->Run {
        let mut out=run.clone();
        let p=paragraph_style.and_then(|id|self.resolved_style(id));
        let c=run.style.as_deref().and_then(|id|self.resolved_style(id));
        for style in [c.as_ref(),p.as_ref(),self.default_run_style.as_ref()].into_iter().flatten() {
            if out.font.is_none(){out.font=style.font.clone();}
            if out.font_size.is_none(){out.font_size=style.size;}
            if out.color.is_none(){out.color=style.color.clone();}
            if !out.bold {out.bold=style.bold_override.unwrap_or(style.bold);}
            if !out.italic {out.italic=style.italic_override.unwrap_or(style.italic);}
        }
        if let Some(color)=out.color.as_ref().and_then(|c|c.strip_prefix("theme:")) {
            if let Some(value)=self.theme_colors.get(color){out.color=Some(value.clone());}
        }
        out
    }
    pub(crate) fn snapshot(&self)->String{
        format!("{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
            self.blocks,self.properties,self.page,self.styles,self.theme_colors,self.default_run_style,self.header,self.footer,self.first_header,self.first_footer,
            self.images,self.footnotes,self.endnotes,self.comments,self.comment_metadata,self.custom_xml,self.charts,self.protection,
            self.watermark,self.track_changes,self.sections,self.final_section,self.glossary,self.embedded_fonts,self.custom_xml_item_props,self.package_overrides,self.removed_package_parts)
    }
    /// Access an original OPC part or a caller-supplied replacement.
    pub fn part(&self,path:&str)->Option<&[u8]> {
        if self.removed_package_parts.contains(path){return None;}
        self.package_overrides.get(path).or_else(||self.original_parts.get(path)).map(Vec::as_slice)
    }
    /// Replace or add a package part. Relationship definitions and content types
    /// must be managed by the caller for arbitrary extension parts.
    pub fn put_part(&mut self,path:impl Into<String>,bytes:Vec<u8>)->io::Result<()> {
        let path=path.into();
        if path.is_empty()||path.starts_with('/')||path.contains('\\')||path.split('/').any(|s|s==".."||s=="."||s.is_empty()) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid OPC part name"));
        }
        if path=="word/document.xml" || path=="[Content_Types].xml" || path=="_rels/.rels" {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,"use the document model or OPC package API for mandatory parts"));
        }
        self.removed_package_parts.remove(&path);self.package_overrides.insert(path,bytes);Ok(())
    }
    pub fn remove_part(&mut self,path:&str)->io::Result<()> {
        if matches!(path,"word/document.xml"|"[Content_Types].xml"|"_rels/.rels") {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,"cannot remove mandatory OPC part"));
        }
        if path.is_empty()||path.starts_with('/')||path.split('/').any(|s|s==".."){
            return Err(io::Error::new(io::ErrorKind::InvalidInput,"invalid OPC part name"));
        }
        self.package_overrides.remove(path);self.removed_package_parts.insert(path.to_owned());Ok(())
    }
    pub fn add_chart(&mut self,title:impl Into<String>,categories:Vec<String>,values:Vec<f64>)->usize {let i=self.charts.len();self.charts.push(Chart{title:title.into(),categories,values});i}
    pub fn add_custom_xml(&mut self,name:impl Into<String>,xml:impl Into<String>){self.custom_xml.push(CustomXml{name:name.into(),xml:xml.into()});}
    /// Resolve an unambiguous absolute child-element XPath used by a content
    /// control. Unsupported XPath expressions return InvalidInput rather than
    /// silently choosing the wrong node. Prefixes compare by local name.
    pub fn custom_xml_value(&self,store_id:&str,xpath:&str)->io::Result<Option<String>> {
        let mut item=None;
        for c in &self.custom_xml {
            if let Some(props)=self.custom_xml_item_props.get(&c.name) {
                if let Ok(root)=xml::parse(props){if root.attr("itemID").map(|v|v.eq_ignore_ascii_case(store_id)).unwrap_or(false){item=Some(c);break;}}
            }
        }
        let Some(item)=item else{return Ok(None)};
        if !xpath.starts_with('/') || xpath.starts_with("//") || xpath.contains("::") || xpath.contains('|') {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,"unsupported custom XML XPath"));
        }
        let root=xml::parse(&item.xml)?;
        let segments=xpath.trim_start_matches('/').split('/').collect::<Vec<_>>();
        let mut current=&root;
        for (i,segment) in segments.iter().enumerate(){
            if let Some(attr)=segment.strip_prefix('@') {
                if i!=segments.len()-1{return Err(io::Error::new(io::ErrorKind::InvalidInput,"XPath attribute must be final"));}
                return Ok(current.attr(attr.rsplit(':').next().unwrap_or(attr)).map(str::to_owned));
            }
            let (tag,index)=if let Some((name,idx))=segment.split_once('[') {
                let n=idx.strip_suffix(']').and_then(|v|v.parse::<usize>().ok()).ok_or_else(||io::Error::new(io::ErrorKind::InvalidInput,"unsupported XPath predicate"))?;
                (name,n)
            }else{(*segment,1)};
            if index==0{return Err(io::Error::new(io::ErrorKind::InvalidInput,"XPath positions start at 1"));}
            let local=tag.rsplit(':').next().unwrap_or(tag);
            if i==0 && current.local()==local && index==1{continue;}
            match current.elements(local).nth(index-1){Some(node)=>current=node,None=>return Ok(None)}
        }
        Ok(Some(current.text()))
    }
    /// Update bound simple textual content controls from custom XML parts.
    pub fn refresh_bound_controls(&mut self)->io::Result<usize> {
        let bindings:Vec<_>=self.blocks.iter().flat_map(|b|match b{Block::Paragraph(p)=>vec![p],Block::Table(t)=>t.rows.iter().flat_map(|r|r.cells.iter().flat_map(|c|c.paragraphs.iter())).collect()})
            .flat_map(|p|p.inlines.iter()).filter_map(|inline|match inline{Inline::StructuredControl(c)=>c.binding.clone(),_=>None})
            .collect();
        let mut values=HashMap::new();
        for b in bindings {if let Some(v)=self.custom_xml_value(&b.store_item_id,&b.xpath)?{values.insert((b.store_item_id,b.xpath),v);}}
        let mut count=0;
        for block in &mut self.blocks{let paras:Vec<&mut Paragraph>=match block{
            Block::Paragraph(p)=>vec![p],Block::Table(t)=>t.rows.iter_mut().flat_map(|r|r.cells.iter_mut().flat_map(|c|c.paragraphs.iter_mut())).collect()
        };for p in paras {for inline in &mut p.inlines{if let Inline::StructuredControl(c)=inline {
            if let Some(b)=&c.binding {if let Some(v)=values.get(&(b.store_item_id.clone(),b.xpath.clone())) {if c.value!=*v{c.value=v.clone();count+=1;}}}
        }}}}
        Ok(count)
    }
    /// Replace literal text across adjacent styled Word runs (not just inside
    /// each run), and in text-bearing document parts.
    pub fn replace_text(&mut self,find:&str,replace:&str)->usize {
        self.replace_literal_everywhere(find,replace).unwrap_or(0)
    }
    /// Regex replacement across adjacent styled runs, notes, comments, and
    /// custom XML text. This retains unchanged run formatting.
    pub fn replace_regex(&mut self,expression:&str,replacement:&str)->io::Result<usize>{
        self.replace_regex_everywhere(expression,replacement)
    }
    pub fn replace_all(&mut self,replacements:&HashMap<&str,&str>)->usize{replacements.iter().map(|(k,v)|self.replace_text(k,v)).sum()}
    /// Merge blocks and associated media/annotations, remapping IDs and duplicate styles.
    pub fn append(&mut self,other:&Document){
        let image_offset=self.images.len();
        let footnote_offset=self.footnotes.iter().map(|n|n.id).max().unwrap_or(0);
        let endnote_offset=self.endnotes.iter().map(|n|n.id).max().unwrap_or(0);
        let comment_offset=self.comments.iter().map(|n|n.id).max().map(|i|i+1).unwrap_or(0);
        let max_num=self.blocks.iter().flat_map(|b|match b{Block::Paragraph(p)=>vec![p],Block::Table(t)=>t.rows.iter().flat_map(|r|r.cells.iter().flat_map(|c|c.paragraphs.iter())).collect()}).filter_map(|p|p.list.map(|v|v.num_id)).max().unwrap_or(2);
        let mut list_remap:HashMap<u32,u32>=HashMap::new();let mut next_num=max_num.max(2)+1;
        for p in other.blocks.iter().flat_map(|b|match b{Block::Paragraph(p)=>vec![p],Block::Table(t)=>t.rows.iter().flat_map(|r|r.cells.iter().flat_map(|c|c.paragraphs.iter())).collect()}){
            if let Some(info)=p.list{list_remap.entry(info.num_id).or_insert_with(||{let n=next_num;next_num+=1;n});}
        }
        let mut style_remap=HashMap::new();
        for style in &other.styles{
            if let Some(existing)=self.styles.iter().find(|s|s.id==style.id){
                if existing.name==style.name && existing.kind==style.kind{style_remap.insert(style.id.clone(),style.id.clone());continue;}
                let mut n=2usize;let new_id=loop{let id=format!("{}_{}",style.id,n);if !self.styles.iter().any(|s|s.id==id){break id;}n+=1;};
                let mut clone=style.clone();clone.id=new_id.clone();self.styles.push(clone);style_remap.insert(style.id.clone(),new_id);
            } else {self.styles.push(style.clone());style_remap.insert(style.id.clone(),style.id.clone());}
        }
        self.images.extend(other.images.iter().cloned());
        self.footnotes.extend(other.footnotes.iter().map(|n|Note{id:n.id+footnote_offset,text:n.text.clone()}));
        self.endnotes.extend(other.endnotes.iter().map(|n|Note{id:n.id+endnote_offset,text:n.text.clone()}));
        self.comments.extend(other.comments.iter().map(|c|Comment{id:c.id+comment_offset,author:c.author.clone(),text:c.text.clone(),parent:c.parent.map(|v|v+comment_offset)}));
        for (&id,meta) in &other.comment_metadata{self.comment_metadata.insert(id+comment_offset,meta.clone());}
        let remap_para=|p:&mut Paragraph|{
            if let Some(ref mut s)=p.style{if let Some(new)=style_remap.get(s){*s=new.clone();}}
            if let Some(ref mut list)=p.list{if let Some(new)=list_remap.get(&list.num_id){list.num_id=*new;}}
            for i in &mut p.inlines {match i {
                Inline::Image(n)=>*n+=image_offset,
                Inline::FootnoteRef(n)=>*n+=footnote_offset,
                Inline::EndnoteRef(n)=>*n+=endnote_offset,
                Inline::CommentStart(n)|Inline::CommentEnd(n)=>*n+=comment_offset,
                Inline::Run(r)=>if let Some(ref mut style)=r.style{if let Some(new)=style_remap.get(style){*style=new.clone();}},
                _=>{}
            }}
        };
        for mut block in other.blocks.iter().cloned(){match &mut block{
            Block::Paragraph(p)=>remap_para(p),
            Block::Table(t)=>for row in &mut t.rows{for cell in &mut row.cells{for p in &mut cell.paragraphs{remap_para(p);}}}
        }self.blocks.push(block);}
    }
    pub fn append_with_break(&mut self,other:&Document,_kind:SectionBreak){self.add_page_break();self.append(other);}
    pub fn text(&self)->String{let mut result=Vec::new();for b in &self.blocks{match b {Block::Paragraph(p)=>result.push(p.text()),Block::Table(t)=>for row in &t.rows{result.push(row.cells.iter().map(Cell::text).collect::<Vec<_>>().join("\t"));}}}result.join("\n")}
    pub fn bookmark_names(&self)->Vec<String>{let mut out=Vec::new();for p in self.paragraphs(){for i in &p.inlines{if let Inline::Bookmark(n)=i{out.push(n.clone());}}}out}
    pub fn open(path:impl AsRef<Path>)->io::Result<Self>{Self::from_bytes(&fs::read(path)?)}
    pub fn save(&self,path:impl AsRef<Path>)->io::Result<()>{fs::write(path,self.to_docx()?)}
}

impl Table {
 /// Determine a cell's effective border, considering explicit edge rules,
 /// neighbouring cell edges and the table's inside borders. Merged-cell
 /// layouts should query the physical grid spans after placing cells.
 pub fn resolved_cell_border(&self,row:usize,col:usize,edge:TableEdge)->Option<Border>{
  let cell=self.rows.get(row)?.cells.get(col)?;
  let border_at=|r:usize,c:usize,e:TableEdge|->Option<&Border>{
    self.rows.get(r)?.cells.get(c)?.edge_borders.as_ref()?.edge(e)
  };
  let explicit=if let Some(edges)=cell.edge_borders.as_ref(){edges.edge(edge)}
    else{cell.borders.as_ref()};
  let neighbor=match edge{
   TableEdge::Top=>row.checked_sub(1).and_then(|r|border_at(r,col,TableEdge::Bottom)),
   TableEdge::Bottom=>border_at(row+1,col,TableEdge::Top),
   TableEdge::Left=>col.checked_sub(1).and_then(|c|border_at(row,c,TableEdge::Right)),
   TableEdge::Right=>border_at(row,col+1,TableEdge::Left),
  };
  let best=stronger_table_border(explicit,neighbor);
  if best.is_some(){return best;}
  let is_perimeter=match edge{
   TableEdge::Top=>row==0,TableEdge::Bottom=>row+1==self.rows.len(),
   TableEdge::Left=>col==0,TableEdge::Right=>col+1==self.rows[row].cells.len(),
  };
  if let Some(edges)=self.edge_borders.as_ref(){
   let candidate=if is_perimeter{edges.edge(edge)}else{match edge{
    TableEdge::Top|TableEdge::Bottom=>edges.inside_h.as_ref(),
    TableEdge::Left|TableEdge::Right=>edges.inside_v.as_ref(),
   }};
   return candidate.cloned();
  }
  self.borders.clone()
 }
}
