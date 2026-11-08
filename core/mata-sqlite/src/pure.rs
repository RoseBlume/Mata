//! Pure Rust SQLite-format rowid-table engine for Mata applications.
//!
//! IMPORTANT: This is a deliberately limited SQL engine, not the complete
//! SQLite virtual machine. It supports StudyForge's current SQL subset and
//! rewrites SQLite-format files using an atomic shadow file. Unsupported SQL,
//! live WAL, or unsupported database features must error rather than risk data.
//!
//! There are no C calls and no unsafe code in this crate.
use crate::error::SqliteError;

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub type Result<T> = std::result::Result<T, SqliteError>;
const MAGIC: &[u8; 16] = b"SQLite format 3\0";
const PAGE_SIZE: usize = 4096;

/// A nonempty WAL must be checkpointed by the previous SQLite owner before
/// this snapshot writer runs. Deleting or ignoring it can roll back real data.
fn reject_live_wal(path:&Path)->Result<()> {
    for ext in ["-wal", "-shm"] {
        let companion=PathBuf::from(format!("{}{ext}",path.display()));
        if companion.exists() && fs::metadata(&companion)?.len() > 0 {
            return Err(SqliteError::Sql(format!(
                "{} exists: checkpoint and close the SQLite WAL database before using the pure-Rust writer; no data was changed",companion.display()
            )));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value { Null, Integer(i64), Real(f64), Text(String), Blob(Vec<u8>) }
impl From<&str> for Value { fn from(v: &str) -> Self { Self::Text(v.to_owned()) } }
impl From<String> for Value { fn from(v: String) -> Self { Self::Text(v) } }
impl From<&String> for Value { fn from(v: &String) -> Self { Self::Text(v.clone()) } }
impl From<i64> for Value { fn from(v: i64) -> Self { Self::Integer(v) } }
impl From<i32> for Value { fn from(v: i32) -> Self { Self::Integer(v as i64) } }
impl From<bool> for Value { fn from(v: bool) -> Self { Self::Integer(if v {1}else{0}) } }
impl From<u32> for Value { fn from(v: u32) -> Self { Self::Integer(v as i64) } }
impl From<usize> for Value { fn from(v: usize) -> Self { Self::Integer(v as i64) } }
impl From<f64> for Value { fn from(v: f64) -> Self { Self::Real(v) } }
impl From<Vec<u8>> for Value { fn from(v: Vec<u8>) -> Self { Self::Blob(v) } }
impl From<&[u8]> for Value { fn from(v: &[u8]) -> Self { Self::Blob(v.to_vec()) } }

#[macro_export]
macro_rules! params {
    () => { Vec::<$crate::Value>::new() };
    ($($x:expr),+ $(,)?) => {{ let mut v = Vec::<$crate::Value>::new(); $(v.push($crate::Value::from($x));)+ v }};
}

pub trait Params { fn into_values(self) -> Vec<Value>; }
impl Params for Vec<Value> { fn into_values(self) -> Vec<Value> { self } }
impl Params for [(); 0] { fn into_values(self) -> Vec<Value> { Vec::new() } }
impl<const N: usize> Params for [Value; N] { fn into_values(self) -> Vec<Value> { self.into() } }

pub trait OptionalExtension<T> { fn optional(self) -> Result<Option<T>>; }
impl<T> OptionalExtension<T> for Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self { Err(SqliteError::QueryReturnedNoRows) => Ok(None), Ok(v) => Ok(Some(v)), Err(e) => Err(e) }
    }
}

pub trait FromValue: Sized { fn from_value(v: &Value) -> Result<Self>; }
impl FromValue for String { fn from_value(v: &Value) -> Result<Self> { match v { Value::Text(s)=>Ok(s.clone()), _=>Err(SqliteError::Type(format!("expected text, got {v:?}"))) } } }
impl FromValue for i64 { fn from_value(v: &Value) -> Result<Self> { match v { Value::Integer(n)=>Ok(*n), _=>Err(SqliteError::Type(format!("expected integer, got {v:?}"))) } } }
impl FromValue for i32 { fn from_value(v: &Value) -> Result<Self> { i64::from_value(v)?.try_into().map_err(|_|SqliteError::Type("integer out of range".into())) } }
impl<T:FromValue> FromValue for Option<T> {fn from_value(v:&Value)->Result<Self>{if matches!(v,Value::Null){Ok(None)}else{T::from_value(v).map(Some)}}}
impl FromValue for u32 { fn from_value(v: &Value) -> Result<Self> { Ok(i64::from_value(v)? as u32) } }
impl FromValue for usize { fn from_value(v: &Value) -> Result<Self> { Ok(i64::from_value(v)? as usize) } }
impl FromValue for f64 { fn from_value(v: &Value) -> Result<Self> { match v { Value::Real(n)=>Ok(*n), Value::Integer(n)=>Ok(*n as f64), _=>Err(SqliteError::Type(format!("expected number, got {v:?}"))) } } }
impl FromValue for Vec<u8> { fn from_value(v: &Value) -> Result<Self> { match v { Value::Blob(b)=>Ok(b.clone()), _=>Err(SqliteError::Type(format!("expected blob, got {v:?}"))) } } }

#[derive(Clone, Debug)]
pub struct Row { values: Vec<Value> }
pub trait RowIndex { fn index(self) -> usize; }
impl RowIndex for usize { fn index(self) -> usize { self } }
impl RowIndex for i32 { fn index(self) -> usize { self.max(0) as usize } }
impl Row { pub fn get<I: RowIndex, T: FromValue>(&self, index: I) -> Result<T> { let i=index.index(); self.values.get(i).ok_or_else(||SqliteError::Type(format!("column {i} out of range"))).and_then(T::from_value) } }

#[derive(Clone, Debug)]
struct Column { name: String }
#[derive(Clone, Debug)]
struct TableSchema { name: String, sql: String, columns: Vec<Column>, pk: Option<usize>, rowid_alias: bool, foreign_keys: Vec<ForeignKey> }
#[derive(Clone, Debug)]
struct ForeignKey { column: usize, parent: String, parent_column: String, cascade: bool }
#[derive(Clone, Debug)]
struct DataRow { rowid: i64, values: Vec<Value> }
#[derive(Clone, Debug)]
struct Table { schema: TableSchema, rows: Vec<DataRow>, next_rowid: i64 }
#[derive(Clone, Debug, Default)]
struct Database { tables: HashMap<String, Table> }

/// Process-local coordination is insufficient for database mutation: an
/// exclusive sidecar prevents two Mata instances from racing to replace a file.
/// This is not an SQLite-compatible advisory lock; external SQLite processes
/// must still be closed before enabling this backend.
struct WriteGuard { path:PathBuf, _handle:File }
impl WriteGuard {
    fn acquire(path:&Path)->Result<Self> {
        let lock=PathBuf::from(format!("{}-mata-lock",path.display()));
        if let Some(parent)=lock.parent(){ fs::create_dir_all(parent)?; }
        let handle=OpenOptions::new().write(true).create_new(true).open(&lock)
            .map_err(|e|SqliteError::Sql(format!("unable to acquire exclusive Mata writer lock {}: {e}",lock.display())))?;
        Ok(Self {path:lock,_handle:handle})
    }
}
impl Drop for WriteGuard {fn drop(&mut self){let _=fs::remove_file(&self.path);}}

pub struct Connection { path: PathBuf, db: RefCell<Database>, dirty: Cell<bool>, in_tx: Cell<bool>, _guard:WriteGuard }
impl Connection {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        reject_live_wal(&path)?;
        let guard=WriteGuard::acquire(&path)?;
        let db = if path.exists() && fs::metadata(&path)?.len() > 0 { read_database(&path)? } else { Database::default() };
        Ok(Self { path, db: RefCell::new(db), dirty: Cell::new(false), in_tx: Cell::new(false), _guard:guard })
    }
    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        for stmt in split_statements(sql) {
            let s=stmt.trim(); if s.is_empty() {continue}
            if s.to_ascii_uppercase().starts_with("PRAGMA ") {
                let normalized=s.to_ascii_uppercase().replace(' ', "");
                if normalized.starts_with("PRAGMAFOREIGN_KEYS=")
                    || normalized=="PRAGMAJOURNAL_MODE=DELETE"
                    || normalized=="PRAGMASYNCHRONOUS=FULL" { continue; }
                return Err(SqliteError::Sql(format!("unsupported PRAGMA in pure Rust backend: {s}")));
            }
            if s.to_ascii_uppercase().starts_with("CREATE INDEX ") { continue; }
            if s.to_ascii_uppercase().starts_with("CREATE TABLE ") { self.create_table(s)?; } else { self.execute(s, [(); 0])?; }
        }
        self.flush_if_needed()
    }
    fn create_table(&self, sql: &str) -> Result<()> {
        let schema=parse_create_table(sql)?; let key=schema.name.to_ascii_lowercase();
        let mut db=self.db.borrow_mut();
        db.tables.entry(key).or_insert_with(|| Table{schema,rows:Vec::new(),next_rowid:1});
        self.dirty.set(true); Ok(())
    }
    pub fn execute<P: Params>(&self, sql: &str, params: P) -> Result<usize> {
        let n=execute_sql(&mut self.db.borrow_mut(),sql,&params.into_values())?; self.dirty.set(true); self.flush_if_needed()?; Ok(n)
    }
    pub fn prepare<'a>(&'a self, sql: &str) -> Result<Statement<'a>> { Ok(Statement{db:&self.db,sql:sql.to_owned()}) }
    pub fn query_row<P,F,T>(&self, sql:&str, params:P, f:F)->Result<T> where P:Params,F:FnOnce(&Row)->Result<T> { let rows=query_sql(&self.db.borrow(),sql,&params.into_values())?; let row=rows.first().ok_or(SqliteError::QueryReturnedNoRows)?; f(row) }
    pub fn transaction(&mut self) -> Result<Transaction<'_>> {
        if self.in_tx.replace(true) { return Err(SqliteError::Sql("nested transactions are not supported".into())); }
        let backup=self.db.borrow().clone(); Ok(Transaction{conn:self,backup:Some(backup),committed:false})
    }
    fn flush_if_needed(&self)->Result<()> { if self.dirty.get() && !self.in_tx.get() {
        reject_live_wal(&self.path)?;
        write_database_atomic(&self.path,&self.db.borrow())?;
        self.dirty.set(false);
    } Ok(()) }
}

