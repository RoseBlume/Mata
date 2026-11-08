//! Read/write of Open Packaging Conventions and WordprocessingML.
use crate::{model::*,xml::{self,Element,Node},zip,package};
use std::collections::{BTreeMap,HashMap};
use std::io::{self,Error,ErrorKind};
const W:&str="http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R:&str="http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG:&str="http://schemas.openxmlformats.org/package/2006/relationships";
fn err(msg:&str)->io::Error{Error::new(ErrorKind::InvalidData,msg)}
fn x(s:&str)->String{xml::escape(s)}
fn bool_prop(xml:&mut String,name:&str,on:bool){if on{xml.push_str(&format!("<w:{name}/>"));}}
fn wtext(s:&str)->String{format!("<w:t xml:space=\"preserve\">{}</w:t>",x(s))}
fn wn(name:&str,val:impl std::fmt::Display)->String{format!("<w:{name} w:val=\"{val}\"/>")}
fn with_font(r:&Run)->String {
    let mut o=String::new();
    if r.bold||r.italic||r.underline||r.strike||r.superscript||r.subscript||r.font_size.is_some()||r.font.is_some()||r.color.is_some()||r.highlight.is_some()||r.style.is_some() {
        o.push_str("<w:rPr>");
        bool_prop(&mut o,"b",r.bold);bool_prop(&mut o,"i",r.italic);bool_prop(&mut o,"strike",r.strike);
        if r.underline{o.push_str("<w:u w:val=\"single\"/>");}
        if r.superscript{o.push_str("<w:vertAlign w:val=\"superscript\"/>");}
        if r.subscript{o.push_str("<w:vertAlign w:val=\"subscript\"/>");}
        if let Some(v)=r.font_size{o.push_str(&wn("sz",(v*2.0).round() as u32));}
        if let Some(v)=&r.font{o.push_str(&format!("<w:rFonts w:ascii=\"{}\" w:hAnsi=\"{}\"/>",x(v),x(v)));}
        if let Some(v)=&r.color{if let Some(key)=v.strip_prefix("theme:"){
            o.push_str(&format!("<w:color w:themeColor=\"{}\"/>",x(key)));
        }else{o.push_str(&wn("color",x(v)));}}
        if let Some(v)=&r.highlight{o.push_str(&wn("highlight",x(v)));}
        if let Some(v)=&r.style{o.push_str(&wn("rStyle",x(v)));}
        o.push_str("</w:rPr>");
    }
    o
}
#[derive(Default)]struct Relations {items:Vec<(String,String,String,bool)>}
impl Relations {
    fn from_part(parts:&BTreeMap<String,Vec<u8>>,path:&str)->Self {
        let mut out=Self::default();
        if let Ok(Some(root))=part_xml(parts,path){for r in root.elements("Relationship") {
            if let (Some(id),Some(kind),Some(target))=(r.attr("Id"),r.attr("Type"),r.attr("Target")){
                if !out.items.iter().any(|i|i.2==id){out.items.push((kind.to_owned(),target.to_owned(),id.to_owned(),r.attr("TargetMode")==Some("External")));}
            }
        }}out
    }
    fn add(&mut self,kind:&str,target:&str,external:bool)->String {
        if let Some((_,_,id,_))=self.items.iter().find(|v|v.0==kind&&v.1==target&&v.3==external){return id.clone();}
        let mut index=1;
        while self.items.iter().any(|i|i.2==format!("rId{index}")){index+=1;}
        let id=format!("rId{index}");self.items.push((kind.to_owned(),target.to_owned(),id.clone(),external));id
    }
    fn to_xml(&self)->String{
        let mut s=format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"{PKG}\">");
        for (kind,target,id,external) in &self.items {
            s.push_str(&format!("<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{} />",x(id),x(kind),x(target),if *external{" TargetMode=\"External\""}else{""}));
        }
        s.push_str("</Relationships>");s
    }
}
fn rel_type(kind:&str)->String{format!("{R}/{kind}")}
/// Render a previously parsed run while retaining unknown Word extensions.
fn run_xml(r:&Run)->String {
    if let Some(source)=&r.original_xml {
        if r.original_fingerprint.as_ref()==Some(&r.fingerprint()) {
            return xml::serialize(source);
        }
        let mut updated=source.clone();
        let known=["rPr","t","delText","tab","cr","br"];
        updated.children.retain(|node|match node {Node::Element(e)=>!known.contains(&e.local()),_=>true});
        let mut model=format!("<w:r xmlns:w=\"{W}\">{}</w:r>",with_font(r));
        if let Ok(new)=xml::parse(&model){
            if let Some(pr)=new.child("rPr") {
                let mut p=pr.clone();
                if let Some(original)=source.child("rPr") {
                    const PROPERTIES:&[&str]=&["b","i","u","strike","vertAlign","color","highlight","rFonts","rStyle","sz"];
                    for n in &original.children {if let Node::Element(e)=n{
                        if !PROPERTIES.contains(&e.local()) {p.children.push(Node::Element(e.clone()));}
                    }}
                    for (key,value) in &original.attrs {p.attrs.entry(key.clone()).or_insert_with(||value.clone());}
                }
                updated.children.insert(0,Node::Element(p));
            } else if let Some(original)=source.child("rPr") {
                let mut p=original.clone();
                const PROPERTIES:&[&str]=&["b","i","u","strike","vertAlign","color","highlight","rFonts","rStyle","sz"];
                p.children.retain(|n|match n{Node::Element(e)=>!PROPERTIES.contains(&e.local()),_=>true});
                if !p.children.is_empty()||!p.attrs.is_empty(){updated.children.insert(0,Node::Element(p));}
            }
        }
        model.clear();
        if !r.text.is_empty(){
            let mut t=Element{name:"w:t".into(),..Default::default()};
            t.attrs.insert("xml:space".into(),"preserve".into());
            t.children.push(Node::Text(r.text.clone()));
            updated.children.push(Node::Element(t));
        }
        if let Some(kind)=r.break_kind {
            let mut br=Element{name:"w:br".into(),..Default::default()};
            match kind{BreakKind::Page=>{br.attrs.insert("w:type".into(),"page".into());},BreakKind::Column=>{br.attrs.insert("w:type".into(),"column".into());},_=>{}}
            updated.children.push(Node::Element(br));
        }
        return xml::serialize(&updated);
    }
    let mut out=String::from("<w:r>");out.push_str(&with_font(r));
    if !r.text.is_empty(){out.push_str(&wtext(&r.text));}
    if let Some(kind)=r.break_kind {out.push_str(match kind{BreakKind::Line=>"<w:br/>",BreakKind::Page=>"<w:br w:type=\"page\"/>",BreakKind::Column=>"<w:br w:type=\"column\"/>"});}
    out.push_str("</w:r>");out
}
/// Keep imported extension nodes and attributes in a modified paragraph.
fn merge_paragraph_extensions(original:&Element,generated:&str)->String {
    let wrapper=format!("<root xmlns:w=\"{W}\" xmlns:r=\"{R}\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\">{generated}</root>");
    let Ok(parsed)=xml::parse(&wrapper) else{return generated.to_owned()};
    let Some(mut updated)=parsed.child("p").cloned() else{return generated.to_owned()};
    for (name,value) in &original.attrs {updated.attrs.entry(name.clone()).or_insert_with(||value.clone());}
    if let Some(old_pr)=original.child("pPr"){
        let old_extras=old_pr.children.iter().filter(|n|match n{Node::Element(e)=>!matches!(e.local(),
            "pStyle"|"jc"|"numPr"|"spacing"|"ind"|"keepNext"|"keepLines"|"pageBreakBefore"|"widowControl"|"snapToGrid"|"contextualSpacing"|"tabs"|"pBdr"|"shd"|"sectPr"),_=>true}).cloned().collect::<Vec<_>>();
        if !old_extras.is_empty() {
            let at=updated.children.iter().position(|n|matches!(n,Node::Element(e) if e.local()=="pPr"));
            if let Some(at)=at {
                if let Node::Element(pr)=&mut updated.children[at] {pr.children.extend(old_extras);}
            } else {
                updated.children.insert(0,Node::Element(Element{name:"w:pPr".into(),children:old_extras,..Default::default()}));
            }
        }
    }
    xml::serialize(&updated)
}
fn revision_xml(kind:RevisionKind,info:&RevisionInfo,content:&[Inline])->String {
    let tag=match kind{RevisionKind::Insert=>"ins",RevisionKind::Delete=>"del",RevisionKind::MoveFrom=>"moveFrom",RevisionKind::MoveTo=>"moveTo"};
    let mut out=format!("<w:{tag} w:id=\"{}\" w:author=\"{}\"",info.id,x(&info.author));
    if let Some(date)=&info.date{out.push_str(&format!(" w:date=\"{}\"",x(date)));}
    out.push('>');
    for item in content {
        match item {
            Inline::Run(run)=>{
                let mut node=run_xml(run);
                if matches!(kind,RevisionKind::Delete|RevisionKind::MoveFrom){
                    node=node.replace("<w:t ","<w:delText ").replace("</w:t>","</w:delText>");
                }
                out.push_str(&node);
            },
            Inline::RawXml(raw)=>out.push_str(raw),
            Inline::CommentStart(id)=>out.push_str(&format!("<w:commentRangeStart w:id=\"{id}\"/>")),
            Inline::CommentEnd(id)=>out.push_str(&format!("<w:commentRangeEnd w:id=\"{id}\"/>")),
            Inline::Revision{kind,info,content}=>out.push_str(&revision_xml(*kind,info,content)),
            _=>{}
        }
    }
    out.push_str(&format!("</w:{tag}>"));out
}
fn structured_control_xml(c:&StructuredContentControl)->String {
    let mut s=String::from("<w:sdt><w:sdtPr>");
    if let Some(id)=c.id{s.push_str(&format!("<w:id w:val=\"{id}\"/>"));}
    if !c.tag.is_empty(){s.push_str(&format!("<w:tag w:val=\"{}\"/>",x(&c.tag)));}
    if !c.alias.is_empty(){s.push_str(&format!("<w:alias w:val=\"{}\"/>",x(&c.alias)));}
    if c.locked{s.push_str("<w:lock w:val=\"sdtLocked\"/>");}
    if let Some(binding)=&c.binding{s.push_str(&format!("<w:dataBinding w:storeItemID=\"{}\" w:xpath=\"{}\" w:prefixMappings=\"{}\"/>",
        x(&binding.store_item_id),x(&binding.xpath),x(&binding.prefix_mappings)));}
    if c.repeating {s.push_str("<w15:repeatingSection xmlns:w15=\"http://schemas.microsoft.com/office/word/2012/wordml\"/>");}
    if !c.rich_text{s.push_str("<w:text/>");}
    if let Some(original)=&c.original{
        if let Some(pr)=original.child("sdtPr") {for node in &pr.children {
            if let Node::Element(e)=node{if !matches!(e.local(),"id"|"tag"|"alias"|"lock"|"dataBinding"|"repeatingSection"|"text") {
                s.push_str(&xml::serialize(e));
            }}
        }}
    }
    s.push_str("</w:sdtPr><w:sdtContent><w:r>");s.push_str(&wtext(&c.value));s.push_str("</w:r></w:sdtContent></w:sdt>");s
}
fn pxml(p:&Paragraph,rel:&mut Relations,images:&[Image],bookid:&mut u32)->String{
    let mut out=String::from("<w:p>");
    if p.style.is_some()||p.alignment!=Alignment::Left||p.list.is_some()||p.spacing_before.is_some()||p.spacing_after.is_some()||p.line_spacing.is_some()||p.line_rule.is_some()||p.indent_left.is_some()||p.indent_right.is_some()||p.first_line.is_some()||p.keep_with_next||p.keep_together||p.page_break_before||p.widow_control.is_some()||p.snap_to_grid.is_some()||p.contextual_spacing||!p.tab_stops.is_empty()||p.shading.is_some()||p.borders.is_some() {
        out.push_str("<w:pPr>");
        if let Some(s)=&p.style {out.push_str(&wn("pStyle",x(s)));}
        if p.alignment!=Alignment::Left {out.push_str(&wn("jc",p.alignment.as_str()));}
        if let Some(num)=p.list{out.push_str(&format!("<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{}\"/></w:numPr>",num.level,num.num_id));}
        let mut attrs=String::new();
        if let Some(v)=p.spacing_before{attrs.push_str(&format!(" w:before=\"{v}\""));}
        if let Some(v)=p.spacing_after{attrs.push_str(&format!(" w:after=\"{v}\""));}
        if let Some(v)=p.line_spacing{attrs.push_str(&format!(" w:line=\"{v}\""));}
        if let Some(v)=p.line_rule{
            let rule=match v{LineSpacingRule::Auto=>"auto",LineSpacingRule::Exact=>"exact",LineSpacingRule::AtLeast=>"atLeast"};
            attrs.push_str(&format!(" w:lineRule=\"{rule}\""));
        }
        if !attrs.is_empty(){out.push_str(&format!("<w:spacing{attrs}/>"));}
        let mut attrs=String::new();if let Some(v)=p.indent_left{attrs.push_str(&format!(" w:left=\"{v}\""));}if let Some(v)=p.indent_right{attrs.push_str(&format!(" w:right=\"{v}\""));}if let Some(v)=p.first_line{attrs.push_str(&format!(" w:firstLine=\"{v}\""));}if !attrs.is_empty(){out.push_str(&format!("<w:ind{attrs}/>"));}
        bool_prop(&mut out,"keepNext",p.keep_with_next);bool_prop(&mut out,"keepLines",p.keep_together);bool_prop(&mut out,"pageBreakBefore",p.page_break_before);
        if let Some(widow)=p.widow_control {out.push_str(&format!("<w:widowControl w:val=\"{}\"/>",if widow{1}else{0}));}
        if let Some(snap)=p.snap_to_grid{out.push_str(&format!("<w:snapToGrid w:val=\"{}\"/>",if snap{1}else{0}));}
        bool_prop(&mut out,"contextualSpacing",p.contextual_spacing);
        if !p.tab_stops.is_empty(){
            out.push_str("<w:tabs>");
            for stop in &p.tab_stops{
                let align=match stop.alignment{TabAlignment::Left=>"left",TabAlignment::Center=>"center",TabAlignment::Right=>"right",TabAlignment::Decimal=>"decimal"};
                let leader=match stop.leader{TabLeader::None=>"none",TabLeader::Dots=>"dot",TabLeader::Dashes=>"hyphen",TabLeader::Line=>"underscore",TabLeader::MiddleDots=>"middleDot"};
                out.push_str(&format!("<w:tab w:val=\"{align}\" w:pos=\"{}\" w:leader=\"{leader}\"/>",stop.position_twips));
            }
            out.push_str("</w:tabs>");
        }
        if let Some(b)=&p.borders{out.push_str("<w:pBdr>");for side in ["top","bottom","left","right"]{out.push_str(&border_xml(side,b));}out.push_str("</w:pBdr>");}
        if let Some(fill)=&p.shading{out.push_str(&format!("<w:shd w:fill=\"{}\"/>",x(fill)));}
        out.push_str("</w:pPr>");
    }
    for inline in &p.inlines{
        match inline {
            Inline::Revision{kind,info,content}=>out.push_str(&revision_xml(*kind,info,content)),
            Inline::StructuredControl(c)=>out.push_str(&structured_control_xml(c)),
            Inline::ComplexField{instruction,result,locked,dirty}=>{
                out.push_str(&format!("<w:r><w:fldChar w:fldCharType=\"begin\"{}{} /></w:r><w:r><w:instrText xml:space=\"preserve\">{}</w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r>{}</w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r>",
                    if *locked{" w:fldLock=\"1\""}else{""},if *dirty{" w:dirty=\"1\""}else{""},x(instruction),wtext(result)));
            },
            Inline::Run(r)=>{
                if let Some(url)=&r.hyperlink {let id=rel.add(&rel_type("hyperlink"),url,true);out.push_str(&format!("<w:hyperlink r:id=\"{id}\">"));}
                if let Some(dest)=&r.internal_link {out.push_str(&format!("<w:hyperlink w:anchor=\"{}\">",x(dest)));}
                out.push_str(&run_xml(r));
                if r.hyperlink.is_some()||r.internal_link.is_some(){out.push_str("</w:hyperlink>");}
            }
            Inline::Bookmark(n)=>{*bookid+=1;out.push_str(&format!("<w:bookmarkStart w:id=\"{}\" w:name=\"{}\"/><w:bookmarkEnd w:id=\"{}\"/>",*bookid,x(n),*bookid));}
            Inline::FootnoteRef(id)=>out.push_str(&format!("<w:r><w:footnoteReference w:id=\"{id}\"/></w:r>")),
            Inline::EndnoteRef(id)=>out.push_str(&format!("<w:r><w:endnoteReference w:id=\"{id}\"/></w:r>")),
            Inline::CommentStart(id)=>out.push_str(&format!("<w:commentRangeStart w:id=\"{id}\"/>")),
            Inline::CommentEnd(id)=>out.push_str(&format!("<w:commentRangeEnd w:id=\"{id}\"/><w:r><w:commentReference w:id=\"{id}\"/></w:r>")),
            Inline::Field(s)=>out.push_str(&format!("<w:fldSimple w:instr=\"{}\"/>",x(s))),
            Inline::RawXml(s)=>out.push_str(s),
            Inline::ContentControl{tag,value}=>out.push_str(&format!("<w:sdt><w:sdtPr><w:alias w:val=\"{}\"/><w:tag w:val=\"{}\"/><w:text/></w:sdtPr><w:sdtContent><w:r>{}</w:r></w:sdtContent></w:sdt>",x(tag),x(tag),wtext(value))),
            Inline::Checkbox{tag,checked}=>out.push_str(&format!("<w:sdt><w:sdtPr><w:tag w:val=\"{}\"/><w14:checkbox xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\"><w14:checked w14:val=\"{}\"/></w14:checkbox></w:sdtPr><w:sdtContent><w:r>{}</w:r></w:sdtContent></w:sdt>",x(tag),if *checked{"1"}else{"0"},wtext(if *checked{"☒"}else{"☐"}))),
            Inline::Dropdown{tag,options,selected}=>{out.push_str(&format!("<w:sdt><w:sdtPr><w:tag w:val=\"{}\"/><w:dropDownList>",x(tag)));for item in options{out.push_str(&format!("<w:listItem w:displayText=\"{}\" w:value=\"{}\"/>",x(item),x(item)));}out.push_str("</w:dropDownList></w:sdtPr><w:sdtContent><w:r>");out.push_str(&wtext(options.get(*selected).map(String::as_str).unwrap_or("")));out.push_str("</w:r></w:sdtContent></w:sdt>");},
            Inline::MathExpr(expr)=>out.push_str(&math_expr_xml(expr)),
            Inline::Math(formula)=>out.push_str(&format!("<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><m:r><m:t>{}</m:t></m:r></m:oMath>",x(formula))),
            Inline::Shape{width,height,fill,text}=>out.push_str(&format!("<w:r><w:pict><v:shape xmlns:v=\"urn:schemas-microsoft-com:vml\" style=\"width:{}pt;height:{}pt\" fillcolor=\"#{}\"><v:textbox><w:txbxContent><w:p><w:r>{}</w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r>",width.0/12700,height.0/12700,x(fill),wtext(text))),
            Inline::TrackedInsert{author,text}=>{*bookid+=1;out.push_str(&format!("<w:ins w:id=\"{}\" w:author=\"{}\"><w:r>{}</w:r></w:ins>",*bookid,x(author),wtext(text)));},
            Inline::TrackedDelete{author,text}=>{*bookid+=1;out.push_str(&format!("<w:del w:id=\"{}\" w:author=\"{}\"><w:r><w:delText>{}</w:delText></w:r></w:del>",*bookid,x(author),x(text)));},
            Inline::Chart(index)=>{let id=rel.add(&rel_type("chart"),&format!("charts/chart{}.xml",index+1),false);out.push_str(&format!("<w:r><w:drawing><wp:inline xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\"><wp:extent cx=\"5486400\" cy=\"3200400\"/><wp:docPr id=\"{}\" name=\"Chart\"/><a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:chart xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" r:id=\"{}\"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>",index+100,id));},
            Inline::Image(index)=>if let Some(img)=images.get(*index){
                let extension=img.filename.rsplit('.').next().unwrap_or("png");
                let target=format!("media/image{}.{}",index+1,extension);
                let id=rel.add(&rel_type("image"),&target,false);
                let mut markup=format!(concat!("<w:r><w:drawing><wp:inline xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\">",
                    "<wp:extent cx=\"{}\" cy=\"{}\"/><wp:docPr id=\"{}\" name=\"Picture {}\"/>",
                    "<a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">",
                    "<pic:pic xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:nvPicPr><pic:cNvPr id=\"0\" name=\"{}\"/><pic:cNvPicPr/></pic:nvPicPr>",
                    "<pic:blipFill><a:blip r:embed=\"{}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>",
                    "<pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>",
                    "</a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"),img.width.0,img.height.0,index+1,index+1,x(&img.filename),id,img.width.0,img.height.0);
                if img.floating {
                    let pos=img.positioning.clone().unwrap_or_default();
                    let anchor=format!("<wp:anchor xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" distT=\"{}\" distB=\"{}\" distL=\"{}\" distR=\"{}\" simplePos=\"0\" relativeHeight=\"{}\" behindDoc=\"{}\" locked=\"0\" layoutInCell=\"{}\" allowOverlap=\"{}\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"{}\"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"{}\"><wp:posOffset>{}</wp:posOffset></wp:positionV>",
                        pos.distance_top,pos.distance_bottom,pos.distance_left,pos.distance_right,pos.z_order,
                        if pos.behind_text{1}else{0},if pos.layout_in_cell{1}else{0},if pos.allow_overlap{1}else{0},
                        pos.horizontal.as_str(),pos.x_emu,pos.vertical.as_str(),pos.y_emu);
                    markup=markup.replacen("<wp:inline xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\">",&anchor,1);
                    let wrap=match pos.wrap {
                        ImageWrap::Square=>"<wp:wrapSquare wrapText=\"bothSides\"/>",
                        ImageWrap::TopAndBottom=>"<wp:wrapTopAndBottom/>",
                        ImageWrap::None=>"<wp:wrapNone/>",
                        ImageWrap::Tight|ImageWrap::Through=>"",
                    };
                    let wrap_owned;
                    let wrap=if matches!(pos.wrap,ImageWrap::Tight|ImageWrap::Through){
                        let tag=if pos.wrap==ImageWrap::Tight{"wrapTight"}else{"wrapThrough"};
                        let polygon=if pos.wrap_polygon.len()>=3 && pos.wrap_polygon.len()<=4096 {
                            pos.wrap_polygon.clone()
                        } else {vec![(0,0),(21600,0),(21600,21600),(0,21600)]};
                        let (first_x,first_y)=polygon[0];
                        wrap_owned=format!("<wp:{tag} wrapText=\"bothSides\"><wp:wrapPolygon edited=\"0\"><wp:start x=\"{first_x}\" y=\"{first_y}\"/>{}</wp:wrapPolygon></wp:{tag}>",
                            polygon.iter().skip(1).map(|(x,y)|format!("<wp:lineTo x=\"{x}\" y=\"{y}\"/>")).collect::<String>());
                        wrap_owned.as_str()
                    }else{wrap};
                    markup=markup.replacen("<wp:docPr ",&format!("{wrap}<wp:docPr "),1)
                        .replacen("</wp:inline>","</wp:anchor>",1);
                    if pos.rotation_degrees.is_finite() && pos.rotation_degrees!=0.0{
                        markup=markup.replacen("<a:xfrm>",&format!("<a:xfrm rot=\"{}\">",(pos.rotation_degrees*60000.0).round() as i64),1);
                    }
                }
                out.push_str(&markup);
            },
        }
    }
    out.push_str("</w:p>");out
}
fn border_xml(tag:&str,border:&Border)->String{
    let style=if border.style.is_empty(){"single"}else{&border.style};
    let color=if border.color.is_empty(){"auto"}else{&border.color};
    format!("<w:{tag} w:val=\"{}\" w:sz=\"{}\" w:color=\"{}\"/>",x(style),border.size.max(4),x(color))
}
fn parse_border(element:&Element)->Option<Border>{
    let c=element.child("top").or_else(||element.child("bottom")).or_else(||element.child("left"))?;
    Some(Border{color:c.attr("color").unwrap_or("auto").to_owned(),size:attr_num(c,"sz").unwrap_or(4),style:c.attr("val").unwrap_or("single").to_owned()})
}
fn parse_border_edges(element:&Element)->Option<BorderEdges>{
 let get=|tag:&str|element.child(tag).map(|v|Border{
  color:v.attr("color").unwrap_or("auto").to_owned(),
  size:attr_num(v,"sz").unwrap_or(4),style:v.attr("val").unwrap_or("single").to_owned(),
 });
 let edges=BorderEdges{top:get("top"),bottom:get("bottom"),left:get("left"),right:get("right"),
  inside_h:get("insideH"),inside_v:get("insideV"),start:get("start"),end:get("end")};
 if edges.top.is_none()&&edges.bottom.is_none()&&edges.left.is_none()&&edges.right.is_none()&&
    edges.inside_h.is_none()&&edges.inside_v.is_none()&&edges.start.is_none()&&edges.end.is_none(){None}else{Some(edges)}
}
fn border_edges_xml(edges:&BorderEdges)->String{
 let mut out=String::new();
 for (tag,value) in [("top",edges.top.as_ref()),("left",edges.left.as_ref()),
  ("bottom",edges.bottom.as_ref()),("right",edges.right.as_ref()),
  ("insideH",edges.inside_h.as_ref()),("insideV",edges.inside_v.as_ref()),
  ("start",edges.start.as_ref()),("end",edges.end.as_ref())]{
  if let Some(border)=value{out.push_str(&border_xml(tag,border));}
 }
 out
}
fn tablexml(t:&Table,rel:&mut Relations,images:&[Image],bookid:&mut u32)->String{
    let mut s=String::from("<w:tbl><w:tblPr>");
    if let Some(st)=&t.style{s.push_str(&wn("tblStyle",x(st)));}
    if let Some(p)=t.width_percent{s.push_str(&format!("<w:tblW w:w=\"{}\" w:type=\"pct\"/>",(p.clamp(0.,1000.)*50.).round() as u32));}
    else if let Some(w)=t.width_twips{s.push_str(&format!("<w:tblW w:w=\"{w}\" w:type=\"dxa\"/>"));}
    if t.layout_fixed{s.push_str("<w:tblLayout w:type=\"fixed\"/>");}
    if t.alignment!=Alignment::Left{s.push_str(&wn("jc",t.alignment.as_str()));}
    if let Some(ind)=t.indent_twips{s.push_str(&format!("<w:tblInd w:w=\"{ind}\" w:type=\"dxa\"/>"));}
    if let Some(pad)=t.cell_margin_twips{s.push_str("<w:tblCellMar>");for edge in ["top","left","bottom","right"]{s.push_str(&format!("<w:{edge} w:w=\"{pad}\" w:type=\"dxa\"/>"));}s.push_str("</w:tblCellMar>");}
    if let Some(b)=&t.edge_borders{s.push_str("<w:tblBorders>");s.push_str(&border_edges_xml(b));s.push_str("</w:tblBorders>");}
    else if let Some(b)=&t.borders {s.push_str("<w:tblBorders>");for edge in ["top","left","bottom","right","insideH","insideV"] {s.push_str(&border_xml(edge,b));}s.push_str("</w:tblBorders>");}
    s.push_str("</w:tblPr><w:tblGrid>");for w in &t.column_widths{s.push_str(&format!("<w:gridCol w:w=\"{w}\"/>"));}s.push_str("</w:tblGrid>");
    for row in &t.rows {s.push_str("<w:tr>");if row.is_header||row.height_twips.is_some()||row.cant_split||row.grid_before!=0||row.grid_after!=0{s.push_str("<w:trPr>");if row.grid_before!=0{s.push_str(&wn("gridBefore",row.grid_before));}if row.grid_after!=0{s.push_str(&wn("gridAfter",row.grid_after));}if row.is_header{s.push_str("<w:tblHeader/>");}if row.cant_split{s.push_str("<w:cantSplit/>");}if let Some(v)=row.height_twips{
            let rule=match row.height_rule{RowHeightRule::Auto=>"auto",RowHeightRule::AtLeast=>"atLeast",RowHeightRule::Exact=>"exact"};
            s.push_str(&format!("<w:trHeight w:val=\"{v}\" w:hRule=\"{rule}\"/>"));
        }s.push_str("</w:trPr>");}
        for cell in &row.cells {s.push_str("<w:tc><w:tcPr>");if let Some(w)=cell.width_twips{s.push_str(&format!("<w:tcW w:w=\"{w}\" w:type=\"dxa\"/>"));}if cell.colspan>1{s.push_str(&wn("gridSpan",cell.colspan));}if let Some(v)=cell.vertical_merge{s.push_str(if v{"<w:vMerge w:val=\"restart\"/>"}else{"<w:vMerge/>"});}if let Some(c)=&cell.shading{s.push_str(&format!("<w:shd w:fill=\"{}\"/>",x(c)));}if cell.valign!=VerticalAlignment::Top{s.push_str(&wn("vAlign",if cell.valign==VerticalAlignment::Center{"center"}else{"bottom"}));}
            if let Some(dir)=&cell.text_direction{s.push_str(&wn("textDirection",x(dir)));}
            if let Some(b)=&cell.edge_borders{s.push_str("<w:tcBorders>");s.push_str(&border_edges_xml(b));s.push_str("</w:tcBorders>");}
            else if let Some(b)=&cell.borders{s.push_str("<w:tcBorders>");for edge in ["top","left","bottom","right"]{s.push_str(&border_xml(edge,b));}s.push_str("</w:tcBorders>");}
            s.push_str("</w:tcPr>");if cell.paragraphs.is_empty(){s.push_str("<w:p/>");}else{for p in &cell.paragraphs{s.push_str(&pxml(p,rel,images,bookid));}}s.push_str("</w:tc>");
        }s.push_str("</w:tr>");}
    s.push_str("</w:tbl>");s
}
fn stylesxml(styles:&[Style])->String{
    let mut s=format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:styles xmlns:w=\"{W}\"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\"/><w:sz w:val=\"22\"/></w:rPr></w:rPrDefault></w:docDefaults>");
    s.push_str("<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style>");
    for level in 1..=9{s.push_str(&format!("<w:style w:type=\"paragraph\" w:styleId=\"Heading{level}\"><w:name w:val=\"heading {level}\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before=\"360\" w:after=\"120\"/></w:pPr><w:rPr><w:b/><w:sz w:val=\"{}\"/></w:rPr></w:style>", (34i32-2*level).max(20)));}
    for style in styles {
        let kind=match style.kind{StyleType::Paragraph=>"paragraph",StyleType::Character=>"character",StyleType::Table=>"table"};
        s.push_str(&format!("<w:style w:type=\"{kind}\" w:styleId=\"{}\"><w:name w:val=\"{}\"/>",x(&style.id),x(&style.name)));
        if let Some(b)=&style.based_on{s.push_str(&wn("basedOn",x(b)));}
        s.push_str("<w:rPr>");if let Some(enabled)=style.bold_override {s.push_str(&format!("<w:b w:val=\"{}\"/>",if enabled{1}else{0}));}else{bool_prop(&mut s,"b",style.bold);}
        if let Some(enabled)=style.italic_override {s.push_str(&format!("<w:i w:val=\"{}\"/>",if enabled{1}else{0}));}else{bool_prop(&mut s,"i",style.italic);}
        if let Some(v)=&style.font{s.push_str(&format!("<w:rFonts w:ascii=\"{}\" w:hAnsi=\"{}\"/>",x(v),x(v)));}
        if let Some(v)=style.size{s.push_str(&wn("sz",(v*2.0) as u32));}
        if let Some(v)=&style.color{if let Some(key)=v.strip_prefix("theme:"){
            s.push_str(&format!("<w:color w:themeColor=\"{}\"/>",x(key)));
        }else{s.push_str(&wn("color",x(v)));}}s.push_str("</w:rPr></w:style>");
    }
    s.push_str("</w:styles>");s
}
fn numberingxml(doc:&Document)->String{
    let mut s=format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:numbering xmlns:w=\"{W}\">");
    let mut nums=std::collections::BTreeMap::new();nums.insert(1u32,false);nums.insert(2u32,true);
    let paras=doc.blocks.iter().flat_map(|block|match block{
        Block::Paragraph(p)=>vec![p],
        Block::Table(t)=>t.rows.iter().flat_map(|r|r.cells.iter().flat_map(|c|c.paragraphs.iter())).collect()
    });
    for p in paras {if let Some(num)=p.list{nums.insert(num.num_id,num.bullet);}}
    for (i,bullet) in nums{
        let fmt=if bullet{"bullet"}else{"decimal"};let value=if bullet{"•"}else{"%1."};
        s.push_str(&format!("<w:abstractNum w:abstractNumId=\"{i}\">"));
        for level in 0..=8{s.push_str(&format!("<w:lvl w:ilvl=\"{level}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{fmt}\"/><w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"left\"/></w:lvl>",x(value)));}
        s.push_str(&format!("</w:abstractNum><w:num w:numId=\"{i}\"><w:abstractNumId w:val=\"{i}\"/></w:num>"));
    }
    s.push_str("</w:numbering>");s
}
fn notesxml(notes:&[Note],tag:&str)->String{
    let mut s=format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:{tag}s xmlns:w=\"{W}\">");
    for n in notes{s.push_str(&format!("<w:{tag} w:id=\"{}\"><w:p><w:r><w:{tag}Ref/></w:r><w:r>{}</w:r></w:p></w:{tag}>",n.id,wtext(&n.text)));}
    s.push_str(&format!("</w:{tag}s>"));s
}
fn comment_para_id(c:&Comment,meta:&BTreeMap<u32,CommentMetadata>)->String {
    meta.get(&c.id).and_then(|m|m.paragraph_id.clone()).unwrap_or_else(||format!("{:08X}",c.id))
}
fn commentsxml(comments:&[Comment],meta:&BTreeMap<u32,CommentMetadata>)->String{
    let mut s=format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:comments xmlns:w=\"{W}\" xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\">");
    for c in comments {
        s.push_str(&format!("<w:comment w:id=\"{}\" w:author=\"{}\"",c.id,x(&c.author)));
        if let Some(info)=meta.get(&c.id){
            if let Some(initials)=&info.initials {s.push_str(&format!(" w:initials=\"{}\"",x(initials)));}
            if let Some(date)=&info.created {s.push_str(&format!(" w:date=\"{}\"",x(date)));}
        }
        s.push_str(&format!("><w:p w14:paraId=\"{}\"><w:r>{}</w:r></w:p></w:comment>",x(&comment_para_id(c,meta)),wtext(&c.text)));
    }
    s.push_str("</w:comments>");s
}
fn comment_extensions_xml(comments:&[Comment],meta:&BTreeMap<u32,CommentMetadata>)->String{
    let mut s=String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w15:commentsEx xmlns:w15=\"http://schemas.microsoft.com/office/word/2012/wordml\">");
    let ids:HashMap<u32,String>=comments.iter().map(|c|(c.id,comment_para_id(c,meta))).collect();
    for c in comments {
        let done=meta.get(&c.id).map(|m|m.resolved).unwrap_or(false);
        s.push_str(&format!("<w15:commentEx w15:paraId=\"{}\"{} w15:done=\"{}\"/>",x(&comment_para_id(c,meta)),
            c.parent.and_then(|p|ids.get(&p)).map(|v|format!(" w15:paraIdParent=\"{}\"",x(v))).unwrap_or_default(),if done{1}else{0}));
    }
    s.push_str("</w15:commentsEx>");s
}
fn comment_ids_xml(comments:&[Comment],meta:&BTreeMap<u32,CommentMetadata>)->String{
    let mut s=String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w16cid:commentsIds xmlns:w16cid=\"http://schemas.microsoft.com/office/word/2016/wordml/cid\">");
    for c in comments {
        if let Some(id)=meta.get(&c.id).and_then(|v|v.durable_id.as_deref()){
            s.push_str(&format!("<w16cid:commentId w16cid:paraId=\"{}\" w16cid:durableId=\"{}\"/>",x(&comment_para_id(c,meta)),x(id)));
        }
    }
    s.push_str("</w16cid:commentsIds>");s
}
fn comment_people_xml(comments:&[Comment],meta:&BTreeMap<u32,CommentMetadata>)->String{
    let mut s=String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w15:people xmlns:w15=\"http://schemas.microsoft.com/office/word/2012/wordml\">");
    let mut authors=std::collections::BTreeSet::new();
    for c in comments {if authors.insert(&c.author){
        s.push_str(&format!("<w15:person w15:author=\"{}\">",x(&c.author)));
        if let Some(info)=meta.get(&c.id){
            if info.user_id.is_some()||info.provider_id.is_some(){
                s.push_str("<w15:presenceInfo");
                if let Some(provider)=&info.provider_id{s.push_str(&format!(" w15:providerId=\"{}\"",x(provider)));}
                if let Some(user)=&info.user_id{s.push_str(&format!(" w15:userId=\"{}\"",x(user)));}
                s.push_str("/>");
            }
        }
        s.push_str("</w15:person>");
    }}
    s.push_str("</w15:people>");s
}
fn glossary_xml(doc:&Document)->String{
    let mut s=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:glossaryDocument xmlns:w=\"{W}\"><w:docParts>");
    let mut unused=Relations::default();let mut id=0;
    for block in &doc.glossary {
        s.push_str(&format!("<w:docPart><w:docPartPr><w:name w:val=\"{}\"/><w:category><w:name w:val=\"{}\"/></w:category></w:docPartPr><w:docPartBody>",x(&block.name),x(&block.category)));
        for p in &block.paragraphs{s.push_str(&pxml(p,&mut unused,&doc.images,&mut id));}
        s.push_str("</w:docPartBody></w:docPart>");
    }
    s.push_str("</w:docParts></w:glossaryDocument>");s
}
fn font_table_xml(fonts:&[EmbeddedFont])->String{
    let mut s=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:fonts xmlns:w=\"{W}\" xmlns:r=\"{R}\">");
    for (i,f) in fonts.iter().enumerate(){s.push_str(&format!("<w:font w:name=\"{}\"><w:embedRegular r:id=\"rId{}\" w:fontKey=\"{}\"/></w:font>",x(&f.name),i+1,x(&f.key_guid)));}
    s.push_str("</w:fonts>");s
}
fn corexml(c:&CoreProperties)->String{
    let mut s=String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">");
    for (tag,v) in [("dc:title",&c.title),("dc:subject",&c.subject),("dc:creator",&c.creator),("cp:keywords",&c.keywords),("dc:description",&c.description),("cp:lastModifiedBy",&c.last_modified_by),("cp:category",&c.category),("cp:contentStatus",&c.content_status),("dc:language",&c.language),("dc:identifier",&c.identifier),("cp:version",&c.version)]{if !v.is_empty(){s.push_str(&format!("<{tag}>{}</{tag}>",x(v)));}}
    if let Some(v)=c.revision{s.push_str(&format!("<cp:revision>{v}</cp:revision>"));}
    for (tag,v) in [("created",&c.created),("modified",&c.modified)] {if !v.is_empty(){s.push_str(&format!("<dcterms:{tag} xsi:type=\"dcterms:W3CDTF\">{}</dcterms:{tag}>",x(v)));}}
    s.push_str("</cp:coreProperties>");s
}
fn content_types(parts:&BTreeMap<String,Vec<u8>>)->String{
    let mut s=String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>");
    for (ext,typ) in [("png","image/png"),("jpeg","image/jpeg"),("jpg","image/jpeg"),("gif","image/gif"),("bmp","image/bmp"),("svg","image/svg+xml"),("emf","image/x-emf")]{if parts.keys().any(|k|k.ends_with(&format!(".{ext}"))){s.push_str(&format!("<Default Extension=\"{ext}\" ContentType=\"{typ}\"/>"));}}
    for (name,kind) in [("word/document.xml","wordprocessingml.document.main+xml"),("word/styles.xml","wordprocessingml.styles+xml"),("word/settings.xml","wordprocessingml.settings+xml"),("word/numbering.xml","wordprocessingml.numbering+xml"),("word/header1.xml","wordprocessingml.header+xml"),("word/footer1.xml","wordprocessingml.footer+xml"),("word/header2.xml","wordprocessingml.header+xml"),("word/footer2.xml","wordprocessingml.footer+xml"),("word/footnotes.xml","wordprocessingml.footnotes+xml"),("word/endnotes.xml","wordprocessingml.endnotes+xml"),("word/comments.xml","wordprocessingml.comments+xml"),("word/glossary/document.xml","wordprocessingml.document.glossary+xml"),("word/fontTable.xml","wordprocessingml.fontTable+xml")]{if parts.contains_key(name){s.push_str(&format!("<Override PartName=\"/{name}\" ContentType=\"application/vnd.openxmlformats-officedocument.{kind}\"/>"));}}
    if parts.contains_key("word/commentsExtended.xml"){
        s.push_str("<Override PartName=\"/word/commentsExtended.xml\" ContentType=\"application/vnd.ms-word.commentsExtended+xml\"/>");
    }
    if parts.contains_key("word/people.xml"){
        s.push_str("<Override PartName=\"/word/people.xml\" ContentType=\"application/vnd.ms-word.people+xml\"/>");
    }
    if parts.contains_key("word/commentsIds.xml") {
        s.push_str("<Override PartName=\"/word/commentsIds.xml\" ContentType=\"application/vnd.ms-word.commentsIds+xml\"/>");
    }
    if parts.keys().any(|p|p.ends_with(".odttf")){s.push_str("<Default Extension=\"odttf\" ContentType=\"application/vnd.openxmlformats-officedocument.obfuscatedFont\"/>");}
    for name in parts.keys().filter(|p|p.starts_with("customXml/itemProps")&&p.ends_with(".xml")){
        s.push_str(&format!("<Override PartName=\"/{name}\" ContentType=\"application/vnd.openxmlformats-officedocument.customXmlProperties+xml\"/>"));
    }
    if parts.contains_key("docProps/core.xml"){s.push_str("<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>");}
    for key in parts.keys() {if key.starts_with("word/charts/chart") {s.push_str(&format!("<Override PartName=\"/{key}\" ContentType=\"application/vnd.openxmlformats-officedocument.drawingml.chart+xml\"/>"));}}
    s.push_str("</Types>");s
}
fn chartxml(chart:&Chart)->String{
    let mut s=String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><c:chart>");
    if !chart.title.is_empty(){s.push_str(&format!("<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p></c:rich></c:tx></c:title>",x(&chart.title)));}
    s.push_str("<c:plotArea><c:layout/><c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/><c:ser><c:idx val=\"0\"/><c:order val=\"0\"/><c:tx><c:v>Series 1</c:v></c:tx><c:cat><c:strLit>");
    s.push_str(&format!("<c:ptCount val=\"{}\"/>",chart.categories.len()));
    for (i,category) in chart.categories.iter().enumerate(){s.push_str(&format!("<c:pt idx=\"{i}\"><c:v>{}</c:v></c:pt>",x(category)));}
    s.push_str("</c:strLit></c:cat><c:val><c:numLit><c:formatCode>General</c:formatCode>");
    s.push_str(&format!("<c:ptCount val=\"{}\"/>",chart.values.len()));
    for (i,val) in chart.values.iter().enumerate(){if val.is_finite(){s.push_str(&format!("<c:pt idx=\"{i}\"><c:v>{val}</c:v></c:pt>"));}}
    s.push_str("</c:numLit></c:val></c:ser><c:axId val=\"12345\"/><c:axId val=\"12346\"/></c:barChart><c:catAx><c:axId val=\"12345\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:axPos val=\"b\"/><c:crossAx val=\"12346\"/><c:crosses val=\"autoZero\"/></c:catAx><c:valAx><c:axId val=\"12346\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:axPos val=\"l\"/><c:crossAx val=\"12345\"/><c:crosses val=\"autoZero\"/></c:valAx></c:plotArea><c:legend><c:legendPos val=\"b\"/></c:legend></c:chart></c:chartSpace>");s
}
fn merged_content_types(parts:&BTreeMap<String,Vec<u8>>,fresh:&str)->String {
    let Some(old)=parts.get("[Content_Types].xml") else{return fresh.to_owned()};
    let Ok(old)=std::str::from_utf8(old) else{return fresh.to_owned()};
    let (Ok(mut original),Ok(latest))=(xml::parse(old),xml::parse(fresh)) else{return fresh.to_owned()};
    original.children.retain(|node|if let Node::Element(e)=node{
        e.local()!="Override" || e.attr("PartName").map(|name|parts.contains_key(name.trim_start_matches('/'))).unwrap_or(false)
    }else{true});
    let mut seen=std::collections::BTreeSet::new();
    for child in &original.children {if let Node::Element(e)=child {
        if let Some(key)=e.attr(if e.local()=="Default"{"Extension"}else{"PartName"}){seen.insert((e.local().to_string(),key.to_string()));}
    }}
    for child in latest.children {if let Node::Element(ref e)=child {
        if let Some(key)=e.attr(if e.local()=="Default"{"Extension"}else{"PartName"}){
            let marker=(e.local().to_string(),key.to_string());
            if seen.insert(marker.clone()){original.children.push(Node::Element(e.clone()));}
            else if let Some(existing)=original.children.iter_mut().filter_map(|n|if let Node::Element(e)=n{Some(e)}else{None}).find(|old|old.local()==e.local() && old.attr(if e.local()=="Default"{"Extension"}else{"PartName"})==Some(key)){
                if existing.attr("ContentType")!=e.attr("ContentType") {*existing=e.clone();}
            }
        }
    }}
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>{}",xml::serialize(&original))
}
/// Build a section's WordprocessingML without dropping unknown section nodes.
fn section_full_xml(section:&Section)->String {
    let mut out=String::from("<w:sectPr>");
    for (kind,refs) in [("header",&section.header_refs),("footer",&section.footer_refs)] {
        for (style,id) in refs {out.push_str(&format!("<w:{kind}Reference w:type=\"{}\" r:id=\"{}\"/>",x(style),x(id)));}
    }
    out.push_str(&format!("<w:type w:val=\"{}\"/>",match section.break_kind{
        SectionBreak::Continuous=>"continuous",SectionBreak::NextPage=>"nextPage",
        SectionBreak::EvenPage=>"evenPage",SectionBreak::OddPage=>"oddPage"}));
    let p=&section.page;
    out.push_str(&format!("<w:pgSz w:w=\"{}\" w:h=\"{}\" w:orient=\"{}\"/>",p.width_twips,p.height_twips,
        if p.orientation==Orientation::Landscape{"landscape"}else{"portrait"}));
    out.push_str(&format!("<w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"{}\" w:footer=\"{}\" w:gutter=\"{}\"/>",
        p.margin_top,p.margin_right,p.margin_bottom,p.margin_left,section.header_distance_twips,section.footer_distance_twips,section.gutter_twips));
    if section.page_number_start.is_some() || section.page_number_format.is_some(){
        out.push_str("<w:pgNumType");
        if let Some(n)=section.page_number_start{out.push_str(&format!(" w:start=\"{n}\""));}
        if let Some(fmt)=&section.page_number_format{out.push_str(&format!(" w:fmt=\"{}\"",x(fmt)));}
        out.push_str("/>");
    }
    out.push_str(&format!("<w:cols w:num=\"{}\" w:space=\"{}\" w:equalWidth=\"{}\"",section.columns,section.column_spacing_twips,if section.columns_equal_width{1}else{0}));
    if section.column_widths_twips.is_empty(){out.push_str("/>");}
    else {out.push('>');for (width,space) in &section.column_widths_twips{out.push_str(&format!("<w:col w:w=\"{width}\" w:space=\"{space}\"/>"));}out.push_str("</w:cols>");}
    if section.title_page{out.push_str("<w:titlePg/>");}
    if let Some(grid)=&section.doc_grid {
        out.push_str(&format!("<w:docGrid w:type=\"{}\"",x(&grid.kind)));
        if let Some(pitch)=grid.line_pitch_twips {out.push_str(&format!(" w:linePitch=\"{pitch}\""));}
        if let Some(space)=grid.char_space {out.push_str(&format!(" w:charSpace=\"{space}\""));}
        out.push_str("/>");
    }
    if let Some(original)=&section.original_xml {
        for child in &original.children {
            if let Node::Element(e)=child {
                if !matches!(e.local(),"headerReference"|"footerReference"|"type"|"pgSz"|"pgMar"|"pgNumType"|"cols"|"titlePg"|"docGrid") {
                    out.push_str(&xml::serialize(e));
                }
            }
        }
    }
    for fragment in &section.extra_properties_xml{out.push_str(fragment);}
    out.push_str("</w:sectPr>");out
}
fn section_break_paragraph(xml:&str,section:&Section)->String {
    let sect=section_full_xml(section);
    if let Some(index)=xml.find("</w:pPr>") {
        let mut out=xml.to_owned();out.insert_str(index,&sect);return out;
    }
    if let Some(index)=xml.find('>'){
        let mut out=xml.to_owned();out.insert_str(index+1,&format!("<w:pPr>{sect}</w:pPr>"));return out;
    }
    xml.to_owned()
}
fn parse_section(sp:&Element,end_after:usize,base_page:&PageSettings)->Section {
    let kind=match sp.child("type").and_then(|v|v.attr("val")){
        Some("continuous")=>SectionBreak::Continuous,Some("evenPage")=>SectionBreak::EvenPage,
        Some("oddPage")=>SectionBreak::OddPage,_=>SectionBreak::NextPage};
    let mut s=Section::new(end_after,kind,base_page.clone());
    if let Some(sz)=sp.child("pgSz"){
        s.page.width_twips=attr_num(sz,"w").unwrap_or(s.page.width_twips);
        s.page.height_twips=attr_num(sz,"h").unwrap_or(s.page.height_twips);
        s.page.orientation=if sz.attr("orient")==Some("landscape"){Orientation::Landscape}else{Orientation::Portrait};
    }
    if let Some(m)=sp.child("pgMar"){
        s.page.margin_top=attr_num(m,"top").unwrap_or(s.page.margin_top);
        s.page.margin_bottom=attr_num(m,"bottom").unwrap_or(s.page.margin_bottom);
        s.page.margin_left=attr_num(m,"left").unwrap_or(s.page.margin_left);
        s.page.margin_right=attr_num(m,"right").unwrap_or(s.page.margin_right);
        s.header_distance_twips=attr_num(m,"header").unwrap_or(720);
        s.footer_distance_twips=attr_num(m,"footer").unwrap_or(720);
        s.gutter_twips=attr_num(m,"gutter").unwrap_or(0);
    }
    if let Some(c)=sp.child("cols") {
        s.columns=attr_num(c,"num").unwrap_or(1).min(u16::MAX as u32) as u16;
        s.column_spacing_twips=attr_num(c,"space").unwrap_or(720);
        s.columns_equal_width=c.attr("equalWidth")!=Some("0");
        s.column_widths_twips=c.elements("col").map(|v|(attr_num(v,"w").unwrap_or(0),attr_num(v,"space").unwrap_or(0))).collect();
    }
    s.page_number_start=sp.child("pgNumType").and_then(|v|attr_num(v,"start"));
    s.page_number_format=sp.child("pgNumType").and_then(|v|v.attr("fmt")).map(str::to_owned);
    s.title_page=sp.child("titlePg").is_some();
    s.doc_grid=sp.child("docGrid").map(|g|DocumentGrid{
        kind:g.attr("type").unwrap_or("lines").to_string(),
        line_pitch_twips:attr_num(g,"linePitch"),
        char_space:g.attr("charSpace").and_then(|v|v.parse::<i32>().ok()),
    });
    for el in sp.elements("headerReference") {if let (Some(style),Some(id))=(el.attr("type"),el.attr("id")){s.header_refs.insert(style.to_owned(),id.to_owned());}}
    for el in sp.elements("footerReference") {if let (Some(style),Some(id))=(el.attr("type"),el.attr("id")){s.footer_refs.insert(style.to_owned(),id.to_owned());}}
    s.original_xml=Some(sp.clone());s
}
fn math_expr_part(expr:&MathExpr)->String{
    match expr{
        MathExpr::Text(t)=>format!("<m:r><m:t>{}</m:t></m:r>",x(t)),
        MathExpr::Row(items)=>items.iter().map(math_expr_part).collect(),
        MathExpr::Fraction(a,b)=>format!("<m:f><m:num>{}</m:num><m:den>{}</m:den></m:f>",math_expr_part(a),math_expr_part(b)),
        MathExpr::Superscript(a,b)=>format!("<m:sSup><m:e>{}</m:e><m:sup>{}</m:sup></m:sSup>",math_expr_part(a),math_expr_part(b)),
        MathExpr::Subscript(a,b)=>format!("<m:sSub><m:e>{}</m:e><m:sub>{}</m:sub></m:sSub>",math_expr_part(a),math_expr_part(b)),
        MathExpr::Radical(a)=>format!("<m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e>{}</m:e></m:rad>",math_expr_part(a)),
        MathExpr::Nary{symbol,lower,upper,body}=>format!("<m:nary><m:naryPr><m:chr m:val=\"{}\"/></m:naryPr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:nary>",x(symbol),lower.as_ref().map(|v|math_expr_part(v)).unwrap_or_default(),upper.as_ref().map(|v|math_expr_part(v)).unwrap_or_default(),math_expr_part(body)),
        MathExpr::Matrix(rows)=>{let mut out=String::from("<m:m>");for row in rows {out.push_str("<m:mr>");for value in row{out.push_str(&format!("<m:e>{}</m:e>",math_expr_part(value)));}out.push_str("</m:mr>");}out.push_str("</m:m>");out},
        MathExpr::Delimiter{begin,end,content}=>format!("<m:d><m:dPr><m:begChr m:val=\"{}\"/><m:endChr m:val=\"{}\"/></m:dPr><m:e>{}</m:e></m:d>",x(begin),x(end),math_expr_part(content)),
        MathExpr::Function{function,argument}=>format!("<m:func><m:fName>{}</m:fName><m:e>{}</m:e></m:func>",math_expr_part(function),math_expr_part(argument)),
        MathExpr::SubSup{base,sub,sup}=>format!("<m:sSubSup><m:e>{}</m:e><m:sub>{}</m:sub><m:sup>{}</m:sup></m:sSubSup>",math_expr_part(base),math_expr_part(sub),math_expr_part(sup)),
        MathExpr::Accent{symbol,body}=>format!("<m:acc><m:accPr><m:chr m:val=\"{}\"/></m:accPr><m:e>{}</m:e></m:acc>",x(symbol),math_expr_part(body)),
        MathExpr::EquationArray(items)=>format!("<m:eqArr>{}</m:eqArr>",items.iter().map(|v|format!("<m:e>{}</m:e>",math_expr_part(v))).collect::<String>()),
        MathExpr::RawXml(raw)=>raw.clone(),
    }
}
fn math_expr_xml(expr:&MathExpr)->String{format!("<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\">{}</m:oMath>",math_expr_part(expr))}
fn sectionxml(page:&PageSettings,header:Option<&str>,footer:Option<&str>,first_h:Option<&str>,first_f:Option<&str>)->String{
    let mut s=String::from("<w:sectPr>");
    for (kind,id) in [("header",header),("footer",footer),("header",first_h),("footer",first_f)] {if let Some(id)=id{s.push_str(&format!("<w:{kind}Reference w:type=\"{}\" r:id=\"{id}\"/>",if (kind=="header"&&first_h==Some(id))||(kind=="footer"&&first_f==Some(id)){"first"}else{"default"}));}}
    if first_h.is_some()||first_f.is_some(){s.push_str("<w:titlePg/>");}
    s.push_str(&format!("<w:pgSz w:w=\"{}\" w:h=\"{}\" w:orient=\"{}\"/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/>",page.width_twips,page.height_twips,if page.orientation==Orientation::Landscape{"landscape"}else{"portrait"},page.margin_top,page.margin_right,page.margin_bottom,page.margin_left));
    s.push_str("</w:sectPr>");s
}
fn headerxml(paragraphs:&[Paragraph],rel:&mut Relations,images:&[Image],bookid:&mut u32,watermark:Option<&str>,header:bool)->String{
    let tag=if header{"hdr"}else{"ftr"};let mut s=format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:{tag} xmlns:w=\"{W}\" xmlns:r=\"{R}\" xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\">");
    if let Some(w)=watermark{s.push_str(&format!("<w:p><w:r><w:pict><v:shape id=\"PowerPlusWaterMarkObject1\" o:spid=\"_x0000_s1025\" type=\"#_x0000_t136\" style=\"position:absolute;margin-left:0;margin-top:0;width:400pt;height:100pt;rotation:315;z-index:-251654144;mso-position-horizontal:center;mso-position-vertical:center\" fillcolor=\"silver\" stroked=\"f\"><v:textpath style=\"font-family:Calibri;font-size:1pt\" string=\"{}\"/></v:shape></w:pict></w:r></w:p>",x(w)));}
    for p in paragraphs{s.push_str(&pxml(p,rel,images,bookid));}if paragraphs.is_empty()&&watermark.is_none(){s.push_str("<w:p/>");}s.push_str(&format!("</w:{tag}>"));s
}
impl Document {
    pub fn to_docx(&self)->io::Result<Vec<u8>>{
        // Existing optional parts remain untouched unless their modeled data changed.

        if self.original_snapshot.as_deref()==Some(self.snapshot().as_str()) {
            return zip::encode(&self.original_parts);
        }
        let mut parts=self.original_parts.clone();
        for path in &self.removed_package_parts{parts.remove(path);}
        for (path,bytes) in &self.package_overrides{parts.insert(path.clone(),bytes.clone());}
        let mut pkg=package::Package{parts};pkg.invalidate_signatures();parts=pkg.parts;
        let mut rels=Relations::from_part(&parts,"word/_rels/document.xml.rels");
        rels.add(&rel_type("styles"),"styles.xml",false);rels.add(&rel_type("settings"),"settings.xml",false);
        rels.add(&rel_type("numbering"),"numbering.xml",false);
        if !self.footnotes.is_empty(){rels.add(&rel_type("footnotes"),"footnotes.xml",false);}
        if !self.endnotes.is_empty(){rels.add(&rel_type("endnotes"),"endnotes.xml",false);}
        if !self.comments.is_empty(){
            rels.add(&rel_type("comments"),"comments.xml",false);
            if self.comments.iter().any(|c|c.parent.is_some()) || !self.comment_metadata.is_empty() {
                rels.add("http://schemas.microsoft.com/office/2011/relationships/commentsExtended","commentsExtended.xml",false);
                rels.add("http://schemas.microsoft.com/office/2011/relationships/people","people.xml",false);
            }
            if self.comment_metadata.values().any(|m|m.durable_id.is_some()) {
                rels.add("http://schemas.microsoft.com/office/2016/09/relationships/commentsIds","commentsIds.xml",false);
            }
        }
        if !self.glossary.is_empty(){rels.add(&rel_type("glossaryDocument"),"glossary/document.xml",false);}
        if !self.embedded_fonts.is_empty(){rels.add(&rel_type("fontTable"),"fontTable.xml",false);}
        let header=if !self.header.is_empty()||self.watermark.is_some(){Some(rels.add(&rel_type("header"),"header1.xml",false))}else{None};
        let footer=if !self.footer.is_empty(){Some(rels.add(&rel_type("footer"),"footer1.xml",false))}else{None};
        let fh=if !self.first_header.is_empty(){Some(rels.add(&rel_type("header"),"header2.xml",false))}else{None};
        let ff=if !self.first_footer.is_empty(){Some(rels.add(&rel_type("footer"),"footer2.xml",false))}else{None};
        let mut bookid=0u32;
        let mut document=format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document xmlns:w=\"{W}\" xmlns:r=\"{R}\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\"><w:body>");
        let original_body=self.original_body.as_ref().and_then(|r|r.child("body"));
        let old_count=original_body.map(|b|b.children.iter().filter(|n|matches!(n,Node::Element(e) if e.local()=="p"||e.local()=="tbl")).count()).unwrap_or(0);
        let can_preserve=old_count==self.blocks.len()
            && self.original_components.get("images")==Some(&format!("{:?}",self.images))
            && self.original_components.get("sections")==Some(&format!("{:?}",self.sections));
        let mut body_index=0usize;
        let mut write_block=|index:usize,block:&Block,document:&mut String|{
            let current_debug=format!("{block:?}");
            if can_preserve && self.original_block_debug.get(index)==Some(&current_debug) {
                if let Some(original)=original_body.and_then(|b|b.children.iter().filter_map(|n|if let Node::Element(e)=n{if e.local()=="p"||e.local()=="tbl"{Some(e)}else{None}}else{None}).nth(index)) {
                    document.push_str(&xml::serialize(original));return;
                }
            }
            let mut rendered=match block{Block::Paragraph(p)=>pxml(p,&mut rels,&self.images,&mut bookid),Block::Table(t)=>tablexml(t,&mut rels,&self.images,&mut bookid)};
            if let Some(original)=original_body.and_then(|b|b.children.iter().filter_map(|n|if let Node::Element(e)=n{if e.local()=="p"||e.local()=="tbl"{Some(e)}else{None}}else{None}).nth(index)) {
                if matches!(block,Block::Paragraph(_)) && original.local()=="p" {rendered=merge_paragraph_extensions(original,&rendered);}
            }
            if let Some(section)=self.sections.iter().find(|v|v.end_after==index+1) {document.push_str(&section_break_paragraph(&rendered,section));}
            else{document.push_str(&rendered);}
        };
        if let Some(original)=original_body {
            for n in &original.children{if let Node::Element(e)=n{match e.local(){
                "p"|"tbl"=>{if let Some(block)=self.blocks.get(body_index){write_block(body_index,block,&mut document);body_index+=1;}},
                "sectPr"=>{},_=>document.push_str(&xml::serialize(e)),
            }}}
        }
        for (i,block) in self.blocks.iter().enumerate().skip(body_index){write_block(i,block,&mut document);}
        let original_section=original_body.and_then(|b|b.child("sectPr"));
        let section_unchanged=self.original_components.get("section")==Some(&format!("{:?}{:?}{:?}{:?}{:?}{:?}",self.page,self.header,self.footer,self.first_header,self.first_footer,self.final_section));
        if section_unchanged {if let Some(section)=original_section{document.push_str(&xml::serialize(section));}else{document.push_str(&sectionxml(&self.page,header.as_deref(),footer.as_deref(),fh.as_deref(),ff.as_deref()));}}
        else if let Some(section)=&self.final_section {
            let mut section=section.clone();
            if self.original_components.get("page")!=Some(&format!("{:?}",self.page)){section.page=self.page.clone();}
            if let Some(id)=&header{section.header_refs.insert("default".into(),id.clone());}
            if let Some(id)=&footer{section.footer_refs.insert("default".into(),id.clone());}
            if let Some(id)=&fh{section.header_refs.insert("first".into(),id.clone());section.title_page=true;}
            if let Some(id)=&ff{section.footer_refs.insert("first".into(),id.clone());section.title_page=true;}
            document.push_str(&section_full_xml(&section));
        } else {document.push_str(&sectionxml(&self.page,header.as_deref(),footer.as_deref(),fh.as_deref(),ff.as_deref()));}
        document.push_str("</w:body></w:document>");
        if let Some(root)=&self.original_body {
            let mut attributes=String::new();
            for (key,val) in &root.attrs {
                if !matches!(key.as_str(),"xmlns:w"|"xmlns:r"|"xmlns:wp"){
                    attributes.push_str(&format!(" {}=\"{}\"",key,x(val)));
                }
            }
            if !attributes.is_empty(){document=document.replacen("<w:document ",&format!("<w:document{attributes} "),1);}
        }
        parts.insert("word/document.xml".to_owned(),document.into_bytes());
        if self.original_components.get("styles")!=Some(&format!("{:?}",self.styles)) || !parts.contains_key("word/styles.xml"){
            parts.insert("word/styles.xml".to_owned(),stylesxml(&self.styles).into_bytes());
        }
        if self.original_components.get("blocks")!=Some(&format!("{:?}",self.blocks)) || !parts.contains_key("word/numbering.xml"){
            parts.insert("word/numbering.xml".to_owned(),numberingxml(self).into_bytes());
        }
        for (i,chart) in self.charts.iter().enumerate(){parts.insert(format!("word/charts/chart{}.xml",i+1),chartxml(chart).into_bytes());}
        let mut settings=format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:settings xmlns:w=\"{W}\">");
        if let Some(mode)=self.protection{settings.push_str(&format!("<w:documentProtection w:edit=\"{}\" w:enforcement=\"1\"/>",match mode{Protection::ReadOnly=>"readOnly",Protection::FormsOnly=>"forms",Protection::CommentsOnly=>"comments"}));}
        if self.track_changes.is_some(){settings.push_str("<w:trackRevisions/>");}
        if self.final_section.as_ref().map(|s|s.mirror_margins).unwrap_or(false)
            || self.sections.iter().any(|s|s.mirror_margins){settings.push_str("<w:mirrorMargins/>");}
        settings.push_str("</w:settings>");
        if self.original_components.get("settings")!=Some(&format!("{:?}{:?}",self.protection,self.track_changes)) || !parts.contains_key("word/settings.xml"){
            parts.insert("word/settings.xml".to_owned(),settings.into_bytes());
        }
        if !self.footnotes.is_empty(){parts.insert("word/footnotes.xml".into(),notesxml(&self.footnotes,"footnote").into_bytes());}
        if !self.endnotes.is_empty(){parts.insert("word/endnotes.xml".into(),notesxml(&self.endnotes,"endnote").into_bytes());}
        if !self.comments.is_empty(){parts.insert("word/comments.xml".into(),commentsxml(&self.comments,&self.comment_metadata).into_bytes());
            if self.comments.iter().any(|c|c.parent.is_some())||!self.comment_metadata.is_empty(){
                parts.insert("word/commentsExtended.xml".into(),comment_extensions_xml(&self.comments,&self.comment_metadata).into_bytes());
                parts.insert("word/people.xml".into(),comment_people_xml(&self.comments,&self.comment_metadata).into_bytes());
                if self.comment_metadata.values().any(|m|m.durable_id.is_some()){
                    parts.insert("word/commentsIds.xml".into(),comment_ids_xml(&self.comments,&self.comment_metadata).into_bytes());
                }
            }
        }
        if !self.glossary.is_empty(){parts.insert("word/glossary/document.xml".into(),glossary_xml(self).into_bytes());}
        if !self.embedded_fonts.is_empty(){
            parts.insert("word/fontTable.xml".into(),font_table_xml(&self.embedded_fonts).into_bytes());
            let mut frels=Relations::default();
            for (i,font) in self.embedded_fonts.iter().enumerate(){
                let name=format!("fonts/font{}.odttf",i+1);
                frels.add(&rel_type("font"),&name,false);
                parts.insert(format!("word/{name}"),font.obfuscated_data.clone());
            }
            parts.insert("word/_rels/fontTable.xml.rels".into(),frels.to_xml().into_bytes());
        }
        for (index,img) in self.images.iter().enumerate(){let ext=img.filename.rsplit('.').next().unwrap_or("png");let name=format!("word/media/image{}.{}",index+1,ext);parts.insert(name,img.bytes.clone());}
        for (name,p,wm,header) in [("word/header1.xml",&self.header,self.watermark.as_deref(),true),("word/footer1.xml",&self.footer,None,false),("word/header2.xml",&self.first_header,None,true),("word/footer2.xml",&self.first_footer,None,false)]{
            if (name.ends_with("header1.xml")&&self.watermark.is_some())||!p.is_empty(){
                let source_key=match name{"word/header1.xml"=>"header","word/footer1.xml"=>"footer","word/header2.xml"=>"first_header",_=>"first_footer"};
                let old_value=self.original_components.get(source_key);
                if old_value==Some(&format!("{:?}",p)) && parts.contains_key(name) {continue;}
                let mut hrels=Relations::from_part(&parts,&format!("word/_rels/{}.rels",name.rsplit('/').next().unwrap()));let bytes=headerxml(p,&mut hrels,&self.images,&mut bookid,wm,header).into_bytes();parts.insert(name.to_string(),bytes);if !hrels.items.is_empty(){let fname=name.rsplit('/').next().unwrap();parts.insert(format!("word/_rels/{fname}.rels"),hrels.to_xml().into_bytes());}}
        }
        for (i,c) in self.custom_xml.iter().enumerate(){
            let name=if c.name.is_empty(){format!("item{}.xml",i+1)}else{c.name.clone()};
            if !name.contains("..") && !name.starts_with('/') && !name.contains('\\') {
                let path=format!("customXml/{name}");
                rels.add(&rel_type("customXml"),&format!("../{path}"),false);
                parts.insert(path,c.xml.as_bytes().to_vec());
                if let Some(props)=self.custom_xml_item_props.get(&name){
                    let id=i+1;
                    let filename=format!("itemProps{id}.xml");
                    parts.insert(format!("customXml/{filename}"),props.as_bytes().to_vec());
                    let mut rels=Relations::default();rels.add(&rel_type("customXmlProps"),&filename,false);
                    parts.insert(format!("customXml/_rels/{name}.rels"),rels.to_xml().into_bytes());
                }
            }
        }
        parts.insert("word/_rels/document.xml.rels".into(),rels.to_xml().into_bytes());
        let mut rootrels=Relations::from_part(&parts,"_rels/.rels");
        rootrels.add(&rel_type("officeDocument"),"word/document.xml",false);
        rootrels.add("http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties","docProps/core.xml",false);
        parts.insert("_rels/.rels".into(),rootrels.to_xml().into_bytes());
        if self.original_components.get("core")!=Some(&format!("{:?}",self.properties)) || !parts.contains_key("docProps/core.xml"){
            parts.insert("docProps/core.xml".into(),corexml(&self.properties).into_bytes());
        }
        let fresh=content_types(&parts);let ct=merged_content_types(&parts,&fresh);parts.insert("[Content_Types].xml".into(),ct.into_bytes());
        zip::encode(&parts)
    }
}

