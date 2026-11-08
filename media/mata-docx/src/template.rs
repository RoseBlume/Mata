//! Word template rendering, without a JSON or regex crate.
//!
//! Supported syntax: `{{customer.name}}`, `{{this}}`, `{{@index}}`,
//! `{{#if path}}...{{else}}...{{/if}}`, `{{#unless path}}`,
//! `{{#each array}}...{{else}}...{{/each}}`, parent lookup (`../path`).
//! A directive in its own paragraph repeats/conditions entire document blocks;
//! a directive in its own table cell paragraph can repeat/condition table rows.
//! Variables spanning styled `w:r` run boundaries are replaced without
//! flattening formatting outside the placeholder range.
use std::collections::BTreeMap;
use std::io::{self, Error, ErrorKind};
use crate::model::{Block, Document, Inline, Paragraph, Row, Run};
use crate::xml::{self,Node};

fn invalid(message:&str)->io::Error{Error::new(ErrorKind::InvalidInput,message)}
#[derive(Debug,Clone,PartialEq)]pub enum Value {
    Null,Bool(bool),Number(f64),Text(String),Array(Vec<Value>),Object(BTreeMap<String,Value>),
}
impl Value {
    pub fn object(fields:impl IntoIterator<Item=(String,Value)>)->Self{Self::Object(fields.into_iter().collect())}
    pub fn text(value:impl Into<String>)->Self{Self::Text(value.into())}
    pub fn truthy(&self)->bool{match self{Self::Null=>false,Self::Bool(v)=>*v,Self::Number(v)=>*v!=0.0&&!v.is_nan(),Self::Text(v)=>!v.is_empty(),Self::Array(v)=>!v.is_empty(),Self::Object(v)=>!v.is_empty()}}
    pub fn display(&self)->String{match self{Self::Null=>String::new(),Self::Bool(v)=>v.to_string(),Self::Number(v)=>v.to_string(),Self::Text(v)=>v.clone(),Self::Array(v)=>v.iter().map(Self::display).collect::<Vec<_>>().join(", "),Self::Object(_)=>String::new()}}
    fn member(&self,name:&str)->Option<&Value>{match self{Self::Object(map)=>map.get(name),Self::Array(a)=>name.parse::<usize>().ok().and_then(|i|a.get(i)),_=>None}}
}
impl From<&str> for Value{fn from(value:&str)->Self{Self::Text(value.to_string())}}
impl From<String> for Value{fn from(value:String)->Self{Self::Text(value)}}
impl From<bool> for Value{fn from(value:bool)->Self{Self::Bool(value)}}
impl From<i32> for Value{fn from(value:i32)->Self{Self::Number(value as f64)}}
impl From<f64> for Value{fn from(value:f64)->Self{Self::Number(value)}}

