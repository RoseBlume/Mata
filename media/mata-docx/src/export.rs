//! Standalone HTML, GFM Markdown, basic PDF, and 1-bit-like PNG preview exports.
//! PDF and PNG use deliberately simple layout; unlike a full shaping engine,
//! these do not claim Word-identical pagination or complex-script rendering.
use crate::model::*;
use crate::xml;
use crate::zip::crc32;
use std::fs;
use std::io::{self,Error,ErrorKind};
use std::path::Path;
fn esc(s:&str)->String{xml::escape(s)}
fn base64(data:&[u8])->String{
    const CH:&[u8]=b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s=String::new();
    for c in data.chunks(3){let a=c[0] as usize;let b=c.get(1).copied().unwrap_or(0) as usize;let d=c.get(2).copied().unwrap_or(0) as usize;
        s.push(CH[a>>2] as char);s.push(CH[((a&3)<<4)|(b>>4)] as char);
        s.push(if c.len()>1{CH[((b&15)<<2)|(d>>6)] as char}else{'='});
        s.push(if c.len()>2{CH[d&63] as char}else{'='});
    }s
}
fn html_p(p:&Paragraph,images:&[Image])->String{
    let tag=p.style.as_ref().and_then(|s|s.strip_prefix("Heading")).and_then(|s|s.parse::<u8>().ok()).filter(|&v|v>=1&&v<=6).map(|v|format!("h{v}")).unwrap_or_else(||"p".to_owned());
    let mut html=format!("<{tag} style=\"text-align:{}\">",p.alignment.as_str().replace("both","justify"));
    for i in &p.inlines{match i{
        Inline::Run(r)=>{
            let mut t=esc(&r.text).replace('\n',"<br>");
            if r.bold{t=format!("<strong>{t}</strong>");}if r.italic{t=format!("<em>{t}</em>");}
            if r.underline{t=format!("<u>{t}</u>");}if r.strike{t=format!("<del>{t}</del>");}
            if r.superscript{t=format!("<sup>{t}</sup>");}if r.subscript{t=format!("<sub>{t}</sub>");}
            if r.font.is_some()||r.font_size.is_some()||r.color.is_some()||r.highlight.is_some(){
                let mut st=String::new();if let Some(v)=&r.font{st.push_str(&format!("font-family:'{}';",esc(v)));}
                if let Some(v)=r.font_size{st.push_str(&format!("font-size:{v}pt;"));}if let Some(v)=&r.color{st.push_str(&format!("color:#{};",esc(v)));}if let Some(v)=&r.highlight{st.push_str(&format!("background-color:{};",esc(v)));}t=format!("<span style=\"{st}\">{t}</span>");
            }
            if let Some(u)=&r.hyperlink{t=format!("<a href=\"{}\">{t}</a>",esc(u));}
            if let Some(u)=&r.internal_link{t=format!("<a href=\"#{}\">{t}</a>",esc(u));}
            html.push_str(&t);
        }
        Inline::Bookmark(name)=>html.push_str(&format!("<span id=\"{}\"></span>",esc(name))),
        Inline::Image(idx)=>if let Some(i)=images.get(*idx){html.push_str(&format!("<img alt=\"{}\" width=\"{}\" height=\"{}\" src=\"data:{};base64,{}\">",esc(&i.filename),i.width.0/9525,i.height.0/9525,esc(&i.content_type),base64(&i.bytes)));},
        Inline::ContentControl{value,..}=>html.push_str(&esc(value)),
        Inline::StructuredControl(c)=>html.push_str(&esc(&c.value)),
        Inline::ComplexField{result,..}=>html.push_str(&esc(result)),
        Inline::Revision{kind,content,..}=>if matches!(kind,RevisionKind::Insert|RevisionKind::MoveTo){
            for item in content{if let Inline::Run(r)=item{html.push_str(&esc(&r.text));}}
        },
        Inline::Checkbox{checked,..}=>html.push_str(if *checked{"☒"}else{"☐"}),
        Inline::Dropdown{options,selected,..}=>html.push_str(&esc(options.get(*selected).map(String::as_str).unwrap_or(""))),
        Inline::FootnoteRef(id)=>html.push_str(&format!("<sup>[{id}]</sup>")),
        _=>{}
    }}html.push_str(&format!("</{tag}>"));html
}
fn md_escape(s:&str)->String{s.replace('\\',"\\\\").replace('|',"\\|").replace('*',"\\*").replace('_',"\\_").replace('`',"\\`")}
fn md_p(p:&Paragraph,images:&[Image])->String{
    let mut out=String::new();
    if let Some(style)=&p.style {if let Some(level)=style.strip_prefix("Heading").and_then(|s|s.parse::<usize>().ok()){out.push_str(&"#".repeat(level.min(6)));out.push(' ');}}
    if let Some(num)=p.list{out.push_str(if num.bullet{"- "}else{"1. "});}
    for inline in &p.inlines {match inline {
        Inline::Run(r)=>{let mut t=md_escape(&r.text);if r.bold{t=format!("**{t}**");}if r.italic{t=format!("*{t}*");}if r.strike{t=format!("~~{t}~~");}if let Some(u)=&r.hyperlink{t=format!("[{t}]({u})");}if let Some(b)=&r.internal_link{t=format!("[{t}](#{b})");}out.push_str(&t);}
        Inline::Image(idx)=>if let Some(img)=images.get(*idx){out.push_str(&format!("![{}](data:{};base64,{})",md_escape(&img.filename),img.content_type,base64(&img.bytes)));},
        Inline::ContentControl{value,..}=>out.push_str(&md_escape(value)),
        Inline::StructuredControl(c)=>out.push_str(&md_escape(&c.value)),
        Inline::ComplexField{result,..}=>out.push_str(&md_escape(result)),
        Inline::Revision{kind,content,..}=>if matches!(kind,RevisionKind::Insert|RevisionKind::MoveTo){
            for item in content{if let Inline::Run(r)=item{out.push_str(&md_escape(&r.text));}}
        },
        Inline::Checkbox{checked,..}=>out.push_str(if *checked{"[x] "}else{"[ ] "}),
        Inline::Dropdown{options,selected,..}=>out.push_str(options.get(*selected).map(String::as_str).unwrap_or("")),
        Inline::FootnoteRef(id)=>out.push_str(&format!("[^{id}]")),
        _=>{}
    }}out
}
impl Document {
    /// Convert document to HTML with CSS and data-URI embedded images.
    pub fn to_html(&self)->String {
        let mut s=format!("<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>body{{font-family:Calibri,Arial,sans-serif;max-width:850px;margin:auto;padding:2em;line-height:1.5}}table{{border-collapse:collapse;margin:1em 0}}td,th{{border:1px solid #aaa;padding:.3em .6em}}img{{max-width:100%}}</style></head><body>",esc(&self.properties.title));
        for block in &self.blocks{match block{
            Block::Paragraph(p)=>s.push_str(&html_p(p,&self.images)),
            Block::Table(t)=>{s.push_str("<table>");for row in &t.rows{s.push_str("<tr>");for cell in &row.cells{let tag=if row.is_header{"th"}else{"td"};s.push_str(&format!("<{tag}{}{}>",if cell.colspan>1{format!(" colspan=\"{}\"",cell.colspan)}else{String::new()},if let Some(v)=&cell.shading{format!(" style=\"background-color:#{}\"",esc(v))}else{String::new()}));for p in &cell.paragraphs{s.push_str(&html_p(p,&self.images));}s.push_str(&format!("</{tag}>"));}s.push_str("</tr>");}s.push_str("</table>");}
        }}
        if !self.footnotes.is_empty(){s.push_str("<hr><section><h2>Footnotes</h2><ol>");for n in &self.footnotes{s.push_str(&format!("<li id=\"fn{}\">{}</li>",n.id,esc(&n.text)));}s.push_str("</ol></section>");}
        s.push_str("</body></html>");s
    }
    /// GFM-style Markdown conversion with pipe tables.
    pub fn to_markdown(&self)->String{
        let mut chunks=Vec::new();for b in &self.blocks{match b{
            Block::Paragraph(p)=>chunks.push(md_p(p,&self.images)),
            Block::Table(t)=>{let mut s=String::new();for (index,row) in t.rows.iter().enumerate(){s.push('|');for cell in &row.cells{s.push(' ');s.push_str(&cell.paragraphs.iter().map(|p|md_p(p,&self.images)).collect::<Vec<_>>().join(" ").replace('\n'," "));s.push_str(" |");}s.push('\n');if index==0 {s.push('|');for _ in &row.cells{s.push_str(" --- |");}s.push('\n');}}chunks.push(s);}
        }}
        for n in &self.footnotes{chunks.push(format!("[^{}]: {}",n.id,n.text));}
        chunks.join("\n\n")
    }
    pub fn save_html(&self,path:impl AsRef<Path>)->io::Result<()>{fs::write(path,self.to_html())}
    pub fn save_markdown(&self,path:impl AsRef<Path>)->io::Result<()>{fs::write(path,self.to_markdown())}
    /// Generate a static table of contents from Heading1..Heading9 paragraphs.
    pub fn generate_toc(&mut self){
        let mut headings=Vec::new();let mut index=0usize;
        for para in self.paragraphs_mut(){
            if let Some(level)=para.style.as_deref().and_then(|s|s.strip_prefix("Heading")).and_then(|s|s.parse::<u8>().ok()) {
                let text=para.text();let name=format!("_toc{index}");
                para.inlines.insert(0,Inline::Bookmark(name.clone()));
                headings.push((level,text,name));index+=1;
            }
        }
        let mut title=Paragraph::new("Table of Contents");title.style("Heading1");
        let mut entries=vec![Block::Paragraph(title)];
        for (level,text,bookmark) in headings {
            let mut entry=Paragraph::new(format!("{}{}", "  ".repeat(level.saturating_sub(1) as usize),text));
            entry.add_run(" ................................ ");
            entry.add_field(format!("PAGEREF {bookmark} \\h"));
            entries.push(Block::Paragraph(entry));
        }
        self.blocks.splice(0..0,entries);
    }
}

