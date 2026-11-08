use std::{collections::BTreeMap, rc::Rc, cell::RefCell, fmt, sync::mpsc};
use crate::bridge::{CommandFuture, Completion, FrontendEvent, FrontendHandle, Requests, spawn_command};
use crate::{Error, Result, ast::{Expr, Node, Operator, Statement}};

#[derive(Debug, Clone, PartialEq)]
pub enum Value { Null, Bool(bool), Number(f64), String(String), List(Vec<Value>) }
impl Value {
    pub fn display(&self) -> String {
        match self {
            Value::Null => "null".into(),
            Value::Bool(v) => v.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(v) => v.clone(),
            Value::List(v) => format!("[{}]", v.iter().map(Value::display).collect::<Vec<_>>().join(", ")),
        }
    }
    pub fn number(&self) -> Result<f64> {
        match self {
            Value::Number(x) => Ok(*x),
            Value::String(x) => x.parse().map_err(|_| Error::runtime(format!("not a number: '{x}'"))),
            _ => Err(Error::runtime("expected a numeric value")),
        }
    }
    pub fn truthy(&self) -> bool {
        match self { Value::Null => false, Value::Bool(v) => *v, Value::Number(n) => *n != 0.0, Value::String(s) => !s.is_empty(), Value::List(xs) => !xs.is_empty() }
    }
}
impl From<&str> for Value { fn from(x: &str) -> Self { Self::String(x.into()) } }
impl From<String> for Value { fn from(x: String) -> Self { Self::String(x) } }
impl From<bool> for Value { fn from(x: bool) -> Self { Self::Bool(x) } }
impl From<f64> for Value { fn from(x: f64) -> Self { Self::Number(x) } }
impl From<i32> for Value { fn from(x: i32) -> Self { Self::Number(x as f64) } }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LayoutRect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }

/// Compact count models avoid allocating a `Value` for every integer row.
#[derive(Debug, Clone, PartialEq)]
enum ModelSource { Count(usize), Items(Rc<Vec<Value>>) }
impl ModelSource {
    fn len(&self) -> usize {match self {Self::Count(n)=>*n,Self::Items(v)=>v.len()}}
    fn at(&self, index: usize) -> Value { match self {
        Self::Count(_) => Value::Number(index as f64),
        Self::Items(v) => v[index].clone(),
    }}
}

type CommandHandler = Rc<RefCell<Box<dyn FnMut(&[Value]) -> Result<Value>>>>;
type AsyncHandler = Rc<dyn Fn(&[Value]) -> Result<CommandFuture>>;

#[derive(Clone, Default)]
struct CommandRegistry {
    sync: BTreeMap<String, CommandHandler>,
    asynchronous: BTreeMap<String, AsyncHandler>,
}
impl fmt::Debug for CommandRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandRegistry").field("sync", &self.sync.keys()).field("async", &self.asynchronous.keys()).finish()
    }
}

#[derive(Debug, Clone)]
struct Binding { expression: Expr, ty: Option<String> }

#[derive(Debug, Clone)]
struct Instance {
    kind: String,
    id: Option<String>,
    parent: Option<usize>,
    children: Vec<usize>,
    bindings: BTreeMap<String, Binding>,
    overrides: BTreeMap<String, Value>,
    handlers: BTreeMap<String, Vec<Statement>>,
    delegate: Option<Node>,
}

/// Executable, live, reactive document. Evaluates dependencies on demand, so any
/// mutation of a source property is immediately visible to all dependent properties.
/// Circular bindings and unknown references produce an error, not a stale value.
#[derive(Debug, Clone)]
pub struct Engine {
    nodes: Vec<Instance>,
    ids: BTreeMap<String, usize>,
    document: Node,
    commands: CommandRegistry,
    frontend_tx: mpsc::Sender<FrontendEvent>,
    frontend_rx: Rc<RefCell<mpsc::Receiver<FrontendEvent>>>,
    completion_tx: mpsc::Sender<Completion>,
    completion_rx: Rc<RefCell<mpsc::Receiver<Completion>>>,
    request_ids: Requests,
    listener_hits: BTreeMap<usize, u64>,
    /// Non-delegate nodes retain stable indices across model regeneration.
    static_count: usize,
    /// Changes whenever model-backed delegates are re-instantiated.
    generation: u64,
    /// Changes whenever a property override or frontend-delivered value changes.
    /// Unlike `generation`, this also tracks ordinary reactive UI updates.
    revision: u64,
    model_values: BTreeMap<usize, ModelSource>,
    view_ranges: BTreeMap<usize, (usize, usize)>,
    // Measurements provided by Mata's native font renderer. The std-only
    // interpreter intentionally retains a platform-independent fallback.
    native_text_metrics: BTreeMap<usize, (u32, u32)>,
    native_wrapped_text_metrics: BTreeMap<usize, (u32, (u32, u32))>,
}

