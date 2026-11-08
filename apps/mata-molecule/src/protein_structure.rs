//! Opt-in download of a **predicted**, accession-matched AlphaFold protein model.
//! Uses the OS curl executable (no extra Rust dependencies). The API supplies
//! the current versioned PDB URL, rather than constructing a fragile v4 URL.
//! Download is run on a worker thread by the caller; never block the UI loop.
use mata_molecule::{Molecule, read_structures};
use std::{fs, path::Path, process::Command};

const MAX_PDB: u64 = 96 * 1024 * 1024;
const MAX_METADATA: u64 = 2 * 1024 * 1024;

pub fn valid_accession(id: &str) -> bool {
    !id.is_empty() && id.len() <= 20 && id.bytes().all(|c|c.is_ascii_alphanumeric() || c == b'-')
}

// Read one JSON string field without relying on `serde_json`. This does not
// attempt to parse arbitrary JSON; it extracts only a URI whose alphabet is
// restricted further by `allowed_url` before performing an HTTP request.
fn json_field(src: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let at = src.find(&needle)? + needle.len();
    let bytes = src.as_bytes();
    let mut i = at;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {i += 1;}
    if bytes.get(i) != Some(&b':') {return None;}
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {i += 1;}
    if bytes.get(i) != Some(&b'"') {return None;}
    i += 1;
    let start = i;
    while i < bytes.len() {
        if bytes[i] == b'\\' {return None;}
        if bytes[i] == b'"' {return Some(src[start..i].to_owned());}
        if bytes[i] < 32 {return None;}
        i += 1;
    }
    None
}

fn allowed_url(url: &str) -> bool {
    (url.starts_with("https://alphafold.ebi.ac.uk/files/") ||
        url.starts_with("https://ftp.ebi.ac.uk/pub/databases/alphafold/")) &&
    url.len() < 512 &&
    url.bytes().all(|b|b.is_ascii_alphanumeric() || b"-._~:/%+".contains(&b)) &&
    (url.ends_with(".pdb") || url.ends_with(".cif"))
}

fn fetch_file(url: &str, output: &Path, max_bytes: u64) -> Result<(),String> {
    let result = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--max-redirs", "4",
            "--connect-timeout", "12", "--max-time", "120"])
        .arg("--max-filesize").arg(max_bytes.to_string())
        .arg("--output").arg(output).arg("--").arg(url)
        .output().map_err(|e|format!("curl is required for protein downloads ({e}); alternatively open a local PDB/mmCIF file"))?;
    if !result.status.success() {
        return Err(format!("AlphaFold download failed: {}",String::from_utf8_lossy(&result.stderr).trim()));
    }
    let size = fs::metadata(output).map_err(|e|e.to_string())?.len();
    if size < 100 || size > max_bytes {
        return Err(format!("AlphaFold response has an unexpected size ({size} bytes)"));
    }
    Ok(())
}

fn matching_record<'a>(json: &'a str, accession: &str) -> Option<&'a str> {
    // The API returns an array. Limit string-search scope to the object with
    // the exact UniProt accession; avoid blindly selecting an isoform.
    for chunk in json.split('{').skip(1) {
        let object = chunk.split('}').next()?;
        if json_field(object, "uniprotAccession").as_deref() == Some(accession) ||
           json_field(object, "uniprotId").as_deref() == Some(accession) {
            return Some(object);
        }
    }
    None
}

pub fn fetch_prediction(accession: &str) -> Result<(Vec<Molecule>,String),String> {
    if !valid_accession(accession) {return Err("Invalid UniProt accession: use only letters, digits, and hyphens".into());}
    let cache = std::env::temp_dir().join("mata-molecule-alphafold");
    fs::create_dir_all(&cache).map_err(|e|format!("Create model cache: {e}"))?;
    let target = cache.join(format!("AF-{accession}.pdb"));
    // A previously verified downloaded model is safe to reuse offline.
    if target.is_file() {
        if let Ok(structures) = read_structures(&target) {
            if !structures.is_empty() {return Ok((structures,format!("AlphaFold predicted model ({accession}, cached)")));}
        }
    }
    let metadata_path = cache.join(format!("AF-{accession}-{}.json",std::process::id()));
    let api = format!("https://alphafold.ebi.ac.uk/api/prediction/{accession}");
    let metadata = (|| {
        fetch_file(&api, &metadata_path, MAX_METADATA)?;
        fs::read_to_string(&metadata_path).map_err(|e|format!("Read AlphaFold API response: {e}"))
    })();
    let _ = fs::remove_file(&metadata_path);
    let metadata = metadata?;
    let record = matching_record(&metadata, accession).ok_or_else(||format!("AlphaFold API has no prediction for {accession}; open a PDB/mmCIF file instead"))?;
    let pdb = json_field(record,"pdbUrl").filter(|u|allowed_url(u));
    let cif = json_field(record,"cifUrl").filter(|u|allowed_url(u));
    let (url,ext) = if let Some(pdb) = pdb { (pdb,"pdb") }
        else if let Some(cif) = cif { (cif,"cif") }
        else {return Err("AlphaFold API did not provide a supported model URL".into())};
    let staging = cache.join(format!("AF-{accession}-{}.download.{ext}",std::process::id()));
    let final_path = if ext == "pdb" {target} else {cache.join(format!("AF-{accession}.cif"))};
    let result = (|| {
        fetch_file(&url,&staging,MAX_PDB)?;
        // Parse before caching; this catches HTML error pages and corrupt data.
        let structures = read_structures(&staging)?;
        if structures.is_empty() || structures[0].atoms.is_empty() {
            return Err("Downloaded model contains no 3D atoms".into());
        }
        fs::rename(&staging,&final_path).or_else(|_|fs::copy(&staging,&final_path).map(|_|())).map_err(|e|format!("Cache predicted model: {e}"))?;
        Ok((structures,format!("AlphaFold predicted model for {accession} (not an experimentally measured structure)")))
    })();
    let _ = fs::remove_file(&staging);
    result
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn url_and_accession_validation() {
        assert!(valid_accession("P00533"));
        assert!(!valid_accession("../file"));
        assert!(!allowed_url("https://evil.example.com/model.pdb"));
        assert!(allowed_url("https://alphafold.ebi.ac.uk/files/AF-P00533-F1-model_v6.pdb"));
    }
    #[test] fn parse_api_metadata() {
        let json = r#"[{"uniprotAccession":"OTHER","pdbUrl":"https://evil.example.com/a.pdb"},{"uniprotAccession":"P00533","pdbUrl":"https://alphafold.ebi.ac.uk/files/AF-P00533-F1-model_v6.pdb"}]"#;
        let obj = matching_record(json,"P00533").unwrap();
        assert_eq!(json_field(obj,"pdbUrl").as_deref(),Some("https://alphafold.ebi.ac.uk/files/AF-P00533-F1-model_v6.pdb"));
    }
}
