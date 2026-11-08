// Phase 3: typed common compute intrinsics.  All emitters here are device-
// independent: no GPU driver, SDK or compiler is needed to construct the IR.
fn compute_result(op:ComputeOp,types:&[DataType])->Result<DataType,IrError>{
    use ComputeOp::*;
    let expected=match op {
        Abs|Sqrt|Sin|Cos|Tan|Exp|Log|Floor|Ceil|Length|
        Trunc|Round|Rint|Exp2|Exp10|Expm1|Log2|Log10|Log1p|
        Cbrt|Erf|Erfc|Tgamma|Lgamma|Asin|Acos|Atan|
        Sinh|Cosh|Tanh|Asinh|Acosh|Atanh|Fabs|
        Rsqrt|Reciprocal|Fract|SinPi|CosPi|TanPi|Popcount|Clz|Ctz|SinCos=>1,
        Min|Max|Pow|Dot|Hypot|Atan2|Fmod|Copysign|Fdim|NextAfter|AddSat|SubSat|AbsDiff|Rotate|MulHi=>2,
        Clamp|Fma=>3,
    };
    if types.len()!=expected{return Err(IrError::TypeMismatch)}
    let first=&types[0];
    let (number,width)=match first {
        DataType::Scalar(n)=>(*n,1),DataType::Vector(n,w)=>(*n,*w),_=>return Err(IrError::InvalidType)
    };
    let is_integer=matches!(op,AddSat|SubSat|AbsDiff|Rotate|Popcount|Clz|Ctz|MulHi);
    let valid_scalar=if is_integer {matches!(number,Number::I8|Number::U8|Number::I16|Number::U16|Number::I32|Number::U32|Number::I64|Number::U64)}
        else {matches!(number,Number::F32|Number::F64)};
    if !valid_scalar || width>4 || width==0 || width==1 && matches!(first,DataType::Vector(_,_)) {
        return Err(IrError::InvalidType);
    }
    if op==AbsDiff && !matches!(number,Number::U8|Number::U16|Number::U32|Number::U64) {
        return Err(IrError::InvalidType);
    }
    for t in &types[1..] {if t!=first{return Err(IrError::TypeMismatch)}}
    if (op==Length || op==Dot) && width==1{return Err(IrError::InvalidType)}
    Ok(if op==Dot || op==Length {DataType::Scalar(number)}else if op==SinCos {
        DataType::Tuple(alloc::vec![first.clone(),first.clone()])
    }else{first.clone()})
}

