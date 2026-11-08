//! Word-style document flow/pagination geometry independent from PDF/PNG export.
//!
//! Provides paragraph wrapping using font advances, page sections, margins,
//! columns, alignment, paragraph spacing, keep-with-next, basic widow/orphan
//! control, fixed table cells, page/column breaks, floating image geometry,
//! and repeating headers/footers. Exact Word line composition requires its
//! proprietary compatibility rules, OS font access, hyphenation dictionaries,
//! complete Unicode shaping and DrawingML wrap geometry; this engine deliberately
//! reports geometry but does not claim pixel-identical Word pagination.
use std::io::{self,Error,ErrorKind};
use std::collections::{BTreeSet,VecDeque};
use crate::model::{Alignment,Block,BreakKind,Document,Inline,PageSettings,Paragraph,RelativePosition,Run,Section,SectionBreak,Table,TableEdge,Border,VerticalAlignment,TabAlignment,TabLeader,LineSpacingRule,RowHeightRule};
use crate::opentype::{FontCollection,ShapeOptions};
use crate::unicode;
fn invalid(message:&str)->io::Error{Error::new(ErrorKind::InvalidInput,message)}
/// Format PAGE/SECTION field numbering according to Word's section
/// numbering format or an explicit `\\*` field-format switch.
fn number_format(number:u32,format:&str)->String{
    let kind=format.to_ascii_lowercase();
    if kind.contains("roman") && (1..=3999).contains(&number){
        let mut n=number;
        let mut s=String::new();
        for (value,symbol) in [(1000,"M"),(900,"CM"),(500,"D"),(400,"CD"),
            (100,"C"),(90,"XC"),(50,"L"),(40,"XL"),(10,"X"),(9,"IX"),(5,"V"),(4,"IV"),(1,"I")]{
            while n>=value{n-=value;s.push_str(symbol);}
        }
        return if kind.starts_with("lower")||format=="roman"{s.to_ascii_lowercase()}else{s};
    }
    if (kind.contains("letter")||kind.contains("alphabetic"))&&number>0 {
        let mut n=number;let mut bytes=Vec::new();
        while n>0 && bytes.len()<16 {n-=1;bytes.push(b'A'+(n%26) as u8);n/=26;}
        bytes.reverse();let s=String::from_utf8(bytes).unwrap_or_default();
        return if kind.starts_with("lower")||format=="alphabetic"{s.to_ascii_lowercase()}else{s};
    }
    number.to_string()
}
#[derive(Debug,Clone,Copy,Default)]pub struct Rect{pub x:f32,pub y:f32,pub width:f32,pub height:f32}
impl Rect{pub fn right(self)->f32{self.x+self.width}pub fn bottom(self)->f32{self.y+self.height}}
#[derive(Debug,Clone)]pub struct PositionedText{
    pub text:String,pub region:Rect,pub size_pt:f32,pub font:Option<String>,
    pub bold:bool,pub italic:bool,pub source_block:usize,
}
#[derive(Debug,Clone)]pub struct PositionedImage{pub image_index:usize,pub region:Rect,pub floating:bool,pub source_block:usize}
#[derive(Debug,Clone)]pub struct PositionedCell{
    pub row:usize,pub col:usize,pub region:Rect,pub source_block:usize,
    /// Resolved top, bottom, left, right cell borders in OOXML eighths of a point.
    pub edges:[Option<Border>;4],
    /// OOXML RGB cell shading, when present.
    pub shading:Option<String>,
}
#[derive(Debug,Clone)]pub struct LayoutPage{
    pub index:usize,pub section_index:usize,pub number:u32,pub width:f32,pub height:f32,
    pub text:Vec<PositionedText>,pub pictures:Vec<PositionedImage>,pub cells:Vec<PositionedCell>,
    pub headers:Vec<PositionedText>,pub footers:Vec<PositionedText>,
    pub footnotes:Vec<PositionedText>,pub footnote_ids:Vec<u32>,
}
#[derive(Debug,Clone,Default)]pub struct LayoutDocument{pub pages:Vec<LayoutPage>,pub warnings:Vec<String>}
#[derive(Debug,Clone)]pub struct LayoutConfig{
    pub default_font_size_pt:f32,pub default_line_multiplier:f32,
    pub min_widow_lines:usize,pub min_orphan_lines:usize,pub max_pages:usize,
    pub minimum_line_height_pt:f32,
}
impl Default for LayoutConfig{fn default()->Self{Self{
    default_font_size_pt:11.0,default_line_multiplier:1.2,
    min_widow_lines:2,min_orphan_lines:2,max_pages:10_000,minimum_line_height_pt:7.0,
}}}
#[derive(Default)]pub struct Engine{pub fonts:FontCollection,pub config:LayoutConfig,
    pub hyphenator:Option<crate::hyphenation::Hyphenator>}
#[derive(Debug,Clone)]struct Token{
    text:String,advance:f32,height:f32,size:f32,font:Option<String>,bold:bool,italic:bool,
    space:bool,break_after:bool,hyphen:bool,forced:Option<BreakKind>,image:Option<usize>,footnote:Option<u32>,
}
impl Token{
    fn new(text:String,size:f32,font:Option<String>,bold:bool,italic:bool,advance:f32)->Self{
        let space=text.chars().all(char::is_whitespace);
        let break_after=space||text.ends_with('-')||text.ends_with('\u{00ad}')||
            text.chars().last().map(|c|matches!(unicode::script(c),unicode::Script::Han|unicode::Script::Hangul)).unwrap_or(false);
        Self{text,advance,height:size*1.2,size,font,bold,italic,space,break_after,hyphen:false,forced:None,image:None,footnote:None}
    }
}
#[derive(Debug,Clone)]struct Line{
    tokens:Vec<Token>,width:f32,height:f32,break_after:Option<BreakKind>,justify:bool
}
#[derive(Debug,Clone)]struct CellLines{cell_index:usize,grid_col:usize,width:f32,lines:Vec<Line>,vertical_merge:Option<bool>,valign:VerticalAlignment,
    edges:[Option<Border>;4],shading:Option<String>}

