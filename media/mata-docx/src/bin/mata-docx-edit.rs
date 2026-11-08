//! CLI for loss-minimizing edits of DOCX content using the current `Document` API.
//!
//! Usage: mata-docx-edit INPUT.docx OUTPUT.docx COMMAND [ARGS...]
//! The input and output paths must differ, and the output must not exist.
use mata_docx::{Block, Document, Length, Orientation, Paragraph};
use std::{env, fs, io::{self, ErrorKind, Write}, path::Path};

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, message.into())
}

fn required<'a>(args: &'a [String], index: usize, name: &str) -> io::Result<&'a str> {
    args.get(index).map(String::as_str).ok_or_else(|| invalid(format!("missing {name}")))
}

fn usize_arg(args: &[String], index: usize, name: &str) -> io::Result<usize> {
    required(args, index, name)?.parse().map_err(|_| invalid(format!("invalid {name}")))
}

fn u32_arg(args: &[String], index: usize, name: &str) -> io::Result<u32> {
    required(args, index, name)?.parse().map_err(|_| invalid(format!("invalid {name}")))
}

fn positive_emu(args: &[String], index: usize, name: &str) -> io::Result<Length> {
    let value: i64 = required(args, index, name)?
        .parse().map_err(|_| invalid(format!("invalid {name}")))?;
    if value <= 0 { return Err(invalid(format!("{name} must be positive"))); }
    Ok(Length::emu(value))
}

fn remainder(args: &[String], index: usize) -> String {
    args.get(index..).unwrap_or(&[]).join(" ")
}

fn reject_extra(args: &[String], expected: usize, command: &str) -> io::Result<()> {
    if args.len() != expected { return Err(invalid(format!("wrong number of arguments for {command}"))); }
    Ok(())
}

fn help() -> &'static str {
    "Usage: mata-docx-edit INPUT.docx OUTPUT.docx COMMAND [ARGS...]\n\
     Commands:\n\
       inspect\n\
       table-add ROWS COLUMNS\n\
       cell-set BLOCK ROW COLUMN TEXT...\n\
       cell-span BLOCK ROW COLUMN SPAN\n\
       image-add FILE ALT WIDTH_EMU HEIGHT_EMU\n\
       image-resize INDEX_OR_FILENAME WIDTH_EMU HEIGHT_EMU\n\
       section PAGE_WIDTH_TWIPS PAGE_HEIGHT_TWIPS MARGIN_TWIPS COLUMNS\n\
       header TEXT...\n\
       footer TEXT...\n\
       bookmark NAME\n\
       hyperlink LABEL URL\n\
       footnote TEXT...\n\
       endnote TEXT...\n\
       toc\n\
       bibliography\n\
       citation SOURCE_TAG DISPLAY...\n\
       cross-ref BOOKMARK DISPLAY...\n\
       paragraph TEXT...\n\
     INSPECT does not write OUTPUT. All other commands create a NEW file.\n"
}