fn native_math_name(op:ComputeOp,n:Number,b:Backend)->Result<&'static str,IrError>{
    use ComputeOp::*;
    let float32=n==Number::F32;
    let opencl=b==Backend::IntelOpenCl;
    let name=match op {
        Abs=>if opencl {"fabs"}else if float32 {"fabsf"}else{"fabs"},
        Sqrt=>if float32 && !opencl {"sqrtf"}else{"sqrt"},
        Sin=>if float32 && !opencl {"sinf"}else{"sin"},
        Cos=>if float32 && !opencl {"cosf"}else{"cos"},
        Tan=>if float32 && !opencl {"tanf"}else{"tan"},
        Exp=>if float32 && !opencl {"expf"}else{"exp"},
        Log=>if float32 && !opencl {"logf"}else{"log"},
        Floor=>if float32 && !opencl {"floorf"}else{"floor"},
        Ceil=>if float32 && !opencl {"ceilf"}else{"ceil"},
        Min=>if float32 && !opencl {"fminf"}else{"fmin"},
        Max=>if float32 && !opencl {"fmaxf"}else{"fmax"},
        Clamp=>"clamp", // CUDA/HIP uses explicit fmin/fmax composition below.
        Fma=>if float32 && !opencl {"fmaf"}else{"fma"},
        Pow=>if float32 && !opencl {"powf"}else{"pow"},
        Trunc=>if float32 && !opencl {"truncf"}else{"trunc"},
        Round=>if float32 && !opencl {"roundf"}else{"round"},
        Rint=>if float32 && !opencl {"rintf"}else{"rint"},
        Exp2=>if float32 && !opencl {"exp2f"}else{"exp2"},
        Exp10=>if opencl {"exp10"}else{return Err(IrError::Unsupported{backend:b,detail:"exp10 requires OpenCL or validated native extension"})},
        Expm1=>if float32 && !opencl {"expm1f"}else{"expm1"},
        Log2=>if float32 && !opencl {"log2f"}else{"log2"},
        Log10=>if float32 && !opencl {"log10f"}else{"log10"},
        Log1p=>if float32 && !opencl {"log1pf"}else{"log1p"},
        Cbrt=>if float32 && !opencl {"cbrtf"}else{"cbrt"},
        Erf=>if float32 && !opencl {"erff"}else{"erf"},
        Erfc=>if float32 && !opencl {"erfcf"}else{"erfc"},
        Tgamma=>if float32 && !opencl {"tgammaf"}else{"tgamma"},
        Lgamma=>if float32 && !opencl {"lgammaf"}else{"lgamma"},
        Asin=>if float32 && !opencl {"asinf"}else{"asin"},
        Acos=>if float32 && !opencl {"acosf"}else{"acos"},
        Atan=>if float32 && !opencl {"atanf"}else{"atan"},
        Sinh=>if float32 && !opencl {"sinhf"}else{"sinh"},
        Cosh=>if float32 && !opencl {"coshf"}else{"cosh"},
        Tanh=>if float32 && !opencl {"tanhf"}else{"tanh"},
        Asinh=>if float32 && !opencl {"asinhf"}else{"asinh"},
        Acosh=>if float32 && !opencl {"acoshf"}else{"acosh"},
        Atanh=>if float32 && !opencl {"atanhf"}else{"atanh"},
        Fabs=>if float32 && !opencl {"fabsf"}else{"fabs"},
        Hypot=>if float32 && !opencl {"hypotf"}else{"hypot"},
        Atan2=>if float32 && !opencl {"atan2f"}else{"atan2"},
        Fmod=>if float32 && !opencl {"fmodf"}else{"fmod"},
        Copysign=>if float32 && !opencl {"copysignf"}else{"copysign"},
        Fdim=>if float32 && !opencl {"fdimf"}else{"fdim"},
        NextAfter=>if float32 && !opencl {"nextafterf"}else{"nextafter"},
        SinPi=>if opencl {"sinpi"}else{return Err(IrError::Unsupported{backend:b,detail:"pi-normalized math requires OpenCL or an audited fallback"})},
        CosPi=>if opencl {"cospi"}else{return Err(IrError::Unsupported{backend:b,detail:"pi-normalized math requires OpenCL or an audited fallback"})},
        TanPi=>if opencl {"tanpi"}else{return Err(IrError::Unsupported{backend:b,detail:"pi-normalized math requires OpenCL or an audited fallback"})},
        Rsqrt=>if opencl {"rsqrt"}else if float32 {"rsqrtf"}else{"rsqrt"},
        Reciprocal|Fract|Dot|Length|AddSat|SubSat|AbsDiff|Rotate|Popcount|Clz|Ctz|MulHi|SinCos=>return Err(IrError::InvalidType),
    };
    Ok(name)
}