#[derive(Debug,Clone)]pub struct Options{
    /// Prevent accidental exponential template expansion.
    pub max_depth:usize,pub max_output_blocks:usize,pub max_output_bytes:usize,
    /// Also replace placeholders in XML text nodes of caller-managed package
    /// overrides and non-generated original parts. Markup/attributes are not evaluated.
    pub process_package_xml:bool,
}
impl Default for Options {fn default()->Self{Self{max_depth:32,max_output_blocks:100_000,max_output_bytes:32*1024*1024,process_package_xml:false}}}
#[derive(Debug,Clone,Default)]pub struct Report {pub substitutions:usize,pub expanded_blocks:usize,pub changed_xml_parts:usize}
#[derive(Clone)]struct Scope<'a>{root:&'a Value,stack:Vec<Value>,index:Option<usize>,length:usize}
impl Scope<'_> {
    fn value(&self,path:&str)->Value{
        let mut path=path.trim();let mut depth=0;
        while path.starts_with("../"){path=&path[3..];depth+=1;}
        if path=="@index"{return self.index.map(|v|Value::Number(v as f64)).unwrap_or(Value::Null);}
        if path=="@first"{return Value::Bool(self.index==Some(0));}
        if path=="@last"{return Value::Bool(self.index.map(|i|i+1==self.length).unwrap_or(false));}
        let value=if path=="@root"||path.starts_with("@root."){path=path.strip_prefix("@root.").unwrap_or("");self.root}
        else{self.stack.get(self.stack.len().saturating_sub(depth+1)).unwrap_or(self.root)};
        if path.is_empty()||path=="this"||path=="."{return value.clone();}
        let path=path.strip_prefix("this.").unwrap_or(path);
        let mut current=value;
        for segment in path.split('.'){
            let Some(found)=current.member(segment)else{return Value::Null;};current=found;
        }
        current.clone()
    }
    fn child(&self,v:Value,index:Option<usize>,length:usize)->Self{
        let mut scope=self.clone();scope.stack.push(v);scope.index=index;scope.length=length;scope
    }
}
#[derive(Debug,Clone)]enum NodeT{Text(String),Variable(String),If{expr:String,unless:bool,yes:Vec<NodeT>,no:Vec<NodeT>},Each{expr:String,yes:Vec<NodeT>,no:Vec<NodeT>}}
fn parse_template(src:&str)->io::Result<Vec<NodeT>>{
    fn parse(src:&str,mut at:usize,level:usize)->io::Result<(Vec<NodeT>,usize,Option<String>)>{
        if level>64{return Err(invalid("template nesting too deep"));}
        let mut nodes=Vec::new();
        loop{
            let Some(rel)=src[at..].find("{{")else{nodes.push(NodeT::Text(src[at..].to_string()));return Ok((nodes,src.len(),None));};
            let start=at+rel;if start>at{nodes.push(NodeT::Text(src[at..start].to_string()));}
            let after=start+2;let end=after+src[after..].find("}}").ok_or_else(||invalid("unclosed template expression"))?;
            let marker=src[after..end].trim();at=end+2;
            if marker=="else"||marker.starts_with('/') {return Ok((nodes,at,Some(marker.to_string())));}
            if let Some(expression)=marker.strip_prefix("#if ").or_else(||marker.strip_prefix("#unless ")){
                let unless=marker.starts_with("#unless ");
                let (yes,at_next,ended)=parse(src,at,level+1)?;at=at_next;
                let (no,ending)=if ended.as_deref()==Some("else"){
                    let (otherwise,after,closing)=parse(src,at,level+1)?;at=after;(otherwise,closing)
                }else{(Vec::new(),ended)};
                let target=if unless{"/unless"}else{"/if"};
                if ending.as_deref()!=Some(target){return Err(invalid("mismatched template conditional"));}
                nodes.push(NodeT::If{expr:expression.trim().to_string(),unless,yes,no});continue;
            }
            if let Some(expression)=marker.strip_prefix("#each "){
                let (yes,at_next,ended)=parse(src,at,level+1)?;at=at_next;
                let (no,ending)=if ended.as_deref()==Some("else"){
                    let (otherwise,after,closing)=parse(src,at,level+1)?;at=after;(otherwise,closing)
                }else{(Vec::new(),ended)};
                if ending.as_deref()!=Some("/each"){return Err(invalid("mismatched template iteration"));}
                nodes.push(NodeT::Each{expr:expression.trim().to_string(),yes,no});continue;
            }
            if marker.starts_with('#'){return Err(invalid("unknown template directive"));}
            nodes.push(NodeT::Variable(marker.to_string()));
        }
    }
    let (nodes,_,ending)=parse(src,0,0)?;
    if ending.is_some(){return Err(invalid("unexpected template closing directive"));}
    Ok(nodes)
}
fn render_nodes(nodes:&[NodeT],scope:&Scope<'_>,options:&Options,depth:usize,report:&mut Report,out:&mut String)->io::Result<()> {
    if depth>options.max_depth{return Err(invalid("template maximum depth exceeded"));}
    for node in nodes {
        match node{
            NodeT::Text(t)=>out.push_str(t),
            NodeT::Variable(name)=>{out.push_str(&scope.value(name).display());report.substitutions+=1;},
            NodeT::If{expr,unless,yes,no}=>{
                let matched=scope.value(expr).truthy() != *unless;
                render_nodes(if matched{yes}else{no},scope,options,depth+1,report,out)?;
            },
            NodeT::Each{expr,yes,no}=>{
                match scope.value(expr){
                    Value::Array(items) if !items.is_empty()=>{
                        let len=items.len();for (i,item) in items.into_iter().enumerate(){
                            render_nodes(yes,&scope.child(item,Some(i),len),options,depth+1,report,out)?;
                        }
                    },
                    _=>render_nodes(no,scope,options,depth+1,report,out)?,
                }
            }
        }
        if out.len()>options.max_output_bytes{return Err(invalid("rendered template exceeds output limit"));}
    }
    Ok(())
}
pub fn render_text(src:&str,values:&Value,options:&Options)->io::Result<String>{
    let scope=Scope{root:values,stack:vec![values.clone()],index:None,length:0};
    let mut report=Report::default();render_text_scoped(src,&scope,options,&mut report)
}
fn render_text_scoped(src:&str,scope:&Scope<'_>,options:&Options,report:&mut Report)->io::Result<String>{
    let nodes=parse_template(src)?;let mut out=String::new();render_nodes(&nodes,scope,options,0,report,&mut out)?;Ok(out)
}