impl Engine {
    pub fn parse(source: &str) -> Result<Self> { Self::from_ast(crate::parse(source)?) }
    pub fn from_ast(root: Node) -> Result<Self> {
        if root.kind != "Window" { return Err(Error::runtime("document root must be Window")); }
        let (frontend_tx,frontend_rx)=mpsc::channel();
        let (completion_tx,completion_rx)=mpsc::channel();
        let mut engine = Self {
            nodes: Vec::new(), ids: BTreeMap::new(), document: root.clone(), commands: CommandRegistry::default(),
            frontend_tx, frontend_rx: Rc::new(RefCell::new(frontend_rx)),
            completion_tx, completion_rx: Rc::new(RefCell::new(completion_rx)),
            request_ids: Requests::default(), listener_hits: BTreeMap::new(),
            static_count: 0, generation: 0, revision: 0, model_values: BTreeMap::new(), view_ranges: BTreeMap::new(),
                    native_text_metrics: BTreeMap::new(),
                    native_wrapped_text_metrics: BTreeMap::new(),
        };
        engine.insert(root, None, 0, false)?;
        engine.static_count = engine.nodes.len();
        engine.expand_models()?;
        engine.validate()?;
        crate::bridge::apply_defaults(&mut engine)?;
        Ok(engine)
    }
    fn insert(&mut self, node: Node, parent: Option<usize>, depth: usize, repeated: bool) -> Result<usize> {
        if depth > 64 || self.nodes.len() > 10_000 { return Err(Error::runtime("document is too complex")); }
        let mut bindings = BTreeMap::new();
        let mut id = None;
        for prop in node.properties {
            if prop.name == "id" {
                match prop.expression {
                    Expr::Path(parts) if parts.len() == 1 => id = Some(parts[0].clone()),
                    Expr::String(name) if !name.is_empty() => id = Some(name),
                    _ => return Err(Error::runtime("id must be a simple name or string literal")),
                }
            } else {
                bindings.insert(prop.name, Binding { expression: prop.expression, ty: prop.declared_type });
            }
        }
        let idx = self.nodes.len();
        if repeated && id.is_some() {
            return Err(Error::runtime("delegate instances cannot declare ids; use index and modelData"));
        }
        if let Some(name) = &id {
            if matches!(name.as_str(), "self" | "this" | "parent" | "root" | "window") && !(name == "root" && idx == 0) {
                return Err(Error::runtime(format!("reserved id name '{name}'")));
            }
            if self.ids.insert(name.clone(), idx).is_some() {
                return Err(Error::runtime(format!("duplicate id '{name}'")));
            }
        }
        self.nodes.push(Instance {kind: node.kind, id, parent, children: Vec::new(), bindings,
            overrides: BTreeMap::new(), handlers: node.handlers, delegate: node.delegate.map(|x| *x)});
        for child in node.children {
            let child_idx = self.insert(child, Some(idx), depth + 1, repeated)?;
            self.nodes[idx].children.push(child_idx);
        }
        Ok(idx)
    }
    /// A value change in a model can change the instantiated widget tree.
    /// The static part is recreated at stable node indices; its mutable overrides
    /// are retained. Delegates are rebuilt atomically with strict size caps.
    /// Item size must be constant for virtualization; `itemHeight` overrides
    /// the numeric height declared by the delegate. Dynamic-height delegates
    /// require a future variable-extent index, not a misleading estimate.
    fn delegate_height(&self, view: usize) -> Result<f64> {
        let configured = self.property(view,"itemHeight")?.number()?;
        if configured > 0.0 && configured.is_finite() {return Ok(configured);}
        let template = self.nodes[view].delegate.as_ref().ok_or_else(||Error::runtime("no delegate"))?;
        for prop in &template.properties {
            if prop.name == "height" {
                if let Expr::Number(n) = &prop.expression {
                    if n.is_finite() && *n > 0.0 {return Ok(*n);}
                }
                return Err(Error::runtime("virtualized ListView requires itemHeight for nonconstant delegate height"));
            }
        }
        // Auto-sized Text delegates still need a stable virtual row pitch.
        // Treat their initial row height as 32px unless itemHeight is set.
        if template.kind == "Text" {return Ok(32.0);}
        Ok(builtin(&template.kind,"height").unwrap_or(Value::Number(38.0)).number()?.max(1.0))
    }
    fn grid_columns(&self, view:usize, width:f64) -> Result<usize> {
        let requested = self.property(view,"columns")?.number()?;
        if requested > 0.0 {return Ok((requested as usize).clamp(1,10000));}
        let padding = self.property(view,"padding")?.number()?.max(0.0);
        let spacing = self.property(view,"spacing")?.number()?.max(0.0);
        let cw = self.property(view,"cellWidth")?.number()?.max(1.0);
        Ok(((width-2.0*padding+spacing)/(cw+spacing)).floor().max(1.0) as usize)
    }
    /// Select a bounded range plus a pixel-based overscan margin.
    fn desired_range(&self, view:usize, count:usize, rect:LayoutRect) -> Result<(usize,usize)> {
        if count == 0 {return Ok((0,0));}
        let is_grid = self.kind(view)=="GridView";
        let spacing = self.property(view,"spacing")?.number()?.max(0.0);
        let padding = self.property(view,"padding")?.number()?.max(0.0);
        let cache = self.property(view,"cacheBuffer")?.number()?.clamp(0.0,16384.0);
        let offset = self.property(view,"contentY")?.number()?.max(0.0);
        let row_height = if is_grid {self.property(view,"cellHeight")?.number()?.max(1.0)}
                         else {self.delegate_height(view)?};
        let pitch = (row_height+spacing).max(1.0);
        let columns = if is_grid { self.grid_columns(view,rect.width as f64)? } else {1};
        let first_row = ((offset-padding-cache).max(0.0)/pitch).floor() as usize;
        let last_row = ((offset+rect.height as f64+cache-padding).max(0.0)/pitch).floor() as usize+1;
        let begin = first_row.saturating_mul(columns).min(count);
        let end = last_row.saturating_mul(columns).min(count);
        // Even if a tiny row-height is supplied, never allocate unlimited widgets.
        // Scrolling keeps only the nearest MAX_MATERIALIZED rows active.
        const MAX_MATERIALIZED:usize=512;
        if end.saturating_sub(begin)>MAX_MATERIALIZED {
            return Err(Error::runtime("viewport requires over 512 delegates; increase itemHeight/cellHeight or reduce cacheBuffer"));
        }
        Ok((begin,end))
    }
    fn expand_models(&mut self) -> Result<()> {
        // Only static nodes own models; generated delegates are not nested
        // model views in this version. This bounds memory regardless of count.
        for i in 0..self.static_count {
            if let Some(template) = self.nodes[i].delegate.clone() {
                if !matches!(self.nodes[i].kind.as_str(), "ListView" | "GridView") {
                    return Err(Error::runtime("delegate is supported only on ListView and GridView"));
                }
                if !self.nodes[i].children.is_empty() {
                    return Err(Error::runtime("a model view cannot contain both a delegate and explicit items"));
                }
                let entries = match self.model_values.get(&i) {
                    Some(cached) => cached.clone(),
                    None => model_entries(self.property(i,"model")?)?,
                };
                let count = entries.len();
                self.model_values.insert(i,entries.clone());
                let rect = self.layout()?[i];
                // A replacement with fewer rows must bring the viewport back
                // into its new content bounds, otherwise all delegates vanish.
                let (extent_x,extent_y) = self.content_extent(i,&self.layout()?)?;
                for (axis,max) in [
                    ("contentX", (extent_x as f64 - rect.width as f64).max(0.0)),
                    ("contentY", (extent_y as f64 - rect.height as f64).max(0.0)),
                ] {
                    let old=self.property(i,axis)?.number()?;
                    if old > max || old < 0.0 {
                        self.nodes[i].overrides.insert(axis.into(),Value::Number(old.clamp(0.0,max)));
                    }
                }
                let (start,end) = self.desired_range(i,count,rect)?;
                self.view_ranges.insert(i,(start,end));
                for index in start..end {
                    if self.nodes.len()>12_000 {return Err(Error::runtime("too many instantiated view nodes"));}
                    let j = self.insert(template.clone(),Some(i),0,true)?;
                    self.nodes[j].overrides.insert("index".into(),Value::Number(index as f64));
                    self.nodes[j].overrides.insert("modelData".into(),entries.at(index));
                    self.nodes[i].children.push(j);
                }
            } else if self.has_explicit(i,"model") && matches!(self.nodes[i].kind.as_str(), "ListView" | "GridView") {
                return Err(Error::runtime("a ListView/GridView with model requires a delegate"));
            }
        }
        Ok(())
    }
    fn refresh_models(&mut self) -> Result<()> {
        if self.model_values.is_empty() { return Ok(()); }
        let mut changed=false;
        for (&view,old) in &self.model_values {
            let next = model_entries(self.property(view,"model")?)?;
            let range = self.desired_range(view,next.len(),self.layout()?[view])?;
            if next!=*old || self.view_ranges.get(&view)!=Some(&range) {changed=true;break;}
        }
        if !changed {return Ok(());}
        self.recompose(false)
    }
    /// Update only the materialized range. Scroll animations reuse their cached
    /// array model without re-evaluating or cloning every element.
    fn refresh_ranges(&mut self) -> Result<()> {
        let mut changed=false;
        for (&view,source) in &self.model_values {
            let range=self.desired_range(view,source.len(),self.layout()?[view])?;
            if self.view_ranges.get(&view)!=Some(&range) {changed=true;break;}
        }
        if changed {self.recompose(true)?;}
        Ok(())
    }
    fn recompose(&mut self, reuse_cached:bool) -> Result<()> {
        let mut next = Self {
            nodes: Vec::new(), ids: BTreeMap::new(), document: self.document.clone(), commands: self.commands.clone(),
            frontend_tx:self.frontend_tx.clone(), frontend_rx:self.frontend_rx.clone(),
            completion_tx:self.completion_tx.clone(), completion_rx:self.completion_rx.clone(),
            request_ids:self.request_ids.clone(), listener_hits:self.listener_hits.clone(),
            static_count: 0, generation: self.generation.wrapping_add(1),
            revision: self.revision.wrapping_add(1),
            model_values: if reuse_cached {self.model_values.clone()} else {BTreeMap::new()},
            view_ranges: BTreeMap::new(),
                    native_text_metrics: self.native_text_metrics.iter()
                .filter(|(index, _)| **index < self.static_count)
                .map(|(index, size)| (*index, *size)).collect(),
                    native_wrapped_text_metrics: self.native_wrapped_text_metrics.iter()
                .filter(|(index, _)| **index < self.static_count)
                .map(|(index, size)| (*index, *size)).collect(),
        };
        next.insert(next.document.clone(),None,0,false)?;
        next.static_count=next.nodes.len();
        if next.static_count != self.static_count {return Err(Error::runtime("model regeneration changed static tree"));}
        for i in 0..self.static_count {next.nodes[i].overrides=self.nodes[i].overrides.clone();}
        next.expand_models()?;
        next.validate()?;
        *self=next;
        Ok(())
    }
    /// Refresh materialized delegates after a parent changes size.
    pub fn update_viewports(&mut self) -> Result<()> {self.refresh_models()}
    /// The range of currently materialized delegate indices. None for static views.
    pub fn materialized_range(&self, view:usize) -> Option<(usize,usize)> {self.view_ranges.get(&view).copied()}
    pub fn generation(&self) -> u64 { self.generation }
    /// Monotonic change counter for UI invalidation. Reading properties or
    /// computing layout does not advance it, so idle windows can skip layout.
    pub fn revision(&self) -> u64 { self.revision }
    pub fn is_delegate(&self, index: usize) -> bool { index >= self.static_count }
    /// Scroll a viewport by logical pixels, clamped to its full model extent.
    pub fn scroll(&mut self, id: &str, dx: f64, dy: f64) -> Result<()> {
        let view=self.lookup(id)?;
        self.scroll_at(view,dx,dy)
    }
    pub fn scroll_at(&mut self, view:usize, dx:f64, dy:f64) -> Result<()> {
        if view>=self.nodes.len() || !self.is_scrollable(view)? {return Err(Error::runtime("object is not a scrollable view"));}
        let bounds=self.layout()?;
        let (w,h)=self.content_extent(view,&bounds)?;
        let old_x=self.property(view,"contentX")?.number()?;
        let old_y=self.property(view,"contentY")?.number()?;
        let blocked_x=self.property(view,"overflowX")?.display()=="hidden";
        let blocked_y=self.property(view,"overflowY")?.display()=="hidden";
        let x=if blocked_x {0.0} else {(old_x+dx).clamp(0.0,(w as f64-bounds[view].width as f64).max(0.0))};
        let y=if blocked_y {0.0} else {(old_y+dy).clamp(0.0,(h as f64-bounds[view].height as f64).max(0.0))};
        if x==old_x && y==old_y {return Ok(());}
        let snapshot=self.clone();
        let changed=(|| -> Result<()> {
            if x!=old_x {self.write(view,"contentX",Value::Number(x))?;}
            if y!=old_y {self.write(view,"contentY",Value::Number(y))?;}
            // Preserve model bindings that explicitly depend on scroll offsets.
            // Ordinary large array models take the cached, O(visible) path.
            let changes_model=self.model_values.keys().any(|&id|
                self.nodes[id].bindings.get("model")
                    .is_some_and(|binding|expression_mentions_scroll(&binding.expression)));
            if changes_model {self.refresh_models()?;} else {self.refresh_ranges()?;}
            if x!=old_x {self.emit_at(view,"contentXChanged")?;}
            if y!=old_y {self.emit_at(view,"contentYChanged")?;}
            Ok(())
        })();
        if let Err(e)=changed {*self=snapshot;return Err(e);}
        Ok(())
    }
    pub fn view_index_for_child(&self, mut node: usize) -> Option<(usize, usize)> {
        while let Some(parent) = self.nodes[node].parent {
            if matches!(self.kind(parent),"ListView" | "GridView" | "StackView" | "SwipeView") {
                let index = if self.model_values.contains_key(&parent) {
                    self.nodes[parent].children.iter().position(|&x|x==node)
                        .map(|local|local+self.view_ranges.get(&parent).map(|(s,_)|*s).unwrap_or(0))
                } else {self.nodes[parent].children.iter().position(|&x|x==node)};
                return index.map(|index|(parent,index));
            }
            node = parent;
        }
        None
    }
    /// Register a synchronous command for `invoke("name", ...)` in MIVL event handlers.
    pub fn register_command<F>(&mut self, name: &str, callback: F) -> Result<()>
    where F: FnMut(&[Value]) -> Result<Value> + 'static {
        self.check_new_command(name)?;
        self.commands.sync.insert(name.into(), Rc::new(RefCell::new(Box::new(callback))));
        Ok(())
    }
    /// Register an asynchronous command; the future must be owned and Send.
    /// Its completion is delivered to Window.onCommandCompleted/onCommandFailed
    /// on the UI thread, after pump_messages() is called.
    pub fn register_async_command<F>(&mut self,name: &str,callback:F) -> Result<()>
    where F: Fn(&[Value])->Result<CommandFuture> + 'static {
        self.check_new_command(name)?;
        self.commands.asynchronous.insert(name.into(),Rc::new(callback));
        Ok(())
    }
    fn check_new_command(&self,name:&str)->Result<()> {
        if !valid_command_name(name){return Err(Error::command(format!("invalid command name '{name}'")));}
        if self.has_command(name){return Err(Error::command(format!("duplicate command '{name}'")));}
        Ok(())
    }
    pub fn has_command(&self,name:&str)->bool {
        self.commands.sync.contains_key(name)||self.commands.asynchronous.contains_key(name)
    }
    pub fn unregister_command(&mut self,name:&str)->bool {
        let a=self.commands.sync.remove(name).is_some();
        let b=self.commands.asynchronous.remove(name).is_some();
        a||b
    }
    pub fn invoke_command(&self,name:&str,args:&[Value])->Result<Value> {
        if self.commands.asynchronous.contains_key(name) {
            return Err(Error::command(format!("'{name}' is asynchronous; use invoke_async(...)")));
        }
        let handler=self.commands.sync.get(name).ok_or_else(||Error::command(format!("unknown command '{name}'")))?;
        let mut handler=handler.try_borrow_mut().map_err(|_|Error::command("command already running"))?;
        handler(args)
    }
    /// Spawn an async Rust function and return a request ID. No UI thread is blocked.
    pub fn invoke_async_command(&self,name:&str,args:&[Value])->Result<u64> {
        let factory=self.commands.asynchronous.get(name)
            .ok_or_else(||Error::command(format!("unknown asynchronous command '{name}'")))?;
        let future=factory(args)?;
        let id=self.request_ids.next()?;
        spawn_command(id,name.into(),future,self.completion_tx.clone(),self.request_ids.active())?;
        Ok(id)
    }
    /// Thread-safe sender, usable from any Rust module or worker thread.
    pub fn frontend_handle(&self)->FrontendHandle {FrontendHandle(self.frontend_tx.clone())}
    /// Direct delivery on the UI thread, for callers already holding &mut Engine.
    pub fn dispatch_frontend(&mut self,name:&str,payload:impl Into<Value>)->Result<usize> {
        let payload=payload.into();
        let mut delivered=0;
        // Only static listeners are eligible; delegates are not persistent subscriptions.
        for index in 0..self.static_count {
            if self.kind(index)!="EventListener" {continue;}
            if self.property(index,"event")?.display()!=name {continue;}
            if !self.property(index,"enabled")?.truthy(){continue;}
            let n=self.property(index,"times")?.number()?;
            if !n.is_finite() || n.fract()!=0.0 || n < -1.0 || n > 1_000_000.0 {
                return Err(Error::runtime("EventListener.times must be -1 or 0..1_000_000"));
            }
            let infinite=self.property(index,"repeat")?.truthy() || n==-1.0;
            let already=*self.listener_hits.get(&index).unwrap_or(&0);
            if !infinite && already >= n as u64 {continue;}
            self.nodes[index].overrides.insert("eventData".into(),payload.clone());
            self.nodes[index].overrides.insert("eventName".into(),Value::String(name.into()));
            self.revision = self.revision.wrapping_add(1);
            self.emit_at(index,"triggered")?;
            *self.listener_hits.entry(index).or_insert(0)+=1;
            delivered+=1;
        }
        Ok(delivered)
    }
    /// Enable/disable a listener using its declared id. Disabled listeners do
    /// not receive events; re-enabling preserves remaining delivery count.
    pub fn set_listener_enabled(&mut self,id:&str,enabled:bool)->Result<()> {
        let index=self.lookup(id)?;
        if self.kind(index)!="EventListener" {return Err(Error::runtime("id is not an EventListener"));}
        self.set_at(index,"enabled",enabled)
    }
    /// Reset a finite listener's delivery counter to permit new deliveries.
    pub fn reset_listener(&mut self,id:&str)->Result<()> {
        let index=self.lookup(id)?;
        if self.kind(index)!="EventListener" {return Err(Error::runtime("id is not an EventListener"));}
        self.listener_hits.remove(&index);
        Ok(())
    }
    /// Drain Rust -> frontend events and async completions on the UI thread.
    /// Call each frame for headless clients; NativeApp handles this automatically.
    pub fn pump_messages(&mut self)->Result<usize> {
        let mut processed=0;
        const MAX_PER_PUMP:usize=1024;
        while processed<MAX_PER_PUMP {
            let event={self.frontend_rx.borrow().try_recv().ok()};
            let Some(event)=event else {break};
            self.dispatch_frontend(&event.name,event.payload)?;
            processed+=1;
        }
        while processed<MAX_PER_PUMP {
            let completion={self.completion_rx.borrow().try_recv().ok()};
            let Some(completion)=completion else {break};
            self.revision = self.revision.wrapping_add(1);
            self.nodes[0].overrides.insert("commandName".into(),Value::String(completion.name));
            self.nodes[0].overrides.insert("commandRequestId".into(),Value::Number(completion.id as f64));
            match completion.result {
                Ok(value)=>{
                    self.nodes[0].overrides.insert("commandResult".into(),value);
                    self.nodes[0].overrides.insert("commandError".into(),Value::String(String::new()));
                    self.emit_at(0,"commandCompleted")?;
                }
                Err(error)=>{
                    self.nodes[0].overrides.insert("commandResult".into(),Value::Null);
                    self.nodes[0].overrides.insert("commandError".into(),Value::String(error.to_string()));
                    self.emit_at(0,"commandFailed")?;
                }
            }
            processed+=1;
        }
        Ok(processed)
    }
    pub fn count(&self) -> usize { self.nodes.len() }
    pub fn kind(&self, index: usize) -> &str { &self.nodes[index].kind }
    pub fn id(&self, index: usize) -> Option<&str> { self.nodes[index].id.as_deref() }
    pub fn parent(&self, index: usize) -> Option<usize> { self.nodes[index].parent }
    pub fn children(&self, index: usize) -> &[usize] { &self.nodes[index].children }
    pub fn lookup(&self, id: &str) -> Result<usize> {
        match id { "root" | "window" => Ok(0), _ => self.ids.get(id).copied().ok_or_else(|| Error::runtime(format!("unknown object id '{id}'"))) }
    }
    pub fn has_explicit(&self, index: usize, key: &str) -> bool {
        self.nodes[index].bindings.contains_key(key) || self.nodes[index].overrides.contains_key(key)
    }
    pub fn property(&self, index: usize, key: &str) -> Result<Value> {
        self.read(index, key, &mut Vec::new())
    }
    pub fn get(&self, path: &str) -> Result<Value> {
        let parts = split_path(path)?;
        self.resolve(0, &parts, &mut Vec::new())
    }
    pub fn set(&mut self, path: &str, value: impl Into<Value>) -> Result<()> {
        let parts = split_path(path)?;
        let (target, key) = self.resolve_target(0, &parts)?;
        let old = self.clone();
        self.write(target, &key, value.into())?;
        if let Err(e) = self.refresh_models() { *self = old; return Err(e); }
        Ok(())
    }
    pub fn set_at(&mut self, index: usize, key: &str, value: impl Into<Value>) -> Result<()> {
        let old = self.clone();
        self.write(index, key, value.into())?;
        if let Err(e) = self.refresh_models() { *self = old; return Err(e); }
        Ok(())
    }
    pub fn emit(&mut self, target_id: &str, signal: &str) -> Result<()> {
        let node = self.lookup(target_id)?;
        self.emit_at(node, signal)
    }
    pub fn emit_at(&mut self, node: usize, signal: &str) -> Result<()> {
        if node >= self.nodes.len() { return Err(Error::runtime("invalid widget index")); }
        let handler_name = if signal.starts_with("on") { signal.into() }
            else { let mut s = "on".to_string(); let mut chars = signal.chars();
                if let Some(x) = chars.next() { s.extend(x.to_uppercase()); s.extend(chars); } s };
        let instructions = self.nodes[node].handlers.get(&handler_name).cloned().unwrap_or_default();
        if instructions.is_empty() {return Ok(());}
        let old = self.clone();
        for stmt in &instructions {
            if let Err(e) = self.execute(node,stmt) { *self = old; return Err(e); }
        }
        if let Err(e) = self.refresh_models() { *self = old; return Err(e); }
        Ok(())
    }
    fn execute(&mut self, context: usize, stmt: &Statement) -> Result<()> {
        match stmt {
            Statement::Call(expr) => { self.eval_action(context, expr, &mut Vec::new())?; Ok(()) }
            Statement::Assign { target, operator, expression } => {
                let (node,key) = self.resolve_target(context, target)?;
                let rhs = self.eval_action(context, expression, &mut Vec::new())?;
                let result = if *operator == Operator::Eq { rhs }
                    else { let old = self.property(node, &key)?; binary(old, *operator, rhs)? };
                self.write(node, &key, result)
            }
        }
    }
    fn resolve_target(&self, context: usize, parts: &[String]) -> Result<(usize, String)> {
        if parts.len() == 1 {
            let key = &parts[0];
            let mut node = Some(context);
            while let Some(i) = node {
                if self.has_key(i, key) { return Ok((i, key.clone())); }
                node = self.nodes[i].parent;
            }
            return Err(Error::runtime(format!("unknown writable property '{key}'")));
        }
        if parts.len() == 2 {
            let i = self.resolve_node_name(context, &parts[0])?;
            return Ok((i, parts[1].clone()));
        }
        Err(Error::runtime("property paths may have at most two segments"))
    }
    fn resolve_node_name(&self, context: usize, name: &str) -> Result<usize> {
        match name {
            "self" | "this" => Ok(context),
            "parent" => self.nodes[context].parent.ok_or_else(|| Error::runtime("root has no parent")),
            "root" | "window" => Ok(0),
            _ => self.lookup(name),
        }
    }
    fn write(&mut self, node: usize, key: &str, value: Value) -> Result<()> {
        if node >= self.nodes.len() { return Err(Error::runtime("invalid node index")); }
        if key == "id" { return Err(Error::runtime("id is read-only")); }
        if !self.has_key(node, key) {
            return Err(Error::runtime(format!("{} has no property '{key}'", self.nodes[node].kind)));
        }
        let value = self.coerce(node, key, value)?;
        // Avoid invalidating the UI for a setter that does not change its value.
        if self.nodes[node].overrides.get(key) == Some(&value) { return Ok(()); }
        self.nodes[node].overrides.insert(key.into(), value);
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    fn coerce(&self, node: usize, key: &str, value: Value) -> Result<Value> {
        let ty = self.nodes[node].bindings.get(key).and_then(|p| p.ty.as_deref());
        cast(value, ty)
    }
    fn has_key(&self, node: usize, key: &str) -> bool {
        key == "id" || self.nodes[node].bindings.contains_key(key)
            || self.nodes[node].overrides.contains_key(key)
            || builtin(&self.nodes[node].kind, key).is_some()
    }
    fn read(&self, node: usize, key: &str, stack: &mut Vec<(usize, String)>) -> Result<Value> {
        if key == "id" { return Ok(self.nodes[node].id.clone().map(Value::String).unwrap_or(Value::Null)); }
        if stack.iter().any(|(i, p)| *i == node && p == key) {
            let mut chain = stack.iter().map(|(i,p)| format!("{}.{p}", self.nodes[*i].id.as_deref().unwrap_or(&self.nodes[*i].kind))).collect::<Vec<_>>();
            chain.push(format!("{}.{key}", self.nodes[node].id.as_deref().unwrap_or(&self.nodes[node].kind)));
            return Err(Error::runtime(format!("cyclic binding: {}", chain.join(" -> "))));
        }
        if let Some(v) = self.nodes[node].overrides.get(key) { return Ok(v.clone()); }
        if !self.nodes[node].bindings.contains_key(key) && matches!(key,"implicitWidth" | "implicitHeight") {
            return Ok(Value::Number(self.intrinsic_size(node,key=="implicitWidth",0)? as f64));
        }
        if !self.nodes[node].bindings.contains_key(key)
            && key == "count" && matches!(self.kind(node),"ListView" | "GridView" | "StackView" | "SwipeView") {
            return Ok(Value::Number(self.model_values.get(&node).map(ModelSource::len).unwrap_or(self.nodes[node].children.len()) as f64));
        }
        if !self.nodes[node].bindings.contains_key(key)
            && matches!(key,"contentHeight" | "contentWidth")
            && matches!(self.kind(node),"ListView" | "GridView") {
            stack.push((node,key.to_string()));
            let computed = self.view_content_metric(node,key,stack);
            stack.pop();
            return computed;
        }
        let binding = self.nodes[node].bindings.get(key);
        if let Some(binding) = binding {
            if stack.len() > 128 { return Err(Error::runtime("binding evaluation depth exceeded")); }
            stack.push((node, key.into()));
            let result = self.eval(node, &binding.expression, stack).and_then(|v| cast(v, binding.ty.as_deref()));
            stack.pop();
            result
        } else {
            builtin(&self.nodes[node].kind, key).ok_or_else(|| Error::runtime(format!("unknown property '{}.{}'", self.nodes[node].kind, key)))
        }
    }
    fn view_content_metric(&self, node: usize, key: &str, stack: &mut Vec<(usize,String)>) -> Result<Value> {
        let kind=self.kind(node);
        let count=self.model_values.get(&node).map(ModelSource::len).unwrap_or(self.children(node).len());
        let padding=self.read(node,"padding",stack)?.number()?.max(0.0);
        let spacing=self.read(node,"spacing",stack)?.number()?.max(0.0);
        if kind=="ListView" {
            if key=="contentHeight" {
                let height=self.delegate_height(node)?;
                return checked_number(2.0*padding + (count as f64)*height + count.saturating_sub(1) as f64*spacing);
            }
            return self.read(node,"width",stack);
        }
        let cw=self.read(node,"cellWidth",stack)?.number()?.max(1.0);
        let ch=self.read(node,"cellHeight",stack)?.number()?.max(1.0);
        let columns=self.grid_columns(node,self.read(node,"width",stack)?.number()?)?;
        if key=="contentHeight" {
            let rows=count.div_ceil(columns);
            return checked_number(2.0*padding + rows as f64*ch + rows.saturating_sub(1) as f64*spacing);
        }
        let n=count.min(columns);
        checked_number(2.0*padding + n as f64*cw + n.saturating_sub(1) as f64*spacing)
    }
    fn resolve(&self, context: usize, parts: &[String], stack: &mut Vec<(usize, String)>) -> Result<Value> {
        match parts {
            [only] => {
                let mut node = Some(context);
                while let Some(i) = node {
                    if self.has_key(i, only) { return self.read(i, only, stack); }
                    node = self.nodes[i].parent;
                }
                Err(Error::runtime(format!("unknown property '{only}'")))
            }
            [name, key] => self.read(self.resolve_node_name(context, name)?, key, stack),
            _ => Err(Error::runtime("property paths may have at most two segments")),
        }
    }
    /// Resolve lengths while their containing property and node are known.
    /// Percentages are relative to the containing block's corresponding axis;
    /// viewport units always use the top-level window's current dimensions.
    fn resolve_length(&self, context: usize, amount: f64, unit: &str,
                      stack: &mut Vec<(usize,String)>) -> Result<Value> {
        let property = stack.last().map(|(_,key)| key.clone()).unwrap_or_else(||"width".into());
        let property = property.as_str();
        let horizontal = !matches!(property,
            "height" | "y" | "top" | "bottom" | "minHeight" | "maxHeight"
            | "marginTop" | "marginBottom" | "paddingTop" | "paddingBottom"
            | "borderTopWidth" | "borderBottomWidth");
        let parent = self.parent(context).unwrap_or(0);
        let reference = |engine: &Self, index: usize, key: &str,
                         stack: &mut Vec<(usize,String)>| -> Result<f64> {
            // The root viewport cannot be defined as a percentage of itself.
            // Its default viewport is 800x600, overridable by concrete dimensions
            // or native resize notifications.
            if context == 0 && index == 0 {return Ok(if key == "width" {800.0} else {600.0});}
            engine.read(index,key,stack)?.number()
        };
        let pixels = match unit {
            "px" => amount,
            "pt" => amount * 96.0 / 72.0,
            "%" if property=="fontSize" => amount / 100.0 * if context==0 {
                16.0
            } else {self.read(parent,"fontSize",stack)?.number()?},
            "%" => amount / 100.0 * reference(self,parent,if horizontal {"width"} else {"height"},stack)?,
            "vw" => amount / 100.0 * reference(self,0,"width",stack)?,
            "vh" => amount / 100.0 * reference(self,0,"height",stack)?,
            "vmin" | "vmax" => {
                let w = reference(self,0,"width",stack)?;
                let h = reference(self,0,"height",stack)?;
                amount / 100.0 * if unit == "vmin" {w.min(h)} else {w.max(h)}
            }
            "em" | "rem" | "ch" | "ex" => {
                let font_owner = if unit == "rem" {0}
                    else if property == "fontSize" {self.parent(context).unwrap_or(0)}
                    else {context};
                // Use the root default only while resolving the root font itself.
                // Child fontSize: 0.95em and rem units must read the actual
                // root fontSize property, including a user-supplied value.
                let font = if font_owner == 0 && context == 0 && property == "fontSize" {
                    16.0
                } else { self.read(font_owner,"fontSize",stack)?.number()? };
                amount * font * match unit {"ch" => 0.56, "ex" => 0.5, _ => 1.0}
            }
            _ => return Err(Error::runtime(format!("unknown length unit '{unit}'"))),
        };
        checked_number(pixels)
    }

    fn edge(&self, node: usize, key: &str, fallback: &str) -> Result<i32> {
        let name = if self.has_explicit(node,key) {key} else {fallback};
        Ok(as_coord(self.property(node,name)?)?.max(0))
    }
    fn insets(&self, node: usize, prefix: &str) -> Result<(i32,i32,i32,i32)> {
        let fallback = if prefix == "padding" && self.has_explicit(node,"borderPadding")
            {"borderPadding"} else {prefix};
        Ok((self.edge(node,&format!("{prefix}Left"),fallback)?,
            self.edge(node,&format!("{prefix}Top"),fallback)?,
            self.edge(node,&format!("{prefix}Right"),fallback)?,
            self.edge(node,&format!("{prefix}Bottom"),fallback)?))
    }
    /// The native backend injects metrics from the same Mata font renderer
    /// that paints glyphs. The interpreter keeps a conservative fallback.
    pub(crate) fn set_native_text_metrics(&mut self, metrics: BTreeMap<usize, (u32, u32)>,
        wrapped: BTreeMap<usize, (u32, (u32, u32))>) {
        self.native_text_metrics = metrics;
        self.native_wrapped_text_metrics = wrapped;
    }
    /// Exact in the native backend, approximate but consistently line-broken
    /// in the std-only backend. `wrap_width` excludes padding and borders.
    fn text_intrinsic(&self, node: usize, wrap_width: Option<u32>) -> Result<(u32,u32)> {
        use crate::text_layout::{layout_text,WrapMode};
        let text=self.property(node,"text")?.display();
        let fs=self.property(node,"fontSize")?.number()?.max(1.0);
        let step=(self.property(node,"lineHeight")?.number()?.max(0.5)*fs).ceil().max(1.0) as u32;
        let wrap=WrapMode::from_properties(self.property(node,"wrap")?.truthy(),
            &self.property(node,"wrapMode")?.display());
        let maximum=self.property(node,"maxLines")?.number()?.max(0.0) as usize;
        let (l,t,r,b)=self.insets(node,"padding")?;
        let border=self.property(node,"borderWidth")?.number()?.max(0.0) as u32;
        let chrome=(l as u32+r as u32+border.saturating_mul(2),
                    t as u32+b as u32+border.saturating_mul(2));
        let known=match wrap_width {
            Some(width) => self.native_wrapped_text_metrics.get(&node)
                .filter(|(cached_width,_)|*cached_width==width).map(|(_,dims)|*dims),
            None => self.native_text_metrics.get(&node).copied(),
        };
        let (width,height)=if let Some(measured)=known {measured} else {
            let approx=|ch:char|if ch==' ' { (fs*0.35).round() as u32 }
                else if ch.is_ascii_punctuation() {(fs*0.42).ceil() as u32}
                else {(fs*0.68).ceil() as u32};
            let layout=layout_text(&text,wrap_width,wrap,step,
                if maximum==0 {None} else {Some(maximum)},
                self.property(node,"textOverflow")?.display()=="ellipsis",approx);
            (layout.width,layout.height)
        };
        Ok((width.saturating_add(chrome.0).min(8192),
            height.saturating_add(chrome.1).min(8192)))
    }
    fn text_wrap_width(&self,node:usize)->Result<Option<u32>> {
        let wrap=crate::text_layout::WrapMode::from_properties(
            self.property(node,"wrap")?.truthy(),&self.property(node,"wrapMode")?.display());
        if wrap==crate::text_layout::WrapMode::None {return Ok(None);}
        let auto=self.property(node,"width")? == Value::String("auto".into());
        if auto && !self.has_explicit(node,"maxWidth") {return Ok(None);}
        let (l,_,r,_)=self.insets(node,"padding")?;
        let border=self.property(node,"borderWidth")?.number()?.max(0.0) as u32;
        let outer=if auto {as_size(self.property(node,"maxWidth")?)?}
            else {self.size_of(node,"width")?};
        Ok(Some(outer.saturating_sub((l+r) as u32+border.saturating_mul(2))))
    }
    fn intrinsic_size(&self, node: usize, horizontal: bool, depth: usize) -> Result<u32> {
        if depth > 64 {return Err(Error::runtime("intrinsic sizing nesting limit exceeded"));}
        if matches!(self.kind(node),"Text" | "Button" | "TextField") {
            let constraint=if horizontal {None} else {self.text_wrap_width(node)?};
            let (w,h)=self.text_intrinsic(node,constraint)?;
            return Ok(if horizontal {w} else {h});
        }
        let (l,t,r,b)=self.insets(node,"padding")?;
        let flow=self.kind(node);
        let spacing=self.property(node,"spacing")?.number()?.max(0.0);
        let mut total=0f64;
        let mut maximum=0f64;
        let mut count=0usize;
        for &child in self.children(node) {
            if self.property(child,"position")?.display()=="absolute" {continue;}
            let key=if horizontal {"width"} else {"height"};
            let raw=self.property(child,key)?;
            let child_size=if raw==Value::String("auto".into()) {
                self.intrinsic_size(child,horizontal,depth+1)?
            } else {as_size(raw)?} as f64;
            let margins=self.insets(child,"margin")?;
            let extra=if horizontal {margins.0+margins.2} else {margins.1+margins.3};
            total+=child_size+extra as f64;
            maximum=maximum.max(child_size+extra as f64);
            count+=1;
        }
        if count>1 {total+=(count-1) as f64 * spacing;}
        let basis=if (flow=="Row" && horizontal) || (flow=="Column" && !horizontal) {total} else {maximum};
        Ok((basis + if horizontal {(l+r) as f64} else {(t+b) as f64}).clamp(0.0,8192.0) as u32)
    }
    fn size_of(&self, node: usize, key: &str) -> Result<u32> {
        let v=self.property(node,key)?;
        let mut size = if v==Value::String("auto".into()) {
            self.intrinsic_size(node,key=="width",0)?
        } else {as_size(v)?};
        // Clamp after intrinsic sizing; min/max are independently reactive.
        let (min_key,max_key) = if key=="width" {("minWidth","maxWidth")} else {("minHeight","maxHeight")};
        let min=as_size(self.property(node,min_key)?)?;
        let max=as_size(self.property(node,max_key)?)?;
        size=size.max(min).min(max.max(min));
        Ok(size)
    }
    fn eval(&self, context: usize, expr: &Expr, stack: &mut Vec<(usize,String)>) -> Result<Value> {
        self.eval_inner(context, expr, stack, false)
    }
    fn eval_action(&self, context:usize, expr:&Expr, stack:&mut Vec<(usize,String)>) -> Result<Value> {
        self.eval_inner(context, expr, stack, true)
    }
    fn eval_inner(&self, context: usize, expr: &Expr, stack: &mut Vec<(usize,String)>, allow_commands: bool) -> Result<Value> {
        match expr {
            Expr::Null => Ok(Value::Null),
            Expr::Bool(x) => Ok(Value::Bool(*x)),
            Expr::Number(x) => Ok(Value::Number(*x)),
            Expr::Length(number, unit) => self.resolve_length(context, *number, unit, stack),
            Expr::String(x) => Ok(Value::String(x.clone())),
            Expr::List(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items { values.push(self.eval_inner(context,item,stack,allow_commands)?); }
                Ok(Value::List(values))
            },
            Expr::Path(parts) => self.resolve(context, parts, stack),
            Expr::Unary(op, expr) => {
                let v = self.eval_inner(context, expr, stack, allow_commands)?;
                match op { Operator::Not => Ok(Value::Bool(!v.truthy())), Operator::Sub => checked_number(-v.number()?), _ => Err(Error::runtime("unsupported unary operator")) }
            }
            Expr::Binary(a, Operator::And, b) => {
                if !self.eval_inner(context,a,stack,allow_commands)?.truthy() { Ok(Value::Bool(false)) }
                else { Ok(Value::Bool(self.eval_inner(context,b,stack,allow_commands)?.truthy())) }
            }
            Expr::Binary(a, Operator::Or, b) => {
                if self.eval_inner(context,a,stack,allow_commands)?.truthy() { Ok(Value::Bool(true)) }
                else { Ok(Value::Bool(self.eval_inner(context,b,stack,allow_commands)?.truthy())) }
            }
            Expr::Binary(a, op, b) => binary(self.eval_inner(context,a,stack,allow_commands)?, *op, self.eval_inner(context,b,stack,allow_commands)?),
            Expr::Conditional(c, yes, no) => {
                let branch = if self.eval_inner(context,c,stack,allow_commands)?.truthy() {yes} else {no};
                self.eval_inner(context, branch, stack, allow_commands)
            }
            Expr::Call(name, args) => {
                let values = args.iter().map(|x| self.eval_inner(context,x,stack,allow_commands)).collect::<Result<Vec<_>>>()?;
                if name == "invoke_async" {
                    if !allow_commands {return Err(Error::command("invoke_async() is allowed only in event handlers"));}
                    let (Value::String(command),arguments)=values.split_first()
                        .ok_or_else(||Error::command("invoke_async() expects a command name"))?
                        else {return Err(Error::command("invoke_async() expects a string command name"));};
                    Ok(Value::Number(self.invoke_async_command(command,arguments)? as f64))
                } else if name == "invoke" {
                    if !allow_commands {return Err(Error::command("invoke() is allowed only in event handlers"));}
                    let (Value::String(command), arguments) = values.split_first().ok_or_else(|| Error::command("invoke() expects a command name"))?
                        else {return Err(Error::command("invoke() expects a string command name"));};
                    self.invoke_command(command, arguments)
                } else { builtin_call(name, &values) }
            }
        }
    }
    pub fn validate(&self) -> Result<()> {
        for index in 0..self.nodes.len() {
            for (name, binding) in &self.nodes[index].bindings {
                if name == "anchors.fill" || name == "anchors.centerIn" {
                    match &binding.expression {
                        Expr::Path(parts) if parts.len() == 1 && matches!(parts[0].as_str(), "parent" | "root" | "window") && index != 0 => {},
                        _ => return Err(Error::runtime(format!("{name} currently accepts only parent, root or window"))),
                    }
                } else { self.property(index, name)?; }
            }
        }
        self.layout()?;
        Ok(())
    }
    /// Calculate absolute bounds for all instantiated items, including off-screen ones.
    pub fn layout(&self) -> Result<Vec<LayoutRect>> {
        let mut rects = vec![LayoutRect::default(); self.nodes.len()];
        rects[0] = LayoutRect { x: 0, y: 0,
            width: self.size_of(0,"width")?, height: self.size_of(0,"height")? };
        self.place_children(0, &mut rects, 0)?;
        Ok(rects)
    }
    fn anchor_rect(&self, parent: usize, expr: &Expr, rects: &[LayoutRect]) -> Result<LayoutRect> {
        let Expr::Path(parts) = expr else { return Err(Error::runtime("anchor target must be an object")); };
        match parts.as_slice() {
            [name] if name == "parent" => Ok(rects[parent]),
            [name] if name == "window" || name == "root" => Ok(rects[0]),
            _ => Err(Error::runtime("unsupported anchor target")),
        }
    }
    fn place_children(&self, parent: usize, rects: &mut Vec<LayoutRect>, depth: usize) -> Result<()> {
        if depth > 64 {return Err(Error::runtime("layout nesting limit exceeded"));}
        let rect=rects[parent];
        let kind=self.kind(parent);
        let flow=matches!(kind,"Row" | "Column" | "ListView" | "GridView");
        let spacing=if flow {as_coord(self.property(parent,"spacing")?)?.max(0)} else {0};
        let (pad_l,pad_t,pad_r,pad_b)=self.insets(parent,"padding")?;
        let content_w=(rect.width as i64-pad_l as i64-pad_r as i64).max(0) as u32;
        let content_h=(rect.height as i64-pad_t as i64-pad_b as i64).max(0) as u32;
        let scrollable=self.is_scrollable(parent)?;
        let scroll_x=if scrollable {as_coord(self.property(parent,"contentX")?)?.max(0)} else {0};
        let scroll_y=if scrollable {as_coord(self.property(parent,"contentY")?)?.max(0)} else {0};
        let cell_w=if kind=="GridView" {self.size_of(parent,"cellWidth")?.max(1)} else {0};
        let cell_h=if kind=="GridView" {self.size_of(parent,"cellHeight")?.max(1)} else {0};
        let columns=if kind=="GridView" {
            let configured=as_size(self.property(parent,"columns")?)?;
            if configured>0 {configured as usize} else {
                (content_w.saturating_add(spacing as u32) / cell_w.saturating_add(spacing as u32).max(1)).max(1) as usize
            }
        } else {1};
        let mut cursor=0i32;
        let mut flow_positions=Vec::new();
        for (child_number,&i) in self.nodes[parent].children.iter().enumerate() {
            let logical_index=if self.model_values.contains_key(&parent) {
                child_number+self.view_ranges.get(&parent).map(|(first,_)|*first).unwrap_or(0)
            } else {child_number};
            let position=self.property(i,"position")?.display();
            let absolute=matches!(position.as_str(),"absolute" | "fixed");
            let fixed=position=="fixed";
            let (ml,mt,mr,mb)=self.insets(i,"margin")?;
            let (mut w,mut h)=(self.size_of(i,"width")?,self.size_of(i,"height")?);
            if kind=="ListView" && !self.has_explicit(i,"width") {
                w=content_w.saturating_sub((ml+mr) as u32);
            }
            if kind=="GridView" {
                if !self.has_explicit(i,"width") {w=cell_w.saturating_sub((ml+mr) as u32);}
                if !self.has_explicit(i,"height") {h=cell_h.saturating_sub((mt+mb) as u32);}
            }
            if matches!(kind,"StackView" | "SwipeView") {
                if !self.has_explicit(i,"width") {w=content_w.saturating_sub((ml+mr) as u32);}
                if !self.has_explicit(i,"height") {h=content_h.saturating_sub((mt+mb) as u32);}
            }
            // A Text item with a specified width can grow vertically to fit
            // wrapped text. Its text remains literal; markup remains inert.
            if self.kind(i)=="Text" && !self.has_explicit(i,"height") {
                let wrap=crate::text_layout::WrapMode::from_properties(
                    self.property(i,"wrap")?.truthy(),&self.property(i,"wrapMode")?.display());
                let inner=if wrap!=crate::text_layout::WrapMode::None {
                    let (l,_,r,_)=self.insets(i,"padding")?;
                    let border=self.property(i,"borderWidth")?.number()?.max(0.0) as u32;
                    Some(w.saturating_sub((l+r) as u32+border.saturating_mul(2)))
                } else {None};
                h=self.text_intrinsic(i,inner)?.1;
            }
            let (mut x,mut y)=if absolute {(ml,mt)} else {match kind {
                "Row" => (cursor.saturating_add(ml),mt),
                "Column" => (ml,cursor.saturating_add(mt)),
                "ListView" => {
                    let yy=if self.model_values.contains_key(&parent) {
                        let pitch=self.delegate_height(parent)?+spacing as f64;
                        (logical_index as f64*pitch).clamp(0.0,i32::MAX as f64) as i32
                    } else {cursor};
                    (ml,yy.saturating_add(mt))
                }
                "GridView" => {
                    let col=logical_index%columns;
                    let row=logical_index/columns;
                    ( (col as i64*(cell_w as i64+spacing as i64)).min(i32::MAX as i64) as i32 + ml,
                      (row as i64*(cell_h as i64+spacing as i64)).min(i32::MAX as i64) as i32 + mt )
                }
                _ => (ml,mt),
            }};
            // x/y are relative offsets for flow children and exact positions
            // for regular/absolute children; `left/top` accept CSS-style pins.
            if !flow || absolute {
                if self.has_explicit(i,"x") {x=x.saturating_add(as_coord(self.property(i,"x")?)?);}
                if self.has_explicit(i,"y") {y=y.saturating_add(as_coord(self.property(i,"y")?)?);}
            } else {
                if self.has_explicit(i,"x") {x=x.saturating_add(as_coord(self.property(i,"x")?)?);}
                if self.has_explicit(i,"y") {y=y.saturating_add(as_coord(self.property(i,"y")?)?);}
            }
            if self.has_explicit(i,"left") {x=as_coord(self.property(i,"left")?)?.saturating_add(ml);}
            if self.has_explicit(i,"top") {y=as_coord(self.property(i,"top")?)?.saturating_add(mt);}
            if self.has_explicit(i,"right") && !self.has_explicit(i,"left") {
                x=(content_w as i64-w as i64-as_coord(self.property(i,"right")?)? as i64-mr as i64)
                    .clamp(i32::MIN as i64,i32::MAX as i64) as i32;
            }
            if self.has_explicit(i,"bottom") && !self.has_explicit(i,"top") {
                y=(content_h as i64-h as i64-as_coord(self.property(i,"bottom")?)? as i64-mb as i64)
                    .clamp(i32::MIN as i64,i32::MAX as i64) as i32;
            }
            let origin=if fixed {(rects[0].x,rects[0].y)} else {(rect.x,rect.y)};
            if let Some(anchor)=self.nodes[i].bindings.get("anchors.fill") {
                let target=self.anchor_rect(parent,&anchor.expression,rects)?;
                let margins=as_coord(self.property(i,"anchors.margins")?)?.max(0) as u32;
                w=target.width.saturating_sub(margins.saturating_mul(2));
                h=target.height.saturating_sub(margins.saturating_mul(2));
                x=target.x.saturating_sub(origin.0).saturating_add(margins as i32);
                y=target.y.saturating_sub(origin.1).saturating_add(margins as i32);
            } else if let Some(anchor)=self.nodes[i].bindings.get("anchors.centerIn") {
                let target=self.anchor_rect(parent,&anchor.expression,rects)?;
                x=target.x.saturating_sub(origin.0).saturating_add(
                    ((target.width as i64-w as i64)/2).clamp(i32::MIN as i64,i32::MAX as i64) as i32);
                y=target.y.saturating_sub(origin.1).saturating_add(
                    ((target.height as i64-h as i64)/2).clamp(i32::MIN as i64,i32::MAX as i64) as i32);
            } else {
                x=x.saturating_add(pad_l);
                y=y.saturating_add(pad_t);
                // Optional cross-axis alignment inside row/column layouts.
                let align=self.property(parent,"alignItems")?.display();
                if kind=="Row" && !self.has_explicit(i,"top") && !self.has_explicit(i,"y") {
                    if align=="center" {y=pad_t+(content_h as i64-h as i64).max(0) as i32/2;}
                    else if align=="end" {y=pad_t+(content_h as i64-h as i64-mb as i64).max(0) as i32;}
                } else if kind=="Column" && !self.has_explicit(i,"left") && !self.has_explicit(i,"x") {
                    if align=="center" {x=pad_l+(content_w as i64-w as i64).max(0) as i32/2;}
                    else if align=="end" {x=pad_l+(content_w as i64-w as i64-mr as i64).max(0) as i32;}
                }
                if scrollable && !fixed {x=x.saturating_sub(scroll_x);y=y.saturating_sub(scroll_y);}
            }
            rects[i]=LayoutRect {x:origin.0.saturating_add(x),y:origin.1.saturating_add(y),width:w,height:h};
            if !absolute {
                if kind=="Row" {cursor=cursor.saturating_add(w.min(i32::MAX as u32) as i32).saturating_add(ml+mr+spacing);}
                if matches!(kind,"Column" | "ListView") {cursor=cursor.saturating_add(h.min(i32::MAX as u32) as i32).saturating_add(mt+mb+spacing);}
                flow_positions.push(i);
            }
            self.place_children(i,rects,depth+1)?;
        }
        // Main-axis space distribution, without disturbing absolute children.
        if matches!(kind,"Row" | "Column") && !flow_positions.is_empty() {
            let axis_horizontal=kind=="Row";
            let available=if axis_horizontal {content_w as i64} else {content_h as i64};
            let used=cursor.saturating_sub(spacing) as i64;
            let free=(available-used).max(0);
            let justify=self.property(parent,"justifyContent")?.display();
            let count=flow_positions.len() as i64;
            let (initial,gap)=match justify.as_str() {
                "center" => (free/2,0), "end" => (free,0),
                "spaceBetween" if count>1 => (0,free/(count-1)),
                "spaceAround" => (free/(count*2),free/count),
                "spaceEvenly" => (free/(count+1),free/(count+1)),
                _ => (0,0),
            };
            for (n,&index) in flow_positions.iter().enumerate() {
                let diff=(initial+gap*n as i64).clamp(0,i32::MAX as i64) as i32;
                if diff>0 {self.shift_subtree(index,rects,if axis_horizontal {diff} else {0},if axis_horizontal {0} else {diff});}
            }
        }
        Ok(())
    }
    fn shift_subtree(&self, node:usize, rects:&mut [LayoutRect], dx:i32,dy:i32) {
        rects[node].x=rects[node].x.saturating_add(dx);
        rects[node].y=rects[node].y.saturating_add(dy);
        for &child in self.children(node) {self.shift_subtree(child,rects,dx,dy);}
    }
    /// Size of the scrollable content, independent of current scroll offset.
    pub fn is_scrollable(&self, node: usize) -> Result<bool> {
        let overflow=self.property(node,"overflow")?.display();
        Ok(is_scroll_kind(self.kind(node)) || matches!(overflow.as_str(),"auto" | "scroll")
            || matches!(self.property(node,"overflowX")?.display().as_str(),"auto" | "scroll")
            || matches!(self.property(node,"overflowY")?.display().as_str(),"auto" | "scroll"))
    }
    pub fn is_clipping(&self, node: usize) -> Result<bool> {
        Ok(self.is_scrollable(node)? || self.property(node,"overflow")?.display()=="hidden"
            || self.property(node,"overflowX")?.display()=="hidden"
            || self.property(node,"overflowY")?.display()=="hidden")
    }
    pub fn content_extent(&self, view: usize, rects: &[LayoutRect]) -> Result<(u32,u32)> {
        if !self.is_scrollable(view)? { return Err(Error::runtime("object is not a scrollable view")); }
        if self.model_values.contains_key(&view) {
            let w=self.property(view,"contentWidth")?.number()?.max(0.0).min(u32::MAX as f64) as u32;
            let h=self.property(view,"contentHeight")?.number()?.max(0.0).min(u32::MAX as f64) as u32;
            return Ok((w,h));
        }
        let (offset_x,offset_y) = (as_coord(self.property(view,"contentX")?)?.max(0),as_coord(self.property(view,"contentY")?)?.max(0));
        let r = rects[view];
        let mut max_x = 0i64;
        let mut max_y = 0i64;
        let mut queue = self.children(view).to_vec();
        while let Some(i) = queue.pop() {
            let child = rects[i];
            max_x = max_x.max(child.x as i64 + child.width as i64 - r.x as i64 + offset_x as i64);
            max_y = max_y.max(child.y as i64 + child.height as i64 - r.y as i64 + offset_y as i64);
            if !self.is_scrollable(i)? {
                queue.extend(self.children(i).iter().copied());
            }
        }
        let (_,_,pad_r,pad_b)=self.insets(view,"padding")?;
        Ok(((max_x+pad_r as i64).max(0).min(u32::MAX as i64) as u32,
            (max_y+pad_b as i64).max(0).min(u32::MAX as i64) as u32))
    }
    /// Native widgets receive clipped rectangles; inactive pages and hidden
    /// descendants have None. No widget can draw outside a view viewport.
    pub fn visible_layout(&self, rects: &[LayoutRect]) -> Result<Vec<Option<LayoutRect>>> {
        if rects.len() != self.nodes.len() {return Err(Error::runtime("layout length does not match document"));}
        let mut visible = vec![None;self.nodes.len()];
        let mut active = vec![false;self.nodes.len()];
        active[0] = self.property(0,"visible")?.truthy();
        if active[0] {visible[0] = Some(rects[0]);}
        for i in 1..self.nodes.len() {
            let Some(parent) = self.parent(i) else {continue};
            if !active[parent] || !self.property(i,"visible")?.truthy() {continue;}
            if matches!(self.kind(parent),"StackView" | "SwipeView") {
                let current = as_coord(self.property(parent,"currentIndex")?)?;
                if current < 0 || self.children(parent).get(current as usize) != Some(&i) {continue;}
            }
            active[i] = true;
            let mut rect = rects[i];
            let mut ancestor = if self.property(i,"position")?.display()=="fixed" {None} else {Some(parent)};
            while let Some(a) = ancestor {
                if self.is_clipping(a)? || matches!(self.kind(a),"StackView" | "SwipeView") {
                    if let Some(clipped) = intersect(rect,rects[a]) {rect=clipped;}
                    else {rect.width=0;rect.height=0;break;}
                }
                ancestor = self.parent(a);
            }
            // An off-screen non-clipping Item can still have on-screen children.
            // Do not drop its descendants merely because its own rect is hidden.
            if rect.width != 0 && rect.height != 0 {visible[i] = Some(rect);}
        }
        Ok(visible)
    }

}

