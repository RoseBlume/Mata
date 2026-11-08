//! A dependency-free regular-expression engine for document search and replacement.
//!
//! Supports alternation, captures (numbered/named), noncapturing groups,
//! greedy/lazy quantifiers, character classes, assertions, lookahead, and
//! capture-aware replacement. This is a pure-Rust PCRE2-inspired engine,
//! *not* a complete implementation of PCRE2. The compiler rejects unsupported
//! special syntax instead of silently treating it as a match. Backtracking is
//! bounded by explicit step/state/recursion limits.
#![forbid(unsafe_code)]
use std::collections::BTreeMap;
use crate::casefold_data::FULL_FOLD;
use crate::pcre_script_data as script_data;
use std::io::{self, Error, ErrorKind};
fn err(s:&str)->io::Error{Error::new(ErrorKind::InvalidInput,s)}

#[derive(Debug,Clone,Copy,Default,PartialEq,Eq)]
enum Newline {#[default] Lf, Cr, CrLf, AnyCrLf, Any, Nul}
fn is_newline_char(ch:char,mode:Newline)->bool{
    match mode{
        Newline::Lf=>ch=='\n',Newline::Cr=>ch=='\r',
        Newline::CrLf|Newline::AnyCrLf=>matches!(ch,'\n'|'\r'),
        Newline::Any=>matches!(ch,'\n'|'\r'|'\x0b'|'\x0c'|'\u{0085}'|'\u{2028}'|'\u{2029}'),
        Newline::Nul=>ch=='\0',
    }
}
#[derive(Debug,Clone,Copy,Default,PartialEq,Eq)]
struct Flags {
    insensitive:bool, multiline:bool, dot_all:bool,
    extended:bool, ungreedy:bool, no_auto_capture:bool, ucp:bool, bsr_anycrlf:bool, duplicate_names:bool,
    newline:Newline,
    ascii_digit:bool,ascii_space:bool,ascii_word:bool,ascii_posix:bool,extra_extended:bool,restrict_case:bool,
    turkish_casing:bool,
}
#[derive(Debug,Clone)]enum Class {Char(char), Any, Horizontal, Vertical, ExceptNewline, Digit, Word, Space, NotDigit, NotWord, NotSpace,
    Property(String,bool),Posix(String,bool),Ranges{ranges:Vec<(char,char)>,negated:bool},
    Union{members:Vec<Class>,negated:bool},
    Intersection(Box<Class>,Box<Class>),Difference(Box<Class>,Box<Class>),
    SymmetricDifference(Box<Class>,Box<Class>),Complement(Box<Class>)}
impl Class {
    fn accepts(&self,ch:char,flags:Flags)->bool {
        match self {
            Self::Char(c)=>(!flags.restrict_case||c.is_ascii()==ch.is_ascii())&&eq_char(*c,ch,flags),
            Self::Intersection(a,b)=>a.accepts(ch,flags)&&b.accepts(ch,flags),
            Self::Difference(a,b)=>a.accepts(ch,flags)&&!b.accepts(ch,flags),
            Self::SymmetricDifference(a,b)=>a.accepts(ch,flags)!=b.accepts(ch,flags),
            Self::Complement(a)=>!a.accepts(ch,flags),
            Self::Horizontal=>matches!(ch,'\t'|' '|'\u{00a0}'|'\u{1680}'|'\u{180e}'|'\u{2000}'..='\u{200a}'|'\u{202f}'|'\u{205f}'|'\u{3000}'),
            Self::Vertical=>matches!(ch,'\n'|'\r'|'\u{000b}'|'\u{000c}'|'\u{0085}'|'\u{2028}'|'\u{2029}'),
            Self::ExceptNewline=>!is_newline_char(ch,flags.newline),
            Self::Any=>flags.dot_all||!is_newline_char(ch,flags.newline),
            Self::Digit=>if flags.ucp&&!flags.ascii_digit {ch.to_digit(10).is_some() || crate::unicode::matches_property(ch,"Nd")} else {ch.is_ascii_digit()},
            Self::Word=>if flags.ucp&&!flags.ascii_word {ch.is_alphanumeric() || ch=='_'} else {ch.is_ascii_alphanumeric() || ch=='_'},
            Self::Space=>if flags.ucp&&!flags.ascii_space {ch.is_whitespace()} else {matches!(ch,' '| '\t'|'\n'|'\r'|'\x0b'|'\x0c')},
            Self::NotDigit=>!Self::Digit.accepts(ch,flags),
            Self::NotWord=>!Self::Word.accepts(ch,flags),
            Self::NotSpace=>!Self::Space.accepts(ch,flags),
            Self::Property(name,neg)=>{
                let matches=crate::unicode::matches_property(ch,name);
                matches!=*neg
            },
            Self::Posix(name,neg)=>{
                let ucp=flags.ucp&&!flags.ascii_posix;
                let matched=match name.as_str(){
                    "alnum"=>if ucp {ch.is_alphanumeric()} else {ch.is_ascii_alphanumeric()},
                    "alpha"=>if ucp {ch.is_alphabetic()} else {ch.is_ascii_alphabetic()},
                    "ascii"=>ch.is_ascii(), "blank"=>matches!(ch,' '| '\t'),
                    "cntrl"=>if ucp {ch.is_control()} else {ch.is_ascii_control()},
                    "digit"=>ch.is_ascii_digit(),
                    "graph"=>if ucp {!ch.is_control()&&!ch.is_whitespace()} else {ch.is_ascii_graphic()},
                    "lower"=>if ucp {ch.is_lowercase()} else {ch.is_ascii_lowercase()},
                    "print"=>if ucp {!ch.is_control()} else {ch.is_ascii_graphic()||ch==' '},
                    "punct"=>ch.is_ascii_punctuation(),
                    "space"=>if ucp {ch.is_whitespace()} else {ch.is_ascii_whitespace()},
                    "upper"=>if ucp {ch.is_uppercase()} else {ch.is_ascii_uppercase()},
                    "word"=>Class::Word.accepts(ch,flags),
                    "xdigit"=>ch.is_ascii_hexdigit(),_=>false,
                };
                matched!=*neg
            },
            Self::Union{members,negated}=>members.iter().any(|m|m.accepts(ch,flags))!=*negated,
            Self::Ranges{ranges,negated}=>{
                let matches=ranges.iter().any(|&(lo,hi)|{
                    (lo<=ch&&ch<=hi) || (flags.insensitive && {
                        let lower=ch.to_lowercase().next().unwrap_or(ch);
                        let upper=ch.to_uppercase().next().unwrap_or(ch);
                        (lo<=lower&&lower<=hi)||(lo<=upper&&upper<=hi)
                    })
                });matches != *negated
            }
        }
    }
}
fn fold(c:char)->String {
    let key=c as u32;
    if let Ok(index)=FULL_FOLD.binary_search_by_key(&key,|(n,_)|*n){
        FULL_FOLD[index].1.to_owned()
    }else{c.to_string()}
}
fn fold_for_flags(ch:char,flags:Flags)->String{
    if flags.turkish_casing {
        match ch {
            'I'=>return "\u{0131}".to_string(),
            '\u{0130}'=>return "i".to_string(),
            _=>{},
        }
    }
    fold(ch)
}
fn eq_char(a:char,b:char,flags:Flags)->bool{
    a==b || (flags.insensitive && fold_for_flags(a,flags)==fold_for_flags(b,flags))
}
/// Unicode-default full case folding for a string, including multi-character
/// folds such as U+00DF -> "ss". Locale-specific Turkic folding is intentionally
/// not implicit (case folding cannot be both locale-neutral and locale-aware).
pub fn full_case_fold(text:&str)->String{
    let mut result=String::new();for c in text.chars(){result.push_str(&fold(c));}result
}
#[derive(Debug,Clone,Copy)]enum Assertion{Begin,End,AbsoluteBegin,AbsoluteEnd,SoftAbsoluteEnd,StartOffset,WordBoundary,NonWordBoundary}
#[derive(Debug,Clone)]enum Expr{
    Empty,Atom(Class),Assert(Assertion),Seq(Vec<Expr>),Choice(Vec<Expr>),
    /// PCRE2 \K changes the start of the *reported* match, not the capture spans.
    Keep,Fail,Accept,Skip,Mark(String),NamedBackref(String),
    Prune,Commit,Then,NamedSkip(String),
    Repeat{child:Box<Expr>,min:usize,max:Option<usize>,greedy:bool},
    Capture{slot:usize,child:Box<Expr>},Backref(usize),Lookahead{child:Box<Expr>,positive:bool,non_atomic:bool},
    Lookbehind{child:Box<Expr>,positive:bool,non_atomic:bool},
    Atomic(Box<Expr>),
    /// Extended grapheme cluster (\\X, UAX #29).
    Grapheme,
    /// An atomic CRLF / newline sequence (\\R).
    Linebreak,
    /// Evaluate the enclosed expression with isolated option flags.
    ScopedFlags{child:Box<Expr>,flags:Flags},
    Conditional{condition:Condition,yes:Box<Expr>,no:Box<Expr>},
    /// 0 recurses the complete expression; positive numbers invoke captures.
    Call(usize),CallName(String),
    /// Verify that the consumed text is one Unicode script run.
    ScriptRun{child:Box<Expr>,atomic:bool},
    /// Test a previously captured substring, anchored at the capture start.
    ScanSubstring{references:Vec<ScanReference>,child:Box<Expr>},
    /// A callback site in the pattern. No FFI is involved.
    Callout(CalloutToken),
}
#[derive(Debug,Clone)]enum ScanReference{Number(usize),Name(String)}
/// An explicit PCRE-style callout ID. String arguments may be any UTF-8 text.
#[derive(Debug,Clone,PartialEq,Eq)]pub enum CalloutToken{Number(u8),String(String)}
/// Information delivered to a Rust callout handler during backtracking.
#[derive(Debug,Clone)]pub struct CalloutInfo{
    pub token:CalloutToken,
    pub match_start_byte:usize,
    pub current_byte:usize,
    pub groups:Vec<Option<(usize,usize)>>,
}
/// Handler result. `Abort` returns `ErrorKind::Interrupted` to the caller.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum CalloutDecision{Continue,Reject,Abort}