pub struct Transaction<'a>{conn:&'a mut Connection,backup:Option<Database>,committed:bool}
impl<'a> Transaction<'a>{
    pub fn execute<P:Params>(&self,sql:&str,params:P)->Result<usize>{let n=execute_sql(&mut self.conn.db.borrow_mut(),sql,&params.into_values())?;self.conn.dirty.set(true);Ok(n)}
    pub fn execute_batch(&self,sql:&str)->Result<()> { for s in split_statements(sql){ if !s.trim().is_empty(){self.execute(s,[(); 0])?;} } Ok(()) }
    pub fn query_row<P,F,T>(&self,sql:&str,params:P,f:F)->Result<T> where P:Params,F:FnOnce(&Row)->Result<T>{let rows=query_sql(&self.conn.db.borrow(),sql,&params.into_values())?;let r=rows.first().ok_or(SqliteError::QueryReturnedNoRows)?;f(r)}
    pub fn prepare<'b>(&'b self,sql:&str)->Result<Statement<'b>>{Ok(Statement{db:&self.conn.db,sql:sql.to_owned()})}
    pub fn commit(mut self)->Result<()> { reject_live_wal(&self.conn.path)?;write_database_atomic(&self.conn.path,&self.conn.db.borrow())?;self.conn.dirty.set(false);self.conn.in_tx.set(false);self.committed=true;self.backup=None;Ok(()) }
}
impl Drop for Transaction<'_>{fn drop(&mut self){if !self.committed { if let Some(db)=self.backup.take(){*self.conn.db.borrow_mut()=db;} self.conn.dirty.set(false);self.conn.in_tx.set(false);}}}

pub struct Statement<'a>{db:&'a RefCell<Database>,sql:String}
impl<'a> Statement<'a>{
    fn rows<P:Params>(&self,p:P)->Result<Vec<Row>>{query_sql(&self.db.borrow(),&self.sql,&p.into_values())}
    pub fn query_map<P,F,T>(&mut self,p:P,mut f:F)->Result<MappedRows<T>> where P:Params,F:FnMut(&Row)->Result<T>{let mut out=Vec::new();for r in self.rows(p)?{out.push(f(&r));}Ok(MappedRows{inner:out.into_iter()})}
}
pub struct MappedRows<T>{inner:std::vec::IntoIter<Result<T>>}
impl<T> Iterator for MappedRows<T>{type Item=Result<T>;fn next(&mut self)->Option<Self::Item>{self.inner.next()}}

fn split_statements(sql:&str)->Vec<&str>{let mut out=Vec::new();let mut start=0;let mut depth=0;let mut quote=false;for(i,c)in sql.char_indices(){match c{'\''=>quote=!quote,'(' if !quote=>depth+=1,')' if !quote=>depth-=1,';' if !quote&&depth==0=>{out.push(&sql[start..i]);start=i+1},_=>{}}}if start<sql.len(){out.push(&sql[start..]);}out}
fn split_top(s:&str,sep:char)->Vec<String>{let mut o=Vec::new();let(mut st,mut d,mut q)=(0,0,false);for(i,c)in s.char_indices(){match c{'\''=>q=!q,'(' if !q=>d+=1,')' if !q=>d-=1,_ if c==sep&&!q&&d==0=>{o.push(s[st..i].trim().to_owned());st=i+c.len_utf8()},_=>{}}}o.push(s[st..].trim().to_owned());o}