fn table_mut(doc: &mut Document, block_index: usize) -> io::Result<&mut mata_docx::Table> {
    match doc.blocks.get_mut(block_index) {
        Some(Block::Table(table)) => Ok(table),
        Some(Block::Paragraph(_)) => Err(invalid("BLOCK points to a paragraph, not a table")),
        None => Err(invalid("BLOCK is out of range")),
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("{}", help());
        return Err(invalid("missing command"));
    }
    let input = Path::new(&args[1]);
    let output = Path::new(&args[2]);
    let command = args[3].as_str();
    let values = &args[4..];

    if input == output { return Err(invalid("output must differ from input")); }
    if let (Ok(a), Ok(b)) = (fs::canonicalize(input), fs::canonicalize(output)) {
        if a == b { return Err(invalid("output resolves to input")); }
    }

    let mut doc = Document::from_bytes(&fs::read(input)?)?;
    if command == "inspect" {
        reject_extra(values, 0, command)?;
        for (i, block) in doc.blocks.iter().enumerate() {
            match block {
                Block::Paragraph(p) => println!("{i}: Paragraph ({})", p.text()),
                Block::Table(t) => println!("{i}: Table ({} rows)", t.rows.len()),
            }
        }
        for section in &doc.sections {
            println!("Section ends after block {} ({:?})", section.end_after, section.break_kind);
        }
        println!("{} embedded images, {} footnotes, {} endnotes", doc.images.len(), doc.footnotes.len(), doc.endnotes.len());
        return Ok(());
    }
    if output.exists() {
        return Err(io::Error::new(ErrorKind::AlreadyExists, "output file already exists"));
    }

    match command {
        "table-add" => {
            reject_extra(values, 2, command)?;
            let rows = usize_arg(values, 0, "ROWS")?;
            let cols = usize_arg(values, 1, "COLUMNS")?;
            if rows == 0 || cols == 0 { return Err(invalid("table dimensions must be positive")); }
            doc.add_table(rows, cols);
        }
        "cell-set" => {
            if values.len() < 4 { return Err(invalid("cell-set BLOCK ROW COLUMN TEXT...")); }
            let (block, row, col) = (usize_arg(values, 0, "BLOCK")?, usize_arg(values, 1, "ROW")?, usize_arg(values, 2, "COLUMN")?);
            let table = table_mut(&mut doc, block)?;
            table.cell_mut(row, col).ok_or_else(|| invalid("cell coordinates out of range"))?
                .set_text(remainder(values, 3));
        }
        "cell-span" => {
            reject_extra(values, 4, command)?;
            let (block, row, col) = (usize_arg(values, 0, "BLOCK")?, usize_arg(values, 1, "ROW")?, usize_arg(values, 2, "COLUMN")?);
            let span = u32_arg(values, 3, "SPAN")?;
            let table = table_mut(&mut doc, block)?;
            if span == 0 { return Err(invalid("SPAN must be positive")); }
            let row_data = table.rows.get_mut(row).ok_or_else(|| invalid("ROW out of range"))?;
            let span = usize::try_from(span).map_err(|_| invalid("SPAN too large"))?;
            let end = col.checked_add(span).ok_or_else(|| invalid("SPAN too large"))?;
            if end > row_data.cells.len() { return Err(invalid("SPAN extends beyond row")); }
            // Existing cells covered by gridSpan must not also be emitted as
            // independent cells. Preserve their paragraph content on merge.
            let mut merged = row_data.cells.drain(col + 1..end).collect::<Vec<_>>();
            let target = row_data.cells.get_mut(col).ok_or_else(|| invalid("COLUMN out of range"))?;
            for cell in merged.drain(..) {
                if cell.text().trim().is_empty() { continue; }
                target.paragraphs.extend(cell.paragraphs);
            }
            target.colspan = u32::try_from(span).map_err(|_| invalid("SPAN too large"))?;
        }
        "image-add" => {
            reject_extra(values, 4, command)?;
            let path = Path::new(required(values, 0, "FILE")?);
            let alt = required(values, 1, "ALT")?;
            if !alt.is_empty() {
                eprintln!("warning: the current Image model does not preserve image alternative text");
            }
            let width = positive_emu(values, 2, "WIDTH_EMU")?;
            let height = positive_emu(values, 3, "HEIGHT_EMU")?;
            let filename = path.file_name().and_then(|v| v.to_str()).ok_or_else(|| invalid("FILE must have a UTF-8 filename"))?;
            let index = doc.add_image(filename, fs::read(path)?, width, height);
            doc.add_image_paragraph(index);
        }
        "image-resize" => {
            reject_extra(values, 3, command)?;
            let key = required(values, 0, "INDEX_OR_FILENAME")?;
            let width = positive_emu(values, 1, "WIDTH_EMU")?;
            let height = positive_emu(values, 2, "HEIGHT_EMU")?;
            let index = match key.parse::<usize>() {
                Ok(n) => n,
                Err(_) => doc.images.iter().position(|i| i.filename == key)
                    .ok_or_else(|| invalid("image ID/filename not found"))?,
            };
            let image = doc.images.get_mut(index).ok_or_else(|| invalid("image index out of range"))?;
            image.width = width;
            image.height = height;
        }
        "section" => {
            reject_extra(values, 4, command)?;
            let width = u32_arg(values, 0, "PAGE_WIDTH_TWIPS")?;
            let height = u32_arg(values, 1, "PAGE_HEIGHT_TWIPS")?;
            let margin = u32_arg(values, 2, "MARGIN_TWIPS")?;
            let columns = u32_arg(values, 3, "COLUMNS")?;
            if width == 0 || height == 0 || columns == 0 || columns > u16::MAX as u32 {
                return Err(invalid("invalid section dimensions or column count"));
            }
            let mut page = doc.page.clone();
            page.width_twips = width;
            page.height_twips = height;
            page.margin_top = margin;
            page.margin_bottom = margin;
            page.margin_left = margin;
            page.margin_right = margin;
            page.orientation = if width > height { Orientation::Landscape } else { Orientation::Portrait };
            doc.page = page.clone();
            let section = doc.final_section_mut();
            section.page = page;
            section.columns = columns as u16;
        }
        "header" => doc.header = vec![Paragraph::new(remainder(values, 0))],
        "footer" => doc.footer = vec![Paragraph::new(remainder(values, 0))],
        "bookmark" => {
            reject_extra(values, 1, command)?;
            doc.add_paragraph("").add_bookmark(required(values, 0, "NAME")?);
        }
        "hyperlink" => {
            reject_extra(values, 2, command)?;
            let label = required(values, 0, "LABEL")?;
            let url = required(values, 1, "URL")?;
            doc.add_paragraph("").add_hyperlink(url, label);
        }
        "footnote" => {
            let id = doc.add_footnote(remainder(values, 0));
            doc.add_paragraph("").add_footnote_ref(id);
        }
        "endnote" => {
            let id = doc.add_endnote(remainder(values, 0));
            doc.add_paragraph("").add_endnote_ref(id);
        }
        "toc" => {
            reject_extra(values, 0, command)?;
            doc.add_paragraph("").add_field(r#"TOC \o "1-3" \h \z \u"#);
        }
        "bibliography" => {
            reject_extra(values, 0, command)?;
            doc.add_paragraph("").add_field("BIBLIOGRAPHY");
        }
        "citation" => {
            if values.len() < 2 { return Err(invalid("citation SOURCE_TAG DISPLAY...")); }
            let tag = required(values, 0, "SOURCE_TAG")?;
            doc.add_paragraph("").add_complex_field(format!("CITATION {tag}"), remainder(values, 1));
        }
        "cross-ref" => {
            if values.len() < 2 { return Err(invalid("cross-ref BOOKMARK DISPLAY...")); }
            let bookmark = required(values, 0, "BOOKMARK")?;
            doc.add_paragraph("").add_complex_field(format!(r"REF {bookmark} \h"), remainder(values, 1));
        }
        "paragraph" => { doc.add_paragraph(remainder(values, 0)); }
        _ => { eprintln!("{}", help()); return Err(invalid("unknown command")); }
    }

    // `create_new` preserves the old editor's "never overwrite" contract,
    // even if a file appears between the existence check and the write.
    let data = doc.to_docx()?;
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(output)?;
    file.write_all(&data)?;
    println!("Saved {} bytes to {}", data.len(), output.display());
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("mata-docx-edit: {error}");
        std::process::exit(1);
    }
}