/// A text layout record for the lightweight PDF/PNG renderers.
#[derive(Clone)]struct Line{text:String,heading:bool,page_break:bool}
fn wrap_words(s:&str,maximum:usize)->Vec<String>{
    let mut lines=Vec::new();let mut current=String::new();
    for paragraph in s.split('\n'){
        if !current.is_empty(){lines.push(std::mem::take(&mut current));}
        for word in paragraph.split_whitespace(){if !current.is_empty() && current.chars().count()+word.chars().count()+1>maximum{lines.push(std::mem::take(&mut current));}
            if !current.is_empty(){current.push(' ');}current.push_str(word);}
        lines.push(std::mem::take(&mut current));
    }if lines.is_empty(){lines.push(String::new());}lines
}
fn lines(doc:&Document,maxchars:usize)->Vec<Line>{
    let mut out=Vec::new();
    for block in &doc.blocks{match block{
        Block::Paragraph(p)=>{
            if p.page_break_before || p.inlines.iter().any(|i|matches!(i,Inline::Run(r) if r.break_kind==Some(BreakKind::Page))){out.push(Line{text:String::new(),heading:false,page_break:true});}
            let h=p.style.as_deref().unwrap_or("").starts_with("Heading");
            for text in wrap_words(&p.text(),maxchars){out.push(Line{text,heading:h,page_break:false});}
            out.push(Line{text:String::new(),heading:false,page_break:false});
        },
        Block::Table(t)=>{
            for row in &t.rows{let joined=row.cells.iter().map(|c|c.text()).collect::<Vec<_>>().join("  |  ");for text in wrap_words(&joined,maxchars){out.push(Line{text,heading:row.is_header,page_break:false});}}
            out.push(Line{text:String::new(),heading:false,page_break:false});
        }
    }}out
}
fn pdf_ascii(s:&str)->String{
    let mut out=String::new();for c in s.chars(){let c=if c=='\t'{' '}else if c.is_ascii_graphic()||c==' '{c}else{'?'};match c{'('=>out.push_str("\\("),')'=>out.push_str("\\)"),'\\'=>out.push_str("\\\\"),_=>out.push(c)}}out
}
impl Document {
    /// Simple searchable PDF exporter for Latin-script text. Does not shape Unicode or embed image data.
    pub fn to_pdf(&self)->io::Result<Vec<u8>>{
        let w=self.page.width_twips as f32/20.0;let h=self.page.height_twips as f32/20.0;
        let left=self.page.margin_left as f32/20.0;let top=self.page.margin_top as f32/20.0;
        let available=w-left-(self.page.margin_right as f32/20.0);
        let maxchars=((available/5.8) as usize).max(15);
        let lineheight=14.0f32;
        let maxlines=((h-top-self.page.margin_bottom as f32/20.0)/lineheight).floor().max(1.0) as usize;
        let mut pages:Vec<Vec<Line>>=vec![Vec::new()];
        for ln in lines(self,maxchars) {if ln.page_break{if !pages.last().unwrap().is_empty(){pages.push(Vec::new());}continue;}
            if pages.last().unwrap().len()>=maxlines{pages.push(Vec::new());}pages.last_mut().unwrap().push(ln);
        }
        let mut objects:Vec<Vec<u8>>=Vec::new();
        // object IDs: 1 catalog, 2 pages, 3 font, then two objects per page.
        objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        let mut kids=String::new();for i in 0..pages.len(){kids.push_str(&format!("{} 0 R ",4+i*2));}
        objects.push(format!("<< /Type /Pages /Kids [{kids}] /Count {} >>",pages.len()).into_bytes());
        objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>".to_vec());
        for (i,page) in pages.iter().enumerate(){
            let content_id=5+i*2;
            objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] /Resources << /Font << /F1 3 0 R >> >> /Contents {content_id} 0 R >>").into_bytes());
            let mut stream=String::new();
            for (j,line) in page.iter().enumerate(){let y=h-top-j as f32*lineheight;
                let size=if line.heading{12}else{10};
                stream.push_str(&format!("BT /F1 {size} Tf 1 0 0 1 {left:.2} {y:.2} Tm ({}) Tj ET\n",pdf_ascii(&line.text)));
            }
            let content=format!("<< /Length {} >>\nstream\n{}endstream",stream.len(),stream);
            objects.push(content.into_bytes());
        }
        let mut out=b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();let mut offsets=vec![0usize];
        for (i,obj) in objects.iter().enumerate(){offsets.push(out.len());out.extend(format!("{} 0 obj\n",i+1).as_bytes());out.extend(obj);out.extend(b"\nendobj\n");}
        let xref=out.len();out.extend(format!("xref\n0 {}\n0000000000 65535 f \n",objects.len()+1).as_bytes());
        for off in offsets.into_iter().skip(1){out.extend(format!("{off:010} 00000 n \n").as_bytes());}
        out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",objects.len()+1).as_bytes());Ok(out)
    }
    pub fn save_pdf(&self,path:impl AsRef<Path>)->io::Result<()>{fs::write(path,self.to_pdf()?)}
}
fn glyph(c:char)->[u8;7]{
    match c.to_ascii_uppercase(){
        'A'=>[14,17,17,31,17,17,17],'B'=>[30,17,17,30,17,17,30],'C'=>[14,17,16,16,16,17,14],
        'D'=>[30,17,17,17,17,17,30],'E'=>[31,16,16,30,16,16,31],'F'=>[31,16,16,30,16,16,16],
        'G'=>[14,17,16,23,17,17,14],'H'=>[17,17,17,31,17,17,17],'I'=>[31,4,4,4,4,4,31],
        'J'=>[7,2,2,2,18,18,12],'K'=>[17,18,20,24,20,18,17],'L'=>[16,16,16,16,16,16,31],
        'M'=>[17,27,21,21,17,17,17],'N'=>[17,25,21,19,17,17,17],'O'=>[14,17,17,17,17,17,14],
        'P'=>[30,17,17,30,16,16,16],'Q'=>[14,17,17,17,21,18,13],'R'=>[30,17,17,30,20,18,17],
        'S'=>[15,16,16,14,1,1,30],'T'=>[31,4,4,4,4,4,4],'U'=>[17,17,17,17,17,17,14],
        'V'=>[17,17,17,17,17,10,4],'W'=>[17,17,17,21,21,21,10],'X'=>[17,17,10,4,10,17,17],
        'Y'=>[17,17,10,4,4,4,4],'Z'=>[31,1,2,4,8,16,31],
        '0'=>[14,17,19,21,25,17,14],'1'=>[4,12,4,4,4,4,14],'2'=>[14,17,1,2,4,8,31],
        '3'=>[30,1,1,14,1,1,30],'4'=>[2,6,10,18,31,2,2],'5'=>[31,16,30,1,1,17,14],
        '6'=>[6,8,16,30,17,17,14],'7'=>[31,1,2,4,8,8,8],'8'=>[14,17,17,14,17,17,14],
        '9'=>[14,17,17,15,1,2,12],
        '.'=>[0,0,0,0,0,12,12],','=>[0,0,0,0,12,12,8],':'=>[0,12,12,0,12,12,0],
        '!'=>[4,4,4,4,4,0,4], '?'=>[14,17,1,2,4,0,4],'-'=>[0,0,0,31,0,0,0],
        '/'=>[1,1,2,4,8,16,16], ' '=>[0;7],'|'=>[4,4,4,4,4,4,4],
        '('=>[2,4,8,8,8,4,2],')'=>[8,4,2,2,2,4,8],
        _=>[0,0,14,17,2,0,2]
    }
}
fn chunk(typ:&[u8;4],data:&[u8],out:&mut Vec<u8>){
    out.extend((data.len() as u32).to_be_bytes());out.extend(typ);out.extend(data);
    let mut crcdata=Vec::with_capacity(4+data.len());crcdata.extend(typ);crcdata.extend(data);out.extend(crc32(&crcdata).to_be_bytes());
}
fn adler32(data:&[u8])->u32{let (mut a,mut b)=(1u32,0u32);for &v in data {a=(a+v as u32)%65521;b=(b+a)%65521;}b<<16|a}
fn png_rgb(w:usize,h:usize,data:&[u8])->io::Result<Vec<u8>>{
    if data.len()!=w*h*3{return Err(Error::new(ErrorKind::InvalidInput,"pixel buffer does not match PNG size"));}
    let mut raw=Vec::with_capacity(h*(1+w*3));for y in 0..h{raw.push(0);raw.extend(&data[y*w*3..(y+1)*w*3]);}
    // RFC 1950 zlib stream with RFC 1951 uncompressed blocks, all in standard Rust.
    let mut zlib=vec![0x78,0x01];for (i,part) in raw.chunks(65535).enumerate(){let last=(i+1)*65535>=raw.len();zlib.push(if last{1}else{0});let len=part.len() as u16;zlib.extend(len.to_le_bytes());zlib.extend((!len).to_le_bytes());zlib.extend(part);}
    zlib.extend(adler32(&raw).to_be_bytes());
    let mut out=b"\x89PNG\r\n\x1a\n".to_vec();let mut ihdr=Vec::new();ihdr.extend((w as u32).to_be_bytes());ihdr.extend((h as u32).to_be_bytes());ihdr.extend([8,2,0,0,0]);chunk(b"IHDR",&ihdr,&mut out);chunk(b"IDAT",&zlib,&mut out);chunk(b"IEND",&[],&mut out);Ok(out)
}
impl Document {
    /// Renders page previews to self-contained PNGs with a tiny built-in 5x7 ASCII bitmap font.
    pub fn to_png_pages(&self)->io::Result<Vec<Vec<u8>>>{
        let (w,h)=(816usize,1056usize);let scale=2usize;let widthchars=(w-110)/(6*scale);
        let mut pages=Vec::new();let mut pixels=vec![255u8;w*h*3];let mut y=60usize;
        for ln in lines(self,widthchars){if ln.page_break || y+22>=h-60{pages.push(png_rgb(w,h,&pixels)?);pixels.fill(255);y=60;}
            if ln.page_break{continue;}
            for (i,c) in ln.text.chars().take(widthchars).enumerate(){let bitmap=glyph(c);for (row,bits) in bitmap.iter().enumerate(){for col in 0..5 {if (*bits & (1u8 << (4-col))) != 0{for sy in 0..scale{for sx in 0..scale{let px=55+i*6*scale+col*scale+sx;let py=y+row*scale+sy;if px<w&&py<h{let k=(py*w+px)*3;pixels[k]=0;pixels[k+1]=0;pixels[k+2]=0;}}}}}}}
            y+=if ln.heading{24}else{20};
        }
        pages.push(png_rgb(w,h,&pixels)?);Ok(pages)
    }
    pub fn save_png(&self,path:impl AsRef<Path>)->io::Result<()>{let pages=self.to_png_pages()?;fs::write(path,&pages[0])}
}