/// Preserve formatting outside replacement spans; the replacement inherits
/// the formatting of the first affected run. Ranges use UTF-8 byte offsets
/// in the concatenation of adjacent runs.
fn replace_run_group(runs:&[Run],ranges:&[(usize,usize,String)])->Vec<Inline>{
    let mut boundaries=Vec::with_capacity(runs.len());let mut pos=0;
    for r in runs{boundaries.push((pos,pos+r.text.len()));pos+=r.text.len();}
    let mut result=Vec::new();let mut from=0;
    let copy=|start:usize,end:usize,result:&mut Vec<Inline>|{
        if start>=end{return;}
        for (r,(a,b)) in runs.iter().zip(boundaries.iter().copied()){
            let left=start.max(a);let right=end.min(b);
            if left<right{let mut part=r.clone();part.text=r.text[left-a..right-a].to_string();result.push(Inline::Run(part));}
        }
    };
    for &(start,end,ref replacement) in ranges{
        copy(from,start,&mut result);
        if !replacement.is_empty(){
            let style=runs.iter().zip(boundaries.iter()).find(|(_,(_,b))|*b>start).map(|(r,_)|r).or_else(||runs.last());
            let mut r=style.cloned().unwrap_or_default();r.text=replacement.clone();r.break_kind=None;
            result.push(Inline::Run(r));
        }
        from=end;
    }
    copy(from,pos,&mut result);result
}
fn interpolate_group(runs:&[Run],scope:&Scope<'_>,options:&Options,report:&mut Report)->io::Result<Vec<Inline>>{
    let full=runs.iter().map(|r|r.text.as_str()).collect::<String>();
    if !full.contains("{{"){return Ok(runs.iter().cloned().map(Inline::Run).collect());}
    let mut replacements=Vec::new();let mut at=0;
    while let Some(i)=full[at..].find("{{"){
        let begin=at+i;let Some(end_rel)=full[begin+2..].find("}}")else{return Err(invalid("unclosed template placeholder"))};
        let end=begin+2+end_rel+2;
        let tag=full[begin+2..end-2].trim();
        if tag.starts_with('#')||tag.starts_with('/')||tag=="else"{
            // Inline block directives may span many runs: render the whole
            // group, retaining a representative style for the changed range.
            let replacement=render_text_scoped(&full,scope,options,report)?;
            return Ok(replace_run_group(runs,&[(0,full.len(),replacement)]));
        }
        replacements.push((begin,end,scope.value(tag).display()));report.substitutions+=1;at=end;
    }
    Ok(replace_run_group(runs,&replacements))
}
fn interpolate_paragraph(p:&mut Paragraph,scope:&Scope<'_>,options:&Options,report:&mut Report)->io::Result<()> {
    let original=std::mem::take(&mut p.inlines);let mut rebuilt=Vec::new();let mut runs=Vec::new();
    fn flush(runs:&mut Vec<Run>,out:&mut Vec<Inline>,scope:&Scope<'_>,options:&Options,report:&mut Report)->io::Result<()> {
        if !runs.is_empty(){out.extend(interpolate_group(runs,scope,options,report)?);runs.clear();}Ok(())
    }
    for inline in original{
        match inline {Inline::Run(r) if r.break_kind.is_none()=>runs.push(r),other=>{
            flush(&mut runs,&mut rebuilt,scope,options,report)?;
            rebuilt.push(match other{
                Inline::ContentControl{tag,value}=>Inline::ContentControl{tag,value:render_text_scoped(&value,scope,options,report)?},
                Inline::StructuredControl(mut c)=>{c.value=render_text_scoped(&c.value,scope,options,report)?;Inline::StructuredControl(c)},
                Inline::ComplexField{instruction,result,locked,dirty}=>Inline::ComplexField{instruction,result:render_text_scoped(&result,scope,options,report)?,locked,dirty},
                Inline::RawXml(raw)=>Inline::RawXml(render_raw_fragment(&raw,scope,options,report)?),
                v=>v,
            });
        }}
    }
    flush(&mut runs,&mut rebuilt,scope,options,report)?;
    p.inlines=rebuilt;Ok(())
}
fn marker_for_paragraph(p:&Paragraph)->Option<(String,String)> {
    let text=p.text();let trimmed=text.trim();
    if trimmed.starts_with("{{")&&trimmed.ends_with("}}"){
        let inside=trimmed[2..trimmed.len()-2].trim();
        if inside=="else"||inside.starts_with('#')||inside.starts_with('/'){
            let mut words=inside.splitn(2,' ');return Some((words.next()?.to_string(),words.next().unwrap_or("").trim().to_string()));
        }
    }None
}
fn marker_for_block(b:&Block)->Option<(String,String)>{match b{Block::Paragraph(p)=>marker_for_paragraph(p),_=>None}}
fn find_branch<T>(items:&[T],start:usize,marker:impl Fn(&T)->Option<(String,String)>,kind:&str)->io::Result<(usize,Option<usize>)>{
    let mut nested=0;let mut otherwise=None;
    for idx in start+1..items.len(){if let Some((key,_))=marker(&items[idx]){
        if key.starts_with('#'){nested+=1;}
        else if key.starts_with('/'){
            if nested>0{nested-=1;}else if key==format!("/{kind}"){return Ok((idx,otherwise));}
            else{return Err(invalid("unbalanced template block directive"));}
        }else if key=="else"&&nested==0{otherwise=Some(idx);}
    }}
    Err(invalid("unclosed document template block directive"))
}
fn expand_blocks(blocks:&[Block],scope:&Scope<'_>,options:&Options,report:&mut Report,depth:usize)->io::Result<Vec<Block>>{
    if depth>options.max_depth{return Err(invalid("document block template nesting exceeded"));}
    let mut out=Vec::new();let mut at=0;
    while at<blocks.len(){
        if let Some((kind,expression))=marker_for_block(&blocks[at]){
            if kind.starts_with('#'){
                let name=kind.trim_start_matches('#');let (end,else_at)=find_branch(blocks,at,marker_for_block,name)?;
                let yes=&blocks[at+1..else_at.unwrap_or(end)];let no=else_at.map(|e|&blocks[e+1..end]).unwrap_or(&[]);
                match name {
                    "if"|"unless"=>{
                        let choose=scope.value(&expression).truthy()!=(name=="unless");
                        out.extend(expand_blocks(if choose{yes}else{no},scope,options,report,depth+1)?);
                    },
                    "each"=>match scope.value(&expression){
                        Value::Array(values) if !values.is_empty()=>{
                            let len=values.len();for (index,value) in values.into_iter().enumerate(){
                                out.extend(expand_blocks(yes,&scope.child(value,Some(index),len),options,report,depth+1)?);
                                if out.len()>options.max_output_blocks{return Err(invalid("template expansion block limit exceeded"));}
                            }
                        },_=>out.extend(expand_blocks(no,scope,options,report,depth+1)?),
                    },_=>return Err(invalid("unsupported document template section")),
                }
                report.expanded_blocks+=1;at=end+1;continue;
            }
            if kind.starts_with('/')||kind=="else"{return Err(invalid("orphan document template directive"));}
        }
        let mut block=blocks[at].clone();
        match &mut block{
            Block::Paragraph(p)=>interpolate_paragraph(p,scope,options,report)?,
            Block::Table(t)=>{
                t.rows=expand_rows(&t.rows,scope,options,report,depth+1)?;
                for row in &mut t.rows{for cell in &mut row.cells{for p in &mut cell.paragraphs{interpolate_paragraph(p,scope,options,report)?;}}}
            }
        }
        out.push(block);if out.len()>options.max_output_blocks{return Err(invalid("template expansion block limit exceeded"));}
        at+=1;
    }
    Ok(out)
}
fn row_marker(row:&Row)->Option<(String,String)>{
    // A marker may occupy any one cell, provided the other cells have no
    // content. This permits template rows with a fixed multi-column grid.
    // A directive can occupy several cells in the same row, as long as all
    // occupied cells repeat the same marker. This supports real multi-cell
    // table templates instead of requiring a single isolated marker cell.
    let mut marker:Option<(String,String)>=None;
    for cell in &row.cells {
        if cell.paragraphs.iter().all(|p|p.text().trim().is_empty()){continue;}
        if cell.paragraphs.len()!=1{return None;}
        let current=marker_for_paragraph(&cell.paragraphs[0])?;
        if let Some(ref prior)=marker{if *prior!=current{return None;}}
        else{marker=Some(current);}
    }
    marker
}
fn expand_rows(rows:&[Row],scope:&Scope<'_>,options:&Options,report:&mut Report,depth:usize)->io::Result<Vec<Row>>{
    if depth>options.max_depth{return Err(invalid("table template nesting exceeded"));}
    let mut output=Vec::new();let mut i=0;
    while i<rows.len(){
        if let Some((kind,expression))=row_marker(&rows[i]){
            if kind.starts_with('#'){
                let name=kind.trim_start_matches('#');let (end,otherwise)=find_branch(rows,i,row_marker,name)?;
                let yes=&rows[i+1..otherwise.unwrap_or(end)];let no=otherwise.map(|j|&rows[j+1..end]).unwrap_or(&[]);
                match name{
                    "if"|"unless"=>output.extend(expand_rows(if scope.value(&expression).truthy()!=(name=="unless"){yes}else{no},scope,options,report,depth+1)?),
                    "each"=>match scope.value(&expression){Value::Array(items) if !items.is_empty()=>{
                        let length=items.len();for (j,item) in items.into_iter().enumerate(){output.extend(expand_rows(yes,&scope.child(item,Some(j),length),options,report,depth+1)?);}
                    },_=>output.extend(expand_rows(no,scope,options,report,depth+1)?),},
                    _=>return Err(invalid("unsupported table template directive")),
                }
                report.expanded_blocks+=1;i=end+1;continue;
            }
            if kind=="else"||kind.starts_with('/') {return Err(invalid("orphan table template directive"));}
        }
        output.push(rows[i].clone());i+=1;
        if output.len()>options.max_output_blocks{return Err(invalid("expanded table row count exceeded"));}
    }
    Ok(output)
}
/// Collect the text leaves of DrawingML/WordprocessingML text boxes in XML
/// order. Markup is preserved, and replacements can span separate a:t/w:t
/// elements without operating on XML tag names or attributes.
fn collect_carriers(node:&xml::Element,values:&mut Vec<String>){
    if matches!(node.name.as_str(),"w:t"|"a:t"){
        let text=node.children.iter().filter_map(|c|if let Node::Text(t)=c{Some(t.as_str())}else{None}).collect::<String>();
        values.push(text);
    }else{for child in &node.children{if let Node::Element(e)=child{collect_carriers(e,values);}}}
}
fn write_carriers(node:&mut xml::Element,values:&[String],at:&mut usize){
    if matches!(node.name.as_str(),"w:t"|"a:t"){
        if let Some(text)=values.get(*at){node.children=vec![Node::Text(text.clone())];}
        *at+=1;
    }else{for child in &mut node.children{if let Node::Element(e)=child{write_carriers(e,values,at);}}}
}
fn render_carriers(root:&mut xml::Element,scope:&Scope<'_>,options:&Options,report:&mut Report)->io::Result<bool>{
    let mut carriers=Vec::new();collect_carriers(root,&mut carriers);
    if carriers.is_empty() {return Ok(false);}
    let original=carriers.concat();
    if !original.contains("{{"){return Ok(false);}
    let updated=render_text_scoped(&original,scope,options,report)?;
    if updated==original{return Ok(false);}
    // Retain text in all unaffected leaves; a placeholder spanning leaves is
    // replaced in the first affected one, preserving neighboring formatting.
    let mut offset=0;let mut bounds=Vec::new();
    for text in &carriers{bounds.push((offset,offset+text.len()));offset+=text.len();}
    let mut cuts=Vec::new();let mut search=0;
    while let Some(rel)=original[search..].find("{{"){
        let a=search+rel;let Some(end)=original[a+2..].find("}}")else{break;};let b=a+2+end+2;
        let key=original[a+2..b-2].trim();
        if key.starts_with('#')||key.starts_with('/')||key=="else"{cuts.clear();cuts.push((0,original.len(),updated));break;}
        cuts.push((a,b,scope.value(key).display()));search=b;
    }
    if cuts.is_empty(){return Ok(false);}
    let mut output=vec![String::new();carriers.len()];
    for (i,(begin,end)) in bounds.iter().copied().enumerate(){
        let mut at=begin;
        for (cut_start,cut_end,replacement) in &cuts{
            if *cut_start>=end{break;}
            if *cut_end<=begin{continue;}
            let before=(*cut_start).min(end);
            if before>at{output[i].push_str(&original[at..before]);}
            if *cut_start>=begin&&*cut_start<end{output[i].push_str(replacement);}
            at=(*cut_end).min(end).max(at);
        }
        if at<end{output[i].push_str(&original[at..end]);}
    }
    let mut index=0;write_carriers(root,&output,&mut index);
    Ok(true)
}
fn render_raw_fragment(raw:&str,scope:&Scope<'_>,options:&Options,report:&mut Report)->io::Result<String>{
    if !raw.contains("{{"){return Ok(raw.to_string());}
    let prefix=r#"<mata:root xmlns:mata="urn:mata:template" xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:v="urn:schemas-microsoft-com:vml">"#;
    let wrapped=format!("{prefix}{raw}</mata:root>");
    let Ok(mut root)=xml::parse(&wrapped)else{return Ok(raw.to_string());};
    if !render_carriers(&mut root,scope,options,report)?{return Ok(raw.to_string());}
    let mut rendered=String::new();
    for child in &root.children{match child{Node::Element(e)=>rendered.push_str(&xml::serialize(e)),Node::Text(t)=>rendered.push_str(&xml::escape(t))}}
    Ok(rendered)
}