impl Engine{
    pub fn new()->Self{Self::default()}
    fn measure(&self,text:&str,font:Option<&str>,size:f32)->f32{
        self.fonts.shape(text,font,ShapeOptions{font_size_pt:size,..ShapeOptions::default()}).width_pt
    }
    /// Determine a line box from actual OpenType/MVAR metrics when possible.
    /// Synthetic fallback remains available for unmatched Unicode clusters.
    fn font_line_height(&self,text:&str,font:Option<&str>,size:f32,weight:u16,italic:bool)->f32{
        let default=size*self.config.default_line_multiplier;
        let Some(index)=self.fonts.select_cluster_style(text,font,weight,italic)else{return default;};
        let Some(face)=self.fonts.faces().get(index)else{return default;};
        let (asc,desc,gap)=face.vertical_metrics_instance();
        let height=(asc-desc+gap).max(0.0)/face.units_per_em as f32*size;
        if height.is_finite(){height.max(default).min(size*3.0)}else{default}
    }
    fn apply_hyphenation(&self,tokens:&mut [Token]){
        let Some(dictionary)=self.hyphenator.as_ref()else{return;};
        let mut i=0;
        while i<tokens.len(){
            if tokens[i].text.is_empty() || !tokens[i].text.chars().all(char::is_alphabetic){i+=1;continue;}
            let begin=i;let mut word=String::new();
            while i<tokens.len() && !tokens[i].text.is_empty() && tokens[i].text.chars().all(char::is_alphabetic){
                word.push_str(&tokens[i].text);i+=1;
            }
            let points=dictionary.breaks(&word);let mut at=0;
            for t in &mut tokens[begin..i]{at+=t.text.len();if points.contains(&at){t.break_after=true;t.hyphen=true;}}
        }
    }
    /// Evaluate deterministic Word field instructions without executing code,
    /// launching programs or fetching external resources. Unknown fields
    /// retain their cached result when available.
    fn field_result(&self,doc:&Document,instruction:&str,page:u32,total:u32,section:usize)->Option<String>{
        let mut fields=instruction.split_whitespace();
        let name=fields.next()?.to_ascii_uppercase();
        let switches=instruction.split_whitespace().collect::<Vec<_>>();
        let explicit_format=switches.windows(2).find(|w|w[0]=="\\*").map(|w|w[1]);
        let result=match name.as_str(){
            "PAGE"=>{
                let format=explicit_format.or_else(||doc.sections.get(section).or_else(||doc.final_section.as_ref())
                    .and_then(|s|s.page_number_format.as_deref())).unwrap_or("decimal");
                number_format(page,format)
            },"NUMPAGES"|"PAGES"=>number_format(total,explicit_format.unwrap_or("decimal")),
            "SECTION"=>(section+1).to_string(),
            "TITLE"=>doc.properties.title.clone(),
            "SUBJECT"=>doc.properties.subject.clone(),
            "AUTHOR"=>doc.properties.creator.clone(),
            "KEYWORDS"=>doc.properties.keywords.clone(),
            "COMMENTS"=>doc.properties.description.clone(),
            "LASTSAVEDBY"=>doc.properties.last_modified_by.clone(),
            "REVNUM"=>doc.properties.revision.map(|n|n.to_string()).unwrap_or_default(),
            "DOCPROPERTY"=>{
                match fields.next()?.trim_matches('"').to_ascii_lowercase().as_str(){
                    "title"=>doc.properties.title.clone(),
                    "subject"=>doc.properties.subject.clone(),
                    "author"|"creator"=>doc.properties.creator.clone(),
                    "keywords"=>doc.properties.keywords.clone(),
                    "category"=>doc.properties.category.clone(),
                    "description"=>doc.properties.description.clone(),
                    "language"=>doc.properties.language.clone(),
                    "identifier"=>doc.properties.identifier.clone(),
                    _=>return None,
                }
            },
            _=>return None,
        };
        Some(result)
    }
    fn paragraph_tokens(&self,doc:&Document,p:&Paragraph,page_number:u32,total_pages:u32,section:usize)->Vec<Token>{
        let mut tokens=Vec::new();
        for item in &p.inlines{
            match item{
                Inline::Run(run)=>{
                    let start_index=tokens.len();
                    let r=doc.resolved_run(run,p.style.as_deref());
                    let size=r.font_size.unwrap_or(self.config.default_font_size_pt).clamp(3.0,144.0);
                    // Shape the complete run so GSUB ligatures and GPOS
                    // kerning across character boundaries affect line width.
                    let shaped=self.fonts.shape(&r.text,r.font.as_deref(),ShapeOptions{
                        font_size_pt:size,weight:if r.bold{700}else{400},italic:r.italic,..ShapeOptions::default()
                    });
                    let linebreaks=unicode::line_breaks(&r.text);
                    for group in unicode::graphemes(&r.text){
                        let piece=group.text.to_string();
                        if piece=="\n"||piece=="\r\n"{
                            let mut t=Token::new(String::new(),size,r.font.clone(),r.bold,r.italic,0.0);t.forced=Some(BreakKind::Line);tokens.push(t);
                        }else if piece=="\u{00ad}" {
                            let mut t=Token::new(piece,size,r.font.clone(),r.bold,r.italic,0.0);
                            t.break_after=true;tokens.push(t);
                        }else{
                            let width=shaped.glyphs.iter().filter(|g|g.cluster>=group.byte_range.0&&g.cluster<group.byte_range.1).map(|g|g.advance_pt).sum();
                            let mut t=Token::new(piece,size,r.font.clone(),r.bold,r.italic,width);
                            t.height=self.font_line_height(&t.text,r.font.as_deref(),size,if r.bold{700}else{400},r.italic);
                            t.break_after|=linebreaks.binary_search(&group.byte_range.1).is_ok();
                            tokens.push(t);
                        }
                    }
                    self.apply_hyphenation(&mut tokens[start_index..]);
                    if let Some(break_kind)=r.break_kind{
                        let mut t=Token::new(String::new(),size,r.font.clone(),r.bold,r.italic,0.0);t.forced=Some(break_kind);tokens.push(t);
                    }
                },
                Inline::FootnoteRef(id)=>{
                    let mut t=Token::new(format!("{}",id),self.config.default_font_size_pt*0.7,None,false,false,0.0);
                    t.advance=self.measure(&t.text,None,t.size);t.footnote=Some(*id);tokens.push(t);
                },
                Inline::Image(index)=>{
                    if let Some(image)=doc.images.get(*index){
                        if !image.floating{
                            let width=image.width.0 as f32/12700.0;let height=image.height.0 as f32/12700.0;
                            let mut t=Token::new(String::new(),self.config.default_font_size_pt,None,false,false,width);
                            t.height=height;t.image=Some(*index);tokens.push(t);
                        }
                    }
                },
                Inline::ComplexField{instruction,result,locked,..}=>{
                    let text=if *locked{None}else{self.field_result(doc,instruction,page_number,total_pages,section)};
                    self.append_text(&mut tokens,text.as_deref().unwrap_or(result),None);
                },
                Inline::StructuredControl(v)=>self.append_text(&mut tokens,&v.value,None),
                Inline::ContentControl{value,..}=>self.append_text(&mut tokens,value,None),
                Inline::MathExpr(expr)=>self.append_text(&mut tokens,&expr.plain_text(),None),
                Inline::Math(text)=>self.append_text(&mut tokens,text,None),
                Inline::Field(field)=>{
                    let text=self.field_result(doc,field,page_number,total_pages,section).unwrap_or_default();
                    self.append_text(&mut tokens,&text,None);
                },
                Inline::Checkbox{checked,..}=>self.append_text(&mut tokens,if *checked{"☒"}else{"☐"},None),
                Inline::Dropdown{options,selected,..}=>if let Some(value)=options.get(*selected){self.append_text(&mut tokens,value,None);},
                Inline::Revision{kind,content,..} if matches!(kind,crate::model::RevisionKind::Insert|crate::model::RevisionKind::MoveTo)=>{
                    for v in content{if let Inline::Run(run)=v{self.append_text(&mut tokens,&run.text,Some(run));}}
                },
                Inline::TrackedInsert{text,..}|Inline::Shape{text,..}=>self.append_text(&mut tokens,text,None),
                _=>{}
            }
        }
        if tokens.is_empty(){tokens.push(Token::new(String::new(),self.config.default_font_size_pt,None,false,false,0.0));}
        tokens
    }
    fn append_text(&self,tokens:&mut Vec<Token>,text:&str,style:Option<&Run>){
        let size=style.and_then(|r|r.font_size).unwrap_or(self.config.default_font_size_pt);
        for g in unicode::graphemes(text){
            let mut token=Token::new(g.text.to_string(),size,style.and_then(|r|r.font.clone()),style.map(|r|r.bold).unwrap_or(false),style.map(|r|r.italic).unwrap_or(false),self.measure(g.text,style.and_then(|r|r.font.as_deref()),size));
            token.height=self.font_line_height(&token.text,style.and_then(|r|r.font.as_deref()),size,
                if style.map(|r|r.bold).unwrap_or(false){700}else{400},style.map(|r|r.italic).unwrap_or(false));
            if token.text=="\n"{token.forced=Some(BreakKind::Line);token.advance=0.0;token.text.clear();}
            tokens.push(token);
        }
    }
    /// Resolve default and paragraph-defined Word tab stops. Center/right
    /// tabs align the following tab-delimited field, not the tab glyph itself.
    fn tab_geometry(&self,tokens:&[Token],index:usize,x:f32,p:&Paragraph)->(f32,Option<char>){
        let stop=p.tab_stops.iter().filter(|s|s.position_twips as f32/20.0>x+0.01)
            .min_by_key(|s|s.position_twips);
        let (position,alignment,leader)=if let Some(stop)=stop{
            (stop.position_twips as f32/20.0,stop.alignment,stop.leader)
        }else{(((x/36.0).floor()+1.0)*36.0,TabAlignment::Left,TabLeader::None)};
        let mut following=0.0;let mut before_decimal=0.0;let mut decimal_found=false;
        for t in tokens.iter().skip(index+1){
            if t.text=="\t"||t.forced.is_some(){break;}
            following+=t.advance;
            if !decimal_found{
                if let Some(at)=t.text.find(|c|c=='.'||c==','){
                    before_decimal+=self.measure(&t.text[..at],t.font.as_deref(),t.size);
                    decimal_found=true;
                }else{before_decimal+=t.advance;}
            }
        }
        let target=match alignment{
            TabAlignment::Left=>position,
            TabAlignment::Center=>position-following/2.0,
            TabAlignment::Right=>position-following,
            TabAlignment::Decimal=>position-before_decimal,
        };
        let leader=match leader{
            TabLeader::None=>None,TabLeader::Dots=>Some('.'),TabLeader::Dashes=>Some('-'),
            TabLeader::Line=>Some('_'),TabLeader::MiddleDots=>Some('·'),
        };
        ((target-x).max(1.0),leader)
    }
    fn break_lines(&self,tokens:&[Token],width:f32,p:&Paragraph)->Vec<Line>{
        let mut result=Vec::new();let mut at=0;let mut first=true;
        let rule=p.line_rule.unwrap_or(LineSpacingRule::Auto);
        while at<tokens.len(){
            let indent=if first{p.first_line.unwrap_or(0) as f32/20.0}else{0.0};
            let max=(width-indent).max(1.0);let mut taken=0;let mut w=0.0;
            let mut last_break=None;let mut forced=None;
            while at+taken<tokens.len(){
                let t=&tokens[at+taken];
                if let Some(b)=t.forced{forced=Some(b);taken+=1;break;}
                let adv=if t.text=="\t"{self.tab_geometry(tokens,at+taken,w,p).0}else{t.advance};
                if w+adv>max&&taken>0{break;}
                w+=adv;taken+=1;
                if t.break_after{last_break=Some(taken);}
                if w>max{break;}
            }
            if forced.is_none() && taken>0 && at+taken<tokens.len() && last_break.is_some(){taken=last_break.unwrap();}
            if taken==0{taken=1;}
            let mut chars=tokens[at..(at+taken).min(tokens.len())].to_vec();
            if chars.last().and_then(|c|c.forced).is_some(){chars.pop();}
            while chars.last().map(|c|c.space).unwrap_or(false){chars.pop();}
            let soft_break=at+taken<tokens.len()&&chars.last().map(|t|t.text=="\u{00ad}").unwrap_or(false);
            let mut advance_sum=0.0;
            for i in 0..chars.len(){
                if chars[i].text=="\t"{
                    let (advance,leader)=self.tab_geometry(&chars,i,advance_sum,p);
                    chars[i].advance=advance;
                    chars[i].space=false;
                    if let Some(ch)=leader{
                        let width=self.measure(&ch.to_string(),chars[i].font.as_deref(),chars[i].size).max(0.1);
                        chars[i].text=std::iter::repeat(ch).take((advance/width).floor().min(1000.0) as usize).collect();
                    }else{chars[i].text.clear();}
                }
                if chars[i].text=="\u{00ad}"{chars[i].text.clear();chars[i].advance=0.0;}
                advance_sum+=chars[i].advance;
            }
            if soft_break{if let Some(last)=chars.last_mut(){last.text="-".to_string();last.advance=self.measure("-",last.font.as_deref(),last.size);}}
            else if at+taken<tokens.len()&&chars.last().map(|t|t.hyphen).unwrap_or(false){
                if let Some(last)=chars.last(){
                    let mut dash=Token::new("-".to_string(),last.size,last.font.clone(),last.bold,last.italic,
                        self.measure("-",last.font.as_deref(),last.size));
                    dash.hyphen=false;chars.push(dash);
                }
            }
            let width=chars.iter().map(|t|t.advance).sum();
            let natural=chars.iter().map(|t|t.height).fold(self.config.minimum_line_height_pt, f32::max)
                .max(self.config.default_font_size_pt*self.config.default_line_multiplier);
            let height=match p.line_spacing{
                None=>natural,
                Some(value)=>match rule{
                    // OOXML auto spacing uses 240ths of the normal line height.
                    LineSpacingRule::Auto=>natural*value as f32/240.0,
                    LineSpacingRule::Exact=>(value as f32/20.0).max(1.0),
                    LineSpacingRule::AtLeast=>natural.max(value as f32/20.0),
                },
            }.max(1.0);
            // A wrapped line can be justified. The final line of a paragraph,
            // and any explicitly broken line, must retain natural spacing.
            let justify=at+taken<tokens.len()&&forced.is_none();
            result.push(Line{tokens:chars,width,height,break_after:forced,justify});
            at+=taken;first=false;
        }
        if result.is_empty(){result.push(Line{tokens:Vec::new(),width:0.0,height:self.config.default_font_size_pt*1.2,break_after:None,justify:false});}
        result
    }
    fn page_settings<'a>(&self,doc:&'a Document,section:usize)->(&'a PageSettings,u16,f32,Option<&'a Section>){
        if let Some(s)=doc.sections.get(section).or_else(||doc.final_section.as_ref()){
            (&s.page,s.columns.max(1),s.column_spacing_twips as f32/20.0,Some(s))
        }else{(&doc.page,1,36.0,None)}
    }
    fn column_rect(&self,doc:&Document,section:usize,column:usize)->Rect{
        let (page,count,space,extra)=self.page_settings(doc,section);
        let page_w=page.width_twips as f32/20.0;let page_h=page.height_twips as f32/20.0;
        let gutter=extra.map(|s|s.gutter_twips as f32/20.0).unwrap_or(0.0);
        let left=page.margin_left as f32/20.0+gutter;
        let top=page.margin_top as f32/20.0;
        let usable=(page_w-left-page.margin_right as f32/20.0).max(1.0);
        let n=count as usize;let equal_width=(usable-space*(n.saturating_sub(1)) as f32).max(1.0)/n as f32;
        let mut x=left;let mut width=equal_width;
        if let Some(s)=extra{if !s.columns_equal_width && s.column_widths_twips.len()==n{
            for i in 0..column.min(n){let (w,sp)=s.column_widths_twips[i];x+=w as f32/20.0+sp as f32/20.0;}
            width=s.column_widths_twips[column.min(n-1)].0 as f32/20.0;
        }else{x+=column.min(n-1) as f32*(equal_width+space);}}
        else{x+=column.min(n-1) as f32*(equal_width+space);}
        Rect{x,y:top,width,height:(page_h-top-page.margin_bottom as f32/20.0).max(1.0)}
    }
    fn blank_page(&self,doc:&Document,index:usize,section:usize,number:u32)->LayoutPage{
        let (s,_,_,_)=self.page_settings(doc,section);
        LayoutPage{index,section_index:section,number,width:s.width_twips as f32/20.0,height:s.height_twips as f32/20.0,
            text:Vec::new(),pictures:Vec::new(),cells:Vec::new(),headers:Vec::new(),footers:Vec::new(),footnotes:Vec::new(),footnote_ids:Vec::new()}
    }
    fn next_column(&self,doc:&Document,output:&mut LayoutDocument,section:usize,column:&mut usize,y:&mut f32,number:&mut u32)->io::Result<()> {
        let (_,cols,_,_)=self.page_settings(doc,section);
        *column+=1;
        if *column>=cols as usize{
            *column=0;*number=number.saturating_add(1);
            if output.pages.len()>=self.config.max_pages{return Err(invalid("page limit exceeded"));}
            let page=self.blank_page(doc,output.pages.len(),section,*number);output.pages.push(page);
        }
        *y=self.column_rect(doc,section,*column).y;Ok(())
    }
    fn next_page(&self,doc:&Document,output:&mut LayoutDocument,section:usize,column:&mut usize,y:&mut f32,number:&mut u32)->io::Result<()> {
        if output.pages.len()>=self.config.max_pages{return Err(invalid("page limit exceeded"));}
        *column=0;*number=number.saturating_add(1);
        output.pages.push(self.blank_page(doc,output.pages.len(),section,*number));
        *y=self.column_rect(doc,section,0).y;Ok(())
    }
    fn lay_text(&self,page:&mut LayoutPage,line:&Line,x:f32,y:f32,available_width:f32,align:Alignment,block:usize,is_header:bool,is_footer:bool){
        let mut actual_x=match align{
            Alignment::Left|Alignment::Justify=>x,Alignment::Center=>x+(available_width-line.width).max(0.0)/2.0,
            Alignment::Right=>x+(available_width-line.width).max(0.0),
        };
        let space_count=line.tokens.iter().filter(|t|t.space).count();
        let extra=if line.justify && align==Alignment::Justify&&space_count>0{(available_width-line.width).max(0.0)/space_count as f32}else{0.0};
        for token in &line.tokens{
            let advance=token.advance+if token.space{extra}else{0.0};
            let text=PositionedText{text:token.text.clone(),region:Rect{x:actual_x,y,width:advance,height:line.height},size_pt:token.size,
                font:token.font.clone(),bold:token.bold,italic:token.italic,source_block:block};
            if is_header{page.headers.push(text);}else if is_footer{page.footers.push(text);}else{
                if let Some(id)=token.footnote{if !page.footnote_ids.contains(&id){page.footnote_ids.push(id);}}
                page.text.push(text);
            }
            actual_x+=advance;
        }
    }
    /// Height reserved for notes already referenced on the current physical
    /// page, plus candidate notes referenced by the next line. The reservation
    /// is calculated before placing the line, rather than detecting overlaps
    /// only after pagination.
    fn note_reservation(&self,doc:&Document,page:&LayoutPage,ids:&BTreeSet<u32>)->f32{
        if ids.is_empty(){return 0.0;}
        let (settings,_,_,_)=self.page_settings(doc,page.section_index);
        let gutter=self.page_settings(doc,page.section_index).3.map(|s|s.gutter_twips as f32/20.0).unwrap_or(0.0);
        let width=(page.width-(settings.margin_left+settings.margin_right) as f32/20.0-gutter).max(1.0);
        let mut height=6.0;
        for id in ids{
            if let Some(note)=doc.footnotes.iter().find(|note|note.id==*id){
                let paragraph=Paragraph::new(format!("{id}. {}",note.text));
                height+=self.break_lines(&self.paragraph_tokens(doc,&paragraph,page.number,page.index as u32+1,page.section_index),width,&paragraph)
                    .iter().map(|line|line.height).sum::<f32>();
            }
        }
        height
    }
    fn reserved_bottom(&self,doc:&Document,page:&LayoutPage,rect:Rect,extra:&BTreeSet<u32>)->f32{
        let mut ids=page.footnote_ids.iter().copied().collect::<BTreeSet<_>>();
        ids.extend(extra.iter().copied());
        rect.bottom()-self.note_reservation(doc,page,&ids)
    }
    fn draw_floats(&self,doc:&Document,page:&mut LayoutPage,p:&Paragraph,block_index:usize,anchor_y:f32,column:Rect){
        for item in &p.inlines{
            let Inline::Image(i)=item else{continue;};let Some(image)=doc.images.get(*i)else{continue;};
            if !image.floating{continue;}
            let options=image.positioning.clone().unwrap_or_default();
            let w=image.width.0 as f32/12700.0;let h=image.height.0 as f32/12700.0;
            let (page_settings,_,_,_)=self.page_settings(doc,page.section_index);
            let x=match options.horizontal{
                RelativePosition::Page=>0.0,RelativePosition::Margin=>page_settings.margin_left as f32/20.0,
                _=>column.x,
            }+options.x_emu as f32/12700.0;
            let y=match options.vertical{
                RelativePosition::Page=>0.0,RelativePosition::Margin=>page_settings.margin_top as f32/20.0,
                _=>anchor_y,
            }+options.y_emu as f32/12700.0;
            let mut region=Rect{x,y,width:w,height:h};
            if !options.allow_overlap {
                for _ in 0..page.pictures.len().min(256){
                    let mut bottom=region.y;
                    for previous in &page.pictures{
                        if previous.floating && previous.region.x<region.right() && previous.region.right()>region.x
                            && previous.region.y<region.bottom() && previous.region.bottom()>region.y {
                            bottom=bottom.max(previous.region.bottom()+options.distance_top as f32/12700.0);
                        }
                    }
                    if bottom<=region.y+0.01{break;}
                    region.y=bottom;
                }
            }
            page.pictures.push(PositionedImage{image_index:*i,region,floating:true,source_block:block_index});
        }
    }
    /// Approximate rotated DrawingML wrap geometry conservatively over the
    /// vertical extent of a line box. A single centerline can miss protruding
    /// edges, causing printed text to collide with an anchored object.
    fn polygon_span(&self,polygon:&[(i64,i64)],degrees:f64,picture:Rect,y:f32,height:f32)->Option<(f32,f32)>{
        if polygon.len()<3||polygon.len()>100_000||picture.height<=0.0||picture.width<=0.0{return None;}
        let radians=degrees.to_radians();
        let (sin,cos)=if radians.is_finite(){radians.sin_cos()}else{(0.0,1.0)};
        let vertices=polygon.iter().map(|(x,y)|{
            let x=*x as f64-10800.0;let y=*y as f64-10800.0;
            ((x*cos-y*sin+10800.0)/21600.0,(x*sin+y*cos+10800.0)/21600.0)
        }).collect::<Vec<_>>();
        let mut left=f32::INFINITY;let mut right=f32::NEG_INFINITY;
        for sample in [y+0.01,y+height*0.25,y+height*0.5,y+height*0.75,y+height-0.01]{
            let scan=(sample-picture.y)/picture.height;
            let mut xs=Vec::new();
            for i in 0..vertices.len(){
                let a=vertices[i];let b=vertices[(i+1)%vertices.len()];
                if (a.1<=scan as f64 && (scan as f64)<b.1)||(b.1<=scan as f64 && (scan as f64)<a.1){
                    let t=(scan as f64-a.1)/(b.1-a.1);
                    let x=a.0+t*(b.0-a.0);
                    if x.is_finite(){xs.push((picture.x as f64+x*picture.width as f64) as f32);}
                }
            }
            if !xs.is_empty(){
                left=left.min(xs.iter().copied().fold(f32::INFINITY,f32::min));
                right=right.max(xs.iter().copied().fold(f32::NEG_INFINITY,f32::max));
            }
        }
        if left.is_finite()&&right.is_finite(){Some((left,right))}else{None}
    }
    /// Available line box after excluding floating image polygons and their
    /// rotation/distance extents. The algorithm returns one contiguous text
    /// interval; it intentionally does not lay out text in separated holes.
    fn flow_line_rect(&self,doc:&Document,page:&LayoutPage,region:Rect,y:f32,height:f32)->Rect{
        let mut left=region.x;let mut right=region.right();
        for picture in &page.pictures{
            if !picture.floating||picture.region.bottom()<=y||picture.region.y>=y+height{continue;}
            let Some(img)=doc.images.get(picture.image_index)else{continue;};
            let options=img.positioning.clone().unwrap_or_default();
            if options.behind_text||matches!(options.wrap,crate::model::ImageWrap::None){continue;}
            if options.wrap==crate::model::ImageWrap::TopAndBottom{
                return Rect{x:region.x,y,width:0.0,height};
            }
            let (shape_left,shape_right)=if matches!(options.wrap,crate::model::ImageWrap::Tight|crate::model::ImageWrap::Through){
                self.polygon_span(&options.wrap_polygon,options.rotation_degrees,picture.region,y,height)
                    .unwrap_or((picture.region.x,picture.region.right()))
            }else{(picture.region.x,picture.region.right())};
            let padded_left=shape_left-options.distance_left as f32/12700.0;
            let padded_right=shape_right+options.distance_right as f32/12700.0;
            if padded_left<=left && padded_right>left{left=padded_right.min(right);}
            else if padded_left<right&&padded_right>=right{right=padded_left.max(left);}
            else if padded_left>=left&&padded_right<=right{
                if padded_left-left>=right-padded_right{right=padded_left;}else{left=padded_right;}
            }
        }
        Rect{x:left,y,width:(right-left).max(1.0),height}
    }
    fn reflow_around_floats(&self,doc:&Document,page:&LayoutPage,tokens:&[Token],
                           region:Rect,at_y:f32,p:&Paragraph,pitch:Option<f32>)->Vec<Line>{
        let mut work=self.break_lines(tokens,region.width,p);
        Self::apply_line_pitch(&mut work,pitch);
        let mut i=0;let mut y=at_y;
        // Additional lines introduced by a float can themselves be wrapped;
        // the worklist grows monotonically and is capped for hostile inputs.
        while i<work.len()&&work.len()<100_000{
            let box_rect=self.flow_line_rect(doc,page,region,y,work[i].height);
            if box_rect.width==0.0{
                let bottom=page.pictures.iter().filter(|pic|pic.floating&&pic.region.y<y+work[i].height
                    &&pic.region.bottom()>y).filter(|pic|doc.images.get(pic.image_index).and_then(|img|img.positioning.as_ref())
                    .map(|position|position.wrap==crate::model::ImageWrap::TopAndBottom).unwrap_or(false))
                    .map(|pic|pic.region.bottom()).fold(y,f32::max);
                let blank=(bottom-y).max(work[i].height).min(region.height.max(1.0));
                work.insert(i,Line{tokens:Vec::new(),width:0.0,height:blank,break_after:None,justify:false});
                y+=blank;i+=1;continue;
            }
            if work[i].width>box_rect.width+0.1 && work[i].tokens.len()>1{
                let mut split=self.break_lines(&work[i].tokens,box_rect.width,p);
                Self::apply_line_pitch(&mut split,pitch);
                if split.len()>1 {work.splice(i..i+1,split);continue;}
            }
            y+=work[i].height;i+=1;
        }
        work
    }
    /// OOXML document-grid line pitch is measured in twips. Except for exact
    /// line spacing, body paragraphs snap to grid lines unless w:snapToGrid=0.
    /// Table-cell line boxes do not use this pitch by default.
    fn document_line_pitch(&self,doc:&Document,section:usize,p:&Paragraph)->Option<f32>{
        if p.snap_to_grid==Some(false)||p.line_rule==Some(LineSpacingRule::Exact){return None;}
        let grid=self.page_settings(doc,section).3?.doc_grid.as_ref()?;
        if !matches!(grid.kind.as_str(),"lines"|"linesAndChars"){return None;}
        let pitch=grid.line_pitch_twips? as f32/20.0;
        if !pitch.is_finite()||pitch<1.0||pitch>144.0{return None;}
        Some(pitch)
    }
    fn apply_line_pitch(lines:&mut [Line],pitch:Option<f32>){
        if let Some(pitch)=pitch{
            for line in lines {
                if !matches!(line.break_after,Some(BreakKind::Page|BreakKind::Column)){
                    line.height=(line.height/pitch).ceil().max(1.0)*pitch;
                }
            }
        }
    }
    fn paragraph(&self,doc:&Document,output:&mut LayoutDocument,p:&Paragraph,block:usize,section:usize,
                 column:&mut usize,y:&mut f32,number:&mut u32,total_pages_hint:u32)->io::Result<()> {
        let mut rect=self.column_rect(doc,section,*column);
        let indent=p.indent_left.unwrap_or(0) as f32/20.0;
        let right=p.indent_right.unwrap_or(0) as f32/20.0;
        let before_twips=p.spacing_before.unwrap_or(0);
        let after_twips=p.spacing_after.unwrap_or(0);
        let previous=block.checked_sub(1).and_then(|i|doc.blocks.get(i));
        let next=block.checked_add(1).and_then(|i|doc.blocks.get(i));
        let same_style=|other:&Paragraph|p.style==other.style;
        let before=if let Some(Block::Paragraph(prev))=previous{
            if same_style(prev)&&(p.contextual_spacing||prev.contextual_spacing){0.0}
            else{before_twips.saturating_sub(prev.spacing_after.unwrap_or(0)) as f32/20.0}
        }else{before_twips as f32/20.0};
        let after=if let Some(Block::Paragraph(next))=next{
            if same_style(next)&&(p.contextual_spacing||next.contextual_spacing){0.0}
            else{after_twips as f32/20.0}
        }else{after_twips as f32/20.0};
        if p.page_break_before && *y>rect.y{
            self.next_page(doc,output,section,column,y,number)?;rect=self.column_rect(doc,section,*column);
        }
        let tokens=self.paragraph_tokens(doc,p,*number,total_pages_hint,section);
        let pitch=self.document_line_pitch(doc,section,p);
        let mut provisional=self.break_lines(&tokens,rect.width-indent-right,p);
        Self::apply_line_pitch(&mut provisional,pitch);
        let estimated:f32=provisional.iter().map(|l|l.height).sum::<f32>()+before+after;
        if *y+estimated>self.reserved_bottom(doc,output.pages.last().unwrap(),rect,&BTreeSet::new())
            && estimated<=rect.height && (p.keep_together || p.keep_with_next || provisional.len()<=self.config.min_orphan_lines){
            self.next_column(doc,output,section,column,y,number)?;rect=self.column_rect(doc,section,*column);
        }
        *y+=before;
        if let Some(pitch)=pitch{
            let relative=(*y-rect.y).max(0.0);
            *y=rect.y+(relative/pitch).ceil()*pitch;
        }
        self.draw_floats(doc,output.pages.last_mut().unwrap(),p,block,*y,rect);
        let mut lines=self.reflow_around_floats(doc,output.pages.last().unwrap(),&tokens,rect,*y,p,pitch);
        // w:keepLines must stay on one page if it fits a page column.
        if p.keep_together && !lines.iter().any(|line|matches!(line.break_after,Some(BreakKind::Page|BreakKind::Column))){
            let needed=lines.iter().map(|line|line.height).sum::<f32>()+after;
            let mut notes=BTreeSet::new();
            for line in &lines{for token in &line.tokens{if let Some(id)=token.footnote{notes.insert(id);}}}
            let usable=self.reserved_bottom(doc,output.pages.last().unwrap(),rect,&notes)-*y;
            if needed>usable && needed<=rect.height && *y>rect.y+0.1{
                self.next_column(doc,output,section,column,y,number)?;
                rect=self.column_rect(doc,section,*column);
                self.draw_floats(doc,output.pages.last_mut().unwrap(),p,block,*y,rect);
                lines=self.reflow_around_floats(doc,output.pages.last().unwrap(),&tokens,rect,*y,p,pitch);
            }
        }
        let mut at=0;
        while at<lines.len(){
            rect=self.column_rect(doc,section,*column);
            let remaining=rect.bottom()-*y;
            let mut count=0;let mut used=0.0;
            let mut candidate_notes=BTreeSet::new();
            for line in &lines[at..]{
                for token in &line.tokens{if let Some(id)=token.footnote{candidate_notes.insert(id);}}
                let reserved=self.reserved_bottom(doc,output.pages.last().unwrap(),rect,&candidate_notes)-*y;
                let available=remaining.min(reserved);
                if used+line.height>available{break;}
                count+=1;used+=line.height;
                if let Some(b)=line.break_after{if matches!(b,BreakKind::Page|BreakKind::Column){break;}}
            }
            if count==0{
                if *y<=rect.y+0.1{count=1;output.warnings.push(format!("paragraph line or attached footnote taller than available column at block {block}"));}
                else{self.next_column(doc,output,section,column,y,number)?;continue;}
            }
            let remaining_lines=lines.len()-at;
            if count<remaining_lines{
                let min_orphan=if p.widow_control==Some(false){1}else{self.config.min_orphan_lines};
                let min_widow=if p.widow_control==Some(false){1}else{self.config.min_widow_lines};
                if count<min_orphan && *y>rect.y+0.1{
                    self.next_column(doc,output,section,column,y,number)?;continue;
                }
                if remaining_lines-count<min_widow&&count>min_orphan {
                    count=remaining_lines-min_widow;
                }
            }
            let mut explicitly_broken=false;
            for (j,line) in lines[at..at+count].iter().enumerate(){
                let adjusted=self.flow_line_rect(doc,output.pages.last().unwrap(),rect,*y,line.height);
                let x=adjusted.x+indent+if at+j==0{p.first_line.unwrap_or(0) as f32/20.0}else{0.0};
                let available=(adjusted.width-indent-right).max(1.0);
                self.lay_text(output.pages.last_mut().unwrap(),line,x,*y,available,p.alignment,block,false,false);
                let mut image_x=x;
                for t in &line.tokens{
                    if let Some(image)=t.image{
                        if let Some(img)=doc.images.get(image){
                            output.pages.last_mut().unwrap().pictures.push(PositionedImage{image_index:image,region:Rect{x:image_x,y:*y,width:img.width.0 as f32/12700.0,height:img.height.0 as f32/12700.0},floating:false,source_block:block});
                        }
                    }
                    image_x+=t.advance;
                }
                *y+=line.height;
                if let Some(b)=line.break_after{if matches!(b,BreakKind::Page|BreakKind::Column){
                    if b==BreakKind::Page{
                        self.next_page(doc,output,section,column,y,number)?;
                    }else{
                        self.next_column(doc,output,section,column,y,number)?;
                    }
                    explicitly_broken=true;
                }}
            }
            at+=count;
            if at<lines.len()&&!explicitly_broken{self.next_column(doc,output,section,column,y,number)?;}
        }
        *y+=after;Ok(())
    }
    /// Estimate minimum grid widths from unbreakable words and preferred
    /// cell dimensions. Auto-fit distributes remaining width proportionally,
    /// respecting merged cells instead of assigning equal widths blindly.
    fn autofit_widths(&self,t:&Table,columns:usize,available:f32)->Vec<f32>{
        if columns==0{return Vec::new();}
        if t.layout_fixed && t.column_widths.len()==columns{
            let weights=t.column_widths.iter().map(|n|*n as f32).collect::<Vec<_>>();
            let total=weights.iter().sum::<f32>().max(1.0);
            return weights.iter().map(|w|available*(*w/total)).collect();
        }
        let mut preferred=if t.column_widths.len()==columns{
            t.column_widths.iter().map(|value|(*value as f32/20.0).max(8.0)).collect::<Vec<_>>()
        }else{vec![available/columns as f32;columns]};
        let mut minima=vec![8.0_f32;columns];
        for row in &t.rows {
            let mut col=(row.grid_before as usize).min(columns);
            for cell in &row.cells {
                let span=(cell.colspan.max(1) as usize).min(columns.saturating_sub(col));
                if span==0{break;}
                let longest=cell.paragraphs.iter().flat_map(|p|p.text().split_whitespace().map(str::to_string).collect::<Vec<_>>())
                    .map(|word|self.measure(&word,None,self.config.default_font_size_pt)).fold(0.0,f32::max);
                let required=(longest+8.0).max(cell.width_twips.unwrap_or(0) as f32/20.0);
                let per_column=required/span as f32;
                for width in &mut minima[col..col+span]{*width=(*width).max(per_column);}
                col+=span;
            }
        }
        for (minimum,width) in minima.iter().zip(preferred.iter_mut()) {
            *width=(*width).max(*minimum);
        }
        let minimum_sum=minima.iter().sum::<f32>();
        if minimum_sum>=available{
            // The content cannot physically fit. Scale proportionally but
            // preserve a 4-point minimum column so progress remains bounded.
            let scale=available/minimum_sum.max(1.0);
            return minima.iter().map(|v|(v*scale).max(4.0)).collect();
        }
        let flexible=preferred.iter().map(|w|*w).sum::<f32>().max(1.0);
        let remaining=available-minimum_sum;
        preferred.iter().enumerate().map(|(index,w)|minima[index]+remaining*(*w/flexible)).collect()
    }
    fn table(&self,doc:&Document,output:&mut LayoutDocument,t:&Table,block:usize,section:usize,
             column:&mut usize,y:&mut f32,number:&mut u32,total_pages_hint:u32)->io::Result<()> {
        let rect=self.column_rect(doc,section,*column);
        let grid_cols=t.rows.iter().map(|r|{
            r.cells.iter().fold(r.grid_before.min(256) as usize,|n,c|n.saturating_add(c.colspan.max(1).min(256) as usize))
                .saturating_add(r.grid_after.min(256) as usize)
        }).max().unwrap_or(1).clamp(1,256);
        let table_width=t.width_percent.map(|pct|rect.width*pct.max(0.)/100.)
            .or_else(||t.width_twips.map(|width|width as f32/20.0))
            .unwrap_or(rect.width).min(rect.width*4.).max(1.);
        let column_widths=self.autofit_widths(t,grid_cols,table_width.max(1.0));
        let actual_width=column_widths.iter().sum::<f32>();
        let table_x=|rect:Rect|->f32 {
            let indent=t.indent_twips.unwrap_or(0) as f32/20.0;
            match t.alignment{
                Alignment::Left|Alignment::Justify=>rect.x+indent,
                Alignment::Center=>rect.x+(rect.width-actual_width).max(0.0)/2.0+indent,
                Alignment::Right=>rect.right()-actual_width+indent,
            }
        };
        let margin=t.cell_margin_twips.unwrap_or(80) as f32/20.0;
        let mut header_rows:Vec<(usize,Vec<CellLines>,f32)>=Vec::new();
        let mut content_started=false;
        for (row_index,row) in t.rows.iter().enumerate(){
            let mut grid=(row.grid_before as usize).min(grid_cols);
            let mut cells=Vec::new();
            let specified_height=row.height_twips.unwrap_or(0) as f32/20.0;
            let mut row_height=if row.height_rule==RowHeightRule::Auto{0.0}else{specified_height};
            for (cell_index,cell) in row.cells.iter().enumerate(){
                let start=grid;if start>=grid_cols{break;}
                grid=(grid+cell.colspan.max(1) as usize).min(grid_cols);
                let width=column_widths[start..grid].iter().sum::<f32>().max(1.0);
                let mut lines=Vec::new();
                for paragraph in &cell.paragraphs{
                    let tokens=self.paragraph_tokens(doc,paragraph,*number,total_pages_hint,section);
                    lines.extend(self.break_lines(&tokens,(width-2.0*margin).max(1.0),paragraph));
                }
                let h=lines.iter().map(|line|line.height).sum::<f32>()+margin*2.0;
                row_height=row_height.max(h);
                let edges=[TableEdge::Top,TableEdge::Bottom,TableEdge::Left,TableEdge::Right]
                    .map(|edge|t.resolved_cell_border(row_index,cell_index,edge));
                cells.push(CellLines{cell_index,grid_col:start,width,lines,
                    vertical_merge:cell.vertical_merge,valign:cell.valign,
                    edges,shading:cell.shading.clone()});
            }
            if row.height_rule==RowHeightRule::Exact && row.height_twips.is_some(){
                row_height=specified_height.max(0.1);
            }
            if row_height==0.0{row_height=margin*2.0+self.config.minimum_line_height_pt;}
            let exact_height=row.height_rule==RowHeightRule::Exact && row.height_twips.is_some();
            if exact_height && cells.iter().any(|cell|cell.lines.iter().map(|l|l.height).sum::<f32>()+margin*2.0>row_height){
                output.warnings.push(format!("exact table row {row_index} of block {block} clips overflowing content"));
            }
            let append_header=row.is_header&&!content_started;
            if !row.is_header{content_started=true;}
            let mut offsets=vec![0usize;cells.len()];let mut did_draw=false;
            loop {
                let page_rect=self.column_rect(doc,section,*column);
                let row_notes=cells.iter().flat_map(|cell|cell.lines.iter()).flat_map(|line|line.tokens.iter().filter_map(|token|token.footnote)).collect::<BTreeSet<_>>();
                let space=(self.reserved_bottom(doc,output.pages.last().unwrap(),page_rect,&row_notes)-*y).max(0.0);
                let total_remaining=if exact_height{row_height}else{
                    cells.iter().zip(&offsets).map(|(cell,&off)|{
                        cell.lines[off..].iter().map(|l|l.height).sum::<f32>()+margin*2.0
                    }).fold(0.0,f32::max).max(if did_draw{0.0}else{row_height})
                };
                let can_fit=total_remaining<=space;
                // Never split a cantSplit row, except if it is taller than a
                // complete column; in that case emit the diagnostic and keep
                // the original row intact rather than looping indefinitely.
                if row.cant_split || exact_height || can_fit{
                    if total_remaining>space&&*y>page_rect.y+0.1{
                        self.next_column(doc,output,section,column,y,number)?;
                        self.repeat_table_headers(doc,output,&header_rows,section,column,y,number,block,&column_widths,margin,table_x(self.column_rect(doc,section,*column)))?;
                    }
                    let height=total_remaining.max(if did_draw{0.0}else{row_height});
                    let next_rect=self.column_rect(doc,section,*column);
                    if height>next_rect.height{
                        output.warnings.push(format!("table row {row_index} of block {block} is taller than a page column and cannot split"));
                    }
                    let ends=cells.iter().map(|c|c.lines.len()).collect::<Vec<_>>();
                    self.paint_row(output.pages.last_mut().unwrap(),&cells,&offsets,&ends,&column_widths,table_x(self.column_rect(doc,section,*column)),margin,
                        *y,height,block,row_index);
                    *y+=height;
                    break;
                }
                if space<margin*2.0+self.config.minimum_line_height_pt && *y>page_rect.y+0.1{
                    self.next_column(doc,output,section,column,y,number)?;
                    self.repeat_table_headers(doc,output,&header_rows,section,column,y,number,block,&column_widths,margin,table_x(self.column_rect(doc,section,*column)))?;
                    continue;
                }
                let mut ends=Vec::new();let mut fragment_height=margin*2.0;
                for (cell,&begin) in cells.iter().zip(&offsets){
                    let mut end=begin;let mut used=0.0;
                    while end<cell.lines.len()&&used+cell.lines[end].height+margin*2.0<=space{
                        used+=cell.lines[end].height;end+=1;
                    }
                    ends.push(end);fragment_height=fragment_height.max(used+margin*2.0);
                }
                if ends.iter().zip(&offsets).all(|(end,start)|end==start){
                    // A single line exceeds column height: force one line in
                    // the first cell with pending text to make forward progress.
                    if *y>page_rect.y+0.1{
                        self.next_column(doc,output,section,column,y,number)?;
                        self.repeat_table_headers(doc,output,&header_rows,section,column,y,number,block,&column_widths,margin,table_x(self.column_rect(doc,section,*column)))?;
                        continue;
                    }
                    if let Some(i)=cells.iter().enumerate().position(|(i,c)|offsets[i]<c.lines.len()){
                        ends[i]+=1;fragment_height=cells[i].lines[offsets[i]].height+margin*2.0;
                        output.warnings.push(format!("single table line of block {block} exceeds the available column height"));
                    }else{break;}
                }
                self.paint_row(output.pages.last_mut().unwrap(),&cells,&offsets,&ends,&column_widths,table_x(page_rect),margin,
                    *y,fragment_height,block,row_index);
                did_draw=true;offsets=ends;*y+=fragment_height;
                if cells.iter().zip(&offsets).all(|(c,&off)|off>=c.lines.len()){break;}
                self.next_column(doc,output,section,column,y,number)?;
                self.repeat_table_headers(doc,output,&header_rows,section,column,y,number,block,&column_widths,margin,table_x(self.column_rect(doc,section,*column)))?;
            }
            if append_header{header_rows.push((row_index,cells.clone(),row_height));}
        }
        Ok(())
    }
    fn paint_row(&self,page:&mut LayoutPage,cells:&[CellLines],starts:&[usize],ends:&[usize],
                 widths:&[f32],column_x:f32,margin:f32,y:f32,height:f32,block:usize,row:usize){
        for (i,cell) in cells.iter().enumerate(){
            let base_x=column_x+widths.iter().take(cell.grid_col).sum::<f32>();
            if cell.vertical_merge==Some(false){
                // A continuation extends the visible region of its opening
                // cell on this page. Across a page break it becomes a blank
                // continuation cell so merged borders remain represented.
                if let Some(previous)=page.cells.iter_mut().rev().find(|old|
                    old.source_block==block && (old.region.x-base_x).abs()<0.1 &&
                    (old.region.width-cell.width).abs()<0.1 &&
                    (old.region.bottom()-y).abs()<0.1){
                    previous.region.height+=height;
                    continue;
                }
            }
            page.cells.push(PositionedCell{row,col:cell.cell_index,
                region:Rect{x:base_x,y,width:cell.width,height},source_block:block,
                edges:cell.edges.clone(),shading:cell.shading.clone()});
            if cell.vertical_merge==Some(false){continue;}
            let content_height=cell.lines[starts[i]..ends[i]].iter().map(|line|line.height).sum::<f32>();
            let free=(height-margin*2.0-content_height).max(0.0);
            let full_cell=starts[i]==0 && ends[i]==cell.lines.len();
            let offset=if full_cell{match cell.valign{
                VerticalAlignment::Top=>0.0,VerticalAlignment::Center=>free/2.0,VerticalAlignment::Bottom=>free,
            }}else{0.0};
            let mut ly=y+margin+offset;
            for line in &cell.lines[starts[i]..ends[i]]{
                if ly+line.height>y+height-margin+0.1 {break;}
                self.lay_text(page,line,base_x+margin,ly,(cell.width-2.0*margin).max(1.0),Alignment::Left,block,false,false);
                ly+=line.height;
            }
        }
    }
    fn repeat_table_headers(&self,doc:&Document,output:&mut LayoutDocument,headers:&[(usize,Vec<CellLines>,f32)],
                            section:usize,column:&mut usize,y:&mut f32,number:&mut u32,
                            block:usize,widths:&[f32],margin:f32,table_x:f32)->io::Result<()> {
        if headers.is_empty(){return Ok(());}
        let rect=self.column_rect(doc,section,*column);
        let total=headers.iter().map(|(_,_,h)|*h).sum::<f32>();
        if total>rect.height*0.7{
            output.warnings.push(format!("repeating table headers of block {block} are too tall"));
            return Ok(());
        }
        for (row_index,cells,height) in headers{
            let starts=vec![0;cells.len()];let ends=cells.iter().map(|c|c.lines.len()).collect::<Vec<_>>();
            self.paint_row(output.pages.last_mut().unwrap(),cells,&starts,&ends,widths,table_x,margin,*y,*height,block,*row_index);
            *y+=*height;
        }
        let _=number;Ok(())
    }
    /// Balance a final, text-only section page across equal-width columns.
    ///
    /// Column balancing is intentionally limited to pages without tables,
    /// floating drawings or footnotes, since relocating such objects would
    /// invalidate their anchors and note-area reservations. It preserves line
    /// order, paragraph spacing and the original line-relative indentation.
    fn balance_last_page_columns(&self,doc:&Document,page:&mut LayoutPage){
        let (_,count,_,extra)=self.page_settings(doc,page.section_index);
        if count<2||count>16||page.text.is_empty()||!page.cells.is_empty()||
            !page.pictures.is_empty()||!page.footnote_ids.is_empty(){return;}
        if extra.map(|s|!s.columns_equal_width).unwrap_or(false){return;}
        let first=self.column_rect(doc,page.section_index,0);
        if page.text.iter().any(|g|g.region.x<first.x-1.0||g.region.x>first.right()+1.0 ||
            g.region.y<first.y-1.0||g.region.bottom()>first.bottom()+1.0){return;}
        let mut lines:Vec<(usize,f32,f32,Vec<usize>)>=Vec::new();
        for (index,item) in page.text.iter().enumerate(){
            if let Some(last)=lines.last_mut(){
                if last.0==item.source_block && (last.1-item.region.y).abs()<0.05{
                    last.2=last.2.max(item.region.height);last.3.push(index);continue;
                }
            }
            lines.push((item.source_block,item.region.y,item.region.height,vec![index]));
        }
        if lines.len()<(count as usize)*2{return;}
        let body_start=lines[0].1;
        let body_end=lines.last().map(|l|l.1+l.2).unwrap_or(body_start);
        let total=(body_end-body_start).max(0.0);
        let target=total/count as f32;
        if !target.is_finite()||target<self.config.minimum_line_height_pt*2.0{return;}
        let mut column=0usize;let mut used=0.0f32;let mut old_end=body_start;
        for (source,original_y,height,indices) in lines{
            let gap=(original_y-old_end).max(0.0).min(target/3.0);
            // Never divide a keepLines paragraph across columns when it fits.
            let paragraph_kept=doc.blocks.get(source).and_then(|b|match b{
                Block::Paragraph(p)=>Some(p.keep_together),_=>None,
            }).unwrap_or(false);
            let paragraph_height=if paragraph_kept{
                page.text.iter().filter(|g|g.source_block==source)
                    .map(|g|g.region.bottom()).fold(0.0,f32::max)-original_y
            }else{height};
            if column+1<count as usize && used>0.0 &&
                used+gap+paragraph_height>target+self.config.minimum_line_height_pt*0.5{
                column+=1;used=0.0;
            }
            let rect=self.column_rect(doc,page.section_index,column);
            let extra_space=if used==0.0{0.0}else{gap};
            let new_y=rect.y+used+extra_space;
            let shift_x=rect.x-first.x;
            for index in indices{
                let item=&mut page.text[index];
                item.region.x+=shift_x;
                item.region.y+=new_y-original_y;
            }
            used+=height+extra_space;
            old_end=original_y+height;
        }
    }
    fn adorn_page(&self,doc:&Document,page:&mut LayoutPage,total:u32,first_section_page:bool){
        let (page_set,_,_,section)=self.page_settings(doc,page.section_index);
        let first=first_section_page && section.map(|s|s.title_page).unwrap_or(true);
        let headers=if first&&!doc.first_header.is_empty(){&doc.first_header}else{&doc.header};
        let footers=if first&&!doc.first_footer.is_empty(){&doc.first_footer}else{&doc.footer};
        let head_y=section.map(|s|s.header_distance_twips).unwrap_or(720) as f32/20.0;
        let foot_y=page.height-section.map(|s|s.footer_distance_twips).unwrap_or(720) as f32/20.0;
        let gutter=section.map(|s|s.gutter_twips as f32/20.0).unwrap_or(0.0);
        let width=page.width-(page_set.margin_left+page_set.margin_right) as f32/20.0-gutter;
        for (entries,start_y,is_footer) in [(headers,head_y,false),(footers,foot_y,true)]{
            let mut y=start_y;
            for paragraph in entries{
                let lines=self.break_lines(&self.paragraph_tokens(doc,paragraph,page.number,total,page.section_index),width,paragraph);
                for line in lines{
                    self.lay_text(page,&line,page_set.margin_left as f32/20.0+gutter,y,width,paragraph.alignment,usize::MAX,!is_footer,is_footer);
                    y+=line.height;
                }
            }
        }
    }
    /// Flow referenced footnotes through a bounded bottom-of-page note region.
    /// Continuations move forward to subsequent physical pages rather than
    /// overwriting the text body. Note lines retain their reference order.
    fn flow_footnotes(&self,doc:&Document,output:&mut LayoutDocument,total_hint:u32)->io::Result<()> {
        let mut pending:VecDeque<(u32,Vec<Line>,usize)>=VecDeque::new();
        let mut seen=BTreeSet::new();let mut page_no=0usize;
        while page_no<output.pages.len()||!pending.is_empty(){
            if page_no>=self.config.max_pages{return Err(invalid("footnote continuation page limit exceeded"));}
            if page_no==output.pages.len(){
                let previous=output.pages.last().ok_or_else(||invalid("no page available for footnotes"))?;
                let section=previous.section_index;let number=previous.number.saturating_add(1);
                output.pages.push(self.blank_page(doc,page_no,section,number));
            }
            let page=&mut output.pages[page_no];
            let (settings,_,_,section)=self.page_settings(doc,page.section_index);
            let gutter=section.map(|s|s.gutter_twips as f32/20.0).unwrap_or(0.0);
            let left=settings.margin_left as f32/20.0+gutter;
            let width=(page.width-(settings.margin_left+settings.margin_right) as f32/20.0-gutter).max(1.0);
            let bottom=page.height-settings.margin_bottom as f32/20.0;
            for id in page.footnote_ids.iter().copied(){
                if !seen.insert(id){continue;}
                if let Some(note)=doc.footnotes.iter().find(|n|n.id==id){
                    let paragraph=Paragraph::new(format!("{id}. {}",note.text));
                    let tokens=self.paragraph_tokens(doc,&paragraph,page.number,total_hint,page.section_index);
                    pending.push_back((id,self.break_lines(&tokens,width,&paragraph),0));
                }
            }
            if pending.is_empty(){page_no+=1;continue;}
            // Reserve at least a modest body area even on a footnote-heavy
            // page. The current body bottom always wins to avoid overlap.
            let body_end=page.text.iter().map(|t|t.region.bottom()).fold(0.0f32,f32::max);
            let top=(body_end+6.0).max(self.column_rect(doc,page.section_index,0).y+6.0);
            let capacity=(bottom-top).max(0.0);
            let mut selected:Vec<Line>=Vec::new();let mut used=0.0f32;
            while let Some((_,lines,index))=pending.front_mut(){
                if *index>=lines.len(){pending.pop_front();continue;}
                let line=&lines[*index];
                // A line larger than an empty continuation page is forced
                // once so that malformed font metrics cannot loop forever.
                if used+line.height>capacity && !(selected.is_empty()&&body_end==0.0){break;}
                used+=line.height;selected.push(line.clone());*index+=1;
                if *index==lines.len(){pending.pop_front();}
            }
            if selected.is_empty() && body_end==0.0 && !pending.is_empty(){
                if let Some((_,lines,index))=pending.front_mut(){
                    if let Some(line)=lines.get(*index){selected.push(line.clone());used=line.height;*index+=1;}
                    if *index>=lines.len(){pending.pop_front();}
                }
            }
            let mut y=bottom-used;
            for line in selected{
                let before=page.text.len();
                self.lay_text(page,&line,left,y,width,Alignment::Left,usize::MAX,false,false);
                page.footnotes.extend(page.text.split_off(before));
                y+=line.height;
            }
            if !pending.is_empty()&&page_no+1==output.pages.len(){
                output.warnings.push(format!("footnote continues beyond page {}",page_no+1));
            }
            page_no+=1;
        }
        Ok(())
    }

    fn paginate_once(&self,doc:&Document,total_pages_hint:u32)->io::Result<LayoutDocument>{
        if self.config.max_pages==0{return Err(invalid("layout max_pages must be positive"));}
        let mut output=LayoutDocument::default();
        let mut section=0usize;let mut column=0usize;let mut number=1u32;
        output.pages.push(self.blank_page(doc,0,section,number));
        let mut y=self.column_rect(doc,section,column).y;
        for (index,block) in doc.blocks.iter().enumerate(){
            while let Some(s)=doc.sections.get(section){
                if index<s.end_after{break;}
                let kind=s.break_kind;section+=1;
                if kind!=SectionBreak::Continuous{
                    if output.pages.len()>=self.config.max_pages{return Err(invalid("page limit exceeded"));}
                    column=0;number=self.page_settings(doc,section).3.and_then(|s|s.page_number_start).unwrap_or_else(||number.saturating_add(1));
                    output.pages.push(self.blank_page(doc,output.pages.len(),section,number));
                    if matches!(kind,SectionBreak::OddPage|SectionBreak::EvenPage){
                        let even=kind==SectionBreak::EvenPage;
                        if number%2==(if even{1}else{0}){
                            if output.pages.len()>=self.config.max_pages{return Err(invalid("page limit exceeded"));}
                            number=number.saturating_add(1);
                            output.pages.push(self.blank_page(doc,output.pages.len(),section,number));
                        }
                    }
                    y=self.column_rect(doc,section,column).y;
                }else{
                    // Continuous section: preserve baseline, restart at the
                    // first column. Word may rebalance columns at this point.
                    column=0;
                }
            }
            match block{
                Block::Paragraph(p)=>{
                    if p.keep_with_next {
                        let region=self.column_rect(doc,section,column);
                        let mut required=0.0;
                        let mut next_index=index;
                        // The whole keep-with-next chain must stay together,
                        // including a terminal paragraph/table. Limit preflight
                        // work for hostile documents with long keep chains.
                        while next_index<doc.blocks.len() && next_index-index<2048 {
                            match &doc.blocks[next_index]{
                                Block::Paragraph(next)=>{
                                    let lines=self.break_lines(&self.paragraph_tokens(doc,next,number,total_pages_hint,section),region.width,next);
                                    let full=lines.iter().map(|l|l.height).sum::<f32>()+
                                        (next.spacing_before.unwrap_or(0)+next.spacing_after.unwrap_or(0)) as f32/20.0;
                                    required+=if next_index==index||next.keep_with_next{full}
                                        else{lines.first().map(|l|l.height).unwrap_or(0.0)};
                                    if !next.keep_with_next{break;}
                                },
                                Block::Table(table)=>{
                                    required+=table.rows.first().and_then(|r|r.height_twips).unwrap_or(240) as f32/20.0;
                                    break;
                                }
                            }
                            next_index+=1;
                        }
                        if y+required>region.bottom()&&required<region.height&&y>region.y+0.1{
                            self.next_column(doc,&mut output,section,&mut column,&mut y,&mut number)?;
                        }
                    }
                    self.paragraph(doc,&mut output,p,index,section,&mut column,&mut y,&mut number,total_pages_hint)?;
                },
                Block::Table(t)=>self.table(doc,&mut output,t,index,section,&mut column,&mut y,&mut number,total_pages_hint)?,
            }
        }
        if !doc.endnotes.is_empty(){
            self.next_page(doc,&mut output,section,&mut column,&mut y,&mut number)?;
            for note in &doc.endnotes{
                let p=Paragraph::new(format!("{}. {}",note.id,note.text));
                self.paragraph(doc,&mut output,&p,usize::MAX-2,section,&mut column,&mut y,&mut number,total_pages_hint)?;
            }
        }
        // The final page of a multi-column section is balanced by Word.
        // Limit movement to text-only pages whose content is wholly in the
        // first column; tables, drawings and notes retain their anchored flow.
        for i in 0..output.pages.len(){
            let last_of_section=i+1==output.pages.len() ||
                output.pages[i+1].section_index!=output.pages[i].section_index;
            if last_of_section{self.balance_last_page_columns(doc,&mut output.pages[i]);}
        }
        self.flow_footnotes(doc,&mut output,total_pages_hint)?;
        let total=output.pages.len() as u32;
        let mut previous=None;
        for p in &mut output.pages{
            let first=previous!=Some(p.section_index);
            self.adorn_page(doc,p,total,first);
            previous=Some(p.section_index);
        }
        // Mirrored-margin sections use physical page parity, not the
        // displayed PAGE field number (which may restart per section). A
        // gutter belongs on the binding edge: right on even pages, left on
        // odd pages. Widths are identical; only the horizontal origin moves.
        for page in &mut output.pages{
            let section=doc.sections.get(page.section_index).or_else(||doc.final_section.as_ref());
            if let Some(s)=section{
                if s.mirror_margins && page.index%2==1{
                    let delta=(s.page.margin_right as f32-s.page.margin_left as f32-s.gutter_twips as f32)/20.0;
                    for item in &mut page.text{item.region.x+=delta;}
                    for item in &mut page.headers{item.region.x+=delta;}
                    for item in &mut page.footers{item.region.x+=delta;}
                    for item in &mut page.footnotes{item.region.x+=delta;}
                    for item in &mut page.cells{item.region.x+=delta;}
                    for item in &mut page.pictures{item.region.x+=delta;}
                }
            }
        }
        for page in &output.pages{
            if let Some(start)=page.footnotes.iter().map(|t|t.region.y).reduce(f32::min){
                if page.text.iter().any(|t|t.region.bottom()>start){
                    output.warnings.push(format!("footnote region overlaps body on page {}",page.index+1));
                }
            }
        }
        Ok(output)
    }
    /// Reflow body fields against the final page count. A short fixed-point
    /// iteration handles NUMPAGES values that alter their own line wrapping.
    pub fn paginate(&self,doc:&Document)->io::Result<LayoutDocument>{
        let mut total=1u32;
        let mut last=self.paginate_once(doc,total)?;
        for _ in 0..6{
            let actual=last.pages.len() as u32;
            if actual==total{return Ok(last);}
            total=actual;
            last=self.paginate_once(doc,total)?;
        }
        last.warnings.push("NUMPAGES field pagination did not reach a fixed point in 6 passes".into());
        Ok(last)
    }
}
impl Document{
    /// Compute page/column/text geometry with the provided installed font faces.
    pub fn layout(&self,engine:&Engine)->io::Result<LayoutDocument>{engine.paginate(self)}
}