#[derive(Debug,Clone)]enum Condition {
    Capture(usize),Named(String),Recursing(Option<usize>),
    RecursingNamed(String),Assertion{child:Box<Expr>,positive:bool,behind:bool},
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]enum Cut {Prune,Commit,Skip(usize),Then}
#[derive(Debug,Clone)]pub struct Pattern {expr:Expr,flags:Flags,captures:usize,names:BTreeMap<String,usize>,names_all:BTreeMap<String,Vec<usize>>,group_bodies:BTreeMap<usize,Expr>,step_limit:usize,state_limit:usize,depth_limit:usize,not_empty:bool,not_empty_atstart:bool,anchored:bool}
#[derive(Debug,Clone)]pub struct Captures {pub range:(usize,usize),pub groups:Vec<Option<(usize,usize)>>,pub names:BTreeMap<String,usize>,
    /// All capture numbers for duplicate names, in declaration order.
    pub names_all:BTreeMap<String,Vec<usize>>,
    /// The last successful PCRE-style MARK name, if present.
    pub mark:Option<String>,
}
impl Captures {
    pub fn is_matched(&self,index:usize)->bool{self.groups.get(index).copied().flatten().is_some()}
    pub fn group<'a>(&self,text:&'a str,index:usize)->Option<&'a str>{let (a,b)=self.groups.get(index).copied().flatten()?;text.get(a..b)}
    pub fn named<'a>(&self,text:&'a str,name:&str)->Option<&'a str>{
        if let Some(indices)=self.names_all.get(name){
            for &index in indices {if let Some(value)=self.group(text,index){return Some(value);}}
        }
        self.group(text,*self.names.get(name)?)
    }
    /// Expand PCRE2-style replacement references. This method retains the
    /// existing API: ordinary references never need a regex crate or FFI.
    pub fn expand(&self,text:&str,template:&str)->String {
        self.expand_inner(text,template,false,0)
    }
    /// PCRE2_SUBSTITUTE_EXTENDED-inspired expansion, including conditional
    /// substitutions, escapes and basic case conversion.
    pub fn expand_extended(&self,text:&str,template:&str)->String {
        self.expand_inner(text,template,true,0)
    }
    fn substitute_group(&self,text:&str,token:&str,extended:bool,depth:usize)->String {
        if depth>=32{return String::new();}
        if extended {
            if let Some((group,alternative))=token.split_once(":-") {
                if let Some(value)=self.lookup(text,group){return value;}
                return self.expand_inner(text,alternative,extended,depth+1);
            }
            if let Some((group,options))=token.split_once(":+") {
                let (yes,no)=options.split_once(':').unwrap_or((options,""));
                return self.expand_inner(text,if self.lookup(text,group).is_some(){yes}else{no},extended,depth+1);
            }
        }
        self.lookup(text,token).unwrap_or_default()
    }
    fn lookup(&self,text:&str,token:&str)->Option<String>{
        if token=="*MARK"{return self.mark.clone();}
        if let Ok(index)=token.parse::<usize>(){self.group(text,index).map(str::to_string)}
        else{self.named(text,token).map(str::to_string)}
    }
    fn expand_inner(&self,text:&str,template:&str,extended:bool,depth:usize)->String{
        if depth>=32{return String::new();}
        let chars:Vec<char>=template.chars().collect();
        let mut out=String::new();let mut i=0usize;
        let mut case_mode=0u8;let mut next_case=0u8;let mut quoted=false;
        while i<chars.len(){
            let ch=chars[i];i+=1;
            if extended && ch=='\\' {
                if i>=chars.len(){out.push('\\');break;}
                let c=chars[i];i+=1;
                match c {
                    'Q'=>{quoted=true;continue;},
                    'E'=>{quoted=false;case_mode=0;continue;},
                    'L' if !quoted=>{case_mode=1;continue;},
                    'U' if !quoted=>{case_mode=2;continue;},
                    'l' if !quoted=>{next_case=1;continue;},
                    'u' if !quoted=>{next_case=2;continue;},
                    'n'|'r'|'t'|'f'|'a'|'e'|'b'|'v'=>{
                        let value=match c{'n'=>'\n','r'=>'\r','t'=>'\t','f'=>'\x0c',
                            'a'=>'\x07','e'=>'\x1b','b'=>'\x08',_=>'\x0b'};
                        Self::push_case(&mut out,&value.to_string(),case_mode,&mut next_case);
                        continue;
                    },
                    'g' if !quoted && i<chars.len() && chars[i]=='<'=>{
                        i+=1;let begin=i;
                        while i<chars.len()&&chars[i]!='>'{i+=1;}
                        let token=chars[begin..i].iter().collect::<String>();
                        if i<chars.len(){i+=1;}
                        let value=self.substitute_group(text,&token,extended,depth+1);
                        Self::push_case(&mut out,&value,case_mode,&mut next_case);
                        continue;
                    },
                    _=>{Self::push_case(&mut out,&c.to_string(),case_mode,&mut next_case);continue;},
                }
            }
            if ch!='$'||quoted {
                Self::push_case(&mut out,&ch.to_string(),case_mode,&mut next_case);
                continue;
            }
            let Some(&next)=chars.get(i)else{out.push('$');break;};
            i+=1;
            let value=match next {
                '$'=>"$".to_string(),
                '&'=>self.group(text,0).unwrap_or("").to_owned(),
                '`'=>text.get(..self.range.0).unwrap_or("").to_owned(),
                '\''=>text.get(self.range.1..).unwrap_or("").to_owned(),
                '_'=>text.to_string(),
                '+'=>self.groups.iter().rev().filter_map(|span|span.and_then(|(a,b)|text.get(a..b))).next().unwrap_or("").to_owned(),
                '*'=>{
                    let suffix=chars[i..].iter().take(4).copied().collect::<String>();
                    if suffix=="MARK"{i+=4;self.mark.clone().unwrap_or_default()}
                    else{out.push('$');out.push('*');continue;}
                },
                '<'=>{
                    let begin=i;while i<chars.len()&&chars[i]!='>'{i+=1;}
                    if i==chars.len(){out.push_str("$<");continue;}
                    let token=chars[begin..i].iter().collect::<String>();i+=1;
                    self.substitute_group(text,&token,extended,depth+1)
                },
                '{'=>{
                    let begin=i;let mut nested=1usize;
                    while i<chars.len()&&nested>0{
                        match chars[i]{'{'=>nested+=1,'}'=>nested-=1,_=>{}}
                        if nested>0{i+=1;}
                    }
                    if nested!=0{out.push_str("${");continue;}
                    let token=chars[begin..i].iter().collect::<String>();i+=1;
                    self.substitute_group(text,&token,extended,depth+1)
                },
                '0'..='9'=>{
                    let mut token=next.to_string();
                    while i<chars.len()&&chars[i].is_ascii_digit(){token.push(chars[i]);i+=1;}
                    self.substitute_group(text,&token,extended,depth+1)
                },
                _=>{out.push('$');out.push(next);continue;}
            };
            Self::push_case(&mut out,&value,case_mode,&mut next_case);
        }
        out
    }
    fn push_case(out:&mut String,value:&str,case_mode:u8,next_case:&mut u8){
        for ch in value.chars(){
            let mode=if *next_case!=0{let m=*next_case;*next_case=0;m}else{case_mode};
            match mode{1=>out.extend(ch.to_lowercase()),2=>out.extend(ch.to_uppercase()),_=>out.push(ch)}
        }
    }

}
struct Parser{chars:Vec<char>,at:usize,captures:usize,names:BTreeMap<String,usize>,names_all:BTreeMap<String,Vec<usize>>,group_bodies:BTreeMap<usize,Expr>,flags:Flags,
    match_limit:Option<usize>,depth_limit:Option<usize>,heap_limit_kib:Option<usize>,parse_depth:usize,
    alt_extended_class:bool,uts_depth:usize,not_empty:bool,not_empty_atstart:bool,anchored:bool}