fn parse_create_table(sql:&str)->Result<TableSchema>{
    let up=sql.to_ascii_uppercase(); let mut p=up.find("CREATE TABLE").ok_or_else(||SqliteError::Sql("bad CREATE TABLE".into()))?+12;
    while sql.as_bytes().get(p).map_or(false,|b|b.is_ascii_whitespace()){p+=1}
    if up[p..].starts_with("IF NOT EXISTS") {p+=13;while sql.as_bytes().get(p).map_or(false,|b|b.is_ascii_whitespace()){p+=1}}
    let open=sql[p..].find('(').ok_or_else(||SqliteError::Sql("CREATE TABLE missing (".into()))?+p;let name=sql[p..open].trim().trim_matches('"').to_owned();let close=sql.rfind(')').ok_or_else(||SqliteError::Sql("CREATE TABLE missing )".into()))?;
    let defs=split_top(&sql[open+1..close],',');let mut columns=Vec::new();let mut pk=None;let mut rowid_alias=false;let mut fks=Vec::new();
    for def in defs {let du=def.to_ascii_uppercase();if du.starts_with("FOREIGN KEY")||du.starts_with("CONSTRAINT")||du.starts_with("CHECK")||du.starts_with("UNIQUE")||du.starts_with("PRIMARY KEY"){continue}let cname=def.split_whitespace().next().unwrap_or("").trim_matches('"').to_owned();if cname.is_empty(){continue}let idx=columns.len();if du.contains("PRIMARY KEY"){pk=Some(idx);rowid_alias=du.split_whitespace().nth(1)==Some("INTEGER")};if let Some(rp)=du.find("REFERENCES "){let tail=&def[rp+11..];let parent=tail.split(|c:char| c=='(' || c.is_whitespace()).next().unwrap_or("").trim().to_owned();let pc=tail.find('(').and_then(|a|tail[a+1..].find(')').map(|b|tail[a+1..a+1+b].trim().to_owned())).unwrap_or_else(||"id".into());fks.push(ForeignKey{column:idx,parent,parent_column:pc,cascade:du.contains("ON DELETE CASCADE")});}columns.push(Column{name:cname});}
    if pk.is_none(){pk=columns.iter().position(|c|c.name.eq_ignore_ascii_case("id"));}
    if up.contains("WITHOUT ROWID") {return Err(SqliteError::Sql("WITHOUT ROWID tables are unsupported".into()));}
    Ok(TableSchema{name,sql:sql.trim().to_owned(),columns,pk,rowid_alias,foreign_keys:fks})
}

fn execute_sql(db:&mut Database,sql:&str,params:&[Value])->Result<usize>{let s=sql.trim();let up=s.to_ascii_uppercase();if up.starts_with("INSERT "){exec_insert(db,s,params)}else if up.starts_with("UPDATE "){exec_update(db,s,params)}else if up.starts_with("DELETE "){exec_delete(db,s,params)}else if up.starts_with("PRAGMA ")||up.starts_with("CREATE INDEX "){Ok(0)}else if up.starts_with("CREATE TABLE "){let sch=parse_create_table(s)?;db.tables.entry(sch.name.to_ascii_lowercase()).or_insert(Table{schema:sch,rows:vec![],next_rowid:1});Ok(0)}else{Err(SqliteError::Sql(format!("unsupported statement: {s}")))}}
fn parameter(token:&str,params:&[Value])->Result<Value>{let t=token.trim();if let Some(n)=t.strip_prefix('?'){let i=n.parse::<usize>().unwrap_or(1);return params.get(i-1).cloned().ok_or_else(||SqliteError::Sql(format!("missing parameter ?{i}")));}if t.eq_ignore_ascii_case("NULL"){return Ok(Value::Null)}if t.starts_with('\'')&&t.ends_with('\''){return Ok(Value::Text(t[1..t.len()-1].replace("''","'")))}if let Ok(n)=t.parse::<i64>(){return Ok(Value::Integer(n))}Err(SqliteError::Sql(format!("unsupported value {t}")))}
fn validate_foreign_keys(db:&Database,schema:&TableSchema, values:&[Value])->Result<()> {
    for fk in &schema.foreign_keys {
        let Some(value)=values.get(fk.column) else {continue};
        if *value==Value::Null {continue;}
        let parent=db.tables.get(&fk.parent.to_ascii_lowercase()).ok_or_else(||
            SqliteError::Sql(format!("missing referenced table {}",fk.parent)))?;
        let index=col(parent,&fk.parent_column)?;
        if !parent.rows.iter().any(|row|row.values.get(index)==Some(value)) {
            return Err(SqliteError::Sql(format!(
                "FOREIGN KEY constraint failed for {}",schema.name)));
        }
    }
    Ok(())
}

fn exec_insert(db:&mut Database, sql:&str, params:&[Value])->Result<usize> {
    let upper=sql.to_ascii_uppercase();
    let replace=upper.starts_with("INSERT OR REPLACE");
    let ignore=upper.starts_with("INSERT OR IGNORE");
    let into=upper.find(" INTO ").ok_or_else(||SqliteError::Sql("INSERT missing INTO".into()))?;
    let after=into+6;
    let open=sql[after..].find('(').map(|i|i+after)
        .ok_or_else(||SqliteError::Sql("INSERT column list missing".into()))?;
    let table=sql[after..open].trim().to_ascii_lowercase();
    let close=sql[open..].find(')').map(|i|i+open)
        .ok_or_else(||SqliteError::Sql("INSERT column list is incomplete".into()))?;
    let cols:Vec<_>=sql[open+1..close].split(',').map(str::trim).collect();
    let vpos=upper[close..].find("VALUES").map(|i|i+close)
        .ok_or_else(||SqliteError::Sql("INSERT missing VALUES".into()))?;
    let vo=sql[vpos..].find('(').map(|i|i+vpos)
        .ok_or_else(||SqliteError::Sql("VALUES missing (".into()))?;
    let vc=sql[vo..].rfind(')').map(|i|i+vo)
        .ok_or_else(||SqliteError::Sql("VALUES missing )".into()))?;
    let values=split_top(&sql[vo+1..vc],',');
    if cols.len()!=values.len(){return Err(SqliteError::Sql("INSERT column/value count mismatch".into()));}
    let schema=db.tables.get(&table).ok_or_else(||SqliteError::Sql(format!("no such table: {table}")))?.schema.clone();
    let mut row=vec![Value::Null;schema.columns.len()];
    for (name,token) in cols.iter().zip(values) {
        let index=schema.columns.iter().position(|c|c.name.eq_ignore_ascii_case(name))
            .ok_or_else(||SqliteError::Sql(format!("unknown column {name}")))?;
        row[index]=parameter(&token,params)?;
    }
    validate_foreign_keys(db,&schema,&row)?;
    let t=db.tables.get_mut(&table).unwrap();
    if let Some(pk)=schema.pk {
        if row[pk]!=Value::Null {
            if let Some(index)=t.rows.iter().position(|old|old.values[pk]==row[pk]) {
                if ignore {return Ok(0);}
                if replace {t.rows.remove(index);}
                else {return Err(SqliteError::Sql("UNIQUE constraint failed".into()));}
            }
        }
    }
    let rid=if schema.rowid_alias {
        schema.pk.and_then(|pk|match row.get(pk){Some(Value::Integer(id))=>Some(*id),_=>None})
            .unwrap_or(t.next_rowid)
    }else{t.next_rowid};
    if rid<=0 {return Err(SqliteError::Sql("invalid INTEGER PRIMARY KEY".into()));}
    t.next_rowid=t.next_rowid.max(rid.saturating_add(1));
    if schema.rowid_alias {if let Some(pk)=schema.pk {row[pk]=Value::Integer(rid);}}
    t.rows.push(DataRow{rowid:rid,values:row});
    Ok(1)
}
fn parse_where(where_s:&str,p:&[Value])->Result<Vec<(String,Value,bool)>>{let mut out=Vec::new();for part in where_s.split(" AND "){let(eq,ci)=if let Some(i)=part.find('='){(i,false)}else{return Err(SqliteError::Sql(format!("unsupported WHERE {part}")))};let l=part[..eq].trim();let r=part[eq+1..].trim();if l.to_ascii_lowercase().starts_with("lower("){let name=l[6..].trim_end_matches(')').split('.').last().unwrap().to_owned();let rv=r.strip_prefix("lower(").and_then(|x|x.strip_suffix(')')).unwrap_or(r);out.push((name,parameter(rv,p)?,true));}else{out.push((l.split('.').last().unwrap().to_owned(),parameter(r,p)?,ci));}}Ok(out)}
fn row_matches(t:&Table,r:&DataRow,cond:&[(String,Value,bool)])->bool{cond.iter().all(|(c,v,lower)|{let Some(i)=t.schema.columns.iter().position(|x|x.name.eq_ignore_ascii_case(c))else{return false};if *lower{match(&r.values[i],v){(Value::Text(a),Value::Text(b))=>a.eq_ignore_ascii_case(b),_=>false}}else{r.values[i]==*v}})}
fn exec_update(db:&mut Database,s:&str,p:&[Value])->Result<usize>{let up=s.to_ascii_uppercase();let setp=up.find(" SET ").ok_or_else(||SqliteError::Sql("UPDATE missing SET".into()))?;let table=s[6..setp].trim();let wp=up.find(" WHERE ");let set_end=wp.unwrap_or(s.len());let assigns=split_top(&s[setp+5..set_end],',');let cond=if let Some(w)=wp{parse_where(&s[w+7..],p)?}else{vec![]};let t=db.tables.get_mut(&table.to_ascii_lowercase()).ok_or_else(||SqliteError::Sql(format!("no table {table}")))?;let mut changes=Vec::new();for a in assigns{let i=a.find('=').ok_or_else(||SqliteError::Sql("bad assignment".into()))?;let c=a[..i].trim();let ci=t.schema.columns.iter().position(|x|x.name.eq_ignore_ascii_case(c)).ok_or_else(||SqliteError::Sql(format!("unknown column {c}")))?;changes.push((ci,parameter(&a[i+1..],p)?));}let resolved=resolve_conditions(t,&cond)?;let mut n=0;for r in &mut t.rows{if row_matches_resolved(r,&resolved){for(ci,v)in&changes{r.values[*ci]=v.clone()}n+=1}}Ok(n)}
fn exec_delete(db:&mut Database,s:&str,p:&[Value])->Result<usize>{let up=s.to_ascii_uppercase();let from=up.find("FROM ").ok_or_else(||SqliteError::Sql("DELETE missing FROM".into()))?+5;let wp=up.find(" WHERE ");let table=s[from..wp.unwrap_or(s.len())].trim();let cond=if let Some(w)=wp{parse_where(&s[w+7..],p)?}else{vec![]};let key=table.to_ascii_lowercase();let (ids,deleted_keys)={let t=db.tables.get(&key).ok_or_else(||SqliteError::Sql(format!("no table {table}")))?;let resolved=resolve_conditions(t,&cond)?;let mut ids=Vec::new();let mut keys=Vec::new();for r in&t.rows{if row_matches_resolved(r,&resolved){ids.push(r.rowid);if let Some(pk)=t.schema.pk{keys.push(r.values[pk].clone())}}}(ids,keys)};let n=ids.len();db.tables.get_mut(&key).unwrap().rows.retain(|r|!ids.contains(&r.rowid));if !deleted_keys.is_empty(){cascade_delete(db,table,&deleted_keys)?}Ok(n)}