fn part_xml(parts:&BTreeMap<String,Vec<u8>>,path:&str)->io::Result<Option<Element>>{
    match parts.get(path){None=>Ok(None),Some(v)=>{let s=xml::decode_bytes(v)?;Ok(Some(xml::parse(&s)?))}}
}
fn relationships(parts:&BTreeMap<String,Vec<u8>>,path:&str)->io::Result<HashMap<String,(String,String)>>{
    let mut map=HashMap::new();
    let source=if path=="_rels/.rels"{String::new()}else if let Some((dir,name))=path.rsplit_once("/_rels/") {
        format!("{dir}/{}",name.strip_suffix(".rels").ok_or_else(||err("invalid rel path"))?)
    } else {return Err(err("invalid relationship path"));};
    if let Some(root)=part_xml(parts,path)? {for rel in root.elements("Relationship") {
        if let (Some(id),Some(target),Some(kind))=(rel.attr("Id"),rel.attr("Target"),rel.attr("Type")) {
            let target=if rel.attr("TargetMode")==Some("External"){target.to_owned()}else{package::resolve_target(&source,target)?};
            if map.insert(id.to_string(),(target,kind.to_string())).is_some(){return Err(err("duplicate relationship ID"));}
        }
    }}Ok(map)
}
fn attr_num(e:&Element,key:&str)->Option<u32>{e.attr(key).and_then(|s|s.parse().ok())}
fn prop_bool(e:&Element,kind:&str)->bool{e.child(kind).map(|c|!matches!(c.attr("val"),Some("0"|"false"|"off"))).unwrap_or(false)}
fn parse_run(e:&Element)->Run{
    let mut r=Run::default();
    if let Some(pr)=e.child("rPr") {
        r.bold=prop_bool(pr,"b");r.italic=prop_bool(pr,"i");r.underline=prop_bool(pr,"u");r.strike=prop_bool(pr,"strike");
        if let Some(v)=pr.child("vertAlign").and_then(|v|v.attr("val")){r.superscript=v=="superscript";r.subscript=v=="subscript";}
        r.color=pr.child("color").and_then(|v|v.attr("themeColor").map(|c|format!("theme:{c}")).or_else(||v.attr("val").map(str::to_owned)));
        r.highlight=pr.child("highlight").and_then(|v|v.attr("val")).map(str::to_string);
        r.font=pr.child("rFonts").and_then(|v|v.attr("ascii").or_else(||v.attr("hAnsi"))).map(str::to_string);
        r.style=pr.child("rStyle").and_then(|v|v.attr("val")).map(str::to_string);
        r.font_size=pr.child("sz").and_then(|v|v.attr("val")).and_then(|v|v.parse::<f32>().ok()).map(|v|v/2.0);
    }
    for c in &e.children {if let Node::Element(el)=c {match el.local() {
        "t"|"delText"=>r.text.push_str(&el.text()),"tab"=>r.text.push('\t'),"cr"=>r.text.push('\n'),
        "br"=>{r.break_kind=Some(match el.attr("type"){Some("page")=>BreakKind::Page,Some("column")=>BreakKind::Column,_=>BreakKind::Line});},
        _=>{}
    }}}
    r.original_xml=Some(e.clone());
    r.original_fingerprint=Some(r.fingerprint());
    r
}
fn parse_math_expr(e:&Element)->MathExpr {
    match e.local() {
        "r"=>MathExpr::Text(e.child("t").map(Element::text).unwrap_or_default()),
        "f"=>MathExpr::Fraction(Box::new(e.child("num").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
                                Box::new(e.child("den").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new())))),
        "sSup"=>MathExpr::Superscript(Box::new(e.child("e").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
                                    Box::new(e.child("sup").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new())))),
        "sSub"=>MathExpr::Subscript(Box::new(e.child("e").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
                                  Box::new(e.child("sub").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new())))),
        "rad"=>MathExpr::Radical(Box::new(e.child("e").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new())))),
        "nary"=>MathExpr::Nary{
            symbol:e.child("naryPr").and_then(|p|p.child("chr")).and_then(|p|p.attr("val")).unwrap_or("∑").to_owned(),
            lower:e.child("sub").map(|v|Box::new(parse_math_expr(v))),
            upper:e.child("sup").map(|v|Box::new(parse_math_expr(v))),
            body:Box::new(e.child("e").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
        },
        "oMath"|"oMathPara"|"num"|"den"|"sub"|"sup"|"e"|"fName"=>{
            let items=e.children.iter().filter_map(|n|if let Node::Element(child)=n {Some(parse_math_expr(child))} else {None}).collect::<Vec<_>>();
            if items.len()==1 {items.into_iter().next().unwrap()} else {MathExpr::Row(items)}
        },
        "sSubSup"=>MathExpr::SubSup{
            base:Box::new(e.child("e").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
            sub:Box::new(e.child("sub").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
            sup:Box::new(e.child("sup").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
        },
        "m"=>MathExpr::Matrix(e.elements("mr").map(|row|row.elements("e").map(parse_math_expr).collect()).collect()),
        "d"=>MathExpr::Delimiter{
            begin:e.child("dPr").and_then(|p|p.child("begChr")).and_then(|v|v.attr("val")).unwrap_or("(").to_owned(),
            end:e.child("dPr").and_then(|p|p.child("endChr")).and_then(|v|v.attr("val")).unwrap_or(")").to_owned(),
            content:Box::new(e.child("e").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
        },
        "func"=>MathExpr::Function{
            function:Box::new(e.child("fName").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
            argument:Box::new(e.child("e").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
        },
        "acc"=>MathExpr::Accent{
            symbol:e.child("accPr").and_then(|p|p.child("chr")).and_then(|v|v.attr("val")).unwrap_or("̂").to_owned(),
            body:Box::new(e.child("e").map(parse_math_expr).unwrap_or(MathExpr::Text(String::new()))),
        },
        "eqArr"=>MathExpr::EquationArray(e.elements("e").map(parse_math_expr).collect()),
        _=>MathExpr::RawXml(xml::serialize(e)),
    }
}
fn parse_inline(e:&Element,p:&mut Paragraph,rels:&HashMap<String,(String,String)>,images:&mut Vec<Image>,parts:&BTreeMap<String,Vec<u8>>){
    match e.local() {
        "r"=>{
            if let Some(n)=e.child("footnoteReference").and_then(|v|attr_num(v,"id")){p.add_footnote_ref(n);return;}
            if let Some(n)=e.child("endnoteReference").and_then(|v|attr_num(v,"id")){p.add_endnote_ref(n);return;}
            if let Some(n)=e.child("commentReference").and_then(|v|attr_num(v,"id")){p.comment_range_end(n);return;}
            if let Some(draw)=e.child("drawing"){
                let mut blips=Vec::new();draw.descendants("blip",&mut blips);
                for blip in blips{if let Some(rid)=blip.attr("embed") {if let Some((target,_))=rels.get(rid) {
                    let path=target.clone();
                    if let Some(bytes)=parts.get(&path){let fname=path.rsplit('/').next().unwrap_or("image.png").to_string();let ext=path.rsplit('.').next().unwrap_or("png").to_ascii_lowercase();let mime=match ext.as_str(){"png"=>"image/png","jpg"|"jpeg"=>"image/jpeg","gif"=>"image/gif","bmp"=>"image/bmp",_=>"application/octet-stream"}.to_string();
                    let mut extent=Vec::new();draw.descendants("extent",&mut extent);
                    let width=extent.first().and_then(|e|e.attr("cx")).and_then(|v|v.parse::<i64>().ok()).unwrap_or(914400);
                    let height=extent.first().and_then(|e|e.attr("cy")).and_then(|v|v.parse::<i64>().ok()).unwrap_or(914400);
                    let anchor=draw.child("anchor");let floating=anchor.is_some();
                    let positioning=anchor.map(|a|{
                        let mut f=FloatingOptions::default();
                        f.horizontal=match a.child("positionH").and_then(|v|v.attr("relativeFrom")){
                            Some("page")=>RelativePosition::Page,Some("margin")=>RelativePosition::Margin,
                            Some("character")=>RelativePosition::Character,_=>RelativePosition::Column};
                        f.vertical=match a.child("positionV").and_then(|v|v.attr("relativeFrom")){
                            Some("page")=>RelativePosition::Page,Some("margin")=>RelativePosition::Margin,
                            Some("line")=>RelativePosition::Line,_=>RelativePosition::Paragraph};
                        f.x_emu=a.child("positionH").and_then(|v|v.child("posOffset")).and_then(|v|v.text().parse().ok()).unwrap_or(0);
                        f.y_emu=a.child("positionV").and_then(|v|v.child("posOffset")).and_then(|v|v.text().parse().ok()).unwrap_or(0);
                        f.distance_top=a.attr("distT").and_then(|v|v.parse().ok()).unwrap_or(0);
                        f.distance_bottom=a.attr("distB").and_then(|v|v.parse().ok()).unwrap_or(0);
                        f.distance_left=a.attr("distL").and_then(|v|v.parse().ok()).unwrap_or(114300);
                        f.distance_right=a.attr("distR").and_then(|v|v.parse().ok()).unwrap_or(114300);
                        f.z_order=a.attr("relativeHeight").and_then(|v|v.parse().ok()).unwrap_or(251658240);
                        f.behind_text=a.attr("behindDoc")==Some("1");f.allow_overlap=a.attr("allowOverlap")!=Some("0");
                        f.layout_in_cell=a.attr("layoutInCell")!=Some("0");
                        f.wrap=if a.child("wrapNone").is_some(){ImageWrap::None}
                            else if a.child("wrapTopAndBottom").is_some(){ImageWrap::TopAndBottom}
                            else if a.child("wrapTight").is_some(){ImageWrap::Tight}
                            else if a.child("wrapThrough").is_some(){ImageWrap::Through}else{ImageWrap::Square};
                        if let Some(wrap)=a.child("wrapTight").or_else(||a.child("wrapThrough")){
                            if let Some(poly)=wrap.child("wrapPolygon"){
                                let mut points=Vec::new();
                                for child in &poly.children {
                                    if let crate::xml::Node::Element(e)=child {
                                        if matches!(e.name.rsplit(':').next(),Some("start"|"lineTo")) {
                                            if let (Some(x),Some(y))=(e.attr("x").and_then(|v|v.parse::<i64>().ok()),e.attr("y").and_then(|v|v.parse::<i64>().ok())){
                                                if points.len()<4096{points.push((x,y));}
                                            }
                                        }
                                    }
                                }
                                f.wrap_polygon=points;
                            }
                        }
                        let mut shapes=Vec::new();a.descendants("xfrm",&mut shapes);
                        f.rotation_degrees=shapes.first().and_then(|v|v.attr("rot")).and_then(|v|v.parse::<f64>().ok()).map(|v|v/60000.0).unwrap_or(0.0);
                        f
                    });
                    let ix=images.len();images.push(Image{filename:fname,bytes:bytes.clone(),content_type:mime,width:Length(width),height:Length(height),floating,positioning});p.add_image(ix);
                    } }
                }}return;}
            p.inlines.push(Inline::Run(parse_run(e)));
        }
        "hyperlink"=>{
            let link=e.attr("r:id").and_then(|id|rels.get(id).map(|v|v.0.clone()));
            let anchor=e.attr("anchor").map(str::to_owned);
            for c in e.elements("r") {let mut r=parse_run(c);r.hyperlink=link.clone();r.internal_link=anchor.clone();r.original_fingerprint=Some(r.fingerprint());p.inlines.push(Inline::Run(r));}
        }
        "bookmarkStart"=>if let Some(name)=e.attr("name"){p.add_bookmark(name);},
        "commentRangeStart"=>if let Some(id)=attr_num(e,"id"){p.comment_range_start(id);},
        "commentRangeEnd"=>if let Some(id)=attr_num(e,"id"){p.comment_range_end(id);},
        "fldSimple"=>if let Some(instr)=e.attr("instr"){p.add_field(instr);},
        "sdt"=>{
            let props=e.child("sdtPr");let tag=props.and_then(|pr|pr.child("tag")).and_then(|t|t.attr("val")).unwrap_or("field").to_string();
            let value=e.child("sdtContent").map(Element::text).unwrap_or_default();
            if props.and_then(|pr|pr.child("checkbox")).is_some(){p.add_checkbox(tag,value.contains('☒'));}
            else if let Some(drop)=props.and_then(|pr|pr.child("dropDownList")){let options=drop.elements("listItem").filter_map(|e|e.attr("displayText").or_else(||e.attr("value"))).map(str::to_string).collect::<Vec<_>>();let ix=options.iter().position(|s|s==&value).unwrap_or(0);p.add_dropdown(tag,options,ix);}
            else {
                let binding=props.and_then(|pr|pr.child("dataBinding")).map(|b|ContentBinding{
                    store_item_id:b.attr("storeItemID").unwrap_or("").to_owned(),
                    xpath:b.attr("xpath").unwrap_or("").to_owned(),prefix_mappings:b.attr("prefixMappings").unwrap_or("").to_owned()});
                let control=StructuredContentControl{tag,alias:props.and_then(|pr|pr.child("alias")).and_then(|v|v.attr("val")).unwrap_or("").to_owned(),
                    id:props.and_then(|pr|pr.child("id")).and_then(|v|attr_num(v,"val")),value,
                    locked:props.and_then(|pr|pr.child("lock")).is_some(),
                    repeating:props.and_then(|pr|pr.child("repeatingSection")).is_some(),
                    rich_text:props.and_then(|pr|pr.child("text")).is_none(),binding,original:Some(e.clone())};
                p.add_structured_control(control);
            }
        },
        "ins"|"del"|"moveFrom"|"moveTo"=>{
            let kind=match e.local(){"ins"=>RevisionKind::Insert,"del"=>RevisionKind::Delete,
                "moveFrom"=>RevisionKind::MoveFrom,_=>RevisionKind::MoveTo};
            let info=RevisionInfo{id:attr_num(e,"id").unwrap_or(0),author:e.attr("author").unwrap_or("").to_owned(),date:e.attr("date").map(str::to_owned)};
            let mut nested=Paragraph::default();
            for item in &e.children{if let Node::Element(child)=item{
                parse_inline(child,&mut nested,rels,images,parts);
            }}
            p.add_revision(kind,info,nested.inlines);
        }
        "oMath"|"oMathPara"=>{p.add_math_expression(parse_math_expr(e));},
        "pPr"|"bookmarkEnd"|"proofErr"=>{},
        _=>{p.add_raw_xml(xml::serialize(e));}
    }
}
fn parse_para(e:&Element,rels:&HashMap<String,(String,String)>,images:&mut Vec<Image>,parts:&BTreeMap<String,Vec<u8>>)->Paragraph{
    let mut p=Paragraph::default();
    if let Some(pr)=e.child("pPr"){
        p.style=pr.child("pStyle").and_then(|v|v.attr("val")).map(str::to_string);
        p.alignment=match pr.child("jc").and_then(|v|v.attr("val")){Some("center")=>Alignment::Center,Some("right")=>Alignment::Right,Some("both"|"distribute")=>Alignment::Justify,_=>Alignment::Left};
        p.keep_with_next=prop_bool(pr,"keepNext");p.keep_together=prop_bool(pr,"keepLines");p.page_break_before=prop_bool(pr,"pageBreakBefore");
        p.widow_control=pr.child("widowControl").map(|v|!matches!(v.attr("val"),Some("0"|"false"|"off")));
        p.snap_to_grid=pr.child("snapToGrid").map(|v|!matches!(v.attr("val"),Some("0"|"false"|"off")));
        p.contextual_spacing=prop_bool(pr,"contextualSpacing");
        if let Some(tabs)=pr.child("tabs"){
            for tab in tabs.elements("tab").take(256){
                let Some(position_twips)=tab.attr("pos").and_then(|v|v.parse::<i32>().ok())else{continue;};
                let Some(alignment)= (match tab.attr("val"){
                    Some("left"|"start")=>Some(TabAlignment::Left),Some("right"|"end")=>Some(TabAlignment::Right),
                    Some("center")=>Some(TabAlignment::Center),Some("decimal"|"num")=>Some(TabAlignment::Decimal),_=>None
                })else{continue;};
                let leader=match tab.attr("leader"){
                    Some("dot")=>TabLeader::Dots,Some("hyphen")=>TabLeader::Dashes,
                    Some("underscore")=>TabLeader::Line,Some("middleDot")=>TabLeader::MiddleDots,_=>TabLeader::None
                };
                p.tab_stops.push(TabStop{position_twips,alignment,leader});
            }
            p.tab_stops.sort_by_key(|t|t.position_twips);
        }
        if let Some(n)=pr.child("numPr"){p.list=Some(ListInfo{num_id:n.child("numId").and_then(|v|attr_num(v,"val")).unwrap_or(1),level:n.child("ilvl").and_then(|v|attr_num(v,"val")).unwrap_or(0) as u8,bullet:n.child("numId").and_then(|v|attr_num(v,"val"))==Some(2)});}
        if let Some(s)=pr.child("spacing"){
            p.spacing_before=attr_num(s,"before");p.spacing_after=attr_num(s,"after");p.line_spacing=attr_num(s,"line");
            p.line_rule=match s.attr("lineRule"){
                Some("auto")=>Some(LineSpacingRule::Auto),Some("exact")=>Some(LineSpacingRule::Exact),
                Some("atLeast")=>Some(LineSpacingRule::AtLeast),_=>None,
            };
        }
        if let Some(ind)=pr.child("ind"){p.indent_left=ind.attr("left").and_then(|v|v.parse().ok());p.indent_right=ind.attr("right").and_then(|v|v.parse().ok());p.first_line=ind.attr("firstLine").and_then(|v|v.parse().ok());}
        p.shading=pr.child("shd").and_then(|v|v.attr("fill")).map(str::to_string);
        p.borders=pr.child("pBdr").and_then(parse_border);
    }
    let children=e.children.iter().filter_map(|n|if let Node::Element(c)=n{Some(c)}else{None}).collect::<Vec<_>>();
    let mut i=0;
    while i<children.len() {
        let el=children[i];
        if el.local()=="r" && el.child("fldChar").and_then(|v|v.attr("fldCharType"))==Some("begin") {
            let begin=el.child("fldChar").unwrap();
            let mut instr=String::new();let mut result=String::new();let mut separated=false;
            let mut j=i+1;let mut nested=0usize;let mut complete=false;
            while j<children.len() {
                let c=children[j];
                if let Some(chr)=c.child("fldChar") {
                    match chr.attr("fldCharType") {
                        Some("begin")=>nested+=1,
                        Some("separate") if nested==0=>separated=true,
                        Some("end") if nested==0=>{complete=true;break;},
                        Some("end")=>nested-=1,_=>{}
                    }
                } else if nested==0 {
                    if !separated{for field in c.elements("instrText"){instr.push_str(&field.text());}}
                    else {for value in c.elements("t"){result.push_str(&value.text());}}
                }
                j+=1;
            }
            if complete {
                p.inlines.push(Inline::ComplexField{instruction:instr,result,
                    locked:begin.attr("fldLock")==Some("1"),dirty:begin.attr("dirty")==Some("1")});
                i=j+1;continue;
            }
        }
        parse_inline(el,&mut p,rels,images,parts);i+=1;
    }
    p
}
fn parse_table(e:&Element,rels:&HashMap<String,(String,String)>,images:&mut Vec<Image>,parts:&BTreeMap<String,Vec<u8>>)->Table{
    let mut t=Table::default();
    if let Some(pr)=e.child("tblPr"){
        t.style=pr.child("tblStyle").and_then(|v|v.attr("val")).map(str::to_owned);
        if let Some(width)=pr.child("tblW"){
            if width.attr("type")==Some("pct"){
                t.width_percent=attr_num(width,"w").map(|n:u32|n as f32/50.);
            }else if width.attr("type")!=Some("auto"){
                t.width_twips=attr_num(width,"w");
            }
        }
        t.layout_fixed=pr.child("tblLayout").and_then(|v|v.attr("type"))==Some("fixed");
        t.alignment=match pr.child("jc").and_then(|v|v.attr("val")){Some("center")=>Alignment::Center,Some("right")=>Alignment::Right,Some("both")=>Alignment::Justify,_=>Alignment::Left};
        t.indent_twips=pr.child("tblInd").and_then(|v|v.attr("w")).and_then(|v|v.parse().ok());
        t.cell_margin_twips=pr.child("tblCellMar").and_then(|v|v.child("top")).and_then(|v|attr_num(v,"w"));
        t.borders=pr.child("tblBorders").and_then(parse_border);
        t.edge_borders=pr.child("tblBorders").and_then(parse_border_edges);
    }
    if let Some(grid)=e.child("tblGrid"){for col in grid.elements("gridCol"){t.column_widths.push(attr_num(col,"w").unwrap_or(2160));}}
    for r in e.elements("tr"){
        let mut row=Row::default();if let Some(pr)=r.child("trPr"){row.is_header=prop_bool(pr,"tblHeader");row.cant_split=prop_bool(pr,"cantSplit");row.grid_before=pr.child("gridBefore").and_then(|v|attr_num(v,"val")).unwrap_or(0);row.grid_after=pr.child("gridAfter").and_then(|v|attr_num(v,"val")).unwrap_or(0);if let Some(height)=pr.child("trHeight"){
            row.height_twips=attr_num(height,"val");
            row.height_rule=match height.attr("hRule"){
                Some("exact")=>RowHeightRule::Exact,
                Some("atLeast")=>RowHeightRule::AtLeast,
                _=>RowHeightRule::Auto,
            };
        }}
        for c in r.elements("tc") {
            let mut cell=Cell::default();
            if let Some(pr)=c.child("tcPr"){
                cell.width_twips=pr.child("tcW").and_then(|v|attr_num(v,"w"));
                cell.colspan=pr.child("gridSpan").and_then(|v|attr_num(v,"val")).unwrap_or(1);
                cell.vertical_merge=pr.child("vMerge").map(|v|v.attr("val")==Some("restart"));
                cell.shading=pr.child("shd").and_then(|v|v.attr("fill")).map(str::to_owned);
                cell.borders=pr.child("tcBorders").and_then(parse_border);
                cell.edge_borders=pr.child("tcBorders").and_then(parse_border_edges);
                cell.text_direction=pr.child("textDirection").and_then(|v|v.attr("val")).map(str::to_owned);
                cell.valign=match pr.child("vAlign").and_then(|v|v.attr("val")){Some("center")=>VerticalAlignment::Center,Some("bottom")=>VerticalAlignment::Bottom,_=>VerticalAlignment::Top};
            }else {cell.colspan=1;}
            for p in c.elements("p"){cell.paragraphs.push(parse_para(p,rels,images,parts));}
            if cell.paragraphs.is_empty(){cell.paragraphs.push(Paragraph::default());}
            row.cells.push(cell);
        }
        t.rows.push(row);
    }
    if t.column_widths.is_empty(){let count=t.rows.first().map(|r|r.cells.len()).unwrap_or(0);t.column_widths=vec![2160;count];}
    t
}
fn parse_properties(parts:&BTreeMap<String,Vec<u8>>)->io::Result<CoreProperties>{
    let mut c=CoreProperties::default();if let Some(root)=part_xml(parts,"docProps/core.xml")?{
        for node in &root.children {if let Node::Element(e)=node {let t=e.text();match e.local(){"title"=>c.title=t,"subject"=>c.subject=t,"creator"=>c.creator=t,"keywords"=>c.keywords=t,"description"=>c.description=t,"lastModifiedBy"=>c.last_modified_by=t,"category"=>c.category=t,"contentStatus"=>c.content_status=t,"language"=>c.language=t,"identifier"=>c.identifier=t,"revision"=>c.revision=t.parse().ok(),"version"=>c.version=t,"created"=>c.created=t,"modified"=>c.modified=t,_=>{}}}}
    }Ok(c)
}
fn parse_notes(parts:&BTreeMap<String,Vec<u8>>,path:&str,tag:&str)->io::Result<Vec<Note>>{
    let mut out=Vec::new();if let Some(root)=part_xml(parts,path)?{for n in root.elements(tag){if let Some(id)=attr_num(n,"id"){if id<1000000{let mut texts=Vec::new();n.descendants("t",&mut texts);let text=texts.iter().map(|e|e.text()).collect::<Vec<_>>().join("");out.push(Note{id,text});}}}}Ok(out)
}
fn parse_comments(parts:&BTreeMap<String,Vec<u8>>)->io::Result<Vec<Comment>>{
    let mut out=Vec::new();let mut by_para=HashMap::new();
    if let Some(root)=part_xml(parts,"word/comments.xml")?{for e in root.elements("comment") {
        let id=attr_num(e,"id").unwrap_or(0);let author=e.attr("author").unwrap_or("").to_owned();
        if let Some(para)=e.child("p").and_then(|p|p.attr("paraId")){by_para.insert(para.to_owned(),id);}
        by_para.entry(format!("{:08X}",id)).or_insert(id);
        let mut texts=Vec::new();e.descendants("t",&mut texts);
        out.push(Comment{id,author,text:texts.iter().map(|e|e.text()).collect::<Vec<_>>().join(""),parent:None});
    }}
    if let Some(ext)=part_xml(parts,"word/commentsExtended.xml")?{
        for e in ext.elements("commentEx"){
            let child=e.attr("paraId").and_then(|s|by_para.get(s).copied());
            let parent=e.attr("paraIdParent").and_then(|s|by_para.get(s).copied());
            if let (Some(id),Some(parent))=(child,parent){if let Some(c)=out.iter_mut().find(|c|c.id==id){c.parent=Some(parent);}}
        }
    }Ok(out)
}
fn parse_comment_metadata(parts:&BTreeMap<String,Vec<u8>>)->io::Result<BTreeMap<u32,CommentMetadata>>{
    let mut out=BTreeMap::new();let mut by_para=HashMap::new();
    if let Some(root)=part_xml(parts,"word/comments.xml")?{for c in root.elements("comment") {
        if let Some(id)=attr_num(c,"id") {
            let para=c.child("p").and_then(|p|p.attr("paraId")).map(str::to_owned).unwrap_or_else(||format!("{:08X}",id));
            by_para.insert(para.clone(),id);
            out.insert(id,CommentMetadata{paragraph_id:Some(para),initials:c.attr("initials").map(str::to_owned),created:c.attr("date").map(str::to_owned),..Default::default()});
        }
    }}
    if let Some(root)=part_xml(parts,"word/commentsExtended.xml")?{
        for e in root.elements("commentEx"){
            if let Some(id)=e.attr("paraId").and_then(|p|by_para.get(p).copied()) {
                if let Some(m)=out.get_mut(&id){m.resolved=e.attr("done")==Some("1");}
            }
        }
    }
    if let Some(root)=part_xml(parts,"word/commentsIds.xml")?{
        for e in root.elements("commentId") {
            if let Some(id)=e.attr("paraId").and_then(|p|by_para.get(p).copied()){
                if let Some(m)=out.get_mut(&id){m.durable_id=e.attr("durableId").map(str::to_owned);}
            }
        }
    }
    if let Some(root)=part_xml(parts,"word/people.xml")?{
        for e in root.elements("person") {
            if let Some(name)=e.attr("author") {
                for id in out.keys().copied().collect::<Vec<_>>() {
                    // Match comment authors by consulting the comments part.
                    let name_matches=part_xml(parts,"word/comments.xml")?.map(|comments|comments.elements("comment").any(|c|attr_num(c,"id")==Some(id)&&c.attr("author")==Some(name))).unwrap_or(false);
                    if name_matches {if let (Some(m),Some(p))=(out.get_mut(&id),e.child("presenceInfo")){
                        m.provider_id=p.attr("providerId").map(str::to_owned);
                        m.user_id=p.attr("userId").map(str::to_owned);
                    }}
                }
            }
        }
    }
    Ok(out)
}
fn parse_styles(parts:&BTreeMap<String,Vec<u8>>)->io::Result<Vec<Style>>{
    let mut out=Vec::new();if let Some(root)=part_xml(parts,"word/styles.xml")?{for e in root.elements("style") {
        let kind=match e.attr("type"){Some("character")=>StyleType::Character,Some("table")=>StyleType::Table,_=>StyleType::Paragraph};
        let id=e.attr("styleId").unwrap_or("").to_owned();if id.is_empty()||id=="Normal"||id.starts_with("Heading"){continue;}
        let name=e.child("name").and_then(|v|v.attr("val")).unwrap_or(&id).to_string();
        let pr=e.child("rPr");
        let bold_override=pr.and_then(|p|p.child("b")).map(|v|!matches!(v.attr("val"),Some("0"|"false"|"off")));
        let italic_override=pr.and_then(|p|p.child("i")).map(|v|!matches!(v.attr("val"),Some("0"|"false"|"off")));
        out.push(Style{id,name,kind,based_on:e.child("basedOn").and_then(|v|v.attr("val")).map(str::to_owned),font:pr.and_then(|p|p.child("rFonts")).and_then(|v|v.attr("ascii")).map(str::to_owned),size:pr.and_then(|p|p.child("sz")).and_then(|v|v.attr("val")).and_then(|v|v.parse::<f32>().ok()).map(|v|v/2.0),bold:bold_override.unwrap_or(false),italic:italic_override.unwrap_or(false),color:pr.and_then(|p|p.child("color")).and_then(|v|v.attr("themeColor").map(|v|format!("theme:{v}")).or_else(||v.attr("val").map(str::to_owned))),bold_override,italic_override});
    }}Ok(out)
}
fn parse_theme_colors(parts:&BTreeMap<String,Vec<u8>>)->io::Result<BTreeMap<String,String>>{
    let mut colors=BTreeMap::new();
    if let Some(root)=part_xml(parts,"word/theme/theme1.xml")?{
        if let Some(elements)=root.child("themeElements"){if let Some(scheme)=elements.child("clrScheme"){
            for node in &scheme.children{if let Node::Element(color)=node{
                if let Some(value)=color.child("srgbClr").and_then(|v|v.attr("val"))
                    .or_else(||color.child("sysClr").and_then(|v|v.attr("lastClr"))) {
                    colors.insert(color.local().to_owned(),value.to_owned());
                }
            }}
        }}
    }
    Ok(colors)
}
fn parse_default_run_style(parts:&BTreeMap<String,Vec<u8>>)->io::Result<Option<Style>>{
    let Some(root)=part_xml(parts,"word/styles.xml")? else{return Ok(None)};
    let Some(pr)=root.child("docDefaults").and_then(|v|v.child("rPrDefault")).and_then(|v|v.child("rPr"))else{return Ok(None)};
    let mut default=Style::paragraph("DocDefaults","Document default run style");
    default.font=pr.child("rFonts").and_then(|v|v.attr("ascii").or_else(||v.attr("hAnsi"))).map(str::to_owned);
    default.size=pr.child("sz").and_then(|v|v.attr("val")).and_then(|v|v.parse::<f32>().ok()).map(|n|n/2.0);
    default.color=pr.child("color").and_then(|v|v.attr("themeColor").map(|v|format!("theme:{v}")).or_else(||v.attr("val").map(str::to_owned)));
    default.bold_override=pr.child("b").map(|v|!matches!(v.attr("val"),Some("0"|"false"|"off")));
    default.italic_override=pr.child("i").map(|v|!matches!(v.attr("val"),Some("0"|"false"|"off")));
    default.bold=default.bold_override.unwrap_or(false);
    default.italic=default.italic_override.unwrap_or(false);
    Ok(Some(default))
}
impl Document {
    /// Read a DOCX ZIP, including DEFLATE-compressed Microsoft Office documents.
    pub fn from_bytes(data:&[u8])->io::Result<Self>{
        let parts=zip::decode(data)?;
        let root=part_xml(&parts,"word/document.xml")?.ok_or_else(||err("word/document.xml missing"))?;
        let rels=relationships(&parts,"word/_rels/document.xml.rels")?;
        let mut doc=Self::default();doc.properties=parse_properties(&parts)?;doc.styles=parse_styles(&parts)?;
        doc.theme_colors=parse_theme_colors(&parts)?;doc.default_run_style=parse_default_run_style(&parts)?;
        doc.footnotes=parse_notes(&parts,"word/footnotes.xml","footnote")?;
        doc.endnotes=parse_notes(&parts,"word/endnotes.xml","endnote")?;
        doc.comments=parse_comments(&parts)?;
        doc.comment_metadata=parse_comment_metadata(&parts)?;
        if let Some(body)=root.child("body"){
            for n in &body.children{if let Node::Element(e)=n{match e.local(){
                "p"=>{
                    doc.blocks.push(Block::Paragraph(parse_para(e,&rels,&mut doc.images,&parts)));
                    if let Some(sp)=e.child("pPr").and_then(|p|p.child("sectPr")){
                        doc.sections.push(parse_section(sp,doc.blocks.len(),&doc.page));
                    }
                },
                "tbl"=>doc.blocks.push(Block::Table(parse_table(e,&rels,&mut doc.images,&parts))),
                "sectPr"=>{
                    let section=parse_section(e,doc.blocks.len(),&doc.page);
                    doc.page=section.page.clone();doc.final_section=Some(section);
                },_=>{}}}}
        }
        if let Some(set)=part_xml(&parts,"word/settings.xml")?{if let Some(p)=set.child("documentProtection"){if p.attr("enforcement")==Some("1"){doc.protection=Some(match p.attr("edit"){Some("forms")=>Protection::FormsOnly,Some("comments")=>Protection::CommentsOnly,_=>Protection::ReadOnly});}}if set.child("trackRevisions").is_some(){doc.track_changes=Some(String::new());}
            if set.child("mirrorMargins").is_some(){if let Some(final_section)=&mut doc.final_section{final_section.mirror_margins=true;}}}
        for (idx,paragraphs) in [(1,&mut doc.header),(2,&mut doc.first_header)] {
            if let Some(r)=part_xml(&parts,&format!("word/header{idx}.xml"))?{
                let hrels=relationships(&parts,&format!("word/_rels/header{idx}.xml.rels"))?;
                for p in r.elements("p"){paragraphs.push(parse_para(p,&hrels,&mut doc.images,&parts));}
            }
        }
        for (idx,paragraphs) in [(1,&mut doc.footer),(2,&mut doc.first_footer)] {
            if let Some(r)=part_xml(&parts,&format!("word/footer{idx}.xml"))?{
                let frels=relationships(&parts,&format!("word/_rels/footer{idx}.xml.rels"))?;
                for p in r.elements("p"){paragraphs.push(parse_para(p,&frels,&mut doc.images,&parts));}
            }
        }
        for (name,bytes) in &parts{if name.starts_with("customXml/") && name.ends_with(".xml") && !name.contains("/_rels/"){
            if let Ok(value)=xml::decode_bytes(bytes){
                let local=name.trim_start_matches("customXml/").to_owned();
                if !local.starts_with("itemProps"){
                    doc.custom_xml.push(CustomXml{name:local.clone(),xml:value});
                    let rpath=format!("customXml/_rels/{local}.rels");
                    if let Ok(rels)=relationships(&parts,&rpath){for (target,kind) in rels.values(){
                        if kind.ends_with("/customXmlProps"){
                            if let Some(props)=parts.get(target){if let Ok(props)=xml::decode_bytes(props){doc.custom_xml_item_props.insert(local.clone(),props);}}
                        }
                    }}
                }
            }
        }}
        if let Some(root)=part_xml(&parts,"word/glossary/document.xml")?{
            let grels=relationships(&parts,"word/glossary/_rels/document.xml.rels").unwrap_or_default();
            if let Some(collection)=root.child("docParts"){for part in collection.elements("docPart"){
                let name=part.child("docPartPr").and_then(|v|v.child("name")).and_then(|v|v.attr("val")).unwrap_or("").to_owned();
                let category=part.child("docPartPr").and_then(|v|v.child("category")).and_then(|v|v.child("name")).and_then(|v|v.attr("val")).unwrap_or("").to_owned();
                let paragraphs=part.child("docPartBody").map(|body|body.elements("p").map(|p|parse_para(p,&grels,&mut doc.images,&parts)).collect()).unwrap_or_default();
                doc.glossary.push(BuildingBlock{name,category,paragraphs});
            }}
        }
        if let Some(root)=part_xml(&parts,"word/fontTable.xml")?{
            let frels=relationships(&parts,"word/_rels/fontTable.xml.rels")?;
            for el in root.elements("font"){if let Some(ref_) = el.child("embedRegular"){
                if let Some(id)=ref_.attr("r:id"){if let Some((path,_))=frels.get(id){if let Some(raw)=parts.get(path){
                    doc.embedded_fonts.push(EmbeddedFont{name:el.attr("name").unwrap_or("").to_owned(),obfuscated_data:raw.clone(),key_guid:ref_.attr("fontKey").unwrap_or("").to_owned()});
                }}}
            }}
        }
        doc.original_body=Some(root.clone());
        doc.original_block_debug=doc.blocks.iter().map(|b|format!("{b:?}")).collect();
        doc.original_components.insert("header".into(),format!("{:?}",doc.header));
        doc.original_components.insert("footer".into(),format!("{:?}",doc.footer));
        doc.original_components.insert("first_header".into(),format!("{:?}",doc.first_header));
        doc.original_components.insert("first_footer".into(),format!("{:?}",doc.first_footer));
        doc.original_components.insert("images".into(),format!("{:?}",doc.images));
        doc.original_components.insert("page".into(),format!("{:?}",doc.page));
        doc.original_components.insert("section".into(),format!("{:?}{:?}{:?}{:?}{:?}{:?}",doc.page,doc.header,doc.footer,doc.first_header,doc.first_footer,doc.final_section));
        doc.original_components.insert("sections".into(),format!("{:?}",doc.sections));
        doc.original_components.insert("styles".into(),format!("{:?}",doc.styles));
        doc.original_components.insert("blocks".into(),format!("{:?}",doc.blocks));
        doc.original_components.insert("settings".into(),format!("{:?}{:?}",doc.protection,doc.track_changes));
        doc.original_components.insert("core".into(),format!("{:?}",doc.properties));
        doc.original_parts=parts;
        doc.original_snapshot=Some(doc.snapshot());Ok(doc)
    }
}