impl Parser{
    fn peek(&self)->Option<char>{self.chars.get(self.at).copied()}
    fn next(&mut self)->Option<char>{let v=self.peek()?;self.at+=1;Some(v)}
    fn expect(&mut self,c:char)->io::Result<()> {if self.next()==Some(c){Ok(())}else{Err(err("unexpected regex token"))}}
    /// Extended-mode comments and whitespace are ignored only outside a []
    /// class and escaped/quoted text. Raw peek/next are intentionally retained
    /// for character classes, names, quantifier digits and escapes.
    fn skip_ignored(&mut self) {
        if !self.flags.extended {return;}
        loop {
            let initial=self.at;
            while self.peek().is_some_and(char::is_whitespace) {self.at+=1;}
            if self.peek()==Some('#') {
                while let Some(c)=self.next(){if c=='\n'{break;}}
            }
            if self.at==initial {break;}
        }
    }
    fn peek_token(&mut self)->Option<char>{self.skip_ignored();self.peek()}
    fn next_token(&mut self)->Option<char>{self.skip_ignored();self.next()}
    /// Recognize syntax-setting letters. The parser stores these flags on AST
    /// atoms so mode changes have lexical, not backtracking-dependent, scope.
    fn option_flags(&mut self)->io::Result<(Flags,bool)> {
        let mut options=self.flags;
        let mut enabled=true;
        if self.peek()==Some('^') {self.next();options=Flags::default();}
        let mut saw_minus=false;
        loop {
            match self.peek(){
                Some('-') if !saw_minus => {self.next();enabled=false;saw_minus=true;},
                Some('i')=>{self.next();options.insensitive=enabled;},
                Some('m')=>{self.next();options.multiline=enabled;},
                Some('s')=>{self.next();options.dot_all=enabled;},
                Some('x')=>{
                    self.next();options.extended=enabled;
                    if self.peek()==Some('x') {self.next();options.extra_extended=enabled;}
                    else if !enabled {options.extra_extended=false;}
                },
                Some('U')=>{self.next();options.ungreedy=enabled;},
                Some('n')=>{self.next();options.no_auto_capture=enabled;},
                Some('u')=>{self.next();options.ucp=enabled;},
                Some('r')=>{self.next();options.restrict_case=enabled;},
                Some('a')=>{
                    self.next();
                    match self.peek(){
                        Some('D')=>{self.next();options.ascii_digit=enabled;},
                        Some('S')=>{self.next();options.ascii_space=enabled;},
                        Some('W')=>{self.next();options.ascii_word=enabled;},
                        Some('P')=>{self.next();options.ascii_posix=enabled;},
                        Some('T')=>{self.next();options.ascii_posix=enabled;},
                        _=>{
                            options.ascii_digit=enabled;options.ascii_space=enabled;
                            options.ascii_word=enabled;options.ascii_posix=enabled;
                        }
                    }
                },
                Some('J')=>{self.next();options.duplicate_names=enabled;},
                Some(':')=>{self.next();return Ok((options,true));},
                Some(')')=>{self.next();return Ok((options,false));},
                _=>return Err(err("invalid inline PCRE regex option")),
            }
        }
    }
    fn reference_expr(&self,reference:&str,subroutine:bool)->io::Result<Expr> {
        let reference=reference.trim();
        if reference.is_empty(){return Err(err("empty regex reference"));}
        let numeric=if let Some(rest)=reference.strip_prefix('+'){
            let n=rest.parse::<usize>().map_err(|_|err("invalid relative regex reference"))?;
            self.captures.checked_add(n)
        }else if let Some(rest)=reference.strip_prefix('-'){
            let n=rest.parse::<usize>().map_err(|_|err("invalid relative regex reference"))?;
            self.captures.checked_add(1).and_then(|n0|n0.checked_sub(n))
        }else{reference.parse::<usize>().ok()};
        if let Some(slot)=numeric {return Ok(if subroutine {Expr::Call(slot)} else {Expr::Backref(slot)});}
        if reference.starts_with('+')||reference.starts_with('-')||reference.chars().next().unwrap().is_ascii_digit(){
            return Err(err("invalid regex reference number"));
        }
        Ok(if subroutine{Expr::CallName(reference.to_string())}else{Expr::NamedBackref(reference.to_string())})
    }
    fn parse_choice(&mut self)->io::Result<Expr>{
        if self.parse_depth>=128{return Err(err("regex group nesting exceeds parse limit"));}
        self.parse_depth+=1;
        let parsed:io::Result<Expr>=(||{
            let mut parts=vec![self.parse_seq()?];
            while self.peek_token()==Some('|'){self.next_token();parts.push(self.parse_seq()?);}
            Ok(if parts.len()==1{parts.remove(0)}else{Expr::Choice(parts)})
        })();
        self.parse_depth-=1;
        parsed
    }
    fn parse_seq(&mut self)->io::Result<Expr>{
        let mut parts=Vec::new();
        while let Some(c)=self.peek_token(){if c==')'||c=='|'{break;}parts.push(self.parse_repeat()?);}
        Ok(match parts.len(){0=>Expr::Empty,1=>parts.remove(0),_=>Expr::Seq(parts)})
    }
    fn parse_repeat(&mut self)->io::Result<Expr>{
        let atom_flags=self.flags;
        let mut node=self.parse_atom()?;
        let spec=match self.peek_token(){
            Some('*')=>{self.next();Some((0,None))},Some('+')=>{self.next();Some((1,None))},
            Some('?')=>{self.next();Some((0,Some(1)))},
            Some('{')=>{
                let saved=self.at;self.next();self.brace_spaces();
                let min=if self.peek()==Some(','){Some(0)}else{self.read_number()};
                if let Some(min)=min {
                    self.brace_spaces();
                    let has_comma=self.peek()==Some(',');
                    let max=if has_comma{self.next();self.brace_spaces();self.read_number()}else{Some(min)};
                    self.brace_spaces();
                    if self.peek()==Some('}') && (min!=0 || !has_comma || max.is_some()){
                        self.next();if max.map(|m|m<min).unwrap_or(false){return Err(err("invalid quantifier range"));}
                        if min>1_000_000||max.map(|m|m>1_000_000).unwrap_or(false){return Err(err("regex quantifier too large"));}
                        Some((min,max))
                    }else{self.at=saved;None}
                }else{self.at=saved;None}
            },_=>None
        };
        if let Some((min,max))=spec{
            if matches!(&node,Expr::Empty|Expr::Assert(_)|Expr::Keep|Expr::Mark(_)|Expr::Skip|Expr::NamedSkip(_)|Expr::Prune|Expr::Then|Expr::Commit|Expr::Accept|Expr::Fail){
                return Err(err("quantifier cannot follow a zero-width assertion or control verb"));
            }
            let mut greedy=!self.flags.ungreedy;
            if self.peek()==Some('?'){self.next();greedy=!greedy;}
            let possessive=if self.peek()==Some('+'){self.next();greedy=true;true}else{false};
            if matches!(self.peek(),Some('*'|'+'|'?')){return Err(err("nested quantifier requires parentheses"));}
            node=Expr::Repeat{child:Box::new(node),min,max,greedy};
            if possessive {node=Expr::Atomic(Box::new(node));}
        }
        Ok(Expr::ScopedFlags{child:Box::new(node),flags:atom_flags})
    }
    fn brace_spaces(&mut self) {
        while matches!(self.peek(),Some(' '| '\t')) {self.next();}
    }
    fn read_number(&mut self)->Option<usize>{
        let start=self.at;let mut val=0usize;
        while let Some(c)=self.peek(){if let Some(d)=c.to_digit(10).filter(|_|c.is_ascii_digit()){
            val=val.checked_mul(10)?.checked_add(d as usize)?;self.next();
        }else{break;}}
        if start==self.at{None}else{Some(val)}
    }
    fn parse_atom(&mut self)->io::Result<Expr>{
        let c=self.next_token().ok_or_else(||err("unexpected end of regex"))?;
        match c {
            '^'=>Ok(Expr::Assert(Assertion::Begin)),'$'=>Ok(Expr::Assert(Assertion::End)),
            '.'=>Ok(Expr::Atom(Class::Any)),
            '\\'=>{
                let escaped=self.next().ok_or_else(||err("trailing regex escape"))?;
                match escaped {
                    'X'=>Ok(Expr::Grapheme), 'R'=>Ok(Expr::Linebreak),
                    'h'=>Ok(Expr::Atom(Class::Horizontal)),
                    'H'=>Ok(Expr::Atom(Class::Union{members:vec![Class::Horizontal],negated:true})),
                    'v'=>Ok(Expr::Atom(Class::Vertical)),
                    'V'=>Ok(Expr::Atom(Class::Union{members:vec![Class::Vertical],negated:true})),
                    'N'=>{
                        if self.peek()==Some('{'){
                            self.next();let spec=self.read_until('}')?;
                            let code=spec.strip_prefix("U+").ok_or_else(||err("unsupported Unicode character name"))?;
                            let value=u32::from_str_radix(code,16).map_err(|_|err("invalid U+ escape"))?;
                            Ok(Expr::Atom(Class::Char(char::from_u32(value).ok_or_else(||err("invalid Unicode scalar"))?)))
                        }else{Ok(Expr::Atom(Class::ExceptNewline))}
                    },
                    'Q'=>{
                        let mut quoted=Vec::new();
                        loop{
                            let Some(ch)=self.next()else{break;};
                            if ch=='\\'&&self.peek()==Some('E'){self.next();break;}
                            quoted.push(Expr::Atom(Class::Char(ch)));
                        }
                        Ok(Expr::Seq(quoted))
                    },
                    'b'=>Ok(Expr::Assert(Assertion::WordBoundary)), 'B'=>Ok(Expr::Assert(Assertion::NonWordBoundary)),
                    'A'=>Ok(Expr::Assert(Assertion::AbsoluteBegin)),
                    'G'=>Ok(Expr::Assert(Assertion::StartOffset)),
                    'K'=>Ok(Expr::Keep),
                    'z'=>Ok(Expr::Assert(Assertion::AbsoluteEnd)),
                    'Z'=>Ok(Expr::Assert(Assertion::SoftAbsoluteEnd)),
                    'd'=>Ok(Expr::Atom(Class::Digit)),'D'=>Ok(Expr::Atom(Class::NotDigit)),
                    'w'=>Ok(Expr::Atom(Class::Word)),'W'=>Ok(Expr::Atom(Class::NotWord)),
                    's'=>Ok(Expr::Atom(Class::Space)),'S'=>Ok(Expr::Atom(Class::NotSpace)),
                    'n'=>Ok(Expr::Atom(Class::Char('\n'))),'r'=>Ok(Expr::Atom(Class::Char('\r'))),
                    't'=>Ok(Expr::Atom(Class::Char('\t'))),'f'=>Ok(Expr::Atom(Class::Char('\x0c'))),
                    '0'=>{
                        let mut digits=String::from("0");
                        for _ in 0..2{
                            if self.peek().is_some_and(|c|matches!(c,'0'..='7')){
                                digits.push(self.next().unwrap());
                            }else{break;}
                        }
                        let scalar=u32::from_str_radix(&digits,8).map_err(|_|err("invalid octal escape"))?;
                        Ok(Expr::Atom(Class::Char(char::from_u32(scalar).ok_or_else(||err("invalid octal scalar"))?)))
                    },
                    '1'..='9'=>{
                        let mut number=escaped.to_digit(10).unwrap_or(0) as usize;
                        while let Some(d)=self.peek().and_then(|v|v.to_digit(10)).filter(|_|self.peek().unwrap().is_ascii_digit()){
                            self.next();number=number.saturating_mul(10).saturating_add(d as usize);
                        }
                        Ok(Expr::Backref(number))
                    },
                    'x'=>{
                        let code=if self.peek()==Some('{'){
                            self.next();let digits=self.read_until('}')?;
                            u32::from_str_radix(&digits,16).map_err(|_|err("invalid hex escape"))?
                        }else{
                            let mut digits=String::new();
                            for _ in 0..2 {
                                if self.peek().is_some_and(|c|c.is_ascii_hexdigit()){
                                    digits.push(self.next().unwrap());
                                }else{break;}
                            }
                            if digits.is_empty(){return Err(err("missing hexadecimal digits"));}
                            u32::from_str_radix(&digits,16).map_err(|_|err("invalid hex escape"))?
                        };
                        Ok(Expr::Atom(Class::Char(char::from_u32(code).ok_or_else(||err("invalid Unicode scalar"))?)))
                    },
                    'u'=>{
                        let mut digits=String::new();
                        if self.peek()==Some('{'){self.next();digits=self.read_until('}')?;}
                        else{for _ in 0..4{digits.push(self.next().ok_or_else(||err("truncated Unicode escape"))?);}}
                        let code=u32::from_str_radix(&digits,16).map_err(|_|err("invalid Unicode escape"))?;
                        Ok(Expr::Atom(Class::Char(char::from_u32(code).ok_or_else(||err("invalid Unicode scalar"))?)))
                    },
                    'k'=>{
                        let open=self.next().ok_or_else(||err("missing named backreference delimiter"))?;
                        let close=match open{'<'=>'>' ,'{'=>'}', '\''=>'\'', _=>return Err(err("invalid named backreference delimiter"))};
                        let name=self.read_until(close)?;
                        self.reference_expr(&name,false)
                    },
                    'g'=>{
                        let open=self.peek().ok_or_else(||err("missing regex reference"))?;
                        let (value,subroutine)=if matches!(open,'{'|'<'|'\'') {
                            self.next();let end=match open{'{'=>'}','<'=>'>',_=>'\''};
                            (self.read_until(end)?,open!='{')
                        }else{
                            let mut value=String::new();
                            if matches!(self.peek(),Some('+'|'-')) {value.push(self.next().unwrap());}
                            while self.peek().is_some_and(|ch|ch.is_ascii_digit()){value.push(self.next().unwrap());}
                            (value,false)
                        };
                        self.reference_expr(&value,subroutine)
                    },
                    'o'=>{
                        self.expect('{')?;
                        let digits=self.read_until('}')?;
                        let scalar=u32::from_str_radix(&digits,8).map_err(|_|err("invalid octal escape"))?;
                        Ok(Expr::Atom(Class::Char(char::from_u32(scalar).ok_or_else(||err("invalid octal Unicode scalar"))?)))
                    },
                    'a'=>Ok(Expr::Atom(Class::Char('\x07'))),
                    'e'=>Ok(Expr::Atom(Class::Char('\x1b'))),
                    'c'=>{
                        let c=self.next().ok_or_else(||err("truncated control escape"))?;
                        let upper=c.to_ascii_uppercase();
                        if !upper.is_ascii() {return Err(err("control escape requires ASCII"));}
                        Ok(Expr::Atom(Class::Char((upper as u8 & 0x1f) as char)))
                    },
                    'p'|'P'=>{
                        self.expect('{')?;let name=self.read_until('}')?;
                        if !crate::unicode::property_name_valid(&name){
                            return Err(err("unsupported Unicode regex property"));
                        }
                        Ok(Expr::Atom(Class::Property(name.trim_start_matches('^').to_string(), (escaped=='P') ^ name.starts_with('^'))))
                    },
                    other if other.is_ascii_alphabetic()=>Err(err("unknown PCRE escape sequence")),
                    other=>Ok(Expr::Atom(Class::Char(other)))
                }
            },
            '['=>self.parse_class().map(Expr::Atom),
            '('=>{
                if self.peek()==Some('*') {
                    self.next();
                    let remainder=self.chars[self.at..].iter().copied().collect::<String>();
                    for (prefix,positive,behind,non_atomic) in [
                        ("napla:",true,false,true),("non_atomic_positive_lookahead:",true,false,true),
                        ("naplb:",true,true,true),("non_atomic_positive_lookbehind:",true,true,true),
                        ("pla:",true,false,false),("positive_lookahead:",true,false,false),
                        ("nla:",false,false,false),("negative_lookahead:",false,false,false),
                        ("plb:",true,true,false),("positive_lookbehind:",true,true,false),
                        ("nlb:",false,true,false),("negative_lookbehind:",false,true,false)] {
                        if remainder.starts_with(prefix) {
                            self.at+=prefix.chars().count();
                            let child=self.parse_choice()?;self.expect(')')?;
                            return Ok(if behind {Expr::Lookbehind{child:Box::new(child),positive,non_atomic}}
                                else {Expr::Lookahead{child:Box::new(child),positive,non_atomic}});
                        }
                    }
                    for (prefix,atomic) in [
                        ("script_run:",false),("sr:",false),
                        ("atomic_script_run:",true),("asr:",true),
                    ] {
                        if remainder.starts_with(prefix){
                            self.at+=prefix.len();
                            let child=self.parse_choice()?;self.expect(')')?;
                            return Ok(Expr::ScriptRun{child:Box::new(child),atomic});
                        }
                    }
                    for prefix in ["scan_substring:","scs:"] {
                        if remainder.starts_with(prefix){
                            self.at+=prefix.len();self.expect('(')?;
                            let list=self.read_until(')')?;
                            let mut references=Vec::new();
                            for raw in list.split(',') {
                                let raw=raw.trim();
                                if raw.is_empty(){return Err(err("empty scan-substring group reference"));}
                                let raw=raw.trim_start_matches('<').trim_end_matches('>')
                                    .trim_matches('\'');
                                match self.reference_expr(raw,false)? {
                                    Expr::Backref(slot)=>references.push(ScanReference::Number(slot)),
                                    Expr::NamedBackref(name)=>references.push(ScanReference::Name(name)),
                                    _=>return Err(err("invalid substring reference")),
                                }
                            }
                            let child=self.parse_choice()?;self.expect(')')?;
                            return Ok(Expr::ScanSubstring{references,child:Box::new(child)});
                        }
                    }
                    for prefix in ["atomic:"] {
                        if remainder.starts_with(prefix){
                            self.at+=prefix.len();let child=self.parse_choice()?;self.expect(')')?;
                            return Ok(Expr::Atomic(Box::new(child)));
                        }
                    }
                    let directive=self.read_until(')')?;
                    return match directive.as_str(){
                        "FAIL"|"F"=>Ok(Expr::Fail),
                        "ACCEPT"=>Ok(Expr::Accept),
                        "UTF"=>Ok(Expr::Empty), // `str` is already UTF-8.
                        "UCP"=>{self.flags.ucp=true;Ok(Expr::Empty)},
                        "TURKISH_CASING"=>{self.flags.turkish_casing=true;Ok(Expr::Empty)},
                        "CASELESS_RESTRICT"=>{self.flags.restrict_case=true;Ok(Expr::Empty)},
                        "BSR_ANYCRLF"=>{self.flags.bsr_anycrlf=true;Ok(Expr::Empty)},
                        "BSR_UNICODE"=>{self.flags.bsr_anycrlf=false;Ok(Expr::Empty)},
                        "NO_AUTO_CAPTURE"=>{self.flags.no_auto_capture=true;Ok(Expr::Empty)},
                        "NO_JIT"|"NO_START_OPT"|"NO_DOTSTAR_ANCHOR"=>Ok(Expr::Empty), // No optimizer/JIT generated.
                        "CR"=>{self.flags.newline=Newline::Cr;Ok(Expr::Empty)},
                        "LF"=>{self.flags.newline=Newline::Lf;Ok(Expr::Empty)},
                        "CRLF"=>{self.flags.newline=Newline::CrLf;Ok(Expr::Empty)},
                        "ANYCRLF"=>{self.flags.newline=Newline::AnyCrLf;Ok(Expr::Empty)},
                        "ANY"=>{self.flags.newline=Newline::Any;Ok(Expr::Empty)},
                        "NUL"=>{self.flags.newline=Newline::Nul;Ok(Expr::Empty)},
                        "SKIP"=>Ok(Expr::Skip),
                        "PRUNE"=>Ok(Expr::Prune),
                        "COMMIT"=>Ok(Expr::Commit),
                        "THEN"=>Ok(Expr::Then),
                        "NOTEMPTY"=>{self.not_empty=true;Ok(Expr::Empty)},
                        "NOTEMPTY_ATSTART"=>{self.not_empty_atstart=true;Ok(Expr::Empty)},
                        "ANCHORED"=>{self.anchored=true;Ok(Expr::Empty)},
                        "NO_AUTO_POSSESS"=>Ok(Expr::Empty),
                        _ if directive.starts_with("SKIP:") && directive.len()>5 =>Ok(Expr::NamedSkip(directive[5..].to_owned())),
                        _ if directive.starts_with("PRUNE:") =>Ok(Expr::Seq(vec![Expr::Mark(directive[6..].to_owned()),Expr::Prune])),
                        _ if directive.starts_with("COMMIT:") =>Ok(Expr::Seq(vec![Expr::Mark(directive[7..].to_owned()),Expr::Commit])),
                        _ if directive.starts_with("THEN:") =>Ok(Expr::Seq(vec![Expr::Mark(directive[5..].to_owned()),Expr::Then])),
                        _ if directive.starts_with("ACCEPT:") =>Ok(Expr::Seq(vec![Expr::Mark(directive[7..].to_owned()),Expr::Accept])),
                        _ if directive.starts_with("LIMIT_MATCH=")=>{
                            let value=directive[12..].parse::<usize>().map_err(|_|err("invalid match limit"))?;
                            self.match_limit=Some(self.match_limit.unwrap_or(20_000_000).min(value));
                            Ok(Expr::Empty)
                        },
                        _ if directive.starts_with("LIMIT_DEPTH=") || directive.starts_with("LIMIT_RECURSION=")=>{
                            let value=directive.split_once('=').unwrap().1.parse::<usize>()
                                .map_err(|_|err("invalid depth limit"))?;
                            self.depth_limit=Some(self.depth_limit.unwrap_or(1024).min(value));
                            Ok(Expr::Empty)
                        },
                        _ if directive.starts_with("LIMIT_HEAP=")=>{
                            let value=directive[11..].parse::<usize>().map_err(|_|err("invalid heap limit"))?;
                            self.heap_limit_kib=Some(self.heap_limit_kib.unwrap_or(usize::MAX).min(value));
                            Ok(Expr::Empty)
                        },
                        _ if directive.starts_with("MARK:")=>Ok(Expr::Mark(directive[5..].to_string())),
                        _ if directive.starts_with(":") && directive.len()>1=>Ok(Expr::Mark(directive[1..].to_string())),
                        _=>Err(err("unsupported PCRE backtracking verb or directive")),
                    };
                }
                let outer_flags=self.flags;
                let mut capturing=!self.flags.no_auto_capture;let mut name=None;let mut lookahead=None;let mut lookbehind=None;let mut atomic=false;let mut non_atomic=false;
                if self.peek()==Some('?'){
                    self.next();match self.next(){
                        Some(':')=>capturing=false,
                        Some('>')=>{capturing=false;atomic=true;},
                        Some('#')=>{self.read_until(')')?;return Ok(Expr::Empty);},
                        Some('[')=>{
                            let class=self.parse_set_union()?;
                            self.skip_set_ws();self.expect(']')?;self.expect(')')?;
                            return Ok(Expr::Atom(class));
                        },
                        Some('\'')=>{capturing=true;name=Some(self.read_until('\'')?);},
                        Some('|')=>{
                            // Branch reset: every alternative starts with the
                            // same capture numbers; after the group, numbering
                            // advances by the largest alternative's count.
                            let baseline=self.captures;let mut maximum=baseline;
                            let mut branches=Vec::new();
                            loop{
                                self.captures=baseline;
                                branches.push(self.parse_seq()?);
                                maximum=maximum.max(self.captures);
                                if self.peek_token()!=Some('|'){break;}
                                self.next_token();
                            }
                            self.expect(')')?;
                            self.captures=maximum;
                            self.flags=outer_flags;
                            return Ok(Expr::Choice(branches));
                        },
                        Some('R')=>{self.expect(')')?;return Ok(Expr::Call(0));},
                        Some('0'..='9')=>{
                            self.at-=1;
                            let number=self.read_number().ok_or_else(||err("invalid group subroutine number"))?;
                            self.expect(')')?;
                            return Ok(Expr::Call(number));
                        },
                        Some('&')=>{
                            let name=self.read_until(')')?;
                            return Ok(Expr::CallName(name));
                        },
                        Some('(')=>{
                            if self.peek()==Some('D') {
                                // DEFINE's groups may be invoked later by subroutine.
                                let ahead=self.chars[self.at..].iter().take(7).copied().collect::<String>();
                                if ahead=="DEFINE)" {
                                    self.at+=7;
                                    let _definitions=self.parse_choice()?;
                                    self.expect(')')?;
                                    return Ok(Expr::Empty);
                                }
                            }
                            let condition=if self.peek()==Some('?') {
                                self.next();
                                let (positive,behind)=match self.next(){
                                    Some('=')=>(true,false), Some('!')=>(false,false),
                                    Some('<')=>match self.next(){Some('=')=>(true,true),Some('!')=>(false,true),_=>return Err(err("invalid conditional lookbehind"))},
                                    _=>return Err(err("unsupported regex conditional assertion")),
                                };
                                let inner=self.parse_choice()?;
                                self.expect(')')?;
                                Condition::Assertion{child:Box::new(inner),positive,behind}
                            }else{
                                let test=self.read_until(')')?;
                                if test=="R"{Condition::Recursing(None)}
                                else if let Some(name)=test.strip_prefix("R&") {Condition::RecursingNamed(name.to_string())}
                                else if test.starts_with('R')&&test[1..].chars().all(|c|c.is_ascii_digit()) {
                                    Condition::Recursing(Some(test[1..].parse::<usize>().map_err(|_|err("invalid recursion condition"))?))
                                } else {
                                    let label=test.trim().trim_start_matches('<').trim_end_matches('>').trim_matches('\'');
                                    if label.chars().next().is_some_and(|c|c.is_ascii_digit()||c=='+'||c=='-') {
                                        let Expr::Backref(slot)=self.reference_expr(label,false)? else {return Err(err("invalid conditional numeric group"));};
                                        Condition::Capture(slot)
                                    }else{Condition::Named(label.to_string())}
                                }
                            };
                            let yes=self.parse_seq()?;
                            let no=if self.peek()==Some('|'){self.next();self.parse_seq()?}else{Expr::Empty};
                            self.expect(')')?;
                            return Ok(Expr::Conditional{condition,yes:Box::new(yes),no:Box::new(no)});
                        },
                        Some('*')=>{capturing=false;lookahead=Some(true);non_atomic=true;},
                        Some('=')=>{capturing=false;lookahead=Some(true);},
                        Some('!')=>{capturing=false;lookahead=Some(false);},
                        Some('C')=>{
                            let token=if self.peek()==Some(')') {CalloutToken::Number(0)}
                            else if self.peek().is_some_and(|c|c.is_ascii_digit()) {
                                let number=self.read_number().ok_or_else(||err("invalid callout number"))?;
                                if number>255{return Err(err("PCRE callout number exceeds 255"));}
                                CalloutToken::Number(number as u8)
                            }else{
                                let opening=self.next().ok_or_else(||err("unterminated regex callout"))?;
                                let closing=if opening=='{' {'}'}
                                    else if "'\"^%#$`".contains(opening){opening}
                                    else{return Err(err("invalid PCRE callout delimiter"));};
                                let mut body=String::new();
                                loop{
                                    let c=self.next().ok_or_else(||err("unterminated regex callout string"))?;
                                    if c==closing {
                                        if self.peek()==Some(closing){self.next();body.push(closing);continue;}
                                        break;
                                    }
                                    body.push(c);
                                }
                                CalloutToken::String(body)
                            };
                            self.expect(')')?;
                            return Ok(Expr::Callout(token));
                        },
                        Some('P')=>{
                            match self.next(){
                                Some('<')=>{capturing=true;name=Some(self.read_until('>')?);},
                                Some('>')=>return Ok(Expr::CallName(self.read_until(')')?)),
                                Some('=')=>{
                                    let name=self.read_until(')')?;
                                    return self.reference_expr(&name,false);
                                },
                                _=>return Err(err("unsupported (?P...) regex syntax")),
                            }
                        },
                        Some('i'|'m'|'s'|'x'|'U'|'n'|'u'|'J'|'a'|'r'|'-'|'^'|')')=>{
                            self.at-=1;
                            let (options,scoped)=self.option_flags()?;
                            self.flags=options;
                            if !scoped {return Ok(Expr::Empty);}
                            let parsed=self.parse_choice();
                            self.flags=outer_flags;
                            let inner=parsed?;
                            self.expect(')')?;
                            return Ok(Expr::ScopedFlags{child:Box::new(inner),flags:options});
                        },
                        Some('<')=>{
                            match self.peek(){
                                Some('*')=>{self.next();capturing=false;lookbehind=Some(true);non_atomic=true;},
                                Some('=')=>{self.next();capturing=false;lookbehind=Some(true);},
                                Some('!')=>{self.next();capturing=false;lookbehind=Some(false);},
                                _=>{capturing=true;name=Some(self.read_until('>')?);},
                            }
                        },
                        _=>return Err(err("unsupported regex group syntax")),
                    }
                }
                let slot=if capturing{self.captures+=1;Some(self.captures)}else{None};
                if let (Some(ref key),Some(number))=(&name,slot){
                    if key.is_empty() || key.chars().next().is_some_and(char::is_numeric) ||
                        !key.chars().all(|c|c.is_alphanumeric()||c=='_') {
                        return Err(err("invalid capture name"));
                    }
                    if let Some(old)=self.names.get(key) {
                        if *old!=number && !self.flags.duplicate_names {
                            return Err(err("duplicate capture name requires (?J)"));
                        }
                    }
                    self.names.entry(key.clone()).or_insert(number);
                    let entries=self.names_all.entry(key.clone()).or_default();
                    if !entries.contains(&number){entries.push(number);}
                }
                let inner=self.parse_choice()?;self.expect(')')?;
                self.flags=outer_flags;
                if let Some(slot)=slot{
                    use std::collections::btree_map::Entry;
                    match self.group_bodies.entry(slot){
                        Entry::Vacant(entry)=>{entry.insert(inner.clone());},
                        Entry::Occupied(mut entry)=>{
                            let prior=entry.get().clone();
                            entry.insert(Expr::Choice(vec![prior,inner.clone()]));
                        }
                    }
                }
                if atomic{Ok(Expr::Atomic(Box::new(inner)))}
                else if let Some(positive)=lookahead{Ok(Expr::Lookahead{child:Box::new(inner),positive,non_atomic})}
                else if let Some(positive)=lookbehind{Ok(Expr::Lookbehind{child:Box::new(inner),positive,non_atomic})}
                else if let Some(slot)=slot{Ok(Expr::Capture{slot,child:Box::new(inner)})}
                else{Ok(inner)}
            },
            ')'|'|'|'*'|'+'|'?'=>Err(err("unexpected regex metacharacter")),
            other=>Ok(Expr::Atom(Class::Char(other)))
        }
    }
    /// A useful subset of Perl's (?[]) class expressions. Unescaped bare
    /// literals are rejected, as PCRE2 requires bracketed ordinary classes.
    fn skip_set_ws(&mut self){while self.peek().is_some_and(char::is_whitespace){self.next();}}
    fn parse_set_union(&mut self)->io::Result<Class>{
        let mut left=self.parse_set_intersect()?;
        loop{
            self.skip_set_ws();
            let op=match self.peek(){Some('|'|'+'|'-'|'^')=>self.next().unwrap(),_=>break};
            let right=self.parse_set_intersect()?;
            left=match op{
                '|'|'+'=>Class::Union{members:vec![left,right],negated:false},
                '-'=>Class::Difference(Box::new(left),Box::new(right)),
                '^'=>Class::SymmetricDifference(Box::new(left),Box::new(right)),
                _=>unreachable!(),
            };
        }
        Ok(left)
    }
    fn parse_set_intersect(&mut self)->io::Result<Class>{
        let mut left=self.parse_set_unary()?;
        loop{self.skip_set_ws();if self.peek()!=Some('&'){break;}
            self.next();let right=self.parse_set_unary()?;
            left=Class::Intersection(Box::new(left),Box::new(right));
        }Ok(left)
    }
    fn parse_set_unary(&mut self)->io::Result<Class>{
        self.skip_set_ws();let next=self.next().ok_or_else(||err("unclosed extended class"))?;
        match next {
            '!'=>Ok(Class::Complement(Box::new(self.parse_set_unary()?))),
            '('=>{let value=self.parse_set_union()?;self.skip_set_ws();self.expect(')')?;Ok(value)},
            '[' if self.peek()==Some(':')=>{
                self.next();let name=self.read_until(':')?;self.expect(']')?;
                let key=name.strip_prefix('^').unwrap_or(&name).to_ascii_lowercase();
                if !matches!(key.as_str(),"alnum"|"alpha"|"ascii"|"blank"|"cntrl"|"digit"|"graph"|"lower"|"print"|"punct"|"space"|"upper"|"word"|"xdigit") {return Err(err("unknown POSIX class"));}
                Ok(Class::Posix(key,name.starts_with('^')))
            },
            '['=>self.parse_ordinary_class(),
            '\\'=>{
                match self.next().ok_or_else(||err("truncated extended class escape"))? {
                    'd'=>Ok(Class::Digit),'D'=>Ok(Class::NotDigit),
                    's'=>Ok(Class::Space),'S'=>Ok(Class::NotSpace),
                    'w'=>Ok(Class::Word),'W'=>Ok(Class::NotWord),
                    'h'=>Ok(Class::Horizontal),'H'=>Ok(Class::Complement(Box::new(Class::Horizontal))),
                    'v'=>Ok(Class::Vertical),'V'=>Ok(Class::Complement(Box::new(Class::Vertical))),
                    'p'|'P'=>{
                        let negative=self.chars[self.at-1]=='P';
                        self.expect('{')?;let name=self.read_until('}')?;
                        if !crate::unicode::property_name_valid(&name){return Err(err("unsupported extended Unicode property"));}
                        Ok(Class::Property(name.trim_start_matches('^').to_string(),negative ^ name.starts_with('^')))
                    },
                    'n'=>Ok(Class::Char('\n')),'r'=>Ok(Class::Char('\r')),'t'=>Ok(Class::Char('\t')),
                    c if !c.is_ascii_alphanumeric()=>Ok(Class::Char(c)),
                    _=>Err(err("unsupported extended class escape")),
                }
            },
            _=>Err(err("expected class, escape, or group in (?[])")),
        }
    }
    fn read_until(&mut self,end:char)->io::Result<String>{
        let mut name=String::new();while let Some(c)=self.next(){if c==end{return Ok(name);}name.push(c);}
        Err(err("unterminated named group"))
    }
    fn parse_class(&mut self)->io::Result<Class>{
        if self.alt_extended_class {self.parse_uts_class()}
        else{self.parse_ordinary_class()}
    }
    /// Parse a Unicode Technical Standard #18 style character set. Literal
    /// juxtaposition is an implicit union with higher precedence than an
    /// explicit set operator. Unlike other regex engines, PCRE2 prohibits
    /// mixing distinct explicit operators without nested brackets.
    fn parse_uts_class(&mut self)->io::Result<Class>{
        if self.uts_depth>=128{return Err(err("UTS class nesting exceeds limit"));}
        self.uts_depth+=1;
        let result=(||{
            let negated=if self.peek()==Some('^'){self.next();true}else{false};
            let mut left=self.parse_uts_juxtaposed()?;
            let mut previous_operator:Option<&'static str>=None;
            loop{
                if self.peek()==Some(']'){self.next();break;}
                let (op,kind)=if self.peek()==Some('&')&&self.chars.get(self.at+1)==Some(&'&') {("&&",0)}
                    else if self.peek()==Some('-')&&self.chars.get(self.at+1)==Some(&'-'){("--",1)}
                    else if self.peek()==Some('|')&&self.chars.get(self.at+1)==Some(&'|'){("||",2)}
                    else if self.peek()==Some('~')&&self.chars.get(self.at+1)==Some(&'~'){("~~",3)}
                    else{return Err(err("invalid UTS#18 class operator"));};
                if previous_operator.is_some_and(|prior|prior!=op){
                    return Err(err("mixed UTS#18 set operators need nested brackets"));
                }
                previous_operator=Some(op);
                self.at+=2;
                let right=self.parse_uts_juxtaposed()?;
                left=match kind{
                    0=>Class::Intersection(Box::new(left),Box::new(right)),
                    1=>Class::Difference(Box::new(left),Box::new(right)),
                    2=>Class::Union{members:vec![left,right],negated:false},
                    _=>Class::SymmetricDifference(Box::new(left),Box::new(right)),
                };
            }
            if negated{Ok(Class::Complement(Box::new(left)))}else{Ok(left)}
        })();
        self.uts_depth-=1;
        result
    }
    fn parse_uts_juxtaposed(&mut self)->io::Result<Class>{
        let mut elements=Vec::new();
        loop {
            if self.flags.extra_extended && matches!(self.peek(),Some(' '| '\t')) {self.next();continue;}
            if matches!(self.peek(),None|Some(']')){break;}
            let pair=(self.peek(),self.chars.get(self.at+1).copied());
            if matches!(pair,(Some('&'),Some('&'))|(Some('-'),Some('-'))|
                (Some('|'),Some('|'))|(Some('~'),Some('~'))) {break;}
            if self.peek()==Some('['){
                self.next();
                if self.peek()==Some(':'){
                    self.next();let inner=self.read_until(':')?;self.expect(']')?;
                    let name=inner.strip_prefix('^').unwrap_or(&inner).to_ascii_lowercase();
                    if !matches!(name.as_str(),"alnum"|"alpha"|"ascii"|"blank"|"cntrl"|"digit"|
                        "graph"|"lower"|"print"|"punct"|"space"|"upper"|"word"|"xdigit"){
                        return Err(err("unknown POSIX class"));
                    }
                    elements.push(Class::Posix(name,inner.starts_with('^')));
                } else {elements.push(self.parse_uts_class()?);}
                continue;
            }
            let item=self.parse_uts_atom()?;
            if let Class::Char(start)=item {
                if self.peek()==Some('-') && self.chars.get(self.at+1)!=Some(&'-')
                    && self.chars.get(self.at+1)!=Some(&']') {
                    self.next();
                    let next=self.parse_uts_atom()?;
                    let Class::Char(end)=next else{return Err(err("invalid UTS#18 range endpoint"));};
                    if end<start{return Err(err("reversed UTS class range"));}
                    elements.push(Class::Ranges{ranges:vec![(start,end)],negated:false});
                }else{elements.push(Class::Char(start));}
            }else{elements.push(item);}
        }
        if elements.is_empty(){return Err(err("empty UTS character set operand"));}
        Ok(if elements.len()==1{elements.remove(0)}
            else{Class::Union{members:elements,negated:false}})
    }
    fn parse_uts_atom(&mut self)->io::Result<Class>{
        let c=self.next().ok_or_else(||err("unterminated UTS character class"))?;
        if c!='\\'{return Ok(Class::Char(c));}
        let escaped=self.next().ok_or_else(||err("truncated UTS escape"))?;
        match escaped {
            'd'=>Ok(Class::Digit),'D'=>Ok(Class::NotDigit),
            's'=>Ok(Class::Space),'S'=>Ok(Class::NotSpace),
            'w'=>Ok(Class::Word),'W'=>Ok(Class::NotWord),
            'h'=>Ok(Class::Horizontal),'H'=>Ok(Class::Complement(Box::new(Class::Horizontal))),
            'v'=>Ok(Class::Vertical),'V'=>Ok(Class::Complement(Box::new(Class::Vertical))),
            'p'|'P'=>{
                self.expect('{')?;let name=self.read_until('}')?;
                if !crate::unicode::property_name_valid(&name){return Err(err("invalid Unicode property"));}
                Ok(Class::Property(name.trim_start_matches('^').to_string(),(escaped=='P') ^ name.starts_with('^')))
            },
            'n'=>Ok(Class::Char('\n')),'r'=>Ok(Class::Char('\r')),
            't'=>Ok(Class::Char('\t')),'f'=>Ok(Class::Char('\x0c')),
            'b'=>Ok(Class::Char('\x08')),
            'x'=>{
                let digits=if self.peek()==Some('{'){self.next();self.read_until('}')?}
                    else{let mut buf=String::new();for _ in 0..2{
                        if self.peek().is_some_and(|c|c.is_ascii_hexdigit()){buf.push(self.next().unwrap());}
                        else{break;}}
                        buf};
                let code=u32::from_str_radix(&digits,16).map_err(|_|err("invalid UTS hex escape"))?;
                Ok(Class::Char(char::from_u32(code).ok_or_else(||err("invalid Unicode scalar"))?))
            },
            other if !other.is_ascii_alphanumeric()=>Ok(Class::Char(other)),
            _=>Err(err("unsupported UTS class escape")),
        }
    }
    /// Character classes must decode escapes before building ranges. The old
    /// parser interpreted [\x{41}] as five separate literal characters.
    fn parse_class_escape(&mut self)->io::Result<Class>{
        let c=self.next().ok_or_else(||err("truncated character-class escape"))?;
        match c {
            'd'=>Ok(Class::Digit),'D'=>Ok(Class::NotDigit),
            'w'=>Ok(Class::Word),'W'=>Ok(Class::NotWord),
            's'=>Ok(Class::Space),'S'=>Ok(Class::NotSpace),
            'h'=>Ok(Class::Horizontal),'H'=>Ok(Class::Complement(Box::new(Class::Horizontal))),
            'v'=>Ok(Class::Vertical),'V'=>Ok(Class::Complement(Box::new(Class::Vertical))),
            'p'|'P'=>{
                self.expect('{')?;let name=self.read_until('}')?;
                if !crate::unicode::property_name_valid(&name){return Err(err("unsupported character class Unicode property"));}
                Ok(Class::Property(name.trim_start_matches('^').to_string(),(c=='P') ^ name.starts_with('^')))
            },
            'n'=>Ok(Class::Char('\n')),'r'=>Ok(Class::Char('\r')),
            't'=>Ok(Class::Char('\t')),'f'=>Ok(Class::Char('\x0c')),
            'a'=>Ok(Class::Char('\x07')),'e'=>Ok(Class::Char('\x1b')),
            'b'=>Ok(Class::Char('\x08')),
            'c'=>{
                let next=self.next().ok_or_else(||err("truncated control escape"))?;
                if !next.is_ascii()||next.is_ascii_control(){return Err(err("invalid control escape"));}
                Ok(Class::Char(((next.to_ascii_uppercase() as u8)^0x40) as char))
            },
            'x'|'o'=>{
                let (text,base)=if c=='o'{(self.read_braced_digits()?,8)}
                    else if self.peek()==Some('{'){(self.read_braced_digits()?,16)}
                    else{let mut digits=String::new();for _ in 0..2{
                        if self.peek().is_some_and(|next|next.is_ascii_hexdigit()){
                            digits.push(self.next().unwrap());
                        }else{break;}}
                        (digits,16)};
                let value=u32::from_str_radix(&text,base).map_err(|_|err("invalid numeric class escape"))?;
                Ok(Class::Char(char::from_u32(value).ok_or_else(||err("invalid Unicode scalar"))?))
            },
            '0'..='7'=>{
                let mut text=String::from(c);
                for _ in 0..2{
                    if self.peek().is_some_and(|next|matches!(next,'0'..='7')){
                        text.push(self.next().unwrap());
                    }else{break;}
                }
                let value=u32::from_str_radix(&text,8).map_err(|_|err("invalid octal class escape"))?;
                Ok(Class::Char(char::from_u32(value).ok_or_else(||err("invalid octal scalar"))?))
            },
            '8'|'9'=>Ok(Class::Char(c)),
            other if !other.is_ascii_alphabetic()=>Ok(Class::Char(other)),
            _=>Err(err("unsupported class escape sequence")),
        }
    }
    fn read_braced_digits(&mut self)->io::Result<String>{
        self.expect('{')?;
        self.read_until('}')
    }
    fn parse_ordinary_class(&mut self)->io::Result<Class>{
        let negated=if self.peek()==Some('^'){self.next();true}else{false};
        let mut ranges=Vec::new();let mut members=Vec::new();
        let mut first=true;let mut closed=false;
        while let Some(mut c)=self.next(){
            if self.flags.extra_extended && matches!(c,' '| '\t'){continue;}
            if c==']' && !first {closed=true;break;}
            first=false;
            if c=='[' && self.peek()==Some(':'){
                self.next(); // POSIX class: [[:alpha:]], [[:^digit:]], etc.
                let inner=self.read_until(':')?;
                self.expect(']')?;
                let name=inner.strip_prefix('^').unwrap_or(&inner).to_ascii_lowercase();
                if !matches!(name.as_str(),"alnum"|"alpha"|"ascii"|"blank"|"cntrl"|"digit"|"graph"|"lower"|"print"|"punct"|"space"|"upper"|"word"|"xdigit"){
                    return Err(err("unsupported POSIX character class"));
                }
                members.push(Class::Posix(name,inner.starts_with('^')));
                continue;
            }
            if c=='\\' {
                if self.peek()==Some('Q'){
                    self.next();let mut terminated=false;
                    while let Some(ch)=self.next(){
                        if ch=='\\'&&self.peek()==Some('E'){
                            self.next();terminated=true;break;
                        }
                        ranges.push((ch,ch));
                    }
                    if !terminated{return Err(err("unterminated quoted class literal"));}
                    continue;
                }
                let escaped=self.parse_class_escape()?;
                let Class::Char(literal)=escaped else{
                    members.push(escaped);continue;
                };
                c=literal;
            }
            if self.peek()==Some('-') && self.chars.get(self.at+1)!=Some(&']'){
                self.next();let mut last=self.next().ok_or_else(||err("unfinished class range"))?;
                if last=='\\' {
                    let Class::Char(scalar)=self.parse_class_escape()? else{
                        return Err(err("character class range endpoint must be a character"));
                    };
                    last=scalar;
                }
                if last<c{return Err(err("character class range in reverse order"));}
                ranges.push((c,last));
            }else{ranges.push((c,c));}
        }
        if !closed{return Err(err("unclosed character class"));}
        if !ranges.is_empty(){members.push(Class::Ranges{ranges,negated:false});}
        if members.len()==1&&!negated{return Ok(members.remove(0));}
        Ok(Class::Union{members,negated})
    }
}
#[derive(Clone)]struct State{
    pos:usize, match_start:usize, accepted:bool, mark:Option<String>,
    marks:Vec<(String,usize)>, cut:Option<Cut>,
    groups:Vec<Option<(usize,usize)>>,
}
struct Engine<'a,'callback>{
    input:&'a [char],flags:Flags,steps:usize,max_steps:usize,max_states:usize,
    /// True at each grapheme boundary, including the end of the input.
    grapheme_boundaries:&'a [bool],exhausted:bool,
    depth:usize,max_depth:usize,root:&'a Expr,groups:&'a BTreeMap<usize,Expr>,names:&'a BTreeMap<String,usize>,
    start_offset:usize, skip_to:Option<usize>, committed:bool, cut:Option<Cut>,
    call_stack:Vec<usize>,names_all:&'a BTreeMap<String,Vec<usize>>,
    byte_offsets:&'a [usize],
    callout:Option<&'callback mut dyn FnMut(&CalloutInfo)->CalloutDecision>,
    aborted:bool,
}
/// Unicode 16.0 script-run validation with script-extension intersection and
/// the three permitted Han combinations (Japanese, Korean, Bopomofo + Han).
/// The bundled data comes from Unicode UCD and requires no runtime parser.
fn is_script_run(chars:&[char])->bool{
    if chars.len()<2{return true;}
    let mut allowed=[u64::MAX;4];
    let mut digit_start=None;
    for &ch in chars {
        let cp=ch as u32;
        if let Some(start)=script_data::decimal_set_start(cp){
            match digit_start{Some(s) if s!=start=>return false,None=>digit_start=Some(start),_=>{}}
        }
        let (basic,extensions)=script_data::scripts(cp);
        if basic==0 {return false;}
        // Inherited/Common without an explicit extension does not constrain
        // the required script (UAX #24). Other letters do.
        if extensions.is_none() && matches!(basic,1|4){continue;}
        let mut current=[0u64;4];
        let list=extensions.unwrap_or(std::slice::from_ref(&basic));
        for &id in list {
            let mut add=|id:u16|{
                let index=id as usize/64;
                if index<current.len(){current[index]|=1u64<<(id as usize%64);}
            };
            add(id);
            match id {
                56=>{add(190);add(191);add(192);}, // Han: Japanese/Korean/Taiwanese
                57|58=>add(190), // Hiragana or Katakana: Japanese
                31=>add(191), // Hangul: Korean
                3=>add(192), // Bopomofo: Taiwanese Mandarin
                _=>{},
            }
        }
        for i in 0..4{allowed[i]&=current[i];}
        if allowed.iter().all(|bits|*bits==0){return false;}
    }
    true
}
impl Engine<'_, '_>{
    fn record_cut(&mut self,cut:Option<Cut>){
        match cut {
            Some(Cut::Commit)=>{self.committed=true;self.cut=Some(Cut::Commit);},
            Some(Cut::Skip(pos))=>{
                self.skip_to=Some(self.skip_to.unwrap_or(0).max(pos));
                self.cut=Some(Cut::Skip(pos));
            },
            Some(Cut::Prune)=>{self.cut=Some(Cut::Prune);},
            Some(Cut::Then)=>{self.cut=Some(Cut::Then);},
            None=>{},
        }
    }
    fn tick(&mut self)->bool{
        self.steps=self.steps.saturating_add(1);
        if self.steps>self.max_steps{self.exhausted=true;false}else{true}
    }
    fn newline_len(&self,pos:usize)->Option<usize>{
        let ch=*self.input.get(pos)?;
        match self.flags.newline {
            Newline::Lf if ch=='\n'=>Some(1),
            Newline::Cr if ch=='\r'=>Some(1),
            Newline::CrLf if ch=='\r'&&self.input.get(pos+1)==Some(&'\n')=>Some(2),
            Newline::AnyCrLf|Newline::Any if ch=='\r'&&self.input.get(pos+1)==Some(&'\n')=>Some(2),
            Newline::AnyCrLf if matches!(ch,'\n'|'\r')=>Some(1),
            Newline::Any if is_newline_char(ch,Newline::Any)=>Some(1),
            Newline::Nul if ch=='\0'=>Some(1),
            _=>None,
        }
    }
    fn assert(&self,a:Assertion,pos:usize)->bool{
        let word=|v:char|Class::Word.accepts(v,self.flags);
        let at_line_end=self.newline_len(pos).is_some();
        let before_final=self.newline_len(pos).is_some_and(|len|pos+len==self.input.len());
        let after_line_end=(pos>0&&self.newline_len(pos-1)==Some(1))||
            (pos>=2&&self.newline_len(pos-2)==Some(2));
        match a {
            Assertion::Begin=>pos==0||(self.flags.multiline&&after_line_end),
            Assertion::End=>pos==self.input.len()||before_final||(self.flags.multiline&&at_line_end),
            Assertion::AbsoluteBegin=>pos==0,Assertion::AbsoluteEnd=>pos==self.input.len(),
            Assertion::SoftAbsoluteEnd=>pos==self.input.len()||before_final,
            Assertion::StartOffset=>pos==self.start_offset,
            Assertion::WordBoundary=>self.input.get(pos.wrapping_sub(1)).copied().map(word).unwrap_or(false)!=self.input.get(pos).copied().map(word).unwrap_or(false),
            Assertion::NonWordBoundary=>self.input.get(pos.wrapping_sub(1)).copied().map(word).unwrap_or(false)==self.input.get(pos).copied().map(word).unwrap_or(false),
        }
    }
    fn run(&mut self,expr:&Expr,state:State)->Vec<State>{
        // Every nested call, assertion or subroutine is depth-limited. This
        // prevents both recursive-group loops and nested ASTs from exhausting
        // the process stack on hostile patterns or documents.
        if self.depth>=self.max_depth {self.exhausted=true;return Vec::new();}
        if !self.tick(){return Vec::new();}
        self.depth+=1;
        let pending_cut=state.cut;
        let result=self.run_inner(expr,state);
        if result.is_empty(){self.record_cut(pending_cut);}
        self.depth-=1;
        result
    }
    fn run_inner(&mut self,expr:&Expr,state:State)->Vec<State>{
        match expr{
            Expr::Empty=>vec![state],
            Expr::Keep=>vec![State{match_start:state.pos,..state}],
            Expr::Fail=>{self.record_cut(state.cut);Vec::new()},
            Expr::Accept=>vec![State{accepted:true,..state}],
            Expr::Skip=>vec![State{cut:Some(Cut::Skip(state.pos)),..state}],
            Expr::NamedSkip(name)=>{
                if let Some((_,pos))=state.marks.iter().rev().find(|(n,_)|n==name){
                    vec![State{cut:Some(Cut::Skip(*pos)),..state}]
                }else{vec![state]}
            },
            Expr::Prune=>vec![State{cut:Some(Cut::Prune),..state}],
            Expr::Commit=>vec![State{cut:Some(Cut::Commit),..state}],
            Expr::Then=>vec![State{cut:Some(Cut::Then),..state}],
            Expr::Mark(name)=>{
                let mut next=state;next.mark=Some(name.clone());
                next.marks.push((name.clone(),next.pos));vec![next]
            },
            Expr::NamedBackref(name)=>{
                if let Some(indices)=self.names_all.get(name) {
                    if let Some(&index)=indices.iter().find(|&&id|state.groups.get(id).copied().flatten().is_some()) {
                        return self.run(&Expr::Backref(index),state);
                    }
                }
                Vec::new()
            },
            Expr::ScopedFlags{child,flags}=>{
                let original=self.flags;
                self.flags=*flags;
                let output=self.run(child,state);
                self.flags=original;
                output
            },
            Expr::Callout(token)=>{
                let info=CalloutInfo{
                    token:token.clone(),
                    match_start_byte:self.byte_offsets[state.match_start],
                    current_byte:self.byte_offsets[state.pos],
                    groups:state.groups.iter().map(|entry|entry.map(|(a,b)|
                        (self.byte_offsets[a],self.byte_offsets[b]))).collect(),
                };
                match self.callout.as_mut().map(|callback|callback(&info))
                    .unwrap_or(CalloutDecision::Continue) {
                    CalloutDecision::Continue=>vec![state],
                    CalloutDecision::Reject=>Vec::new(),
                    CalloutDecision::Abort=>{self.aborted=true;Vec::new()},
                }
            },
            Expr::ScriptRun{child,atomic}=>{
                let begin=state.pos;
                let candidates=self.run(child,state);
                let matches=candidates.into_iter().filter(|s|{
                    self.input.get(begin..s.pos).map(is_script_run).unwrap_or(false)
                });
                if *atomic{matches.take(1).collect()}else{matches.collect()}
            },
            Expr::ScanSubstring{references,child}=>{
                let selected=references.iter().find_map(|reference|{
                    match reference{
                        ScanReference::Number(slot)=>state.groups.get(*slot).copied().flatten(),
                        ScanReference::Name(name)=>self.names_all.get(name).and_then(|slots|
                            slots.iter().find_map(|slot|state.groups.get(*slot).copied().flatten())),
                    }
                });
                let Some((begin,end))=selected else{return Vec::new()};
                if begin>end||end>self.input.len(){return Vec::new();}
                // Keep original subject positions and context before the
                // substring, but temporarily change the logical subject end.
                let original=self.input;
                self.input=&original[..end];
                let candidate=State{pos:begin,..state.clone()};
                let results=self.run(child,candidate);
                self.input=original;
                // As an assertion, the scan never consumes subject text.
                results.into_iter().map(|mut found|{found.pos=state.pos;found}).collect()
            },
            Expr::Grapheme=>{
                if state.pos>=self.input.len(){return Vec::new();}
                let mut end=state.pos+1;
                while end<self.input.len() && !self.grapheme_boundaries.get(end).copied().unwrap_or(true){end+=1;}
                vec![State{pos:end,..state}]
            },
            Expr::Linebreak=>{
                let Some(&c)=self.input.get(state.pos)else{return Vec::new();};
                if c=='\r' && self.input.get(state.pos+1)==Some(&'\n'){
                    vec![State{pos:state.pos+2,..state}]
                }else if (if self.flags.bsr_anycrlf {matches!(c,'\n'|'\r')}
                    else{matches!(c,'\n'|'\r'|'\u{000b}'|'\u{000c}'|'\u{0085}'|'\u{2028}'|'\u{2029}')}){
                    vec![State{pos:state.pos+1,..state}]
                }else{Vec::new()}
            },
            Expr::Call(slot)=>{
                let target=if *slot==0{Some(self.root.clone())}else{self.groups.get(slot).cloned()};
                let begin=state.pos;
                if let Some(inner)=target {
                    self.call_stack.push(*slot);
                    let result=self.run(&inner,state);
                    self.call_stack.pop();
                    result.into_iter().map(|mut end|{
                        if *slot!=0 && *slot<end.groups.len(){end.groups[*slot]=Some((begin,end.pos));}
                        end
                    }).collect()
                }else{Vec::new()}
            },
            Expr::CallName(name)=>{
                let slot=self.names.get(name).copied();
                let target=slot.and_then(|n|self.groups.get(&n)).cloned();
                let begin=state.pos;
                if let (Some(inner),Some(slot))=(target,slot){
                    self.call_stack.push(slot);
                    let result=self.run(&inner,state);
                    self.call_stack.pop();
                    result.into_iter().map(|mut end|{
                        if slot<end.groups.len(){end.groups[slot]=Some((begin,end.pos));}
                        end
                    }).collect()
                }else{Vec::new()}
            },
            Expr::Assert(a)=>if self.assert(*a,state.pos){vec![state]}else{Vec::new()},
            Expr::Atom(Class::Char(c)) if self.flags.insensitive=>{
                let expected=fold_for_flags(*c,self.flags);let mut found=String::new();let mut output=Vec::new();
                for consumed in 1..=3 {
                    let Some(&actual)=self.input.get(state.pos+consumed-1)else{break;};
                    found.push_str(&fold_for_flags(actual,self.flags));
                    let allowed=!self.flags.restrict_case || c.is_ascii()==actual.is_ascii();
                    if allowed&&found==expected{output.push(State{pos:state.pos+consumed,..state.clone()});}
                    if found.len()>=expected.len(){break;}
                }
                output
            },
            Expr::Atom(c)=>if self.input.get(state.pos).copied().map(|ch|c.accepts(ch,self.flags)).unwrap_or(false){vec![State{pos:state.pos+1,..state}]}else{Vec::new()},
            Expr::Seq(items)=>{
                let mut states=vec![state];
                for item in items{
                    let mut next=Vec::new();for s in states{
                        if s.accepted {next.push(s);continue;}
                        let cut=s.cut;
                        let output=self.run(item,s);
                        if output.is_empty(){self.record_cut(cut);}else{next.extend(output);}
                        if next.len()>self.max_states{self.exhausted=true;next.truncate(self.max_states);break;}
                    }
                    if next.is_empty(){return next;}
                    if !next.is_empty(){self.cut=None;self.committed=false;}
                    states=next;
                }states
            },
            Expr::Choice(branches)=>{
                let mut out=Vec::new();for branch in branches{
                    let matched=self.run(branch,state.clone());
                    if matched.is_empty(){
                        match self.cut.take(){
                            Some(Cut::Then)=>continue,
                            Some(Cut::Prune)|Some(Cut::Commit)|Some(Cut::Skip(_))=>break,
                            None=>{},
                        }
                    }else{out.extend(matched);}
                    if out.len()>self.max_states{self.exhausted=true;out.truncate(self.max_states);break;}
                }out
            },
            Expr::Capture{slot,child}=>{
                let start=state.pos;
                self.run(child,state).into_iter().map(|mut s|{s.groups[*slot]=Some((start,s.pos));s}).collect()
            },
            Expr::Backref(slot)=>{
                if let Some((a,b))=state.groups.get(*slot).copied().flatten(){
                    if let Some(left)=self.input.get(a..b){
                        if !self.flags.insensitive{
                            if self.input.get(state.pos..state.pos.saturating_add(left.len()))==Some(left){
                                return vec![State{pos:state.pos+left.len(),..state}];
                            }
                        }else{
                            let expected=left.iter().map(|c|fold_for_flags(*c,self.flags)).collect::<String>();
                            let mut matched=String::new();
                            // Full folding may change the number of scalars.
                            // An output longer than the folded capture cannot
                            // become a match, so this search remains bounded.
                            for i in state.pos..self.input.len(){
                                matched.push_str(&fold_for_flags(self.input[i],self.flags));
                                if matched==expected{return vec![State{pos:i+1,..state}];}
                                if matched.len()>expected.len(){break;}
                            }
                            if expected.is_empty(){return vec![state];}
                        }
                    }
                }Vec::new()
            },
            Expr::Lookahead{child,positive,non_atomic}=>{
                let candidates=self.run(child,state.clone());
                if *positive{
                    let it=candidates.into_iter().map(|mut s|{s.pos=state.pos;s});
                    if *non_atomic{it.collect()}else{it.take(1).collect()}
                } else if candidates.is_empty(){vec![state]}else{Vec::new()}
            },
            Expr::Atomic(child)=>self.run(child,state).into_iter().take(1).collect(),
            Expr::Conditional{condition,yes,no}=>{
                let test=match condition {
                    Condition::Capture(slot)=>state.groups.get(*slot).copied().flatten().is_some(),
                    Condition::Named(name)=>self.names_all.get(name).is_some_and(|slots|slots.iter().any(|slot|state.groups.get(*slot).copied().flatten().is_some())),
                    Condition::Recursing(None)=>!self.call_stack.is_empty(),
                    Condition::Recursing(Some(slot))=>self.call_stack.contains(slot),
                    Condition::RecursingNamed(name)=>self.names_all.get(name).is_some_and(|slots|slots.iter().any(|slot|self.call_stack.contains(slot))),
                    Condition::Assertion{child,positive,behind}=>{
                        if *behind {
                            let mut succeeded=false;
                            for start in (0..=state.pos).rev() {
                                let candidate=State{pos:start,..state.clone()};
                                if self.run(child,candidate).iter().any(|end|end.pos==state.pos){succeeded=true;break;}
                                if self.exhausted{break;}
                            }
                            succeeded==*positive
                        }else{(!self.run(child,state.clone()).is_empty())==*positive}
                    }
                };
                self.run(if test{yes}else{no},state)
            },
            Expr::Lookbehind{child,positive,non_atomic}=>{
                // Variable-width lookbehind uses bounded search, without a
                // fixed 256-character cutoff. The global step budget bounds
                // adversarial inputs; successful captures remain accessible.
                let mut match_states=Vec::new();
                for start in (0..=state.pos).rev(){
                    let candidate=State{pos:start,..state.clone()};
                    for found in self.run(child,candidate).into_iter().filter(|end|end.pos==state.pos){
                        match_states.push(found);
                        if !*non_atomic {break;}
                    }
                    if !*non_atomic&&!match_states.is_empty(){break;}
                    if self.exhausted{break;}
                }
                if *positive{match_states.into_iter().map(|mut s|{s.pos=state.pos;s}).collect()}
                else if match_states.is_empty(){vec![state]}else{Vec::new()}
            },
            Expr::Repeat{child,min,max,greedy}=>{
                if state.accepted{return vec![state];}
                let cap=max.unwrap_or(self.input.len().saturating_sub(state.pos).saturating_add(*min).saturating_add(1))
                    .min(self.input.len().saturating_add(*min).saturating_add(1));
                let mut output=Vec::new();self.repeat(child,state,0,*min,cap,*greedy,&mut output);output
            }
        }
    }
    fn repeat(&mut self,child:&Expr,state:State,depth:usize,min:usize,max:usize,greedy:bool,output:&mut Vec<State>){
        // Iterative DFS avoids stack overflow on long quantifiers. Branch
        // ordering preserves greedy/lazy preference and captured groups.
        enum Visit { Explore(State,usize), Emit(State) }
        let mut work=vec![Visit::Explore(state,depth)];
        while let Some(item)=work.pop(){
            if !self.tick(){break;}
            if work.len()>self.max_states.saturating_mul(8){self.exhausted=true;break;}
            if output.len()>=self.max_states{self.exhausted=true;break;}
            match item{
                Visit::Emit(s)=>output.push(s),
                Visit::Explore(s,count)=>{
                    if s.accepted{output.push(s);continue;}
                    if greedy&&count>=min{work.push(Visit::Emit(s.clone()));}
                    let mut continuation=Vec::new();
                    if count<max{
                        for next in self.run(child,s.clone()){
                            // Empty repetitions can satisfy a finite minimum
                            // but must not loop after that requirement is met.
                            if next.pos==s.pos && count>=min{continue;}
                            continuation.push(Visit::Explore(next,count+1));
                        }
                    }
                    for next in continuation.into_iter().rev(){work.push(next);}
                    if !greedy&&count>=min{output.push(s);}
                }
            }
        }
    }
}
impl Pattern {
    pub fn new(expression:&str)->io::Result<Self>{Self::build(expression,false)}
    /// Enable PCRE2_ALT_EXTENDED_CLASS (UTS #18 nested set operations) at
    /// compile time. This option is deliberately not a pattern directive.
    pub fn new_with_alt_extended_classes(expression:&str)->io::Result<Self>{
        Self::build(expression,true)
    }
    fn build(expression:&str,alt_extended_class:bool)->io::Result<Self>{
        let mut p=Parser{chars:expression.chars().collect(),at:0,captures:0,names:BTreeMap::new(),names_all:BTreeMap::new(),group_bodies:BTreeMap::new(),flags:Flags::default(),match_limit:None,depth_limit:None,heap_limit_kib:None,parse_depth:0,alt_extended_class,uts_depth:0,not_empty:false,not_empty_atstart:false,anchored:false};
        let expr=p.parse_choice()?;
        if p.at!=p.chars.len(){return Err(err("unmatched closing parenthesis"));}
        // Reject invalid subroutine targets at compile time, including calls
        // forward to captures that never materialize.
        fn check_calls(e:&Expr,groups:&BTreeMap<usize,Expr>,names:&BTreeMap<String,usize>,captures:usize)->io::Result<()> {
            match e{
                Expr::Call(n) if *n!=0&&!groups.contains_key(n)=>Err(err("unknown regex subroutine number")),
                Expr::Backref(n) if *n==0||*n>captures=>Err(err("unknown regex backreference number")),
                Expr::CallName(n) if !names.contains_key(n)=>Err(err("unknown regex subroutine name")),
                Expr::NamedBackref(n) if !names.contains_key(n)=>Err(err("unknown named backreference")),
                Expr::ScanSubstring{references,child}=>{
                    for reference in references{
                        match reference{
                            ScanReference::Number(n) if *n==0||*n>captures=>return Err(err("invalid scan-substring group number")),
                            ScanReference::Name(name) if !names.contains_key(name)=>return Err(err("unknown scan-substring group name")),
                            _=>{},
                        }
                    }
                    check_calls(child,groups,names,captures)
                },
                Expr::Seq(xs)|Expr::Choice(xs)=>{for x in xs{check_calls(x,groups,names,captures)?;}Ok(())},
                Expr::Repeat{child,..}|Expr::Capture{child,..}|Expr::Atomic(child)|Expr::ScopedFlags{child,..}|Expr::Lookahead{child,..}|Expr::Lookbehind{child,..}|Expr::ScriptRun{child,..}=>check_calls(child,groups,names,captures),
                Expr::Conditional{condition,yes,no}=>{
                    if let Condition::Assertion{child,..}=condition {check_calls(child,groups,names,captures)?;}
                    check_calls(yes,groups,names,captures)?;check_calls(no,groups,names,captures)
                },
                _=>Ok(()),
            }
        }
        check_calls(&expr,&p.group_bodies,&p.names,p.captures)?;
        let step_limit=p.match_limit.unwrap_or(250_000).clamp(1,20_000_000);
        let depth_limit=p.depth_limit.unwrap_or(96).clamp(1,1024);
        // Conservative cap on simultaneous match-state allocations. This is
        // not a byte-for-byte replica of PCRE2's allocator/heap accounting.
        let approx_state_bytes=std::mem::size_of::<State>()
            .saturating_add((p.captures+1).saturating_mul(std::mem::size_of::<Option<(usize,usize)>>()));
        let state_limit=p.heap_limit_kib.map(|kb|kb.saturating_mul(1024)/approx_state_bytes.max(1))
            .unwrap_or(2048).clamp(1,2048);
        Ok(Self{expr,flags:p.flags,captures:p.captures,names:p.names,names_all:p.names_all,
            group_bodies:p.group_bodies,step_limit,state_limit,depth_limit,
            not_empty:p.not_empty,not_empty_atstart:p.not_empty_atstart,anchored:p.anchored})
    }
    pub fn with_step_limit(mut self,limit:usize)->Self{self.step_limit=limit.clamp(1,20_000_000);self}
    /// Bound the number of alternate backtracking paths retained at once.
    pub fn with_state_limit(mut self,limit:usize)->Self{self.state_limit=limit.clamp(1,1_000_000);self}
    pub fn with_depth_limit(mut self,limit:usize)->Self{self.depth_limit=limit.clamp(1,1024);self}
    pub fn try_find(&self,text:&str)->io::Result<Option<(usize,usize)>>{
        Ok(self.try_captures_from(text,0)?.map(|m|m.range))
    }
    pub fn is_match(&self,text:&str)->bool{self.find(text).is_some()}
    /// Return a match or a diagnostic. Unlike `captures_from`, the fallible
    /// API distinguishes no match from a step-budget exhaustion and invalid
    /// UTF-8 offsets, so callers cannot silently accept partial search work.
    pub fn try_captures_from(&self,text:&str,start_byte:usize)->io::Result<Option<Captures>>{
        self.try_captures_from_impl(text,start_byte,None,false,false)
    }
    /// Execute pure-Rust PCRE-style callouts. Callbacks run synchronously
    /// during regex evaluation and may be invoked more than once on backtrack.
    /// Aborting returns `ErrorKind::Interrupted`, while rejecting a candidate
    /// causes backtracking to try another path.
    pub fn try_captures_from_with_callout(
        &self,text:&str,start_byte:usize,
        callback:&mut dyn FnMut(&CalloutInfo)->CalloutDecision,
    )->io::Result<Option<Captures>>{
        self.try_captures_from_impl(text,start_byte,Some(callback),false,false)
    }
    fn try_captures_from_impl(
        &self,text:&str,start_byte:usize,
        callback:Option<&mut dyn FnMut(&CalloutInfo)->CalloutDecision>,
        force_anchored:bool,force_not_empty_atstart:bool,
    )->io::Result<Option<Captures>>{
        if start_byte>text.len()||!text.is_char_boundary(start_byte){
            return Err(err("regex start is not a UTF-8 character boundary"));
        }
        let chars:Vec<char>=text.chars().collect();
        let mut byte_offsets=text.char_indices().map(|(i,_)|i).collect::<Vec<_>>();byte_offsets.push(text.len());
        let first=byte_offsets.binary_search(&start_byte).map_err(|_|err("invalid regex start offset"))?;
        let mut grapheme_boundaries=vec![false;chars.len()+1];
        let mut seen=0usize;
        for cluster in crate::unicode::graphemes(text){
            grapheme_boundaries[seen]=true;
            seen+=cluster.text.chars().count();
        }
        grapheme_boundaries[chars.len()]=true;
        let mut engine=Engine{input:&chars,flags:self.flags,steps:0,max_steps:self.step_limit,max_states:self.state_limit,
            grapheme_boundaries:&grapheme_boundaries,exhausted:false,
            depth:0,max_depth:self.depth_limit,root:&self.expr,groups:&self.group_bodies,names:&self.names,
            start_offset:first,skip_to:None,committed:false,cut:None,
            call_stack:Vec::new(),names_all:&self.names_all,
            byte_offsets:&byte_offsets,callout:callback,aborted:false};
        let mut start=first;
        while start<=chars.len(){
            engine.skip_to=None;
            let state=State{pos:start,match_start:start,accepted:false,mark:None,
                marks:Vec::new(),cut:None,groups:vec![None;self.captures+1]};
            let candidates=engine.run(&self.expr,state);
            if engine.aborted{
                return Err(io::Error::new(ErrorKind::Interrupted,"regex callout requested abort"));
            }
            if engine.exhausted{
                return Err(io::Error::new(ErrorKind::TimedOut,"regex backtracking budget exhausted"));
            }
            if let Some(mut match_state)=candidates.into_iter().find(|s|{
                let empty=s.match_start==s.pos;
                !(empty&&(self.not_empty||((self.not_empty_atstart||force_not_empty_atstart) && start==first)))
            }){
                if match_state.match_start>match_state.pos{return Err(err("regex \\K match start is after end"));}
                match_state.groups[0]=Some((match_state.match_start,match_state.pos));
                return Ok(Some(Captures{
                    range:(byte_offsets[match_state.match_start],byte_offsets[match_state.pos]),
                    groups:match_state.groups.into_iter().map(|v|v.map(|(a,b)|(byte_offsets[a],byte_offsets[b]))).collect(),
                    names:self.names.clone(),names_all:self.names_all.clone(),mark:match_state.mark,
                }));
            }
            if engine.exhausted{
                return Err(io::Error::new(ErrorKind::TimedOut,"regex backtracking budget exceeded"));
            }
            if engine.committed {break;}
            if self.anchored || force_anchored {break;}
            start=engine.skip_to.unwrap_or(start+1).max(start+1);
        }
        Ok(None)
    }
    /// Legacy non-fallible search API, retained for source compatibility.
    /// For untrusted patterns, prefer `try_captures_from`.
    pub fn captures_from(&self,text:&str,start_byte:usize)->Option<Captures>{
        self.try_captures_from(text,start_byte).ok().flatten()
    }
    pub fn captures(&self,text:&str)->Option<Captures>{self.captures_from(text,0)}
    pub fn find(&self,text:&str)->Option<(usize,usize)>{self.captures(text).map(|c|c.range)}
    /// Global replacement with the PCRE2-compatible empty-match retry:
    /// after an empty match, first attempt an ANCHORED|NOTEMPTY_ATSTART match
    /// at that same position before advancing one Unicode scalar.
    fn try_replace_all_inner(&self,text:&str,replacement:&str,extended:bool)
        ->io::Result<(String,usize)>{
        let mut output=String::new();
        let mut at=0usize;
        let mut count=0usize;
        let mut retry_nonempty=false;
        while at<=text.len(){
            let caps=if retry_nonempty {
                self.try_captures_from_impl(text,at,None,true,true)?
            }else{self.try_captures_from(text,at)?};
            let Some(caps)=caps else{
                if retry_nonempty {
                    retry_nonempty=false;
                    if let Some(ch)=text[at..].chars().next(){
                        output.push(ch);at+=ch.len_utf8();continue;
                    }
                }
                break;
            };
            let (begin,end)=caps.range;
            if begin<at{return Err(err("non-monotonic regex match"));}
            output.push_str(&text[at..begin]);
            if extended {output.push_str(&caps.expand_extended(text,replacement));}
            else{output.push_str(&caps.expand(text,replacement));}
            count+=1;
            retry_nonempty=begin==end;
            at=end;
        }
        output.push_str(&text[at..]);
        Ok((output,count))
    }
    /// Fallible global replacement, with an explicit limit diagnostic.
    pub fn try_replace_all(&self,text:&str,replacement:&str)->io::Result<(String,usize)>{
        self.try_replace_all_inner(text,replacement,false)
    }
    /// PCRE2_SUBSTITUTE_EXTENDED-style replacements.
    pub fn try_replace_all_extended(&self,text:&str,replacement:&str)->io::Result<(String,usize)>{
        self.try_replace_all_inner(text,replacement,true)
    }
    pub fn replace_all(&self,text:&str,replacement:&str)->(String,usize){
        self.replace_with(text,|caps|caps.expand(text,replacement))
    }
    /// Compatibility API. For errors from untrusted patterns use the fallible
    /// replacement variants, which return diagnostic information.
    pub fn replace_with(&self,text:&str,mut replace:impl FnMut(&Captures)->String)->(String,usize){
        let mut result=String::new();let mut at=0;let mut count=0;
        let mut retry_nonempty=false;
        while at<=text.len(){
            let caps=if retry_nonempty{
                self.try_captures_from_impl(text,at,None,true,true).ok().flatten()
            }else{self.captures_from(text,at)};
            let Some(caps)=caps else{
                if retry_nonempty {
                    retry_nonempty=false;
                    if let Some(ch)=text[at..].chars().next(){
                        result.push(ch);at+=ch.len_utf8();continue;
                    }
                }
                break;
            };
            let (begin,end)=caps.range;
            if begin<at{break;}
            result.push_str(&text[at..begin]);
            result.push_str(&replace(&caps));count+=1;
            retry_nonempty=begin==end;
            at=end;
        }
        result.push_str(&text[at..]);(result,count)
    }
}