fn resolve_conditions(t:&Table,cond:&[(String,Value,bool)])->Result<Vec<(usize,Value,bool)>>{cond.iter().map(|(c,v,l)|Ok((t.schema.columns.iter().position(|x|x.name.eq_ignore_ascii_case(c)).ok_or_else(||SqliteError::Sql(format!("unknown column {c}")))?,v.clone(),*l))).collect()}
fn row_matches_resolved(r:&DataRow,cond:&[(usize,Value,bool)])->bool{cond.iter().all(|(i,v,lower)|if *lower{match(&r.values[*i],v){(Value::Text(a),Value::Text(b))=>a.eq_ignore_ascii_case(b),_=>false}}else{r.values[*i]==*v})}
fn cascade_delete(db:&mut Database,parent:&str,keys:&[Value])->Result<()> {let child_names:Vec<String>=db.tables.iter().filter(|(_,t)|t.schema.foreign_keys.iter().any(|f|f.cascade&&f.parent.eq_ignore_ascii_case(parent))).map(|(n,_)|n.clone()).collect();for n in child_names{let specs:Vec<_>=db.tables[&n].schema.foreign_keys.iter().filter(|f|f.cascade&&f.parent.eq_ignore_ascii_case(parent)).cloned().collect();let child_deleted:Vec<Value>;{let t=&db.tables[&n];child_deleted=t.rows.iter().filter(|r|specs.iter().any(|f|keys.contains(&r.values[f.column]))).filter_map(|r|t.schema.pk.map(|pk|r.values[pk].clone())).collect();}db.tables.get_mut(&n).unwrap().rows.retain(|r|!specs.iter().any(|f|keys.contains(&r.values[f.column])));if !child_deleted.is_empty(){let name=db.tables[&n].schema.name.clone();cascade_delete(db,&name,&child_deleted)?}}Ok(())}