fn emit_compute(op:ComputeOp,args:&[Value],b:Backend)->Result<String,IrError>{
    if !matches!(b,Backend::IntelOpenCl|Backend::NvidiaCuda|Backend::AmdHip){
        return Err(IrError::Unsupported{backend:b,detail:"Phase 3 math lowering currently targets OpenCL C, CUDA C++, HIP C++"});
    }
    let types=args.iter().map(Value::ty).collect::<Vec<_>>();
    let result=compute_result(op,&types)?;
    if op==ComputeOp::SinCos {
        // This portable multi-result fallback uses two math calls. Do not
        // duplicate a volatile load or a function with side effects.
        if args.len()!=1 || !matches!(&args[0],Value::Var(_,_)|Value::FloatBits(_)|Value::F64Bits(_)) {
            return Err(IrError::Unsupported{backend:b,detail:"materialize sincos input in a temporary before multi-result lowering"});
        }
        let si=emit_compute(ComputeOp::Sin,args,b)?;
        let co=emit_compute(ComputeOp::Cos,args,b)?;
        let name=tuple_name(&result);
        return Ok(if b==Backend::IntelOpenCl {format!("(({}){{{}, {}}})",name,si,co)}
            else {format!("{}{{{}, {}}}",name,si,co)});
    }
    let (number,lanes)=match &types[0] {DataType::Scalar(n)=>(*n,1usize),DataType::Vector(n,w)=>(*n,*w as usize),_=>unreachable!()};
    let values=args.iter().map(|v|val(v,b)).collect::<Result<Vec<_>,_>>()?;
    if b!=Backend::IntelOpenCl && lanes>1 {
        // Component-wise fallback references each operand multiple times.
        // Reject potentially side-effecting expressions rather than re-evaluating
        // a helper call or a memory load once for every vector lane.
        fn stable(v:&Value)->bool { match v {
            Value::Var(_,_)=>true, Value::Field(parent,_,_)=>stable(parent), _=>false,
        }}
        if args.iter().any(|v|!stable(v)) {
            return Err(IrError::Unsupported{backend:b,detail:"materialize vector math operands in local variables before CUDA/HIP component-wise lowering"});
        }
    }
    if b==Backend::IntelOpenCl {
        if matches!(op,ComputeOp::AddSat|ComputeOp::SubSat|ComputeOp::AbsDiff|ComputeOp::Rotate|ComputeOp::Popcount|ComputeOp::Clz|ComputeOp::Ctz|ComputeOp::MulHi){
            let name=match op {
                ComputeOp::AddSat=>"add_sat",ComputeOp::SubSat=>"sub_sat",ComputeOp::AbsDiff=>"abs_diff",
                ComputeOp::Rotate=>"rotate",ComputeOp::Popcount=>"popcount",ComputeOp::Clz=>"clz",
                ComputeOp::Ctz=>"ctz",ComputeOp::MulHi=>"mul_hi",_=>unreachable!(),
            };
            return Ok(format!("{}({})",name,values.join(", ")))
        }
        if op==ComputeOp::Reciprocal {return Ok(format!("(1.0f / ({}))", values[0]));}
        if op==ComputeOp::Fract {return Err(IrError::Unsupported{backend:b,detail:"OpenCL fract requires a pointer output"})}
        let name=match op {ComputeOp::Dot=>"dot",ComputeOp::Length=>"length",_=>native_math_name(op,number,b)?};
        return Ok(format!("{}({})",name,values.join(", ")));
    }
    if matches!(op,ComputeOp::AddSat|ComputeOp::SubSat|ComputeOp::AbsDiff|ComputeOp::Rotate|ComputeOp::Popcount|ComputeOp::Clz|ComputeOp::Ctz|ComputeOp::MulHi) {
        return Err(IrError::Unsupported{backend:b,detail:"integer builtin requires a verified native lowering for this CUDA/HIP target"});
    }
    let components=["x","y","z","w"];
    let component=|v:&str,c:&str|format!("({}).{}",v,c);
    let name=|o|native_math_name(o,number,b);
    let call=|o,vs:&[String]|->Result<String,IrError>{
        if o==ComputeOp::Reciprocal {
            return Ok(format!("((({})1.0) / ({}))",if number==Number::F64{"double"}else{"float"},vs[0]));
        }
        if o==ComputeOp::Fract {
            return Err(IrError::Unsupported{backend:b,detail:"fract requires a rounded-result policy and single-evaluation lowering"});
        }
        if o==ComputeOp::Clamp {
            let lo=format!("{}({}, {})",name(ComputeOp::Max)?,vs[0],vs[1]);
            return Ok(format!("{}({}, {})",name(ComputeOp::Min)?,lo,vs[2]));
        }
        Ok(format!("{}({})",name(o)?,vs.join(", ")))
    };
    if op==ComputeOp::Dot || op==ComputeOp::Length {
        let terms=components[..lanes].iter().map(|c|{
            let a=component(&values[0],c);
            let other=if op==ComputeOp::Dot {component(&values[1],c)}else{a.clone()};
            format!("({} * {})",a,other)
        }).collect::<Vec<_>>();
        let sum=terms.join(" + ");
        return if op==ComputeOp::Dot {Ok(format!("({})",sum))}
        else {Ok(format!("{}({})",name(ComputeOp::Sqrt)?,sum))};
    }
    if lanes==1 {return call(op,&values)}
    // CUDA/HIP's native vector types are structs, not overloaded arithmetic
    // types, so lower each component to typed scalar math and reconstruct.
    let constructor=format!("make_{}{}",if number==Number::F32{"float"}else{"double"},lanes);
    let mut results=Vec::new();
    for c in &components[..lanes] {
        let per_lane=values.iter().map(|v|component(v,c)).collect::<Vec<_>>();
        results.push(call(op,&per_lane)?);
    }
    Ok(format!("{}({})",constructor,results.join(", ")))
}
