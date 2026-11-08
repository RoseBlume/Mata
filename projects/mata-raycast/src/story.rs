//! Complete, compiled-in campaign manuscript and dependency-free reader helpers.
//! The original `Story.md` is embedded at build time: installed games never need
//! to find the source tree or read a file before displaying the story.

pub const MANUSCRIPT: &str = include_str!("../Story.md");
pub const CHAPTER_TITLES: [&str; 9] = [
    "PROLOGUE: THE VOICE BEFORE THE CALL",
    "ACT I: OUTPOST ZERO",
    "ACT II: BLACKOUT REFINERY",
    "ACT III: THE IRON VAULT",
    "ACT IV: ZERO HOUR",
    "ACT V: THE LAST TRANSMISSION",
    "EPILOGUE: WHAT THE LIGHT COULD NOT KEEP",
    "BEFORE THE SILENCE: FIVE RECOVERED SCENES",
    "EVIDENCE INDEX AND CONTINUITY",
];
const MARKERS: [&str; 9] = [
    "# PROLOGUE — THE VOICE BEFORE THE CALL",
    "# ACT I — OUTPOST ZERO",
    "# ACT II — BLACKOUT REFINERY",
    "# ACT III — THE IRON VAULT",
    "# ACT IV — ZERO HOUR",
    "# ACT V — THE LAST TRANSMISSION",
    "# EPILOGUE — WHAT THE LIGHT COULD NOT KEEP",
    "# BEFORE THE SILENCE",
    "# EVIDENCE INDEX AND CONTINUITY",
];

/// Slice the long document by its top-level headings, not by arbitrary bytes.
/// All returned strings borrow the single immutable built-in manuscript.
pub fn chapter(index: usize) -> &'static str {
    let Some(marker) = MARKERS.get(index) else { return ""; };
    let Some(start) = MANUSCRIPT.find(marker) else { return ""; };
    let content_start = start + marker.len();
    let end = MARKERS.get(index + 1)
        .and_then(|next| MANUSCRIPT[content_start..].find(next))
        .map_or(MANUSCRIPT.len(), |offset| content_start + offset);
    MANUSCRIPT[content_start..end].trim()
}

/// Convert markdown paragraphs into narrow, framebuffer-friendly text lines.
/// Single newlines in ordinary prose belong to the same paragraph; empty lines
/// delimit paragraphs. Headings and archive excerpts keep visible separation.
/// This is deliberately small and std-only: no Markdown or text layout crate.
pub fn wrapped_lines(index: usize, max_columns: usize) -> Vec<String> {
    let width = max_columns.max(18);
    let mut result = Vec::new();
    let mut paragraph = String::new();
    let flush = |paragraph: &mut String, result: &mut Vec<String>| {
        if paragraph.is_empty() { return; }
        let mut row = String::new();
        for word in paragraph.split_whitespace() {
            let word_len = word.chars().count();
            let row_len = row.chars().count();
            if !row.is_empty() && row_len + 1 + word_len > width {
                result.push(std::mem::take(&mut row));
            }
            if !row.is_empty() { row.push(' '); }
            if word_len > width {
                for ch in word.chars() {
                    if row.chars().count() == width {
                        result.push(std::mem::take(&mut row));
                    }
                    row.push(ch);
                }
            } else {
                row.push_str(word);
            }
        }
        if !row.is_empty() { result.push(row); }
        paragraph.clear();
    };
    for raw in chapter(index).lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == "---" {
            flush(&mut paragraph, &mut result);
            if result.last().is_some_and(|s| !s.is_empty()) { result.push(String::new()); }
            continue;
        }
        let heading = trimmed.starts_with('#');
        let block = trimmed.starts_with('>') || trimmed.starts_with("- ");
        if heading || block { flush(&mut paragraph, &mut result); }
        // Preserve exact dialogue and punctuation while removing only simple
        // Markdown delimiters that the framebuffer font cannot style.
        let clean = trimmed.trim_start_matches('#').trim_start_matches('>').trim();
        let clean: String = clean.chars().filter(|c| *c != '*' && *c != '`').collect();
        if heading {
            paragraph.push_str(&clean.to_uppercase());
        } else {
            if !paragraph.is_empty() { paragraph.push(' '); }
            paragraph.push_str(&clean);
        }
        if heading || block {
            flush(&mut paragraph, &mut result);
            result.push(String::new());
        }
    }
    flush(&mut paragraph, &mut result);
    while result.last().is_some_and(|line| line.is_empty()) { result.pop(); }
    if result.is_empty() { result.push("No manuscript text available.".into()); }
    result
}


/// Opening image from an act, used in the deployment briefing before any
/// spoiler-bearing chapter pages have been unlocked.
pub fn opening_lines(act_chapter: usize, width: usize, count: usize) -> Vec<String> {
    let mut narrative = false;
    let mut opening = String::new();
    for line in chapter(act_chapter).lines() {
        let t = line.trim();
        if t.starts_with("**Objective:") { narrative = true; continue; }
        if narrative && !t.is_empty() && !t.starts_with('#') {
            opening = t.replace('*', "");
            break;
        }
    }
    let mut result = Vec::new();
    let mut row = String::new();
    for word in opening.split_whitespace() {
        if !row.is_empty() && row.chars().count()+1+word.chars().count()>width {
            result.push(std::mem::take(&mut row));
            if result.len()==count { break; }
        }
        if !row.is_empty() {row.push(' ');}
        row.push_str(word);
    }
    if result.len()<count && !row.is_empty() {result.push(row);}
    result
}

pub fn page_count(lines: usize, lines_per_page: usize) -> usize {
    lines.div_ceil(lines_per_page.max(1)).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn every_act_contains_its_original_prose() {
        for i in 0..CHAPTER_TITLES.len() {
            assert!(chapter(i).len() > 700, "missing or short chapter {i}");
            let lines = wrapped_lines(i, 60);
            assert!(lines.len() > 12, "story chapter was not paginated");
            assert!(lines.iter().all(|s| s.chars().count() <= 60), "line overflow in {i}");
        }
        assert!(chapter(5).contains("Archivist"));
        assert!(chapter(6).contains("Corin"));
        assert!(chapter(7).contains("Mara"));
    }
}