fn split_path(path: &str) -> Result<Vec<String>> {
    let parts = path.split('.').map(str::trim).map(str::to_string).collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|s| s.is_empty()) { return Err(Error::runtime("invalid property path")); }
    Ok(parts)
}
fn as_size(value: Value) -> Result<u32> {
    let n = value.number()?;
    if !n.is_finite() || n < 0.0 || n > 8192.0 { return Err(Error::runtime("layout size must be between 0 and 8192")); }
    Ok(n.round() as u32)
}
fn as_coord(value: Value) -> Result<i32> {
    let n = value.number()?;
    if !n.is_finite() || n.abs() > 2_000_000_000.0 { return Err(Error::runtime("invalid layout coordinate")); }
    Ok(n.round() as i32)
}
fn cast(value: Value, ty: Option<&str>) -> Result<Value> {
    match ty {
        Some("int") => checked_number(value.number()?.trunc()),
        Some("real" | "double") => checked_number(value.number()?),
        Some("string" | "color") => Ok(Value::String(value.display())),
        Some("bool") => Ok(Value::Bool(value.truthy())),
        _ => Ok(value),
    }
}
fn checked_number(n: f64) -> Result<Value> {
    if !n.is_finite() { Err(Error::runtime("numeric result is not finite")) }
    else { Ok(Value::Number(n)) }
}
fn binary(a: Value, op: Operator, b: Value) -> Result<Value> {
    use Operator::*;
    match op {
        Add if matches!(a, Value::String(_)) || matches!(b, Value::String(_)) => {
            let mut out = a.display(); out.push_str(&b.display());
            if out.len() > 2 * 1024 * 1024 { return Err(Error::runtime("string result is too large")); }
            Ok(Value::String(out))
        }
        Eq => Ok(Value::Bool(a == b)), Ne => Ok(Value::Bool(a != b)),
        Add | Sub | Mul | Div | Rem | Lt | Le | Gt | Ge => {
            let x = a.number()?; let y = b.number()?;
            match op {
                Add => checked_number(x+y), Sub => checked_number(x-y), Mul => checked_number(x*y),
                Div | Rem if y == 0.0 => Err(Error::runtime("division by zero")),
                Div => checked_number(x/y), Rem => checked_number(x%y),
                Lt => Ok(Value::Bool(x<y)), Le => Ok(Value::Bool(x<=y)),
                Gt => Ok(Value::Bool(x>y)), Ge => Ok(Value::Bool(x>=y)), _ => unreachable!(),
            }
        }
        _ => Err(Error::runtime("unsupported operator")),
    }
}
fn valid_command_name(name: &str) -> bool {
    name.split('.').all(|part| {
        let mut chars=part.chars();
        chars.next().is_some_and(|c| c=='_' || c.is_ascii_alphabetic())
            && chars.all(|c| c=='_' || c.is_ascii_alphanumeric())
    })
}
fn builtin_call(name: &str, args: &[Value]) -> Result<Value> {
    match (name, args) {
        ("str" | "string", [x]) => Ok(Value::String(x.display())),
        ("number", [x]) => checked_number(x.number()?),
        ("bool", [x]) => Ok(Value::Bool(x.truthy())),
        ("len", [Value::String(x)]) => Ok(Value::Number(x.chars().count() as f64)),
        ("len", [Value::List(xs)]) => Ok(Value::Number(xs.len() as f64)),
        ("abs", [x]) => checked_number(x.number()?.abs()),
        ("round", [x]) => checked_number(x.number()?.round()),
        ("floor", [x]) => checked_number(x.number()?.floor()),
        ("ceil", [x]) => checked_number(x.number()?.ceil()),
        ("min", [x,y]) => checked_number(x.number()?.min(y.number()?)),
        ("max", [x,y]) => checked_number(x.number()?.max(y.number()?)),
        ("clamp", [v,lo,hi]) => {
            let (v,lo,hi) = (v.number()?,lo.number()?,hi.number()?);
            if lo > hi { return Err(Error::runtime("clamp lower bound exceeds upper bound")); }
            checked_number(v.clamp(lo,hi))
        }
        _ => Err(Error::runtime(format!("unknown function or invalid arguments: {name}"))),
    }
}
fn builtin(kind: &str, key: &str) -> Option<Value> {
    let val = match key {
        "x" | "y" | "rotation" => Value::Number(0.0),
        "width" => if kind == "Text" {Value::String("auto".into())} else {Value::Number(match kind { "Window" => 800.0, "EventListener" => 0.0, "Button" => 120.0, "TextField" => 180.0, _ => 100.0 })},
        "height" => if kind == "Text" {Value::String("auto".into())} else {Value::Number(match kind { "Window" => 600.0, "EventListener" => 0.0, "Button" => 38.0, "TextField" => 38.0, _ => 100.0 })},
        "spacing" => Value::Number(8.0),
        "model" if matches!(kind,"ListView" | "GridView") => Value::List(Vec::new()),
        "currentIndex" if matches!(kind,"ListView" | "GridView") => Value::Number(-1.0),
        "currentIndex" if matches!(kind,"StackView" | "SwipeView") => Value::Number(0.0),
        "count" if matches!(kind,"ListView" | "GridView" | "StackView" | "SwipeView") => Value::Number(0.0),
        "contentX" | "contentY" => Value::Number(0.0),
        "contentWidth" | "contentHeight" => Value::Number(0.0),
        "cellWidth" if kind == "GridView" => Value::Number(120.0),
        "cellHeight" if kind == "GridView" => Value::Number(50.0),
        "columns" if kind == "GridView" => Value::Number(0.0),
        "scrollStep" => Value::Number(48.0),
        "cacheBuffer" if matches!(kind,"ListView" | "GridView") => Value::Number(120.0),
        "itemHeight" if kind == "ListView" => Value::Number(0.0),
        "dragEnabled" => Value::Bool(true),
        "anchors.margins" => Value::Number(0.0),
        "padding" | "borderPadding" | "margin" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft"
        | "marginTop" | "marginRight" | "marginBottom" | "marginLeft" => Value::Number(0.0),
        "minWidth" | "minHeight" => Value::Number(0.0),
        "maxWidth" | "maxHeight" => Value::Number(8192.0),
        "left" | "right" | "top" | "bottom" => Value::Number(0.0),
        "borderWidth" | "borderRadius" | "borderRadii" | "border.radius"
        | "borderTopLeftRadius" | "borderTopRightRadius" | "borderBottomLeftRadius" | "borderBottomRightRadius" => Value::Number(0.0),
        "borderColor" => Value::String("#000000".into()),
        "position" => Value::String("relative".into()),
        "overflow" | "overflowX" | "overflowY" => Value::String("visible".into()),
        "scrollbarPolicy" => Value::String("auto".into()),
        "scrollbarArrows" => Value::Bool(false),
        "scrollbarWidth" => Value::Number(10.0),
        "scrollbarColor" => Value::String("#94a3b8".into()),
        "scrollbarTrackColor" => Value::String("#334155".into()),
        "textAlign" => Value::String("left".into()),
        "verticalAlign" => Value::String("top".into()),
        "alignItems" => Value::String("start".into()),
        "justifyContent" => Value::String("start".into()),
        "wrap" => Value::Bool(false),
        "wrapMode" => Value::String("auto".into()),
        "maxLines" => Value::Number(0.0),
        "textOverflow" => Value::String("clip".into()),
        "lineHeight" => Value::Number(1.25),
        "event" | "eventName" if kind=="EventListener" => Value::String(String::new()),
        "eventData" if kind=="EventListener" => Value::Null,
        "times" if kind=="EventListener" => Value::Number(1.0),
        "repeat" if kind=="EventListener" => Value::Bool(false),
        "commandName" | "commandError" if kind=="Window" => Value::String(String::new()),
        "commandResult" if kind=="Window" => Value::Null,
        "commandRequestId" if kind=="Window" => Value::Number(0.0),
        "text" => Value::String(String::new()),
        "source" if kind == "Image" => Value::String(String::new()),
        "title" if kind == "Window" => Value::String("MIVL".into()),
        "fontSize" => Value::Number(16.0),
        "implicitWidth" | "implicitHeight" => Value::Number(0.0),
        "fontFamily" => Value::String("Liberation Sans".into()),
        "visible" | "enabled" => Value::Bool(true),
        "color" => Value::String(match kind { "Window" => "#202633", "Text" => "#ffffff", _ => "#334155" }.into()),
        "opacity" => Value::Number(1.0),
        _ => return None,
    };
    Some(val)
}