fn render_xml_nodes(node:&mut xml::Element,scope:&Scope<'_>,options:&Options,report:&mut Report)->io::Result<bool>{
    let mut changed=false;
    for child in &mut node.children{
        match child{
            Node::Element(e)=>{changed|=render_xml_nodes(e,scope,options,report)?;},
            Node::Text(text)=>if text.contains("{{"){
                let updated=render_text_scoped(text,scope,options,report)?;
                if *text!=updated{*text=updated;changed=true;}
            }
        }
    }
    Ok(changed)
}
/// Detect an isolated block directive in a WordprocessingML paragraph or
/// table row. This also works inside an otherwise opaque nested table part.
fn xml_block_marker(node:&Node)->Option<(String,String)>{
    let Node::Element(element)=node else{return None;};
    if !matches!(element.local(),"p"|"tr"){return None;}
    let text=element.text();let trimmed=text.trim();
    if !trimmed.starts_with("{{")||!trimmed.ends_with("}}") {return None;}
    let marker=trimmed[2..trimmed.len()-2].trim();
    if marker=="else"||marker.starts_with('#')||marker.starts_with('/'){
        let mut parts=marker.splitn(2,' ');
        return Some((parts.next()?.to_string(),parts.next().unwrap_or("").trim().to_string()));
    }
    None
}
/// Expand paragraph/row directives without flattening nested OOXML objects.
/// Unknown attributes and elements are cloned unchanged. Rendering happens
/// on each clone using its iteration scope, so repeated nested tables can
/// refer to different item values.
fn transform_ooxml_element(root:&mut xml::Element,scope:&Scope<'_>,options:&Options,
                           report:&mut Report,depth:usize)->io::Result<bool>{
    if depth>options.max_depth{return Err(invalid("OOXML template nesting exceeds configured limit"));}
    let before=root.clone();
    if matches!(root.local(),"p"){
        render_carriers(root,scope,options,report)?;
    }
    let children=std::mem::take(&mut root.children);
    let mut output=Vec::with_capacity(children.len());
    let mut index=0;
    while index<children.len(){
        if let Some((kind,expression))=xml_block_marker(&children[index]){
            if let Some(name)=kind.strip_prefix('#'){
                if !matches!(name,"if"|"unless"|"each") {return Err(invalid("unsupported OOXML template directive"));}
                let (end,otherwise)=find_branch(&children,index,xml_block_marker,name)?;
                let yes=&children[index+1..otherwise.unwrap_or(end)];
                let no=otherwise.map(|at|&children[at+1..end]).unwrap_or(&[]);
                let mut append=|nodes:&[Node],ctx:&Scope<'_>|->io::Result<()> {
                    for item in nodes{
                        let mut cloned=item.clone();
                        if let Node::Element(ref mut element)=cloned{
                            transform_ooxml_element(element,ctx,options,report,depth+1)?;
                        }else if let Node::Text(ref mut value)=cloned{
                            if value.contains("{{"){*value=render_text_scoped(value,ctx,options,report)?;}
                        }
                        output.push(cloned);
                        if output.len()>options.max_output_blocks{return Err(invalid("OOXML template block expansion limit exceeded"));}
                    }
                    Ok(())
                };
                match name {
                    "if"|"unless"=>append(if scope.value(&expression).truthy()!=(name=="unless"){yes}else{no},scope)?,
                    "each"=>match scope.value(&expression){
                        Value::Array(items) if !items.is_empty()=>{
                            let len=items.len();
                            for (i,item) in items.into_iter().enumerate(){
                                append(yes,&scope.child(item,Some(i),len))?;
                            }
                        },_=>append(no,scope)?,
                    },_=>unreachable!(),
                }
                report.expanded_blocks+=1;
                index=end+1;continue;
            }
            if kind.starts_with('/')||kind=="else"{return Err(invalid("orphan OOXML block directive"));}
        }
        let mut node=children[index].clone();
        match &mut node{
            Node::Element(element)=>{transform_ooxml_element(element,scope,options,report,depth+1)?;},
            Node::Text(value)=>{
                if value.contains("{{"){*value=render_text_scoped(value,scope,options,report)?;}
            },
        }
        output.push(node);index+=1;
    }
    root.children=output;
    Ok(*root!=before)
}

