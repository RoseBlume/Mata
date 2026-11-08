// Phase 7: semantic synchronization and atomic source lowering.
// All constructs are explicit and validated; unsupported exact semantics fail closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomicOp { Load, Store, Exchange, Add, Sub, And, Or, Xor, Min, Max, CompareExchange }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtomicStatement {
    /// `None` discards the old value; `Some(name)` declares a local holding the old value.
    pub result:Option<String>,
    pub op:AtomicOp,
    pub pointer:Value,
    pub index:Value,
    /// Required for stores, exchanges, fetch ops, and CAS (the replacement).
    pub value:Option<Value>,
    /// Expected value for compare-exchange (not a device pointer).
    pub expected:Option<Value>,
    pub order:AtomicOrder,
    pub failure_order:Option<AtomicOrder>,
    pub scope:AtomicScope,
}
impl AtomicStatement {
    pub fn spec(&self)->Result<AtomicSpec,IrError>{
        let (scalar,space)=match self.pointer.ty(){
            DataType::Pointer(ty,sp,Access::ReadWrite) => match *ty {
                DataType::Scalar(n)=>(n,sp),_=>return Err(IrError::InvalidType),
            },_=>return Err(IrError::InvalidType)
        };
        if !matches!(self.index.ty(),DataType::Scalar(Number::I32|Number::U32)) {return Err(IrError::InvalidType)}
        if !matches!(scalar,Number::I32|Number::U32|Number::I64|Number::U64){return Err(IrError::InvalidType)}
        let ty=DataType::Scalar(scalar);
        let expected_value=if self.op==AtomicOp::Load {None}else{Some(ty.clone())};
        let expected_cas=if self.op==AtomicOp::CompareExchange {Some(ty.clone())}else{None};
        if self.value.as_ref().map(Value::ty)!=expected_value || self.expected.as_ref().map(Value::ty)!=expected_cas {
            return Err(IrError::TypeMismatch)
        }
        if let Some(n)=&self.result{if !is_identifier(n)||is_reserved(n){return Err(IrError::InvalidIdentifier)}}
        if self.op==AtomicOp::Store && self.result.is_some() ||
            self.op==AtomicOp::CompareExchange && self.result.is_none() {return Err(IrError::InvalidType)}
        Ok(AtomicSpec{kind:match self.op{AtomicOp::Load=>AtomicKind::Load,AtomicOp::Store=>AtomicKind::Store,AtomicOp::CompareExchange=>AtomicKind::CompareExchange,_=>AtomicKind::ReadModifyWrite},scalar,
            order:self.order,failure:self.failure_order,scope:self.scope,address_space:space,
            alignment:if matches!(scalar,Number::U64|Number::I64){8}else{4}})
    }
}
/// Construct an atomic node. Device capability checks run later in `validate_for`.
pub fn atomic_statement(op:AtomicOp,pointer:Value,index:Value,value:Option<Value>,expected:Option<Value>,
    result:Option<&str>,order:AtomicOrder,scope:AtomicScope,failure_order:Option<AtomicOrder>)->Result<Statement,IrError>{
    let atom=AtomicStatement{result:result.map(String::from),op,pointer,index,value,expected,order,failure_order,scope};
    atom.spec()?;Ok(Statement::Atomic(atom))
}
fn sync_validation(stmt:&Statement,profile:&TargetProfile,kernel:&str,path:&str,uniform:bool)->Result<(),Diagnostic>{
    let error=|kind,detail:&str|Diagnostic::new(profile,kind,Some(kernel),path,detail,None);
    match stmt {
        Statement::Atomic(atom)=>{
            let spec=atom.spec().map_err(|_|error(DiagnosticKind::InvalidAtomicType,"invalid atomic value, pointer, or argument"))?;
            spec.validate(profile)?;
            // OpenCL C 2.x/3.x explicit atomics require an atomic-type object and compiler support.
            if profile.backend==Backend::IntelOpenCl && !profile.language.at_least(2,0) {
                return Err(error(DiagnosticKind::InvalidProfile,"explicit atomics require OpenCL C 2.0 or newer"));
            }
            if matches!(profile.backend,Backend::NvidiaCuda|Backend::AmdHip) &&
                (atom.order!=AtomicOrder::Relaxed || atom.scope!=AtomicScope::Device || atom.op==AtomicOp::CompareExchange && atom.failure_order!=Some(AtomicOrder::Relaxed)) {
                return Err(error(DiagnosticKind::UnsupportedFeature,"CUDA/HIP legacy atomics only implement relaxed device-scope behavior; explicit scoped ordering requires vendor C++ atomics"));
            }
            if matches!(profile.backend,Backend::NvidiaCuda|Backend::AmdHip) && matches!(spec.scalar,Number::I64|Number::U64) {
                return Err(error(DiagnosticKind::UnsupportedFeature,"64-bit legacy atomic overload requires architecture-specific signature verification"));
            }
            if !matches!(profile.backend,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip){
                return Err(error(DiagnosticKind::UnsupportedFeature,"ordered atomics not lowered for shader dialect"));
            }
        }
        Statement::Fence(fence)=>{
            fence.validate(profile)?;
            if !matches!(profile.backend,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip) {
                return Err(error(DiagnosticKind::UnsupportedFeature,"fence backend is unsupported"));
            }
            if profile.backend!=Backend::IntelOpenCl && fence.order!=AtomicOrder::SeqCst {
                return Err(error(DiagnosticKind::UnsupportedFeature,"CUDA/HIP threadfence provides a stronger ordering contract; explicit non-seq-cst fence requests are unsupported"));
            }
        }
        Statement::AsyncCopy{destination,source,count,event}=>{
            if !uniform {return Err(error(DiagnosticKind::DivergentBarrier,"asynchronous cooperative copy must be reached by all work-items"))}
            if !profile.supports(Feature::AsynchronousCopy) {return Err(Diagnostic::new(profile,DiagnosticKind::UnsupportedFeature,Some(kernel),path,"device asynchronous-copy support not confirmed",Some(Feature::AsynchronousCopy)))}
            if profile.backend!=Backend::IntelOpenCl{return Err(error(DiagnosticKind::UnsupportedFeature,"portable copy currently lowers to OpenCL async_work_group_copy only"))}
            if !is_identifier(event)||is_reserved(event){return Err(error(DiagnosticKind::IrValidation,"invalid async event name"))}
            if !matches!(count.ty(),DataType::Scalar(Number::U32|Number::I32)){return Err(error(DiagnosticKind::InvalidAtomicType,"asynchronous copy count must be integer"))}
            let valid=match (destination.ty(),source.ty()) {
                (DataType::Pointer(dst,AddressSpace::Workgroup,Access::ReadWrite),DataType::Pointer(src,AddressSpace::Global,Access::ReadOnly|Access::ReadWrite)) |
                (DataType::Pointer(dst,AddressSpace::Global,Access::ReadWrite),DataType::Pointer(src,AddressSpace::Workgroup,Access::ReadWrite)) =>dst==src,
                _=>false,
            };
            if !valid{return Err(error(DiagnosticKind::IrValidation,"asynchronous copy requires matching global/local pointers"))}
        }
        Statement::AwaitCopy(_)=>if !uniform{return Err(error(DiagnosticKind::DivergentBarrier,"cooperative copy wait must be uniform"))},
        _=>(),
    }
    Ok(())
}
fn sync_emit(s:&mut String,stmt:&Statement,b:Backend,depth:usize)->Result<(),IrError>{
    let pad="    ".repeat(depth);
    match stmt {
        Statement::Atomic(atom)=>{
            let spec=atom.spec()?;
            if !matches!(b,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip){return Err(IrError::Unsupported{backend:b,detail:"atomic operation has no backend lowering"})}
            let dst=format!("&{}[{}]",val(&atom.pointer,b)?,val(&atom.index,b)?);
            let v=atom.value.as_ref().map(|v|val(v,b)).transpose()?;
            let expect=atom.expected.as_ref().map(|v|val(v,b)).transpose()?;
            let intrinsic=if b==Backend::IntelOpenCl {
                let ty=match spec.scalar{Number::I32=>"atomic_int",Number::U32=>"atomic_uint",Number::I64=>"atomic_long",Number::U64=>"atomic_ulong",_=>return Err(IrError::InvalidType)};
                let space=if spec.address_space==AddressSpace::Workgroup{"__local"}else{"__global"};
                let name=match atom.op{AtomicOp::Load=>"atomic_load_explicit",AtomicOp::Store=>"atomic_store_explicit",
                    AtomicOp::Exchange=>"atomic_exchange_explicit",AtomicOp::Add=>"atomic_fetch_add_explicit",
                    AtomicOp::Sub=>"atomic_fetch_sub_explicit",AtomicOp::And=>"atomic_fetch_and_explicit",
                    AtomicOp::Or=>"atomic_fetch_or_explicit",AtomicOp::Xor=>"atomic_fetch_xor_explicit",
                    AtomicOp::Min=>"atomic_fetch_min_explicit",AtomicOp::Max=>"atomic_fetch_max_explicit",
                    AtomicOp::CompareExchange=>"atomic_compare_exchange_strong_explicit"};
                let order=|o:AtomicOrder|match o{AtomicOrder::Relaxed=>"memory_order_relaxed",AtomicOrder::Acquire=>"memory_order_acquire",AtomicOrder::Release=>"memory_order_release",AtomicOrder::AcqRel=>"memory_order_acq_rel",AtomicOrder::SeqCst=>"memory_order_seq_cst"};
                let scope=match atom.scope{AtomicScope::Workgroup=>"memory_scope_work_group",AtomicScope::Device=>"memory_scope_device",AtomicScope::System=>"memory_scope_all_svm_devices"};
                let ptr=format!("((volatile {} {}*)({}))",space,ty,dst);
                if atom.op==AtomicOp::CompareExchange {
                    let result=atom.result.as_ref().ok_or(IrError::InvalidType)?;
                    // Strong compare exchange updates its expected value with the old value on failure.
                    s.push_str(&format!("{}{} _mata_expected_{} = {};\n",pad,scalar(spec.scalar,b)?,result,expect.unwrap()));
                    s.push_str(&format!("{}bool _mata_success_{} = {}({}, &_mata_expected_{}, {}, {}, {}, {});\n",pad,result,name,ptr,result,v.unwrap(),order(atom.order),order(atom.failure_order.unwrap()),scope));
                    s.push_str(&format!("{}{} {} = _mata_expected_{};\n",pad,scalar(spec.scalar,b)?,result,result));
                    return Ok(())
                }
                let mut args=alloc::vec![ptr];if let Some(v)=v {args.push(v)}args.push(order(atom.order).to_string());args.push(scope.to_string());
                format!("{}({})",name,args.join(", "))
            } else {
                if atom.order!=AtomicOrder::Relaxed || atom.scope!=AtomicScope::Device {
                    return Err(IrError::Unsupported{backend:b,detail:"explicit ordering requires vendor C++ atomic support"});
                }
                let name=match atom.op{AtomicOp::Load=>return Err(IrError::Unsupported{backend:b,detail:"atomic load requires ordered vendor atomics"}),
                    AtomicOp::Store=>return Err(IrError::Unsupported{backend:b,detail:"atomic store requires ordered vendor atomics"}),
                    AtomicOp::Exchange=>"atomicExch",AtomicOp::Add=>"atomicAdd",AtomicOp::Sub=>"atomicSub",
                    AtomicOp::And=>"atomicAnd",AtomicOp::Or=>"atomicOr",AtomicOp::Xor=>"atomicXor",
                    AtomicOp::Min=>"atomicMin",AtomicOp::Max=>"atomicMax",AtomicOp::CompareExchange=>"atomicCAS"};
                let args=if atom.op==AtomicOp::CompareExchange{format!("{}, {}, {}",dst,expect.unwrap(),v.unwrap())}else{format!("{}, {}",dst,v.unwrap())};
                format!("{}({})",name,args)
            };
            if let Some(result)=&atom.result{s.push_str(&format!("{}{} {} = {};\n",pad,scalar(spec.scalar,b)?,result,intrinsic))}
            else{s.push_str(&format!("{}{};\n",pad,intrinsic))}
        }
        Statement::Fence(fence)=>{
            let intrinsic=match b {
                Backend::IntelOpenCl=>{
                    let order=match fence.order{AtomicOrder::Relaxed=>"memory_order_relaxed",AtomicOrder::Acquire=>"memory_order_acquire",AtomicOrder::Release=>"memory_order_release",AtomicOrder::AcqRel=>"memory_order_acq_rel",AtomicOrder::SeqCst=>"memory_order_seq_cst"};
                    let scope=match fence.scope{AtomicScope::Workgroup=>"memory_scope_work_group",AtomicScope::Device=>"memory_scope_device",AtomicScope::System=>"memory_scope_all_svm_devices"};
                    let domain=match fence.domain{FenceDomain::Local=>"CLK_LOCAL_MEM_FENCE",FenceDomain::Global=>"CLK_GLOBAL_MEM_FENCE",FenceDomain::Image=>"CLK_IMAGE_MEM_FENCE"};
                    format!("atomic_work_item_fence({}, {}, {})",domain,order,scope)
                }
                Backend::NvidiaCuda|Backend::AmdHip=>{
                    if fence.order!=AtomicOrder::SeqCst{return Err(IrError::Unsupported{backend:b,detail:"non-seq-cst fence has no validated native lowering"})}
                    match fence.scope{AtomicScope::Workgroup=>"__threadfence_block()",AtomicScope::Device=>"__threadfence()",AtomicScope::System=>"__threadfence_system()"}.into()
                }
                _=>return Err(IrError::Unsupported{backend:b,detail:"memory fence unsupported"}),
            };
            s.push_str(&format!("{}{};\n",pad,intrinsic));
        }
        Statement::AsyncCopy{destination,source,count,event}=>{
            if b!=Backend::IntelOpenCl{return Err(IrError::Unsupported{backend:b,detail:"async group copy requires an implemented backend"})}
            s.push_str(&format!("{}event_t {} = async_work_group_copy({}, {}, {}, 0);\n",pad,event,val(destination,b)?,val(source,b)?,val(count,b)?));
        }
        Statement::AwaitCopy(event)=>{
            if b!=Backend::IntelOpenCl{return Err(IrError::Unsupported{backend:b,detail:"async event wait unsupported"})}
            s.push_str(&format!("{}wait_group_events(1, &{});\n",pad,event));
        }
        _=>return Err(IrError::InvalidType),
    }
    Ok(())
}
/// Strict, lexical lifetime analysis for asynchronous workgroup copies.
/// Phase 7 accepts copies only in the top-level kernel body, since safely
/// carrying event handles through branches/loops needs a token-aware CFG.
fn verify_async_lifetimes(body:&[Statement],profile:&TargetProfile,kernel:&str)->Result<(),Diagnostic> {
    let mut pending:Vec<String>=Vec::new();
    fn nested_has_async(body:&[Statement])->bool {
        body.iter().any(|s|match s {
            Statement::AsyncCopy{..}|Statement::AwaitCopy(_)=>true,
            Statement::If(_,a,b)=>nested_has_async(a)||nested_has_async(b),
            Statement::While(_,a)|Statement::ForRange{body:a,..}=>nested_has_async(a),_=>false,
        })
    }
    for (i,stmt) in body.iter().enumerate(){
        let path=format!("body[{}]",i);
        let failure=|message:&str|Diagnostic::new(profile,DiagnosticKind::IrValidation,Some(kernel),&path,message,None);
        match stmt {
            Statement::AsyncCopy{event,..}=>{
                if pending.iter().any(|name|name==event){return Err(failure("duplicate active async copy event name"))}
                pending.push(event.clone());
            }
            Statement::AwaitCopy(event)=>{
                let Some(index)=pending.iter().position(|name|name==event) else {return Err(failure("async event was never created or already awaited"))};
                pending.remove(index);
            }
            Statement::If(_,a,b)=>if nested_has_async(a)||nested_has_async(b){return Err(failure("async copy tokens cannot cross conditional scopes"))},
            Statement::While(_,a)|Statement::ForRange{body:a,..}=>if nested_has_async(a){return Err(failure("async copy tokens cannot cross loop scopes"))},
            Statement::Return(_)=>if !pending.is_empty(){return Err(failure("kernel returns before awaiting cooperative copy"))},
            _=>(),
        }
    }
    if !pending.is_empty(){return Err(Diagnostic::new(profile,DiagnosticKind::IrValidation,Some(kernel),"end","unawaited cooperative copy event(s)",None))}
    Ok(())
}