fn query_sql(db:&Database,sql:&str,p:&[Value])->Result<Vec<Row>>{let norm=sql.split_whitespace().collect::<Vec<_>>().join(" ");let low=norm.to_ascii_lowercase();
    if low.contains("left join questions q") {return query_set_summaries(db,p)}
    if low.contains("join card_sets s")&&low.contains("join units u") {let sid=p.get(0).ok_or_else(||SqliteError::Sql("missing parameter".into()))?;let units=&db.tables["units"];let sets=&db.tables["card_sets"];let qs=&db.tables["questions"];let uid=col(units,"id")?;let usid=col(units,"subject_id")?;let setid=col(sets,"id")?;let suid=col(sets,"unit_id")?;let qset=col(qs,"set_id")?;let uids:Vec<_>=units.rows.iter().filter(|r|r.values[usid]==*sid).map(|r|r.values[uid].clone()).collect();let sids:Vec<_>=sets.rows.iter().filter(|r|uids.contains(&r.values[suid])).map(|r|r.values[setid].clone()).collect();let c=qs.rows.iter().filter(|r|sids.contains(&r.values[qset])).count() as i64;return Ok(vec![Row{values:vec![Value::Integer(c)]}])}
    if low.contains("join card_sets s")&&low.contains("where s.unit_id") {let uid=p.get(0).unwrap();let sets=&db.tables["card_sets"];let qs=&db.tables["questions"];let setid=col(sets,"id")?;let suid=col(sets,"unit_id")?;let qset=col(qs,"set_id")?;let ids:Vec<_>=sets.rows.iter().filter(|r|r.values[suid]==*uid).map(|r|r.values[setid].clone()).collect();let c=qs.rows.iter().filter(|r|ids.contains(&r.values[qset])).count()as i64;return Ok(vec![Row{values:vec![Value::Integer(c)]}])}
    simple_select(db,&norm,p)
}
fn col(t:&Table,name:&str)->Result<usize>{t.schema.columns.iter().position(|c|c.name.eq_ignore_ascii_case(name)).ok_or_else(||SqliteError::Sql(format!("no column {name}")))}
fn query_set_summaries(db:&Database,p:&[Value])->Result<Vec<Row>>{let uid=p.get(0).ok_or_else(||SqliteError::Sql("missing unit id".into()))?;let s=&db.tables["card_sets"];let q=&db.tables["questions"];let si=col(s,"id")?;let st=col(s,"title")?;let sd=col(s,"description")?;let su=col(s,"unit_id")?;let sp=col(s,"position")?;let qs=col(q,"set_id")?;let mut rs:Vec<_>=s.rows.iter().filter(|r|r.values[su]==*uid).collect();rs.sort_by_key(|r|(as_i64(&r.values[sp]),r.rowid));Ok(rs.into_iter().map(|r|Row{values:vec![r.values[si].clone(),r.values[st].clone(),r.values[sd].clone(),Value::Integer(q.rows.iter().filter(|x|x.values[qs]==r.values[si]).count()as i64)]}).collect())}
fn as_i64(v:&Value)->i64{if let Value::Integer(n)=v{*n}else{0}}
fn simple_select(db:&Database,s:&str,p:&[Value])->Result<Vec<Row>>{let low=s.to_ascii_lowercase();let fp=low.find(" from ").ok_or_else(||SqliteError::Sql("SELECT missing FROM".into()))?;let fields=s[6..fp].trim();let rest=&s[fp+6..];let restlow=rest.to_ascii_lowercase();let cut=[" where "," order by "," group by "," limit "].iter().filter_map(|x|restlow.find(x)).min().unwrap_or(rest.len());let table_token=rest[..cut].trim().split_whitespace().next().unwrap();let t=db.tables.get(&table_token.to_ascii_lowercase()).ok_or_else(||SqliteError::Sql(format!("no table {table_token}")))?;let wp=restlow.find(" where ");let op=restlow.find(" order by ");let lp=restlow.find(" limit ");let cond=if let Some(w)=wp{let end=[op,lp].into_iter().flatten().filter(|x|*x>w).min().unwrap_or(rest.len());parse_where(rest[w+7..end].trim(),p)?}else{vec![]};let mut rr:Vec<&DataRow>=t.rows.iter().filter(|r|row_matches(t,r,&cond)).collect();if let Some(o)=op{let end=lp.filter(|x|*x>o).unwrap_or(rest.len());let spec=rest[o+10..end].trim();let mut keys=Vec::new();for k in spec.split(','){let mut it=k.split_whitespace();let c=it.next().unwrap().split('.').last().unwrap();let desc=it.next().map(|x|x.eq_ignore_ascii_case("DESC")).unwrap_or(false);if c.eq_ignore_ascii_case("rowid"){keys.push((None,desc))}else{keys.push((Some(col(t,c)?),desc))}}rr.sort_by(|a,b|{for(k,d)in&keys{let ord=match k{None=>a.rowid.cmp(&b.rowid),Some(i)=>cmp_value(&a.values[*i],&b.values[*i])};if ord!=std::cmp::Ordering::Equal{return if *d{ord.reverse()}else{ord}}}std::cmp::Ordering::Equal});}
    if let Some(l)=lp{let n=rest[l+7..].trim().parse::<usize>().unwrap_or(usize::MAX);rr.truncate(n)}
    let fu=fields.to_ascii_uppercase();if fu=="COUNT(*)"||fu.starts_with("COUNT("){return Ok(vec![Row{values:vec![Value::Integer(rr.len()as i64)]}])}if fu.starts_with("COALESCE(MAX("){let a=fields.find("MAX(").unwrap()+4;let b=fields[a..].find(')').unwrap()+a;let ci=col(t,fields[a..b].trim())?;let m=rr.iter().map(|r|as_i64(&r.values[ci])).max().unwrap_or(-1)+1;return Ok(vec![Row{values:vec![Value::Integer(m)]}])}
    let selectors:Vec<_>=fields.split(',').map(|x|x.trim().split('.').last().unwrap().trim()).collect();let idxs:Vec<_>=selectors.iter().map(|c|col(t,c)).collect::<Result<_>>()?;Ok(rr.into_iter().map(|r|Row{values:idxs.iter().map(|i|r.values[*i].clone()).collect()}).collect())}
fn cmp_value(a:&Value,b:&Value)->std::cmp::Ordering{match(a,b){(Value::Integer(x),Value::Integer(y))=>x.cmp(y),(Value::Text(x),Value::Text(y))=>x.cmp(y),_=>format!("{a:?}").cmp(&format!("{b:?}"))}}

// ---------- SQLite file reader ----------
fn read_database(path:&Path)->Result<Database>{let mut bytes=Vec::new();File::open(path)?.read_to_end(&mut bytes)?;if bytes.len()<100||&bytes[..16]!=MAGIC{return Err(SqliteError::InvalidHeader("signature mismatch".into()))}let ps=match u16::from_be_bytes([bytes[16],bytes[17]]){1=>65536,n=>n as usize};if ps<512||!ps.is_power_of_two()||ps>65536{return Err(SqliteError::InvalidHeader(format!("invalid page size {ps}")))}
// The rewriting backend supports ordinary rowid tables only. Pages that use
// the reserved-byte extension or unsupported read/write versions must not be rewritten.
if bytes[20]!=0 {return Err(SqliteError::InvalidHeader("nonzero reserved bytes per page are unsupported".into()));}
if bytes[18]!=1 || bytes[19]!=1 {return Err(SqliteError::InvalidHeader("WAL-mode SQLite file: checkpoint with the previous SQLite backend first".into()));}
apply_wal(path,ps,&mut bytes)?;let enc=u32::from_be_bytes(bytes[56..60].try_into().unwrap());let schema_rows=read_table_rows(&bytes,ps,1,enc)?;let mut schemas=Vec::new();for r in schema_rows{if r.values.len()<5{continue}if let(Value::Text(kind),Value::Text(name),_,Value::Integer(root),sql)=(&r.values[0],&r.values[1],&r.values[2],&r.values[3],&r.values[4]){if kind=="table"&&name!="sqlite_sequence"{if let Value::Text(sql)=sql{schemas.push((parse_create_table(sql)?,*root as u32));}}}}
    let mut db=Database::default();for(sch,root)in schemas{let mut rows=if root>0{read_table_rows(&bytes,ps,root,enc)?}else{vec![]};
        // SQLite stores the INTEGER PRIMARY KEY alias as a NULL record column;
        // its value is the b-tree rowid, not the serialized field.
        if sch.rowid_alias {if let Some(pk)=sch.pk {for row in &mut rows {if pk<row.values.len(){row.values[pk]=Value::Integer(row.rowid);}}}}
        let next=rows.iter().map(|r|r.rowid).max().unwrap_or(0)+1;db.tables.insert(sch.name.to_ascii_lowercase(),Table{schema:sch,rows,next_rowid:next});}Ok(db)}