impl Document {
    /// Export using the dedicated Stage 6 page geometry, including page sizes,
    /// columns, table cells, headers/footers, and footnote positions.
    ///
    /// The PDF uses a basic Type-1 font, not embedded OpenType outlines. Glyphs
    /// unsupported by WinAnsi remain approximate; the method is intentionally
    /// separate from the legacy `to_pdf()` and does not claim Word fidelity.
    pub fn to_pdf_with_layout(&self,engine:&crate::layout::Engine)->io::Result<Vec<u8>>{
        let pages=engine.paginate(self)?;
        let mut objects:Vec<Vec<u8>>=vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            Vec::new(),
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
        ];
        let kids=(0..pages.pages.len()).map(|i|format!("{} 0 R",4+i*2)).collect::<Vec<_>>().join(" ");
        objects[1]=format!("<< /Type /Pages /Kids [{kids}] /Count {} >>",pages.pages.len()).into_bytes();
        for (i,page) in pages.pages.iter().enumerate(){
            let content_id=5+i*2;
            objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.3} {:.3}] /Resources << /Font << /F1 3 0 R >> >> /Contents {content_id} 0 R >>",page.width,page.height).into_bytes());
            let mut stream=String::new();
            // Word table cells can carry distinct edge widths, colors and
            // shading. Fill first, then stroke explicit borders only.
            for cell in &page.cells {
                if let Some([r,g,b])=cell.shading.as_deref().and_then(parse_rgb){
                    let rect=cell.region;
                    stream.push_str(&format!("{:.3} {:.3} {:.3} rg {:.2} {:.2} {:.2} {:.2} re f\n",
                        r as f32/255.,g as f32/255.,b as f32/255.,
                        rect.x,page.height-rect.bottom(),rect.width,rect.height));
                }
            }
            for cell in &page.cells{
                let r=cell.region;
                for (side,border) in cell.edges.iter().enumerate(){
                    let Some((rgb,thickness))=visible_cell_border(border.as_ref()) else {continue};
                    let [red,green,blue]=rgb;
                    let (x0,y0,x1,y1)=match side{
                        0=>(r.x,page.height-r.y,r.right(),page.height-r.y),
                        1=>(r.x,page.height-r.bottom(),r.right(),page.height-r.bottom()),
                        2=>(r.x,page.height-r.y,r.x,page.height-r.bottom()),
                        _=>(r.right(),page.height-r.y,r.right(),page.height-r.bottom()),
                    };
                    stream.push_str(&format!("{:.3} {:.3} {:.3} RG {:.3} w {:.2} {:.2} m {:.2} {:.2} l S\n",
                        red as f32/255.,green as f32/255.,blue as f32/255.,thickness,
                        x0,y0,x1,y1));
                }
            }
            stream.push_str("0 g\n");
            for text in page.text.iter().chain(page.headers.iter()).chain(page.footers.iter()).chain(page.footnotes.iter()){
                if text.text.is_empty(){continue;}
                let size=text.size_pt.clamp(1.0,144.0);
                let y=page.height-text.region.y-size;
                stream.push_str(&format!("BT /F1 {size:.2} Tf 1 0 0 1 {:.2} {y:.2} Tm ({}) Tj ET\n",
                    text.region.x,pdf_ascii(&text.text)));
            }
            let content=format!("<< /Length {} >>\nstream\n{}endstream",stream.as_bytes().len(),stream);
            objects.push(content.into_bytes());
        }
        let mut out=b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets=Vec::with_capacity(objects.len());
        for (i,obj) in objects.iter().enumerate(){
            offsets.push(out.len());out.extend(format!("{} 0 obj\n",i+1).as_bytes());
            out.extend(obj);out.extend(b"\nendobj\n");
        }
        let xref=out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n",objects.len()+1).as_bytes());
        for offset in offsets{out.extend(format!("{offset:010} 00000 n \n").as_bytes());}
        out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",objects.len()+1).as_bytes());
        Ok(out)
    }

    /// Layout-aware diagnostic PNG preview; uses the same position records as
    /// `to_pdf_with_layout`. Text pixels use the built-in small bitmap fallback
    /// (not `mata-fonts`), so these previews are not high-fidelity rendering.
    pub fn to_png_pages_with_layout(&self,engine:&crate::layout::Engine,dpi:f32)->io::Result<Vec<Vec<u8>>>{
        if !dpi.is_finite()||!(36.0..=300.0).contains(&dpi){
            return Err(Error::new(ErrorKind::InvalidInput,"PNG preview DPI must be between 36 and 300"));
        }
        let layout=engine.paginate(self)?;
        let scale=dpi/72.0;
        let mut pages=Vec::new();
        for page in layout.pages{
            let w=(page.width*scale).ceil() as usize;
            let h=(page.height*scale).ceil() as usize;
            if w==0||h==0||w>8192||h>8192||w.checked_mul(h).unwrap_or(usize::MAX)>16_000_000{
                return Err(Error::new(ErrorKind::InvalidInput,"PNG preview page dimensions too large"));
            }
            let mut pixels=vec![255u8;w*h*3];
            for positioned in page.text.iter().chain(page.headers.iter()).chain(page.footers.iter()).chain(page.footnotes.iter()){
                let font_scale=(positioned.size_pt.max(6.0)*scale/8.0).round().clamp(1.0,16.0) as usize;
                let mut x=(positioned.region.x*scale) as isize;
                let y=(positioned.region.y*scale) as isize;
                for character in positioned.text.chars(){
                    let bitmap=glyph(character);
                    for (row,bits) in bitmap.iter().enumerate(){
                        for col in 0..5{
                            if (*bits & (1u8 << (4-col)))==0{continue;}
                            for dy in 0..font_scale{for dx in 0..font_scale{
                                let px=x+((col*font_scale+dx) as isize);
                                let py=y+((row*font_scale+dy) as isize);
                                if px>=0 && py>=0 && (px as usize)<w && (py as usize)<h{
                                    let offset=(py as usize*w+px as usize)*3;
                                    pixels[offset]=0;pixels[offset+1]=0;pixels[offset+2]=0;
                                }
                            }}
                        }
                    }
                    x+=(6*font_scale) as isize;
                }
            }
            pages.push(png_rgb(w,h,&pixels)?);
        }
        Ok(pages)
    }
}