impl Document {
    /// Expand document templates and replace values across Word runs, headers,
    /// footers, tables, comments, notes, glossary entries and custom XML.
    /// Package XML editing is opt-in because extensions may own those parts.
    pub fn render_template(&mut self,values:&Value,options:&Options)->io::Result<Report>{
        let scope=Scope{root:values,stack:vec![values.clone()],index:None,length:0};let mut report=Report::default();
        // Expand sections independently, so section break positions remain
        // coherent after loops add or remove body blocks.
        if self.sections.is_empty(){
            self.blocks=expand_blocks(&self.blocks,&scope,options,&mut report,0)?;
        }else{
            let old=std::mem::take(&mut self.blocks);
            let mut output=Vec::new();let mut start=0;
            for section in &mut self.sections{
                let end=section.end_after.max(start).min(old.len());
                output.extend(expand_blocks(&old[start..end],&scope,options,&mut report,0)?);
                section.end_after=output.len();start=end;
            }
            output.extend(expand_blocks(&old[start..],&scope,options,&mut report,0)?);
            self.blocks=output;
        }
        for ps in [&mut self.header,&mut self.footer,&mut self.first_header,&mut self.first_footer]{
            for p in ps.iter_mut(){interpolate_paragraph(p,&scope,options,&mut report)?;}
        }
        for note in self.footnotes.iter_mut().chain(self.endnotes.iter_mut()){
            note.text=render_text_scoped(&note.text,&scope,options,&mut report)?;
        }
        for comment in &mut self.comments{comment.text=render_text_scoped(&comment.text,&scope,options,&mut report)?;}
        for glossary in &mut self.glossary{for p in &mut glossary.paragraphs{interpolate_paragraph(p,&scope,options,&mut report)?;}}
        for custom in &mut self.custom_xml{
            if let Ok(mut tree)=xml::parse(&custom.xml){if render_xml_nodes(&mut tree,&scope,options,&mut report)?{custom.xml=xml::serialize(&tree);}}
        }
        if options.process_package_xml{
            let paths=self.original_parts.keys().chain(self.package_overrides.keys()).filter(|p|p.ends_with(".xml")&&p.as_str()!="word/document.xml").cloned().collect::<std::collections::BTreeSet<_>>();
            for path in paths{
                if let Some(data)=self.part(&path){
                    if let Ok(raw)=xml::decode_bytes(data){if let Ok(mut tree)=xml::parse(&raw){
                        if transform_ooxml_element(&mut tree,&scope,options,&mut report,0)?{
                            let bytes=xml::serialize(&tree).into_bytes();
                            if bytes.len()>options.max_output_bytes{return Err(invalid("rendered OOXML package part too large"));}
                            self.put_part(path,bytes)?;report.changed_xml_parts+=1;
                        }
                    }}
                }
            }
        }
        Ok(report)
    }
    /// Search across adjacent styled runs, including header/footer and table
    /// cell paragraphs. The replacement supports `$1`, `${name}`, `$$`.
    pub fn replace_regex_across_runs(&mut self,expression:&str,replacement:&str)->io::Result<usize>{
        let pattern=crate::pattern::Pattern::new(expression)?;
        let mut count=0;
        let mut apply=|p:&mut Paragraph|->io::Result<()> {
            // Only commit a changed paragraph once every search has succeeded.
            // A budget/error must not silently truncate its styled runs.
            let old=p.inlines.clone();let mut out=Vec::new();let mut runs=Vec::new();
            let flush=|runs:&mut Vec<Run>,out:&mut Vec<Inline>,count:&mut usize|->io::Result<()> {
                if runs.is_empty(){return Ok(());}
                let full=runs.iter().map(|r|r.text.as_str()).collect::<String>();
                let mut replacements=Vec::new();let mut start=0;
                while start<=full.len(){
                    let Some(caps)=pattern.try_captures_from(&full,start)?else{break};
                    let (a,b)=caps.range;
                    replacements.push((a,b,caps.expand(&full,replacement)));*count+=1;
                    if a==b{if let Some(c)=full[b..].chars().next(){start=b+c.len_utf8();}else{break;}}
                    else{start=b;}
                }
                if replacements.is_empty(){out.extend(runs.iter().cloned().map(Inline::Run));}
                else{out.extend(replace_run_group(runs,&replacements));}
                runs.clear();Ok(())
            };
            for v in old {match v {Inline::Run(r) if r.break_kind.is_none()=>runs.push(r),other=>{
                flush(&mut runs,&mut out,&mut count)?;out.push(other);
            }}}
            flush(&mut runs,&mut out,&mut count)?;
            p.inlines=out;Ok(())
        };
        for b in &mut self.blocks {match b{
            Block::Paragraph(p)=>apply(p)?,
            Block::Table(t)=>for row in &mut t.rows{for cell in &mut row.cells{for p in &mut cell.paragraphs{apply(p)?;}}},
        }}
        for group in [&mut self.header,&mut self.footer,&mut self.first_header,&mut self.first_footer]{
            for p in group{apply(p)?;}
        }
        Ok(count)
    }
}

