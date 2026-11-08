use mata_docx::{Document,package::Package};
use std::{env,fs,io};
fn usage(){eprintln!("Usage:\n  mata-docx inspect FILE.docx\n  mata-docx text FILE.docx\n  mata-docx validate FILE.docx\n  mata-docx repair FILE.docx OUTPUT.docx\n  mata-docx signatures FILE.docx\n  mata-docx convert FILE.docx OUTPUT.(docx|pdf|html|md|png)\n  mata-docx replace FILE.docx FIND REPLACE OUTPUT.docx\n  mata-docx diff FILE1.docx FILE2.docx");}
fn main()->io::Result<()> {
    let args=env::args().skip(1).collect::<Vec<_>>();if args.is_empty(){usage();return Ok(());}
    match (args[0].as_str(),args.len()) {
        ("inspect",2)=>{let d=Document::open(&args[1])?;println!("Title: {}\nParagraphs: {}\nTables: {}\nImages: {}\nFootnotes: {}\nComments: {}\nProtected: {}",d.properties.title,d.paragraphs().count(),d.tables().count(),d.images.len(),d.footnotes.len(),d.comments.len(),d.is_protected());}
        ("text",2)=>{println!("{}",Document::open(&args[1])?.text());}
        ("validate",2)=>{
            let bytes=fs::read(&args[1])?;
            let package=Package::from_bytes(&bytes)?;
            package.validate()?;
            let d=Document::from_bytes(&bytes)?;
            println!("OK: {} blocks, {} package parts",d.blocks.len(),package.parts.len());
        }
        ("repair",3)=>{
            let mut package=Package::from_bytes(&fs::read(&args[1])?)?;
            let fixed=package.repair_dangling_relationships()?;
            if fixed>0{package.invalidate_signatures();}
            fs::write(&args[2],package.to_bytes()?)?;
            println!("Repaired {fixed} dangling relationships");
        }
        ("signatures",2)=>{
            let package=Package::from_bytes(&fs::read(&args[1])?)?;
            for part in package.signed_parts(){println!("{part}");}
        }
        ("convert",3)=>{let d=Document::open(&args[1])?;let filename=&args[2];match filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str(){"docx"=>d.save(filename)?,"pdf"=>d.save_pdf(filename)?,"html"=>d.save_html(filename)?,"md"=>d.save_markdown(filename)?,"png"=>d.save_png(filename)?,_=>return Err(io::Error::new(io::ErrorKind::InvalidInput,"unsupported output extension"))};println!("Wrote {filename}");}
        ("replace",5)=>{let mut d=Document::open(&args[1])?;let n=d.replace_text(&args[2],&args[3]);d.save(&args[4])?;println!("Replaced {n} run-local occurrence(s)");}
        ("diff",3)=>{let a=Document::open(&args[1])?.text();let b=Document::open(&args[2])?.text();let la=a.lines().collect::<Vec<_>>();let lb=b.lines().collect::<Vec<_>>();let n=la.len().max(lb.len());for i in 0..n{if la.get(i)!=lb.get(i){if let Some(s)=la.get(i){println!("- {s}");}if let Some(s)=lb.get(i){println!("+ {s}");}}}}
        _=>usage()
    }
    Ok(())
}