fn is_scroll_kind(kind: &str) -> bool {
    matches!(kind,"ScrollView" | "ListView" | "GridView" | "Flickable")
}

fn expression_mentions_scroll(expr:&Expr) -> bool {
    match expr {
        Expr::Path(parts) => parts.iter().any(|part|matches!(part.as_str(),"contentX"|"contentY")),
        Expr::Unary(_,arg) => expression_mentions_scroll(arg),
        Expr::Binary(a,_,b) => expression_mentions_scroll(a)||expression_mentions_scroll(b),
        Expr::Conditional(a,b,c) => expression_mentions_scroll(a)||expression_mentions_scroll(b)||expression_mentions_scroll(c),
        Expr::Call(_,args) | Expr::List(args) => args.iter().any(expression_mentions_scroll),
        Expr::Length(..) => false,
        _ => false,
    }
}

fn model_entries(model: Value) -> Result<ModelSource> {
    match model {
        Value::List(xs) if xs.len() <= 1_000_000 => Ok(ModelSource::Items(Rc::new(xs))),
        Value::List(_) => Err(Error::runtime("view model exceeds 1,000,000 elements")),
        Value::Number(n) if n.is_finite() && n.fract() == 0.0 && (0.0..=1_000_000.0).contains(&n) =>
            Ok(ModelSource::Count(n as usize)),
        _ => Err(Error::runtime("view model must be a list or integer between 0 and 1,000,000")),
    }
}