impl Document {
    /// Literal replacement in all text-bearing model parts (including notes,
    /// comments, glossary, and custom XML). Does not alter XML markup/attributes
    /// or binary OPC parts. Unlike a naive text replacement this can cross runs.
    pub fn replace_literal_everywhere(&mut self,find:&str,replacement:&str)->io::Result<usize>{
        if find.is_empty(){return Ok(0);}
        let mut escaped=String::new();
        for ch in find.chars(){
            if matches!(ch,'.'|'*'|'+'|'?'|'['|']'|'('|')'|'{'|'}'|'|'|'^'|'$'|'\\'){
                escaped.push('\\');
            }
            escaped.push(ch);
        }
        self.replace_regex_everywhere(&escaped,&replacement.replace('$',"$$"))
    }
    /// Regex replacement across adjacent runs, text notes, comments and
    /// custom XML text nodes. Replacement references support `$1`, `${name}`.
    pub fn replace_regex_everywhere(&mut self,expression:&str,replacement:&str)->io::Result<usize>{
        let regex=crate::pattern::Pattern::new(expression)?;
        let mut total=self.replace_regex_across_runs(expression,replacement)?;
        for note in self.footnotes.iter_mut().chain(self.endnotes.iter_mut()){
            let (text,count)=regex.try_replace_all(&note.text,replacement)?;note.text=text;total+=count;
        }
        for comment in &mut self.comments{
            let (text,count)=regex.try_replace_all(&comment.text,replacement)?;comment.text=text;total+=count;
        }
        for glossary in &mut self.glossary{
            for paragraph in &mut glossary.paragraphs{
                let source=paragraph.text();let (text,count)=regex.try_replace_all(&source,replacement)?;
                if count>0{paragraph.inlines=vec![Inline::Run(Run::new(text))];total+=count;}
            }
        }
        for item in &mut self.custom_xml{
            if let Ok(mut root)=xml::parse(&item.xml){
                fn replace_node(node:&mut xml::Element,regex:&crate::pattern::Pattern,replacement:&str,total:&mut usize)->io::Result<()> {
                    for child in &mut node.children{
                        match child{
                            Node::Element(e)=>replace_node(e,regex,replacement,total)? ,
                            Node::Text(text)=>{
                                let (new,count)=regex.try_replace_all(text,replacement)?;
                                if count>0{*text=new;*total+=count;}
                            }
                        }
                    }
                    Ok(())
                }
                let mut count=0;replace_node(&mut root,&regex,replacement,&mut count)?;
                if count>0{item.xml=xml::serialize(&root);total+=count;}
            }
        }
        Ok(total)
    }
}
