// Whole-program name resolution, type checking, and dependency ordering.
fn numeric(t:&DataType)->bool {matches!(t,DataType::Scalar(n) if *n!=Number::Bool)}
fn index_type(t:&DataType)->bool {matches!(t,DataType::Scalar(Number::I32|Number::U32|Number::I64|Number::U64))}
fn checked_binary(op:Binary,a:&DataType,b:&DataType)->Result<DataType,IrError>{
    if a!=b{return Err(IrError::TypeMismatch)}
    let comparison=matches!(op,Binary::Lt|Binary::Le|Binary::Gt|Binary::Ge|Binary::Eq|Binary::Ne);
    let logical=matches!(op,Binary::And|Binary::Or);
    let arithmetic=matches!(op,Binary::Add|Binary::Sub|Binary::Mul|Binary::Div|Binary::Rem);
    if logical {if *a!=DataType::Scalar(Number::Bool){return Err(IrError::InvalidType)}}
    if arithmetic && !numeric(a) && !matches!(a,DataType::Vector(_, _)) {return Err(IrError::InvalidType)}
    if matches!(a,DataType::Vector(_, _)) && !arithmetic {return Err(IrError::InvalidType)}
    if comparison && !numeric(a) && *a!=DataType::Scalar(Number::Bool) {return Err(IrError::InvalidType)}
    if matches!(op,Binary::Lt|Binary::Le|Binary::Gt|Binary::Ge) && *a==DataType::Scalar(Number::Bool){return Err(IrError::InvalidType)}
    if op==Binary::Rem && !matches!(a,DataType::Scalar(Number::I8|Number::U8|Number::I16|Number::U16|Number::I32|Number::U32|Number::I64|Number::U64)|DataType::Vector(Number::I8|Number::U8|Number::I16|Number::U16|Number::I32|Number::U32|Number::I64|Number::U64,_)){return Err(IrError::InvalidType)}
    Ok(if logical||comparison {DataType::Scalar(Number::Bool)}else{a.clone()})
}
fn is_reserved(n:&str)->bool {
    n.starts_with("_MataTuple") || n.starts_with("_mata_") || n.starts_with("_Parameters_") ||
    ["_global_id","_local_id","_group_id","_local_size","_num_groups","_global_size","main",
     "bool","int","uint","float","double","half","void","true","false", "return",
     "while","for","if","else","struct","var","fn","const","let","typedef",
     "switch","case","default","break","continue","do","register","layout","uniform",
     "buffer","sampler","texture","override","enable","requires","discard","workgroup"].contains(&n)
}
fn record_dep(t:&DataType,out:&mut Vec<String>) {
    match t {DataType::Record(name)=>out.push(name.clone()),DataType::Array(inner,_)|DataType::Pointer(inner,_,_)=>record_dep(inner,out),DataType::Tuple(items)=>for item in items{record_dep(item,out)},_=>()}
}
fn record_visit<'a>(p:&'a Program,r:&'a Record,stack:&mut Vec<String>,done:&mut Vec<String>,out:&mut Vec<&'a Record>)->Result<(),IrError>{
    if done.iter().any(|n|n==&r.name){return Ok(())}
    if stack.iter().any(|n|n==&r.name){return Err(IrError::InvalidLayout)}
    stack.push(r.name.clone());
    let mut deps=Vec::new();for f in &r.fields {record_dep(&f.ty,&mut deps)}
    for name in deps {let next=p.records.iter().find(|rec|rec.name==name).ok_or(IrError::UnknownRecord)?;record_visit(p,next,stack,done,out)?}
    stack.pop();done.push(r.name.clone());out.push(r);Ok(())
}
fn ordered_records(p:&Program)->Result<Vec<&Record>,IrError>{
    let (mut stack,mut done,mut out)=(Vec::new(),Vec::new(),Vec::new());
    for r in &p.records {record_visit(p,r,&mut stack,&mut done,&mut out)?}Ok(out)
}
fn valid_decl_type(p:&Program,t:&DataType)->Result<(),IrError>{
    t.validate()?;
    match t {DataType::Record(name)=>{
        if p.records.iter().all(|r|r.name!=*name){Err(IrError::UnknownRecord)}else{Ok(())}},
        DataType::Array(inner,_)|DataType::Pointer(inner,_,_)=>valid_decl_type(p,inner),
        DataType::Tuple(items)=>{for item in items{valid_decl_type(p,item)?;}Ok(())},_=>Ok(())}
}
fn validate_value(p:&Program,v:&Value,env:&[(String,DataType)])->Result<DataType,IrError>{
    let ty=match v {
        Value::Var(n,t)=>{
            let existing=env.iter().rev().find(|(name,_)|name==n).ok_or(IrError::UnknownSymbol)?;
            if &existing.1!=t{return Err(IrError::TypeMismatch)}t.clone()
        },
        Value::Bool(_)=>DataType::Scalar(Number::Bool),
        Value::Int(value,t)=>{
            let fits=match t {DataType::Scalar(Number::I8)=>i8::try_from(*value).is_ok(),DataType::Scalar(Number::U8)=>u8::try_from(*value).is_ok(),
                DataType::Scalar(Number::I16)=>i16::try_from(*value).is_ok(),DataType::Scalar(Number::U16)=>u16::try_from(*value).is_ok(),
                DataType::Scalar(Number::I32)=>i32::try_from(*value).is_ok(),DataType::Scalar(Number::U32)=>u32::try_from(*value).is_ok(),
                DataType::Scalar(Number::I64)=>true,DataType::Scalar(Number::U64)=>*value>=0,_=>false};
            if !fits {return Err(IrError::InvalidType)}t.clone()
        },
        Value::FloatBits(bits)=>{if !f32::from_bits(*bits).is_finite(){return Err(IrError::InvalidType)}DataType::Scalar(Number::F32)},
        Value::F64Bits(bits)=>{if !f64::from_bits(*bits).is_finite(){return Err(IrError::InvalidType)}DataType::Scalar(Number::F64)},
        Value::UInt(n,t)=>{if !matches!(t,DataType::Scalar(Number::U8|Number::U16|Number::U32|Number::U64)){return Err(IrError::InvalidType)}
            if matches!(t,DataType::Scalar(Number::U8)) && *n>u8::MAX as u64 || matches!(t,DataType::Scalar(Number::U16)) && *n>u16::MAX as u64 || matches!(t,DataType::Scalar(Number::U32)) && *n>u32::MAX as u64 {return Err(IrError::InvalidType)}
            t.clone()},
        Value::Binary(op,a,b,t)=>{
            let at=validate_value(p,a,env)?;let bt=validate_value(p,b,env)?;
            let computed=checked_binary(*op,&at,&bt)?;
            if &computed!=t {return Err(IrError::TypeMismatch)}computed
        },
        Value::Load(ptr,i,t)=>{
            let pt=validate_value(p,ptr,env)?;if !index_type(&validate_value(p,i,env)?){return Err(IrError::InvalidType)}
            if let DataType::Pointer(inner,_,access)=pt {
                if access==Access::WriteOnly {return Err(IrError::TypeMismatch)}
                if &*inner!=t{return Err(IrError::TypeMismatch)}*inner
            } else {return Err(IrError::InvalidType)}
        },
        Value::Builtin(_,axis)=>{if *axis>2{return Err(IrError::InvalidDimension)}DataType::Scalar(Number::U32)},
        Value::Field(obj,name,t)=>{
            let parent=validate_value(p,obj,env)?;
            if !is_identifier(name){return Err(IrError::InvalidIdentifier)}
            let rec_name=if let DataType::Record(n)=parent {n} else{return Err(IrError::InvalidType)};
            let rec=p.records.iter().find(|r|r.name==rec_name).ok_or(IrError::UnknownRecord)?;
            let field=rec.fields.iter().find(|f|f.name==*name).ok_or(IrError::UnknownField)?;
            if field.ty!=*t{return Err(IrError::TypeMismatch)}t.clone()
        },
        Value::Compute(op,args,result)=>{
            let types=args.iter().map(|a|validate_value(p,a,env)).collect::<Result<Vec<_>,_>>()?;
            let expected=compute_result(*op,&types)?;
            if &expected!=result{return Err(IrError::TypeMismatch)}
            expected
        },
        Value::Call(name,args,result)=>{
            let func=p.functions.iter().find(|f|f.name==*name).ok_or(IrError::UnknownFunction)?;
            if args.len()!=func.args.len(){return Err(IrError::TypeMismatch)}
            for (arg,param) in args.iter().zip(&func.args){if validate_value(p,arg,env)?!=param.ty{return Err(IrError::TypeMismatch)}}
            if *result!=func.result {return Err(IrError::TypeMismatch)}
            result.clone()
        },
        Value::Cast(value,to)=>{
            let from=validate_value(p,value,env)?;
            if !numeric(&from)||!numeric(to){return Err(IrError::InvalidType)}
            to.clone()
        },
        Value::Tuple(items)=>{
            if !(2..=8).contains(&items.len()){return Err(IrError::InvalidType)}
            DataType::Tuple(items.iter().map(|v|validate_value(p,v,env)).collect::<Result<Vec<_>,_>>()?)
        },
        Value::RecordInit(name,fields)=>{
            let rec=p.records.iter().find(|r|r.name==*name).ok_or(IrError::UnknownRecord)?;
            if fields.len()!=rec.fields.len(){return Err(IrError::TypeMismatch)}
            for (given,expected) in fields.iter().zip(&rec.fields){
                if given.0!=expected.name || validate_value(p,&given.1,env)?!=expected.ty {return Err(IrError::TypeMismatch)}
            }
            DataType::Record(name.clone())
        },
        Value::ArrayInit(values,t)=>{
            if let DataType::Array(element,n)=t{
                if *n!=values.len(){return Err(IrError::TypeMismatch)}
                for value in values{if validate_value(p,value,env)?!=**element{return Err(IrError::TypeMismatch)}}
                t.clone()
            }else{return Err(IrError::InvalidType)}
        },
        Value::VectorInit(items,scalar)=>{
            let ty=DataType::Vector(*scalar,items.len() as u8);ty.validate()?;
            for value in items{if validate_value(p,value,env)?!=DataType::Scalar(*scalar){return Err(IrError::TypeMismatch)}}ty
        },
        Value::Index(parent,index,ret)=>{
            if !index_type(&validate_value(p,index,env)?){return Err(IrError::InvalidType)}
            let item=match validate_value(p,parent,env)?{
                DataType::Array(inner,_)|DataType::Pointer(inner,_,_)=>*inner,
                DataType::Vector(n,_)=>DataType::Scalar(n),_=>return Err(IrError::InvalidType)};
            if &item!=ret{return Err(IrError::TypeMismatch)}item
        },
        Value::TupleAt(value,index,result)=>{
            let ty=validate_value(p,value,env)?;
            if let DataType::Tuple(items)=ty {
                if items.get(*index)!=Some(result){return Err(IrError::TypeMismatch)} result.clone()
            }else{return Err(IrError::InvalidType)}
        },
    };
    valid_decl_type(p,&ty)?;Ok(ty)
}
fn stmt_returns(block:&[Statement])->bool {
    block.iter().any(|stmt|match stmt {Statement::Return(_)=>true,
        Statement::If(_,yes,no)=>!no.is_empty()&&stmt_returns(yes)&&stmt_returns(no),_=>false})
}
fn validate_statements(p:&Program,stmts:&[Statement],env:&mut Vec<(String,DataType)>,result:&DataType,kernel:bool,depth:usize,locked:&[String])->Result<(),IrError>{
    if depth>64{return Err(IrError::InvalidControlFlow)}
    let start=env.len();
    for stmt in stmts {
        match stmt {
            Statement::Let(name,ty,val)=>{
                if !is_identifier(name)||is_reserved(name){return Err(IrError::InvalidIdentifier)}
                if env[start..].iter().any(|(n,_)|n==name) || locked.contains(name) ||
                    p.functions.iter().any(|f|f.name==*name)||p.kernels.iter().any(|k|k.name==*name)||p.records.iter().any(|r|r.name==*name){return Err(IrError::DuplicateName)}
                valid_decl_type(p,ty)?;
                if *ty==DataType::Void || matches!(ty,DataType::Pointer(_,_,_)){return Err(IrError::InvalidType)}
                if validate_value(p,val,env)?!=*ty{return Err(IrError::TypeMismatch)}
                env.push((name.clone(),ty.clone()));
            },
            Statement::Store(ptr,index,val)=>{
                let pt=validate_value(p,ptr,env)?;
                if !index_type(&validate_value(p,index,env)?){return Err(IrError::InvalidType)}
                let vt=validate_value(p,val,env)?;
                match pt{DataType::Pointer(inner,_,Access::ReadWrite|Access::WriteOnly) if *inner==vt=>(),_=>return Err(IrError::TypeMismatch)}
            },
            Statement::Assign(name,v)=>{
                let known=env.iter().rev().find(|(n,_)|n==name).ok_or(IrError::UnknownSymbol)?;
                if locked.contains(name){return Err(IrError::InvalidControlFlow)}
                if known.1!=validate_value(p,v,env)?{return Err(IrError::TypeMismatch)}
                if matches!(known.1,DataType::Pointer(_,_,_)){return Err(IrError::InvalidType)}
            },
            Statement::If(cond,yes,no)=>{
                if validate_value(p,cond,env)?!=DataType::Scalar(Number::Bool){return Err(IrError::TypeMismatch)}
                validate_statements(p,yes,&mut env.clone(),result,kernel,depth+1,locked)?;
                validate_statements(p,no,&mut env.clone(),result,kernel,depth+1,locked)?;
            },
            Statement::While(cond,body)=>{
                if validate_value(p,cond,env)?!=DataType::Scalar(Number::Bool){return Err(IrError::TypeMismatch)}
                validate_statements(p,body,&mut env.clone(),result,kernel,depth+1,locked)?;
            },
            Statement::ForRange{index,start,end,body}=>{
                if !is_identifier(index)||is_reserved(index){return Err(IrError::InvalidIdentifier)}
                if validate_value(p,start,env)?!=DataType::Scalar(Number::I32)||validate_value(p,end,env)?!=DataType::Scalar(Number::I32){return Err(IrError::TypeMismatch)}
                let mut nested=env.clone();nested.push((index.clone(),DataType::Scalar(Number::I32)));
                validate_statements(p,body,&mut nested,result,kernel,depth+1,locked)?;
            },
            Statement::Return(v)=>{
                if kernel && v.is_some(){return Err(IrError::InvalidControlFlow)}
                match v{Some(value)=>if validate_value(p,value,env)?!=*result{return Err(IrError::TypeMismatch)},
                    None=>if *result!=DataType::Void{return Err(IrError::TypeMismatch)}}
            },
            Statement::Expr(v)=>{validate_value(p,v,env)?;},
            Statement::SetIndex(parent,index,value)=>{
                if !index_type(&validate_value(p,index,env)?){return Err(IrError::InvalidType)}
                if let Value::Var(name,_) = parent {if locked.contains(name) {return Err(IrError::InvalidControlFlow)}}
                let expected=match validate_value(p,parent,env)?{
                    DataType::Array(inner,_)|DataType::Pointer(inner,_,Access::ReadWrite|Access::WriteOnly)=>*inner,
                    DataType::Vector(n,_)=>DataType::Scalar(n),_=>return Err(IrError::InvalidType)};
                if let Value::Var(name,_) = parent {if let Some((_,DataType::Pointer(_,_,Access::ReadOnly)))=env.iter().rev().find(|(n,_)|n==name){return Err(IrError::TypeMismatch)}}
                if validate_value(p,value,env)?!=expected{return Err(IrError::TypeMismatch)}
            },
            Statement::Shared{name,element,len}=>{
                if !kernel || depth!=0{return Err(IrError::InvalidControlFlow)}
                if !is_identifier(name) || is_reserved(name){return Err(IrError::InvalidIdentifier)}
                if *len==0 || *len>1_000_000 || *element==Number::Bool || *element==Number::Bf16 {
                    return Err(IrError::InvalidType);
                }
                if env.iter().any(|(n,_)|n==name) || p.functions.iter().any(|f|f.name==*name) || p.records.iter().any(|r|r.name==*name){return Err(IrError::DuplicateName)}
                env.push((name.clone(),DataType::Pointer(Box::new(DataType::Scalar(*element)),AddressSpace::Workgroup,Access::ReadWrite)));
            },
            Statement::AtomicAdd{pointer,index,delta}=>{
                let pt=validate_value(p,pointer,env)?;
                let ix=validate_value(p,index,env)?;
                let vt=validate_value(p,delta,env)?;
                if !index_type(&ix) || !matches!(ix,DataType::Scalar(Number::I32|Number::U32)) ||
                   !matches!(vt,DataType::Scalar(Number::I32|Number::U32)) ||
                   !matches!(pt,DataType::Pointer(inner,AddressSpace::Global|AddressSpace::Workgroup,Access::ReadWrite) if *inner==vt) {
                    return Err(IrError::InvalidType);
                }
            },
            Statement::Atomic(atom)=>{
                let spec=atom.spec()?;
                if validate_value(p,&atom.pointer,env)?!=atom.pointer.ty() || validate_value(p,&atom.index,env)?!=atom.index.ty() {return Err(IrError::TypeMismatch)}
                if let Some(v)=&atom.value{validate_value(p,v,env)?;}
                if let Some(v)=&atom.expected{validate_value(p,v,env)?;}
                if let Some(name)=&atom.result{
                    if env.iter().any(|(n,_)|n==name){return Err(IrError::DuplicateName)}
                    env.push((name.clone(),DataType::Scalar(spec.scalar)));
                }
            },
            Statement::Fence(_)=>(),
            Statement::AsyncCopy{destination,source,count,event}=>{
                if !kernel{return Err(IrError::InvalidControlFlow)}
                for v in [destination,source,count]{validate_value(p,v,env)?;}
                if !is_identifier(event)||is_reserved(event)||env.iter().any(|(n,_)|n==event){return Err(IrError::InvalidIdentifier)}
                // event_t is a dedicated device handle, not a numeric scalar.
                // Its declaration belongs to the asynchronous copy statement.
            },
            Statement::AwaitCopy(event)=>{
                if !kernel || !is_identifier(event) {return Err(IrError::InvalidControlFlow)}
                // Event declaration / wait pairing is checked by the separate sync pass.
            },
            Statement::Barrier=>(),
        }
    }
    Ok(())
}
fn called_in_value(value:&Value,names:&mut Vec<String>){
    match value {
        Value::Call(name,args,_)=>{names.push(name.clone());for x in args{called_in_value(x,names)}},
        Value::Compute(_,args,_)=>for x in args{called_in_value(x,names)},
        Value::Binary(_,a,b,_)|Value::Load(a,b,_)=>{called_in_value(a,names);called_in_value(b,names)},
        Value::Field(a,_,_)|Value::Cast(a,_)|Value::TupleAt(a,_,_)=>called_in_value(a,names),
        Value::Tuple(items)|Value::ArrayInit(items,_)|Value::VectorInit(items,_)=>for x in items{called_in_value(x,names)},
        Value::RecordInit(_,fields)=>for (_,v) in fields{called_in_value(v,names)},
        Value::Index(a,b,_)=>{called_in_value(a,names);called_in_value(b,names)},_=>(),
    }
}
fn called_in_block(block:&[Statement],names:&mut Vec<String>){
    for stmt in block{match stmt{
        Statement::Let(_,_,v)|Statement::Assign(_,v)|Statement::Expr(v)=>called_in_value(v,names),
        Statement::Store(a,b,c)|Statement::SetIndex(a,b,c)=>{called_in_value(a,names);called_in_value(b,names);called_in_value(c,names)},
        Statement::AtomicAdd{pointer,index,delta}=>{called_in_value(pointer,names);called_in_value(index,names);called_in_value(delta,names)},
        Statement::Atomic(a)=>{called_in_value(&a.pointer,names);called_in_value(&a.index,names);if let Some(v)=&a.value{called_in_value(v,names)}if let Some(v)=&a.expected{called_in_value(v,names)}},
        Statement::AsyncCopy{destination,source,count,..}=>for v in [destination,source,count]{called_in_value(v,names)},
        Statement::Fence(_)|Statement::AwaitCopy(_)=>(),
        Statement::If(c,a,b)=>{called_in_value(c,names);called_in_block(a,names);called_in_block(b,names)},
        Statement::While(c,body)=>{called_in_value(c,names);called_in_block(body,names)},
        Statement::ForRange{start,end,body,..}=>{called_in_value(start,names);called_in_value(end,names);called_in_block(body,names)},
        Statement::Return(Some(v))=>called_in_value(v,names),_=>(),
    }}
}
fn visit_calls(p:&Program,name:&str,stack:&mut Vec<String>,done:&mut Vec<String>)->Result<(),IrError>{
    if stack.iter().any(|n|n==name){return Err(IrError::InvalidControlFlow)}
    if done.iter().any(|n|n==name){return Ok(())}
    let f=p.functions.iter().find(|f|f.name==name).ok_or(IrError::UnknownFunction)?;
    stack.push(name.into());let mut names=Vec::new();called_in_block(&f.body,&mut names);
    for item in names {visit_calls(p,&item,stack,done)?}
    stack.pop();done.push(name.into());Ok(())
}
impl Program {
    /// Whole-program name/type checking, including signatures, records,
    /// lexical locals, tuple results and recursive-call rejection.
    pub fn validate(&self)->Result<(),IrError>{
        let mut symbols=Vec::new();
        for r in &self.records{
            if !is_identifier(&r.name)||is_reserved(&r.name){return Err(IrError::InvalidIdentifier)}
            if symbols.contains(&r.name){return Err(IrError::DuplicateName)}symbols.push(r.name.clone());
            if r.fields.is_empty(){return Err(IrError::InvalidType)}
            let mut names=Vec::new();
            for f in &r.fields{
                if !is_identifier(&f.name)||is_reserved(&f.name){return Err(IrError::InvalidIdentifier)}
                if names.contains(&f.name){return Err(IrError::DuplicateName)}names.push(f.name.clone());
                valid_decl_type(self,&f.ty)?;
                if matches!(f.ty,DataType::Void|DataType::Pointer(_,_,_)|DataType::Tuple(_)){return Err(IrError::InvalidType)}
            }
            self.record_layout(&r.name)?;
        }
        for f in &self.functions{
            if !is_identifier(&f.name)||is_reserved(&f.name){return Err(IrError::InvalidIdentifier)}
            if symbols.contains(&f.name){return Err(IrError::DuplicateName)}symbols.push(f.name.clone());
            validate_parameters(&f.args)?;valid_decl_type(self,&f.result)?;
            let mut env=Vec::new();for arg in &f.args{valid_decl_type(self,&arg.ty)?;env.push((arg.name.clone(),arg.ty.clone()))}
            validate_statements(self,&f.body,&mut env,&f.result,false,0,&f.args.iter().map(|a|a.name.clone()).collect::<Vec<_>>())?;
            if f.result!=DataType::Void && !stmt_returns(&f.body) {return Err(IrError::InvalidControlFlow)}
        }
        for k in &self.kernels{
            if !is_identifier(&k.name)||is_reserved(&k.name){return Err(IrError::InvalidIdentifier)}
            if symbols.contains(&k.name){return Err(IrError::DuplicateName)}symbols.push(k.name.clone());
            validate_parameters(&k.args)?;
            if k.workgroup.contains(&0)||k.workgroup.iter().any(|&v|v>1024)||k.workgroup.iter().fold(1u64,|a,&v|a*v as u64)>1024{return Err(IrError::InvalidWorkgroup)}
            let mut env=Vec::new();for arg in &k.args{valid_decl_type(self,&arg.ty)?;env.push((arg.name.clone(),arg.ty.clone()))}
            validate_statements(self,&k.body,&mut env,&DataType::Void,true,0,&k.args.iter().map(|a|a.name.clone()).collect::<Vec<_>>())?;
        }
        // Kernel/function arguments also share the target language's global
        // identifier namespace with function and record names.
        for args in self.functions.iter().map(|f|&f.args).chain(self.kernels.iter().map(|k|&k.args)) {
            for arg in args {
                if symbols.iter().any(|name|name==&arg.name) {return Err(IrError::DuplicateName)}
            }
        }
        let (mut stack,mut done)=(Vec::new(),Vec::new());
        for f in &self.functions{visit_calls(self,&f.name,&mut stack,&mut done)?}
        Ok(())
    }
    /// Emit a single entrypoint, retaining shared functions and records.
    pub fn emit_kernel(&self,name:&str,backend:Backend)->Result<String,IrError>{
        self.validate()?;
        let k=self.kernels.iter().find(|k|k.name==name).ok_or(IrError::UnknownSymbol)?;
        let single=Program{records:self.records.clone(),functions:self.functions.clone(),kernels:alloc::vec![k.clone()]};
        single.emit(backend)
    }
}
fn tuple_name(ty:&DataType)->String{
    let repr=format!("{:?}",ty);
    let mut h:u64=0xcbf29ce484222325;
    for byte in repr.bytes(){h^=u64::from(byte);h=h.wrapping_mul(0x100000001b3);}
    format!("_MataTuple{:016x}",h)
}
fn gather_tuple_type(t:&DataType,collected:&mut Vec<DataType>){
    match t{
        DataType::Tuple(items)=>{for item in items{gather_tuple_type(item,collected)}
            if !collected.contains(t){collected.push(t.clone())}},
        DataType::Array(inner,_)|DataType::Pointer(inner,_,_)=>gather_tuple_type(inner,collected),_=>(),
    }
}
fn gather_tuple_value(v:&Value,found:&mut Vec<DataType>){
    gather_tuple_type(&v.ty(),found);
    match v{
        Value::Binary(_,a,b,_)|Value::Load(a,b,_)=>{gather_tuple_value(a,found);gather_tuple_value(b,found)},
        Value::Field(v,_,_)|Value::Cast(v,_)|Value::TupleAt(v,_,_)=>gather_tuple_value(v,found),
        Value::Call(_,args,_)|Value::Compute(_,args,_)|Value::Tuple(args)|Value::ArrayInit(args,_)|Value::VectorInit(args,_)=>for arg in args{gather_tuple_value(arg,found)},
        Value::RecordInit(_,fields)=>for (_,value) in fields{gather_tuple_value(value,found)},
        Value::Index(a,b,_)=>{gather_tuple_value(a,found);gather_tuple_value(b,found)},_=>(),
    }
}
fn gather_tuple_block(stmts:&[Statement],found:&mut Vec<DataType>){
    for stmt in stmts{match stmt{
        Statement::Let(_,ty,v)=>{gather_tuple_type(ty,found);gather_tuple_value(v,found)},
        Statement::Store(a,b,v)|Statement::SetIndex(a,b,v)=>{gather_tuple_value(a,found);gather_tuple_value(b,found);gather_tuple_value(v,found)},
        Statement::AtomicAdd{pointer,index,delta}=>{gather_tuple_value(pointer,found);gather_tuple_value(index,found);gather_tuple_value(delta,found)},
        Statement::Atomic(a)=>{gather_tuple_value(&a.pointer,found);gather_tuple_value(&a.index,found);if let Some(v)=&a.value{gather_tuple_value(v,found)}if let Some(v)=&a.expected{gather_tuple_value(v,found)}},
        Statement::AsyncCopy{destination,source,count,..}=>for v in [destination,source,count]{gather_tuple_value(v,found)},
        Statement::Fence(_)|Statement::AwaitCopy(_)=>(),
        Statement::Assign(_,v)|Statement::Expr(v)=>gather_tuple_value(v,found),
        Statement::If(cond,a,b)=>{gather_tuple_value(cond,found);gather_tuple_block(a,found);gather_tuple_block(b,found)},
        Statement::While(cond,body)=>{gather_tuple_value(cond,found);gather_tuple_block(body,found)},
        Statement::ForRange{start,end,body,..}=>{gather_tuple_value(start,found);gather_tuple_value(end,found);gather_tuple_block(body,found)},
        Statement::Return(Some(v))=>gather_tuple_value(v,found),_=>(),
    }}
}
fn emit_tuple_declarations(p:&Program,b:Backend)->Result<String,IrError>{
    let mut tuples=Vec::new();
    for r in &p.records{for f in &r.fields{gather_tuple_type(&f.ty,&mut tuples)}}
    for f in &p.functions{
        gather_tuple_type(&f.result,&mut tuples);
        for a in &f.args{gather_tuple_type(&a.ty,&mut tuples)}gather_tuple_block(&f.body,&mut tuples);
    }
    for k in &p.kernels {for a in &k.args{gather_tuple_type(&a.ty,&mut tuples)}gather_tuple_block(&k.body,&mut tuples)}
    let mut out=String::new();
    for t in tuples{
        if let DataType::Tuple(items)=&t{
            let name=tuple_name(&t);
            match b{
                Backend::Wgsl=>{
                    out.push_str(&format!("struct {} {{\n",name));
                    for (i,item) in items.iter().enumerate(){out.push_str(&format!("    value{}: {},\n",i,render_ty(item,b)?));}
                    out.push_str("}\n");
                },
                Backend::NvidiaCuda|Backend::AmdHip|Backend::IntelOpenCl=>{
                    out.push_str(&format!("typedef struct {} {{\n",name));
                    for (i,item) in items.iter().enumerate(){out.push_str(&format!("    {} value{};\n",render_ty(item,b)?,i));}
                    out.push_str(&format!("}} {};\n",name));
                },
                _=>{
                    out.push_str(&format!("struct {} {{\n",name));
                    for (i,item) in items.iter().enumerate(){out.push_str(&format!("    {} value{};\n",render_ty(item,b)?,i));}
                    out.push_str("};\n");
                    if b==Backend::Hlsl {
                        let args=items.iter().enumerate().map(|(i,item)|Ok(format!("{} arg{}",render_ty(item,b)?,i))).collect::<Result<Vec<_>,IrError>>()?.join(", ");
                        out.push_str(&format!("{} _mata_ctor_{}({}) {{ {} result;\n",name,name,args,name));
                        for i in 0..items.len(){out.push_str(&format!("    result.value{} = arg{};\n",i,i));}
                        out.push_str("    return result;\n}\n");
                    }
                },
            }
        }
    }Ok(out)
}
fn emit_hlsl_record_constructors(p:&Program)->Result<String,IrError>{
    let mut out=String::new();
    for r in ordered_records(p)?{
        if r.fields.iter().any(|f|matches!(f.ty,DataType::Array(_,_)|DataType::Pointer(_,_,_))) {continue;}
        let args=r.fields.iter().map(|f|Ok(format!("{} arg_{}",render_ty(&f.ty,Backend::Hlsl)?,f.name))).collect::<Result<Vec<_>,IrError>>()?.join(", ");
        out.push_str(&format!("{} _mata_make_{}({}) {{ {} result;\n",r.name,r.name,args,r.name));
        for f in &r.fields{out.push_str(&format!("    result.{} = arg_{};\n",f.name,f.name));}
        out.push_str("    return result;\n}\n");
    }
    Ok(out)
}