fn intersect(a: LayoutRect, b: LayoutRect) -> Option<LayoutRect> {
    let x = (a.x as i64).max(b.x as i64);
    let y = (a.y as i64).max(b.y as i64);
    let right = (a.x as i64+a.width as i64).min(b.x as i64+b.width as i64);
    let bottom = (a.y as i64+a.height as i64).min(b.y as i64+b.height as i64);
    if right <= x || bottom <= y { return None; }
    Some(LayoutRect {x:x as i32,y:y as i32,width:(right-x) as u32,height:(bottom-y) as u32})
}

#[cfg(test)]
mod text_metric_tests {
    use super::*;

    #[test]
    fn font_measurements_reflow_auto_widths() {
        let mut ui=Engine::parse(r#"
            Window {
                Column { id: column; width: auto; height: auto
                    Text { id: label; text: "WWWWWW"; fontSize: 32 }
                }
            }
        "#).unwrap();
        let id=ui.lookup("label").unwrap();
        ui.set_native_text_metrics(BTreeMap::from([(id,(275,44))]),BTreeMap::new());
        assert_eq!(ui.layout().unwrap()[id].width,275);
        assert_eq!(ui.layout().unwrap()[id].height,44);
        assert_eq!(ui.layout().unwrap()[ui.lookup("column").unwrap()].width,275);
        ui.set_native_text_metrics(BTreeMap::from([(id,(300,45))]),BTreeMap::new());
        assert_eq!(ui.layout().unwrap()[id].width,300);
        assert_eq!(ui.layout().unwrap()[ui.lookup("column").unwrap()].width,300);
    }
}