impl Document {
    /// Layout-aware PNG pages using an existing `mata-fonts` TrueType
    /// rasterizer instead of the fixed 5x7 diagnostic font.
    ///
    /// This renders individual Unicode scalars through `Ttf::character`.
    /// An advanced glyph-ID rasterizer is still required to paint GSUB
    /// ligatures, variable-font outlines and complex-script glyphs precisely.
    pub fn to_png_pages_with_mata_font(
        &self,
        engine:&crate::layout::Engine,
        font:&mut mata_fonts::decoders::ttf::Ttf,
        dpi:f32,
    )->io::Result<Vec<Vec<u8>>>{
        if !dpi.is_finite()||!(36.0..=300.0).contains(&dpi){
            return Err(Error::new(ErrorKind::InvalidInput,"PNG DPI must be 36..=300"));
        }
        let scale=dpi/72.0;
        let layout=engine.paginate(self)?;
        let mut output=Vec::with_capacity(layout.pages.len());
        for page in &layout.pages{
            let w=(page.width*scale).ceil() as usize;
            let h=(page.height*scale).ceil() as usize;
            if w==0||h==0||w>8192||h>8192||w.checked_mul(h).unwrap_or(usize::MAX)>16_000_000{
                return Err(Error::new(ErrorKind::InvalidInput,"page dimensions exceed PNG preview limit"));
            }
            let mut pixels=vec![255u8;w*h*3];
            for cell in &page.cells{
                let r=cell.region;
                let left=(r.x*scale).floor().max(0.0) as usize;
                let top=(r.y*scale).floor().max(0.0) as usize;
                let right=(r.right()*scale).ceil().max(0.0) as usize;
                let bottom=(r.bottom()*scale).ceil().max(0.0) as usize;
                for (x,y) in (left..right.min(w)).map(|x|(x,top))
                    .chain((left..right.min(w)).map(|x|(x,bottom.saturating_sub(1))))
                    .chain((top..bottom.min(h)).map(|y|(left,y)))
                    .chain((top..bottom.min(h)).map(|y|(right.saturating_sub(1),y))){
                    if x<w&&y<h{
                        let at=(y*w+x)*3;pixels[at..at+3].copy_from_slice(&[160,160,160]);
                    }
                }
            }
            for text in page.text.iter().chain(page.headers.iter()).chain(page.footers.iter()).chain(page.footnotes.iter()){
                let pt=(text.size_pt*scale).round().clamp(1.0,300.0) as u16;
                let mut pen=(text.region.x*scale).round() as i32;
                let top=(text.region.y*scale).round() as i32;
                for ch in text.text.chars(){
                    let Ok(bitmap)=font.character(pt,ch)else{
                        pen+=(text.size_pt*scale*0.5) as i32;continue;
                    };
                    let at_x=pen+bitmap.bearing_x;
                    let at_y=top+bitmap.baseline as i32-bitmap.bearing_y;
                    let gw=bitmap.width as usize;
                    let gh=bitmap.height as usize;
                    if gw.checked_mul(gh).map(|n|n<=bitmap.pixels.len())==Some(true){
                        for yy in 0..gh{
                            let y=at_y+yy as i32;
                            if y<0||y>=h as i32{continue;}
                            for xx in 0..gw{
                                let x=at_x+xx as i32;
                                if x<0||x>=w as i32{continue;}
                                let opacity=(bitmap.pixels[yy*gw+xx]&255) as u16;
                                if opacity==0{continue;}
                                let dest=(y as usize*w+x as usize)*3;
                                for c in 0..3{
                                    pixels[dest+c]=((pixels[dest+c] as u16*(255-opacity)+127)/255) as u8;
                                }
                            }
                        }
                    }
                    pen+=bitmap.advance_width as i32;
                }
            }
            output.push(png_rgb(w,h,&pixels)?);
        }
        Ok(output)
    }
}