fn type_uses_f16(t:&DataType)->bool {
    match t {DataType::Scalar(Number::F16)|DataType::Vector(Number::F16,_)=>true,
        DataType::Array(inner,_)|DataType::Pointer(inner,_,_)=>type_uses_f16(inner),
        DataType::Tuple(items)=>items.iter().any(type_uses_f16),_=>false}
}
fn value_uses_f16(v:&Value)->bool{
    if type_uses_f16(&v.ty()){return true}
    match v{
        Value::Binary(_,a,b,_)|Value::Load(a,b,_)|Value::Index(a,b,_)=>value_uses_f16(a)||value_uses_f16(b),
        Value::Field(a,_,_)|Value::Cast(a,_)|Value::TupleAt(a,_,_)=>value_uses_f16(a),
        Value::Call(_,args,_)|Value::Compute(_,args,_)|Value::Tuple(args)|Value::ArrayInit(args,_)|Value::VectorInit(args,_)=>args.iter().any(value_uses_f16),
        Value::RecordInit(_,fields)=>fields.iter().any(|(_,v)|value_uses_f16(v)),_=>false,
    }
}
fn block_uses_f16(body:&[Statement])->bool{
    body.iter().any(|statement|match statement{
        Statement::Let(_,ty,v)=>type_uses_f16(ty)||value_uses_f16(v),
        Statement::Assign(_,v)|Statement::Expr(v)=>value_uses_f16(v),
        Statement::Store(a,b,c)|Statement::SetIndex(a,b,c)=>value_uses_f16(a)||value_uses_f16(b)||value_uses_f16(c),
        Statement::AtomicAdd{pointer,index,delta}=>value_uses_f16(pointer)||value_uses_f16(index)||value_uses_f16(delta),
        Statement::Atomic(a)=>value_uses_f16(&a.pointer)||value_uses_f16(&a.index)||a.value.as_ref().map_or(false,value_uses_f16)||a.expected.as_ref().map_or(false,value_uses_f16),
        Statement::AsyncCopy{destination,source,count,..}=>value_uses_f16(destination)||value_uses_f16(source)||value_uses_f16(count),
        Statement::Fence(_)|Statement::AwaitCopy(_)=>false,
        Statement::If(cond,yes,no)=>value_uses_f16(cond)||block_uses_f16(yes)||block_uses_f16(no),
        Statement::While(cond,body)=>value_uses_f16(cond)||block_uses_f16(body),
        Statement::ForRange{start,end,body,..}=>value_uses_f16(start)||value_uses_f16(end)||block_uses_f16(body),
        Statement::Return(v)=>v.as_ref().map_or(false,value_uses_f16),Statement::Shared{element,..}=>*element==Number::F16,Statement::Barrier=>false,
    })
}
fn program_uses_f16(p:&Program)->bool {
    p.records.iter().any(|r|r.fields.iter().any(|f|type_uses_f16(&f.ty))) ||
    p.functions.iter().any(|f|type_uses_f16(&f.result)||f.args.iter().any(|a|type_uses_f16(&a.ty))||block_uses_f16(&f.body)) ||
    p.kernels.iter().any(|k|k.args.iter().any(|a|type_uses_f16(&a.ty))||block_uses_f16(&k.body))
}
fn type_uses_bf16(t:&DataType)->bool {
    match t {DataType::Scalar(Number::Bf16)|DataType::Vector(Number::Bf16,_)=>true,
        DataType::Array(inner,_)|DataType::Pointer(inner,_,_)=>type_uses_bf16(inner),
        DataType::Tuple(items)=>items.iter().any(type_uses_bf16),_=>false}
}
fn value_uses_bf16(v:&Value)->bool{
    if type_uses_bf16(&v.ty()){return true}
    match v{
        Value::Binary(_,a,b,_)|Value::Load(a,b,_)|Value::Index(a,b,_)=>value_uses_bf16(a)||value_uses_bf16(b),
        Value::Field(a,_,_)|Value::Cast(a,_)|Value::TupleAt(a,_,_)=>value_uses_bf16(a),
        Value::Call(_,args,_)|Value::Compute(_,args,_)|Value::Tuple(args)|Value::ArrayInit(args,_)|Value::VectorInit(args,_)=>args.iter().any(value_uses_bf16),
        Value::RecordInit(_,fields)=>fields.iter().any(|(_,v)|value_uses_bf16(v)),_=>false,
    }
}
fn block_uses_bf16(body:&[Statement])->bool{
    body.iter().any(|statement|match statement{
        Statement::Let(_,ty,v)=>type_uses_bf16(ty)||value_uses_bf16(v),
        Statement::Assign(_,v)|Statement::Expr(v)=>value_uses_bf16(v),
        Statement::Store(a,b,c)|Statement::SetIndex(a,b,c)=>value_uses_bf16(a)||value_uses_bf16(b)||value_uses_bf16(c),
        Statement::AtomicAdd{pointer,index,delta}=>value_uses_bf16(pointer)||value_uses_bf16(index)||value_uses_bf16(delta),
        Statement::Atomic(a)=>value_uses_bf16(&a.pointer)||value_uses_bf16(&a.index)||a.value.as_ref().map_or(false,value_uses_bf16)||a.expected.as_ref().map_or(false,value_uses_bf16),
        Statement::AsyncCopy{destination,source,count,..}=>value_uses_bf16(destination)||value_uses_bf16(source)||value_uses_bf16(count),
        Statement::Fence(_)|Statement::AwaitCopy(_)=>false,
        Statement::If(cond,yes,no)=>value_uses_bf16(cond)||block_uses_bf16(yes)||block_uses_bf16(no),
        Statement::While(cond,body)=>value_uses_bf16(cond)||block_uses_bf16(body),
        Statement::ForRange{start,end,body,..}=>value_uses_bf16(start)||value_uses_bf16(end)||block_uses_bf16(body),
        Statement::Return(v)=>v.as_ref().map_or(false,value_uses_bf16),Statement::Shared{element,..}=>*element==Number::Bf16,Statement::Barrier=>false,
    })
}
fn program_uses_bf16(p:&Program)->bool {
    p.records.iter().any(|r|r.fields.iter().any(|f|type_uses_bf16(&f.ty))) ||
    p.functions.iter().any(|f|type_uses_bf16(&f.result)||f.args.iter().any(|a|type_uses_bf16(&a.ty))||block_uses_bf16(&f.body)) ||
    p.kernels.iter().any(|k|k.args.iter().any(|a|type_uses_bf16(&a.ty))||block_uses_bf16(&k.body))
}