fn apply_wal(path:&Path,ps:usize,db:&mut Vec<u8>)->Result<()> {
    let wal=PathBuf::from(format!("{}-wal",path.display()));
    if !wal.exists(){return Ok(())}
    let mut w=Vec::new();File::open(&wal)?.read_to_end(&mut w)?;if w.len()<32{return Ok(())}
    let magic=u32::from_be_bytes(w[0..4].try_into().unwrap());if magic!=0x377f0682&&magic!=0x377f0683{return Ok(())}
    let wps=u32::from_be_bytes(w[8..12].try_into().unwrap())as usize;if wps!=0&&wps!=ps{return Err(SqliteError::Corrupt("WAL page size differs from database".into()))}
    let salt1=&w[16..20];let salt2=&w[20..24];let frame_size=24+ps;let frames=(w.len()-32)/frame_size;let mut last_commit=None;let mut commit_pages=0usize;
    for i in 0..frames{let h=32+i*frame_size;if &w[h+8..h+12]!=salt1||&w[h+12..h+16]!=salt2{break}let dbsize=u32::from_be_bytes(w[h+4..h+8].try_into().unwrap())as usize;if dbsize!=0{last_commit=Some(i);commit_pages=dbsize}}
    let Some(last)=last_commit else{return Ok(())};if commit_pages>0{db.resize(commit_pages*ps,0)}
    for i in 0..=last{let h=32+i*frame_size;let pg=u32::from_be_bytes(w[h..h+4].try_into().unwrap())as usize;if pg==0{continue}let off=(pg-1)*ps;if off+ps>db.len(){db.resize(off+ps,0)}db[off..off+ps].copy_from_slice(&w[h+24..h+24+ps]);}
    Ok(())
}
fn read_table_rows(bytes:&[u8],ps:usize,root:u32,enc:u32)->Result<Vec<DataRow>>{let mut out=Vec::new();walk_table(bytes,ps,root,enc,&mut out)?;Ok(out)}
fn walk_table(bytes:&[u8],ps:usize,pg:u32,enc:u32,out:&mut Vec<DataRow>)->Result<()> {let base=(pg as usize-1)*ps;if base>=bytes.len(){return Err(SqliteError::Corrupt(format!("page {pg} out of range")))}let h=if pg==1{base+100}else{base};if h+12>bytes.len(){return Err(SqliteError::Corrupt("truncated table page".into()))}match bytes[h]{0x0d=>{let n=u16::from_be_bytes([bytes[h+3],bytes[h+4]])as usize;for i in 0..n{let po=u16::from_be_bytes([bytes[h+8+i*2],bytes[h+9+i*2]])as usize;let c=base+po;let(payload,n1)=get_varint(&bytes[c..])?;let(rowid,n2)=get_varint(&bytes[c+n1..])?;let local=local_payload(payload as usize,ps);let start=c+n1+n2;let mut data=bytes[start..start+local].to_vec();if local<payload as usize{let ov=u32::from_be_bytes(bytes[start+local..start+local+4].try_into().unwrap());read_overflow(bytes,ps,ov,payload as usize-local,&mut data)?}out.push(DataRow{rowid:rowid as i64,values:decode_record(&data,enc)?});}}0x05=>{let n=u16::from_be_bytes([bytes[h+3],bytes[h+4]])as usize;for i in 0..n{let po=u16::from_be_bytes([bytes[h+12+i*2],bytes[h+13+i*2]])as usize;let c=base+po;let child=u32::from_be_bytes(bytes[c..c+4].try_into().unwrap());walk_table(bytes,ps,child,enc,out)?}let right=u32::from_be_bytes(bytes[h+8..h+12].try_into().unwrap());walk_table(bytes,ps,right,enc,out)?}_=>return Err(SqliteError::Corrupt(format!("unsupported table page type {:02x}",bytes[h])))}Ok(())}
fn local_payload(p:usize,ps:usize)->usize{let usable=ps;let max=usable-35;if p<=max{return p}let min=((usable-12)*32/255)-23;let k=min+((p-min)%(usable-4));if k<=max{k}else{min}}
fn read_overflow(bytes:&[u8],ps:usize,mut pg:u32,mut left:usize,out:&mut Vec<u8>)->Result<()> {while left>0{if pg==0{return Err(SqliteError::Corrupt("short overflow chain".into()))}let b=(pg as usize-1)*ps;if b+ps>bytes.len(){return Err(SqliteError::Corrupt("overflow page out of range".into()))}let next=u32::from_be_bytes(bytes[b..b+4].try_into().unwrap());let n=left.min(ps-4);out.extend_from_slice(&bytes[b+4..b+4+n]);left-=n;pg=next;}Ok(())}
fn get_varint(s:&[u8])->Result<(u64,usize)>{let mut v=0u64;for i in 0..9{let b=*s.get(i).ok_or_else(||SqliteError::Corrupt("truncated varint".into()))?;if i==8{return Ok(((v<<8)|b as u64,9))}v=(v<<7)|(b&0x7f)as u64;if b<128{return Ok((v,i+1))}}unreachable!()}
fn decode_record(data:&[u8],enc:u32)->Result<Vec<Value>>{let(hs,n)=get_varint(data)?;let mut pos=n;let mut serials=Vec::new();while pos<hs as usize{let(v,k)=get_varint(&data[pos..])?;serials.push(v);pos+=k}let mut body=hs as usize;let mut out=Vec::new();for st in serials{let(v,len)=decode_serial(st,&data[body..],enc)?;body+=len;out.push(v)}Ok(out)}
fn decode_serial(st:u64,s:&[u8],enc:u32)->Result<(Value,usize)>{let int=|n:usize|->Result<i64>{if s.len()<n{return Err(SqliteError::Corrupt("short integer".into()))}let mut v=0i64;for &b in&s[..n]{v=(v<<8)|b as i64}let shift=(8-n)*8;Ok((v<<shift)>>shift)};match st{0=>Ok((Value::Null,0)),1=>Ok((Value::Integer(int(1)?),1)),2=>Ok((Value::Integer(int(2)?),2)),3=>Ok((Value::Integer(int(3)?),3)),4=>Ok((Value::Integer(int(4)?),4)),5=>Ok((Value::Integer(int(6)?),6)),6=>Ok((Value::Integer(int(8)?),8)),7=>{if s.len()<8{return Err(SqliteError::Corrupt("short real".into()))}Ok((Value::Real(f64::from_bits(u64::from_be_bytes(s[..8].try_into().unwrap()))),8))},8=>Ok((Value::Integer(0),0)),9=>Ok((Value::Integer(1),0)),n if n>=12=>{let len=((n-if n%2==0{12}else{13})/2)as usize;if s.len()<len{return Err(SqliteError::Corrupt("short record field".into()))}if n%2==0{Ok((Value::Blob(s[..len].to_vec()),len))}else{let text=decode_text(&s[..len],enc)?;Ok((Value::Text(text),len))}},_=>Err(SqliteError::Corrupt(format!("reserved serial type {st}")))}}
fn decode_text(b:&[u8],enc:u32)->Result<String>{match enc{1|0=>String::from_utf8(b.to_vec()).map_err(|_|SqliteError::Corrupt("invalid UTF-8".into())),2|3=>{if b.len()%2!=0{return Err(SqliteError::Corrupt("odd UTF-16 length".into()))}let u:Vec<u16>=b.chunks_exact(2).map(|x|if enc==2{u16::from_le_bytes([x[0],x[1]])}else{u16::from_be_bytes([x[0],x[1]])}).collect();String::from_utf16(&u).map_err(|_|SqliteError::Corrupt("invalid UTF-16".into()))},_=>Err(SqliteError::Corrupt("unknown encoding".into()))}}