/// Convert an OOXML RGB hex color to three channels without allocating.
fn parse_rgb(rgb:&str)->Option<[u8;3]>{
    let s=rgb.strip_prefix('#').unwrap_or(rgb);
    if s.len()!=6{return None;}
    let r=u8::from_str_radix(s.get(0..2)?,16).ok()?;
    let g=u8::from_str_radix(s.get(2..4)?,16).ok()?;
    let b=u8::from_str_radix(s.get(4..6)?,16).ok()?;
    Some([r,g,b])
}
fn visible_cell_border(border:Option<&crate::model::Border>)->Option<([u8;3],f32)>{
    let b=border?;
    if b.style=="none"||b.style=="nil"{return None;}
    Some((parse_rgb(&b.color).unwrap_or([150,150,150]),b.size.max(1) as f32/8.0))
}
impl Document {
    /// Rasterize laid-out text using shaped OpenType glyph IDs rather than
    /// one Unicode scalar at a time. This supports GSUB ligatures, interpolated
    /// TrueType/CFF2 outlines and COLR v0/CPAL layer colors through `Engine`.
    ///
    /// The layout is Word-inspired but not Word-identical; drawing objects,
    /// color bitmap fonts and COLR v1 paint graphs require additional renderers.
    pub fn to_png_pages_shaped(&self,engine:&crate::layout::Engine,dpi:f32)->io::Result<Vec<Vec<u8>>>{
        if !dpi.is_finite()||!(36.0..=300.0).contains(&dpi){
            return Err(Error::new(ErrorKind::InvalidInput,"PNG DPI must be 36..=300"));
        }
        let scale=dpi/72.0;
        let layout=engine.paginate(self)?;
        let mut result=Vec::with_capacity(layout.pages.len());
        for page in &layout.pages {
            let width=(page.width*scale).ceil() as usize;
            let height=(page.height*scale).ceil() as usize;
            if width==0||height==0||width>8192||height>8192||
                width.checked_mul(height).map(|n|n>16_000_000).unwrap_or(true){
                return Err(Error::new(ErrorKind::InvalidInput,"page exceeds shaped PNG raster limits"));
            }
            let mut pixels=vec![255u8;width*height*3];
            // Fill all cells before drawing borders, so a neighbour's
            // shading cannot erase a shared edge.
            for cell in &page.cells {
                if let Some(color)=cell.shading.as_deref().and_then(parse_rgb){
                    let r=cell.region;
                    let x0=(r.x*scale).floor().max(0.0) as usize;
                    let y0=(r.y*scale).floor().max(0.0) as usize;
                    let x1=(r.right()*scale).ceil().max(0.0) as usize;
                    let y1=(r.bottom()*scale).ceil().max(0.0) as usize;
                    for y in y0.min(height)..y1.min(height){
                        for x in x0.min(width)..x1.min(width){
                            let offset=(y*width+x)*3;pixels[offset..offset+3].copy_from_slice(&color);
                        }
                    }
                }
            }
            for cell in &page.cells {
                let r=cell.region;
                let x0=(r.x*scale).floor() as i32;let x1=(r.right()*scale).ceil() as i32-1;
                let y0=(r.y*scale).floor() as i32;let y1=(r.bottom()*scale).ceil() as i32-1;
                for (side,border) in cell.edges.iter().enumerate(){
                    let Some((color,thickness))=visible_cell_border(border.as_ref()) else {continue};
                    let t=(thickness*scale).round().clamp(1.,24.) as i32;
                    let (left,top,right,bottom)=match side{
                        0=>(x0,y0,x1,y0+t-1),1=>(x0,y1-t+1,x1,y1),
                        2=>(x0,y0,x0+t-1,y1),_=>(x1-t+1,y0,x1,y1),
                    };
                    for y in top.max(0)..=bottom.min(height as i32-1){
                        for x in left.max(0)..=right.min(width as i32-1){
                            let at=(y as usize*width+x as usize)*3;
                            pixels[at..at+3].copy_from_slice(&color);
                        }
                    }
                }
            }
            for run in page.text.iter().chain(page.headers.iter()).chain(page.footers.iter()).chain(page.footnotes.iter()){
                let size=run.size_pt.max(1.0);
                let shaped=engine.fonts.shape(&run.text,run.font.as_deref(),crate::opentype::ShapeOptions{
                    font_size_pt:size,weight:if run.bold{700}else{400},italic:run.italic,
                    ..crate::opentype::ShapeOptions::default()
                });
                let mut pen=(run.region.x*scale).round();
                let line_top=(run.region.y*scale).round();
                for glyph in shaped.glyphs{
                    let advance=glyph.advance_pt*scale;
                    let Some(face)=engine.fonts.faces().get(glyph.face_index)else{pen+=advance;continue};
                    let ppem=size*scale;
                    let (ascent,_,_)=face.vertical_metrics_instance();
                    let baseline=line_top+ascent/face.units_per_em as f32*ppem;
                    let base_x=(pen+glyph.x_offset_pt*scale).round() as i32;
                    let base_y=(baseline+glyph.y_offset_pt*scale).round() as i32;
                    if let Ok(Some(bitmap))=face.rasterize_colr_glyph_instance(glyph.glyph_id,ppem,0,0x000000ff){
                        let left=base_x+bitmap.bearing_x;let top=base_y-bitmap.bearing_y;
                        for y in 0..bitmap.height as usize{for x in 0..bitmap.width as usize{
                            let px=left+x as i32;let py=top+y as i32;
                            if px<0||py<0||px as usize>=width||py as usize>=height{continue;}
                            let color=bitmap.pixels[y*bitmap.width as usize+x];
                            let alpha=color&255;if alpha==0{continue;}
                            let at=(py as usize*width+px as usize)*3;
                            for (channel,shift) in [24,16,8].iter().enumerate(){
                                let src=(color>>*shift)&255;
                                let dst=pixels[at+channel] as u32;
                                pixels[at+channel]=((src*alpha+dst*(255-alpha)+127)/255) as u8;
                            }
                        }}
                    }else if let Ok(bitmap)=face.rasterize_glyph_instance(glyph.glyph_id,ppem){
                        let left=base_x+bitmap.bearing_x;let top=base_y-bitmap.bearing_y;
                        for y in 0..bitmap.height as usize{for x in 0..bitmap.width as usize{
                            let px=left+x as i32;let py=top+y as i32;
                            if px<0||py<0||px as usize>=width||py as usize>=height{continue;}
                            let alpha=bitmap.pixels[y*bitmap.width as usize+x].min(255);
                            let at=(py as usize*width+px as usize)*3;
                            for c in 0..3{pixels[at+c]=((pixels[at+c] as u32*(255-alpha)+127)/255) as u8;}
                        }}
                    }
                    pen+=advance;
                }
            }
            result.push(png_rgb(width,height,&pixels)?);
        }
        Ok(result)
    }
}
