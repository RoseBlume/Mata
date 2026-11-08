//! TeX-pattern dictionary hyphenation with UTF-8-safe output indices.
//! The library does not bundle language-specific word lists; applications
//! select a dictionary whose license permits their intended use.
use std::collections::BTreeMap;
use std::io::{self,Error,ErrorKind};

#[derive(Clone,Debug)]struct Pattern{letters:Vec<char>,weights:Vec<u8>}
#[derive(Clone,Debug)]pub struct Hyphenator{
    patterns:Vec<Pattern>,exceptions:BTreeMap<String,Vec<usize>>,
    pub minimum_left:usize,pub minimum_right:usize,
}
impl Default for Hyphenator{
    fn default()->Self{Self{patterns:Vec::new(),exceptions:BTreeMap::new(),minimum_left:2,minimum_right:2}}
}
impl Hyphenator{
    pub fn new()->Self{Self::default()}
    /// Accept TeX pattern tokens such as `a1bc`, `2tion`, `.ab4`.
    /// Comments beginning with `%` are ignored until newline.
    pub fn load_patterns(&mut self,source:&str)->io::Result<usize>{
        let mut count=0;
        for line in source.lines(){
            let content=line.split('%').next().unwrap_or("");
            for token in content.split_whitespace(){
                if token.starts_with('\\') || token.starts_with('{') || token.starts_with('}') {continue;}
                let mut letters=Vec::new();let mut weights=vec![0u8];
                for ch in token.chars(){
                    if let Some(digit)=ch.to_digit(10).filter(|_|ch.is_ascii_digit()){
                        if let Some(last)=weights.last_mut(){*last=digit as u8;}
                    }else{
                        if ch.is_control(){return Err(Error::new(ErrorKind::InvalidData,"control character in hyphenation pattern"));}
                        letters.push(ch);weights.push(0);
                    }
                }
                if letters.is_empty(){continue;}
                if letters.len()>100{return Err(Error::new(ErrorKind::InvalidData,"hyphenation pattern too long"));}
                self.patterns.push(Pattern{letters,weights});count+=1;
            }
        }
        Ok(count)
    }
    /// Add an exception spelled with `-` at permitted break positions.
    pub fn add_exception(&mut self,marked:&str){
        let mut word=String::new();let mut positions=Vec::new();
        for c in marked.chars(){if c=='-'{positions.push(word.chars().count());}else{word.push(c);}}
        self.exceptions.insert(word.to_lowercase(),positions);
    }
    /// Return discretionary break positions, as UTF-8 byte offsets within
    /// `word`. These always lie on valid Unicode scalar boundaries.
    pub fn breaks(&self,word:&str)->Vec<usize>{
        let chars=word.chars().collect::<Vec<_>>();let n=chars.len();
        if n<self.minimum_left+self.minimum_right+1{return Vec::new();}
        let lowered=word.to_lowercase();
        if let Some(points)=self.exceptions.get(&lowered){
            let offsets=word.char_indices().map(|(i,_)|i).chain(std::iter::once(word.len())).collect::<Vec<_>>();
            return points.iter().filter(|&&i|i>=self.minimum_left&&n-i>=self.minimum_right).filter_map(|&i|offsets.get(i).copied()).collect();
        }
        let mut padded=Vec::with_capacity(n+2);padded.push('.');padded.extend(lowered.chars());padded.push('.');
        if padded.len()!=n+2{return Vec::new();} // Case-fold expansions need language tailoring.
        let mut weights=vec![0u8;padded.len()+1];
        for pattern in &self.patterns{
            if pattern.letters.len()>padded.len(){continue;}
            for i in 0..=padded.len()-pattern.letters.len(){
                if padded[i..i+pattern.letters.len()]==pattern.letters[..]{
                    for (j,&weight) in pattern.weights.iter().enumerate(){weights[i+j]=weights[i+j].max(weight);}
                }
            }
        }
        let offsets=word.char_indices().map(|(i,_)|i).chain(std::iter::once(word.len())).collect::<Vec<_>>();
        (self.minimum_left..=n-self.minimum_right).filter(|&i|weights[i+1]%2==1).map(|i|offsets[i]).collect()
    }
}