// ---------- SQLite file writer ----------
#[derive(Default)]struct Pager{pages:Vec<Vec<u8>>}
impl Pager{fn new()->Self{Self{pages:vec![vec![0;PAGE_SIZE]]}}fn alloc(&mut self)->u32{self.pages.push(vec![0;PAGE_SIZE]);self.pages.len()as u32}fn page_mut(&mut self,n:u32)->&mut[u8]{&mut self.pages[n as usize-1]}}
fn write_database_atomic(path:&Path,db:&Database)->Result<()> {if let Some(parent)=path.parent(){fs::create_dir_all(parent)?}let mut pager=Pager::new();let mut roots=HashMap::new();let mut tables:Vec<&Table>=db.tables.values().collect();tables.sort_by(|a,b|a.schema.name.cmp(&b.schema.name));for t in &tables{
    let mut disk_rows=t.rows.clone();
    if t.schema.rowid_alias {
        if let Some(pk)=t.schema.pk {
            for row in &mut disk_rows { if pk<row.values.len() { row.values[pk]=Value::Null; } }
        }
    }
    let root=build_table(&mut pager,&disk_rows,None)?;
    roots.insert(t.schema.name.to_ascii_lowercase(),root);
}let mut schema_rows=Vec::new();let mut rid=1;for t in &tables{schema_rows.push(DataRow{rowid:rid,values:vec![Value::Text("table".into()),Value::Text(t.schema.name.clone()),Value::Text(t.schema.name.clone()),Value::Integer(roots[&t.schema.name.to_ascii_lowercase()]as i64),Value::Text(t.schema.sql.clone())]});rid+=1;}build_table(&mut pager,&schema_rows,Some(1))?;let page_count=pager.pages.len() as u32;write_header(&mut pager.pages[0],page_count);let tmp=path.with_extension(format!("{}.tmp",path.extension().and_then(|s|s.to_str()).unwrap_or("db")));{let mut f=File::create(&tmp)?;for p in&pager.pages{f.write_all(p)?}f.sync_all()?;}if path.exists(){
    let bak=path.with_extension("bak");
    fs::copy(path,&bak)?;
    // On Unix, rename over the destination replaces it atomically. For
    // unsupported platforms, leave both the original and backup intact.
    match fs::rename(&tmp,path){
        Ok(())=>{let _=fs::remove_file(bak);},
        Err(e)=>{let _=fs::remove_file(&tmp);return Err(e.into())}
    }
}else{fs::rename(&tmp,path)?}
Ok(())}
fn write_header(p:&mut[u8],count:u32){p[..16].copy_from_slice(MAGIC);p[16..18].copy_from_slice(&(PAGE_SIZE as u16).to_be_bytes());p[18]=1;p[19]=1;p[20]=0;p[21]=64;p[22]=32;p[23]=32;p[24..28].copy_from_slice(&1u32.to_be_bytes());p[28..32].copy_from_slice(&count.to_be_bytes());p[40..44].copy_from_slice(&1u32.to_be_bytes());p[44..48].copy_from_slice(&4u32.to_be_bytes());p[56..60].copy_from_slice(&1u32.to_be_bytes());p[92..96].copy_from_slice(&1u32.to_be_bytes());p[96..100].copy_from_slice(&3045000u32.to_be_bytes());}
fn build_table(pager:&mut Pager,rows:&[DataRow],fixed_root:Option<u32>)->Result<u32>{if rows.is_empty(){let root=fixed_root.unwrap_or_else(||pager.alloc());write_leaf(pager,root,&[])?;return Ok(root)}let mut encoded=Vec::new();for r in rows{encoded.push((r.rowid,encode_record(&r.values)?));}encoded.sort_by_key(|x|x.0);let mut groups:Vec<Vec<(i64,Vec<u8>)>>=Vec::new();let mut cur=Vec::new();let mut used=8usize;for item in encoded{let local=local_payload(item.1.len(),PAGE_SIZE);let cell=varint_len(item.1.len()as u64)+varint_len(item.0 as u64)+local+if local<item.1.len(){4}else{0};if !cur.is_empty()&&used+2+cell>PAGE_SIZE-16{groups.push(cur);cur=Vec::new();used=8;}used+=2+cell;cur.push(item);}if !cur.is_empty(){groups.push(cur)}if groups.len()==1{let root=fixed_root.unwrap_or_else(||pager.alloc());write_leaf(pager,root,&groups[0])?;return Ok(root)}let mut nodes=Vec::new();for g in groups{let pg=pager.alloc();let max=g.last().unwrap().0;write_leaf(pager,pg,&g)?;nodes.push((pg,max));}while nodes.len()>1{if let Some(root)=fixed_root{if interior_fits(&nodes,root==1){write_interior(pager,root,&nodes)?;return Ok(root)}}let mut next=Vec::new();let mut i=0;while i<nodes.len(){let mut end=(i+1).min(nodes.len());while end<nodes.len()&&interior_fits(&nodes[i..=end],false){end+=1}let slice=&nodes[i..end];let pg=pager.alloc();write_interior(pager,pg,slice)?;next.push((pg,slice.last().unwrap().1));i=end;}nodes=next;}Ok(nodes[0].0)}
fn interior_fits(nodes:&[(u32,i64)],page1:bool)->bool{if nodes.len()<2{return true}let header=if page1{112}else{12};let cells:usize=nodes[..nodes.len()-1].iter().map(|x|4+varint_len(x.1 as u64)+2).sum();header+cells<PAGE_SIZE-8}
fn write_leaf(p:&mut Pager,pg:u32,rows:&[(i64,Vec<u8>)])->Result<()> {let base=if pg==1{100}else{0};let mut ptr=PAGE_SIZE;let mut offsets=Vec::new();for(rowid,payload)in rows.iter().rev(){let local=local_payload(payload.len(),PAGE_SIZE);let mut cell=Vec::new();put_varint(payload.len()as u64,&mut cell);put_varint(*rowid as u64,&mut cell);cell.extend_from_slice(&payload[..local]);if local<payload.len(){let first=write_overflow(p,&payload[local..]);cell.extend_from_slice(&first.to_be_bytes());}ptr-=cell.len();p.page_mut(pg)[ptr..ptr+cell.len()].copy_from_slice(&cell);offsets.push(ptr as u16);}offsets.reverse();let page=p.page_mut(pg);page[base]=0x0d;page[base+1..base+3].copy_from_slice(&0u16.to_be_bytes());page[base+3..base+5].copy_from_slice(&(rows.len()as u16).to_be_bytes());page[base+5..base+7].copy_from_slice(&(ptr as u16).to_be_bytes());page[base+7]=0;for(i,o)in offsets.iter().enumerate(){page[base+8+i*2..base+10+i*2].copy_from_slice(&o.to_be_bytes());}Ok(())}
fn write_overflow(p:&mut Pager,data:&[u8])->u32{let chunks=(data.len()+PAGE_SIZE-5)/(PAGE_SIZE-4);let pgs:Vec<u32>=(0..chunks).map(|_|p.alloc()).collect();let mut at=0;for(i,pg)in pgs.iter().enumerate(){let n=(data.len()-at).min(PAGE_SIZE-4);let next=pgs.get(i+1).copied().unwrap_or(0);let page=p.page_mut(*pg);page[..4].copy_from_slice(&next.to_be_bytes());page[4..4+n].copy_from_slice(&data[at..at+n]);at+=n;}pgs[0]}
fn write_interior(p:&mut Pager,pg:u32,nodes:&[(u32,i64)])->Result<()> {let base=if pg==1{100}else{0};let mut ptr=PAGE_SIZE;let mut offs=Vec::new();for(child,key)in nodes[..nodes.len()-1].iter().rev(){let mut cell=Vec::new();cell.extend_from_slice(&child.to_be_bytes());put_varint(*key as u64,&mut cell);ptr-=cell.len();p.page_mut(pg)[ptr..ptr+cell.len()].copy_from_slice(&cell);offs.push(ptr as u16)}offs.reverse();let page=p.page_mut(pg);page[base]=0x05;page[base+1..base+3].copy_from_slice(&0u16.to_be_bytes());page[base+3..base+5].copy_from_slice(&((nodes.len()-1)as u16).to_be_bytes());page[base+5..base+7].copy_from_slice(&(ptr as u16).to_be_bytes());page[base+7]=0;page[base+8..base+12].copy_from_slice(&nodes.last().unwrap().0.to_be_bytes());for(i,o)in offs.iter().enumerate(){page[base+12+i*2..base+14+i*2].copy_from_slice(&o.to_be_bytes())}Ok(())}
fn encode_record(vals:&[Value])->Result<Vec<u8>>{let mut sts=Vec::new();let mut bodies=Vec::new();for v in vals{let(st,b)=encode_value(v);sts.push(st);bodies.push(b)}let mut hlen=1+sts.iter().map(|x|varint_len(*x)).sum::<usize>();loop{let n=varint_len(hlen as u64);let nh=n+sts.iter().map(|x|varint_len(*x)).sum::<usize>();if nh==hlen{break}hlen=nh}let mut out=Vec::new();put_varint(hlen as u64,&mut out);for st in sts{put_varint(st,&mut out)}for b in bodies{out.extend_from_slice(&b)}Ok(out)}
fn encode_value(v:&Value)->(u64,Vec<u8>){match v{Value::Null=>(0,vec![]),Value::Integer(0)=>(8,vec![]),Value::Integer(1)=>(9,vec![]),Value::Integer(n)=>{let b=n.to_be_bytes();let(len,st)=if *n>=-128&&*n<=127{(1,1)}else if *n>=-32768&&*n<=32767{(2,2)}else if *n>=-8388608&&*n<=8388607{(3,3)}else if *n>=i32::MIN as i64&&*n<=i32::MAX as i64{(4,4)}else{(8,6)};(st,b[8-len..].to_vec())},Value::Real(x)=>(7,x.to_bits().to_be_bytes().to_vec()),Value::Blob(b)=>(12+2*b.len()as u64,b.clone()),Value::Text(s)=>{let b=s.as_bytes().to_vec();(13+2*b.len()as u64,b)}}}
fn varint_len(v:u64)->usize{if v<=0x7f{1}else if v<=0x3fff{2}else if v<=0x1fffff{3}else if v<=0xfffffff{4}else if v<=0x7ffffffff{5}else if v<=0x3ffffffffff{6}else if v<=0x1ffffffffffff{7}else if v<=0xffffffffffffff{8}else{9}}
fn put_varint(mut v:u64,o:&mut Vec<u8>){let n=varint_len(v);if n==9{let last=v as u8;v>>=8;let mut tmp=[0u8;8];for i in(0..8).rev(){tmp[i]=(v as u8&0x7f)|0x80;v>>=7}o.extend_from_slice(&tmp);o.push(last);return}let mut tmp=[0u8;8];for i in(0..n).rev(){tmp[i]=(v as u8)&0x7f;if i+1<n{tmp[i]|=0x80}v>>=7}o.extend_from_slice(&tmp[..n]);}

