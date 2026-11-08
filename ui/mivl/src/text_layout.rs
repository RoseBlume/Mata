//! Shared, dependency-free line breaking used by headless sizing and Mata rendering.
//! Font backends supply *advance widths*, keeping the wrap decision and painted
//! glyph origins consistent. Only UTF-8 character boundaries are ever split.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WrapMode { None, Word, Anywhere, WordOnly }

impl WrapMode {
    pub(crate) fn from_properties(wrap: bool, name: &str) -> Self {
        match name {
            "none" | "noWrap" => Self::None,
            "anywhere" | "char" => Self::Anywhere,
            "wordOnly" => Self::WordOnly,
            "word" | "wordOrAnywhere" => Self::Word,
            _ => if wrap {Self::Word} else {Self::None},
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextLine { pub text: String, pub width: u32 }
#[derive(Debug, Clone)]
pub(crate) struct TextLayout {
    pub lines: Vec<TextLine>,
    pub width: u32,
    pub height: u32,
    pub line_height: u32,
    pub truncated: bool,
}

fn line_width(chars:&[(char,u32)]) -> u32 {
    chars.iter().fold(0u32,|sum,(_,advance)|sum.saturating_add(*advance))
}
fn finish_line(chars:&[(char,u32)], width: u32) -> TextLine {
    TextLine {text:chars.iter().map(|(ch,_)|*ch).collect(),width}
}

/// `max_width` is the inner text content width, excluding padding/borders.
/// A `None` width yields intrinsic unwrapped size (even when wrapping is on).
/// `max_lines == None` means unlimited. `ellipsis` adds a single U+2026
/// to the last visible line when lines are suppressed.
pub(crate) fn layout_text(
    text: &str, max_width: Option<u32>, mode: WrapMode,
    line_height: u32, max_lines: Option<usize>, ellipsis: bool,
    mut advance: impl FnMut(char) -> u32,
) -> TextLayout {
    let step = line_height.max(1);
    let limit = if mode == WrapMode::None {None} else {max_width.map(|x|x.max(1))};
    let mut lines: Vec<TextLine> = Vec::new();
    let mut current: Vec<(char,u32)> = Vec::new();
    let mut width: u32 = 0;
    // Remember a whitespace break position (spaces/tabs are omitted at the
    // boundary; explicit newlines are never collapsed).
    let mut break_at: Option<usize> = None;
    let normal = text.replace("\r\n","\n").replace('\r',"\n").replace('\t',"    ");
    for ch in normal.chars() {
        if ch == '\n' {
            lines.push(finish_line(&current,width));
            current.clear();width=0;break_at=None;
            continue;
        }
        let w=advance(ch);
        if let Some(max) = limit {
            // Only break if there is already something to break. A single
            // oversized glyph may overflow; the native raster clips it.
            if !current.is_empty() && width.saturating_add(w)>max {
                match (mode,break_at) {
                    (WrapMode::Word|WrapMode::WordOnly,Some(at)) => {
                        let mut before=current[..at].to_vec();
                        while before.last().is_some_and(|(c,_)|c.is_whitespace()) {before.pop();}
                        let before_width=line_width(&before);
                        lines.push(finish_line(&before,before_width));
                        let mut remainder=current[at..].to_vec();
                        while remainder.first().is_some_and(|(c,_)|c.is_whitespace()) {remainder.remove(0);}
                        current=remainder;
                        width=line_width(&current);
                    }
                    (WrapMode::WordOnly,_) => {},
                    _ => {
                        lines.push(finish_line(&current,width));
                        current.clear();width=0;
                    }
                }
                break_at=current.iter().rposition(|(c,_)|c.is_whitespace()).map(|i|i+1);
                // In word mode, a long token falls back to character splitting.
                if mode == WrapMode::Word && width.saturating_add(w)>max && !current.is_empty() {
                    lines.push(finish_line(&current,width));
                    current.clear();width=0;break_at=None;
                }
            }
        }
        current.push((ch,w));
        width=width.saturating_add(w);
        if ch.is_whitespace() {break_at=Some(current.len());}
    }
    lines.push(finish_line(&current,width));
    let mut truncated=false;
    if let Some(cap)=max_lines.filter(|n|*n>0) {
        if lines.len()>cap {lines.truncate(cap);truncated=true;}
    }
    if ellipsis && !lines.is_empty() {
        let last=lines.last_mut().expect("nonempty lines");
        let available=max_width.unwrap_or(u32::MAX);
        // Ellipsize both vertically suppressed lines and a single line
        // that exceeds a fixed width with wrapping disabled.
        if truncated || last.width>available {
            truncated=true;
            let mark='…'; let mark_width=advance(mark);
            let mut chars:Vec<(char,u32)>=last.text.chars().map(|ch|(ch,advance(ch))).collect();
            let mut used=line_width(&chars);
            while !chars.is_empty() && used.saturating_add(mark_width)>available {
                if let Some((_,removed))=chars.pop(){used=used.saturating_sub(removed);}
            }
            if mark_width<=available {
                last.text=chars.iter().map(|(c,_)|*c).chain(std::iter::once(mark)).collect();
                last.width=used.saturating_add(mark_width);
            } else {last.text.clear();last.width=0;}
        }
    }
    let max_width=lines.iter().map(|line|line.width).max().unwrap_or(0);
    let h=step.saturating_mul(lines.len().min(u32::MAX as usize) as u32);
    TextLayout {lines,width:max_width,height:h,line_height:step,truncated}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn layout(s:&str,w:Option<u32>,m:WrapMode)->TextLayout {
        layout_text(s,w,m,10,None,false,|c|if c==' ' {4}else{8})
    }
    #[test] fn hard_breaks_and_empty_lines() {
        let out=layout("a\n\nxyz\r\n",None,WrapMode::Word);
        assert_eq!(out.lines.iter().map(|l|l.text.as_str()).collect::<Vec<_>>(),vec!["a","","xyz",""]);
        assert_eq!(out.height,40);
    }
    #[test] fn wraps_words_and_long_words() {
        let out=layout("hello world",Some(44),WrapMode::Word);
        assert_eq!(out.lines.iter().map(|l|l.text.as_str()).collect::<Vec<_>>(),vec!["hello","world"]);
        let out=layout("abcdefgh",Some(24),WrapMode::Word);
        assert_eq!(out.lines.iter().map(|l|l.text.as_str()).collect::<Vec<_>>(),vec!["abc","def","gh"]);
    }
    #[test] fn no_wrap_keeps_wide_line() {
        assert_eq!(layout("abcdefgh",Some(10),WrapMode::None).lines.len(),1);
    }
    #[test] fn unicode_stays_valid() {
        let out=layout("猫猫猫猫",Some(16),WrapMode::Anywhere);
        assert_eq!(out.lines.iter().map(|l|l.text.as_str()).collect::<Vec<_>>(),vec!["猫猫","猫猫"]);
    }
    #[test] fn single_line_ellipsis_respects_width() {
        let x=layout_text("abcdefgh",Some(24),WrapMode::None,10,None,true,|_|8);
        assert!(x.truncated);
        assert_eq!(x.lines[0].text,"ab…");
    }
    #[test] fn max_lines_and_ellipsis() {
        let out=layout_text("one two three",Some(28),WrapMode::Word,12,Some(2),true,|c|if c==' ' {4}else{8});
        assert!(out.truncated);
        assert_eq!(out.lines.len(),2);
        assert!(out.lines[1].text.ends_with('…'));
        assert!(out.lines[1].width<=28);
    }
}