#[cfg(test)]mod tests{use super::*;#[test]fn record_roundtrip(){let v=vec![Value::Text("hello".into()),Value::Integer(42),Value::Null];let b=encode_record(&v).unwrap();assert_eq!(decode_record(&b,1).unwrap(),v)}#[test]fn create_parse(){let s=parse_create_table("CREATE TABLE IF NOT EXISTS x (id TEXT PRIMARY KEY, name TEXT NOT NULL)").unwrap();assert_eq!(s.name,"x");assert_eq!(s.columns.len(),2)}}

#[cfg(test)]
mod studyforge_compatibility_tests {
    use super::*;
    fn test_path() -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("mata-pure-sqlite-{}-{nonce}.db",std::process::id()))
    }
    #[test]
    fn persistent_settings_and_card_relations() {
        let path=test_path();
        {
            let mut conn=Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE subjects (id TEXT PRIMARY KEY, name TEXT NOT NULL, position INTEGER NOT NULL DEFAULT 0); CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id=1), payload_json TEXT NOT NULL); CREATE TABLE units (id TEXT PRIMARY KEY, subject_id TEXT REFERENCES subjects(id) ON DELETE CASCADE, name TEXT NOT NULL);").unwrap();
            conn.execute("INSERT INTO subjects (id,name,position) VALUES (?1,?2,?3)",params!["sub", "Anatomy",0]).unwrap();
            conn.execute("INSERT INTO units (id,subject_id,name) VALUES (?1,?2,?3)",params!["unit","sub","Basics"]).unwrap();
            conn.execute("INSERT OR REPLACE INTO settings(id,payload_json) VALUES (1,?1)",params!["{\"dark\":true}"]).unwrap();
            {
                let tx=conn.transaction().unwrap();
                tx.execute("UPDATE subjects SET name=?2 WHERE id=?1",params!["sub","Temp"]).unwrap();
            }
            let name:String=conn.query_row("SELECT name FROM subjects WHERE id=?1",params!["sub"],|r|r.get(0)).unwrap();
            assert_eq!(name,"Anatomy");
        }
        {
            let conn=Connection::open(&path).unwrap();
            let json:String=conn.query_row("SELECT payload_json FROM settings WHERE id=1",[],|r|r.get(0)).unwrap();
            assert_eq!(json,"{\"dark\":true}");
            let count:i64=conn.query_row("SELECT COUNT(*) FROM units WHERE subject_id=?1",params!["sub"],|r|r.get(0)).unwrap();
            assert_eq!(count,1);
            conn.execute("DELETE FROM subjects WHERE id=?1",params!["sub"]).unwrap();
            let count:i64=conn.query_row("SELECT COUNT(*) FROM units",[],|r|r.get(0)).unwrap();
            assert_eq!(count,0);
        }
        let _=fs::remove_file(path);
    }
    #[test]
    fn opens_database_created_by_sqlite3() {
        let path=test_path();
        fs::write(&path,include_bytes!("../tests/fixtures/legacy_studyforge.db")).unwrap();
        {
            let conn=Connection::open(&path).unwrap();
            let title:String=conn.query_row("SELECT name FROM subjects WHERE id=?1",params!["bio"],|r|r.get(0)).unwrap();
            assert_eq!(title,"Biology");
            let rows:i64=conn.query_row("SELECT COUNT(*) FROM questions WHERE set_id=?1",params!["set"],|r|r.get(0)).unwrap();
            assert_eq!(rows,1);
            let mut stmt=conn.prepare("SELECT id,name FROM subjects ORDER BY position,rowid").unwrap();
            assert_eq!(stmt.query_map([],|r|r.get::<_,String>(0)).unwrap().count(),1);
        }
        let _=fs::remove_file(path);
    }
    #[test]
    fn only_one_writer_per_file() {
        let path=test_path();
        let db=Connection::open(&path).unwrap();
        assert!(Connection::open(&path).is_err());
        drop(db);
        assert!(Connection::open(&path).is_ok());
        let _=fs::remove_file(path);
    }
    #[test]
    fn refuses_nonempty_wal() {
        let path=test_path();let wal=PathBuf::from(format!("{}-wal",path.display()));
        fs::write(&wal,b"SQLite WAL data").unwrap();
        assert!(Connection::open(&path).is_err());
        let _=fs::remove_file(wal);
    }
}
