//! Reusable typed GPU-kernel fixtures for tests and examples.
//! No GPU, CUDA toolkit, HIP SDK, or OpenCL ICD is needed to run these tests.
use mata_gpu_intrinsics::{ir::*, Backend};
use std::boxed::Box;

pub(crate) fn t(n:Number)->DataType{DataType::Scalar(n)}
pub(crate) fn ptr(n:Number,a:Access)->DataType{DataType::Pointer(Box::new(t(n)),AddressSpace::Global,a)}
pub(crate) fn p(name:&str,ty:DataType)->Parameter{Parameter{name:name.into(),ty}}
pub(crate) fn v(name:&str,ty:DataType)->Value{variable(name,ty).unwrap()}
pub(crate) fn u(n:u64)->Value{Value::UInt(n,t(Number::U32))}
pub(crate) fn f(n:f32)->Value{Value::FloatBits(n.to_bits())}
pub(crate) fn b(op:Binary,x:Value,y:Value)->Value{binary(op,x,y).unwrap()}
pub(crate) fn c(op:ComputeOp,vals:Vec<Value>)->Value{compute(op,vals).unwrap()}
pub(crate) fn finish(name:&str,args:Vec<Parameter>,body:Vec<Statement>,size:u32)->Program{
    let mut pr=Program::default();
    pr.add_kernel(Kernel{name:name.into(),args,body,workgroup:[size,1,1]}).unwrap();pr
}
pub(crate) fn lower_all(p:&Program)->[String;3]{
    [Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip].map(|backend|{
        let profile=TargetProfile::offline(backend);
        p.emit_for(&profile).unwrap()
    })
}
pub(crate) fn saxpy()->Program{
    let x=ptr(Number::F32,Access::ReadOnly);
    let y=ptr(Number::F32,Access::ReadOnly);
    let output=ptr(Number::F32,Access::WriteOnly);
    let id=global_id(0).unwrap();
    let a=v("alpha",t(Number::F32));
    let fused=c(ComputeOp::Fma,vec![a,load(v("x",x.clone()),id.clone()).unwrap(),load(v("y",y.clone()),id.clone()).unwrap()]);
    let body=vec![Statement::If(b(Binary::Lt,id.clone(),v("count",t(Number::U32))),
        vec![Statement::Store(v("out",output.clone()),id,fused)],vec![])];
    finish("saxpy",vec![p("x",x),p("y",y),p("out",output),p("alpha",t(Number::F32)),p("count",t(Number::U32))],body,64)
}
pub(crate) fn reduction()->Program{
    let x=ptr(Number::F32,Access::ReadOnly);
    let out=ptr(Number::F32,Access::WriteOnly);
    let slot=shared("scratch",Number::F32).unwrap();
    let gid=global_id(0).unwrap();
    let lid=Value::Builtin(Builtin::LocalId,0);
    let group=Value::Builtin(Builtin::GroupId,0);
    let mut statements=vec![Statement::Shared{name:"scratch".into(),element:Number::F32,len:64},
        Statement::Store(slot.clone(),lid.clone(),f(0.0)),
        Statement::If(b(Binary::Lt,gid.clone(),v("count",t(Number::U32))),
            vec![Statement::Store(slot.clone(),lid.clone(),load(v("x",x.clone()),gid).unwrap())],vec![]),
        Statement::Barrier];
    for distance in [32u64,16,8,4,2,1] {
        let other=b(Binary::Add,lid.clone(),u(distance));
        let sum=b(Binary::Add,load(slot.clone(),lid.clone()).unwrap(),load(slot.clone(),other).unwrap());
        statements.push(Statement::If(b(Binary::Lt,lid.clone(),u(distance)),
            vec![Statement::Store(slot.clone(),lid.clone(),sum)],vec![]));
        statements.push(Statement::Barrier);
    }
    statements.push(Statement::If(b(Binary::Eq,lid,u(0)),
        vec![Statement::Store(v("out",out.clone()),group,load(slot,u(0)).unwrap())],vec![]));
    finish("sum_workgroup",vec![p("x",x),p("out",out),p("count",t(Number::U32))],statements,64)
}
pub(crate) fn stencil()->Program{
    let x=ptr(Number::F32,Access::ReadOnly);
    let out=ptr(Number::F32,Access::WriteOnly);
    let id=global_id(0).unwrap();
    let prev=b(Binary::Sub,id.clone(),u(1));
    let next=b(Binary::Add,id.clone(),u(1));
    let left=load(v("x",x.clone()),prev).unwrap();
    let center=load(v("x",x.clone()),id.clone()).unwrap();
    let right=load(v("x",x.clone()),next.clone()).unwrap();
    let weighted=b(Binary::Add,b(Binary::Add,b(Binary::Mul,left,f(0.25)),b(Binary::Mul,center,f(0.5))),b(Binary::Mul,right,f(0.25)));
    let valid=b(Binary::And,b(Binary::Gt,id.clone(),u(0)),b(Binary::Lt,next,v("count",t(Number::U32))));
    finish("stencil3",vec![p("x",x),p("out",out.clone()),p("count",t(Number::U32))],
        vec![Statement::If(valid,vec![Statement::Store(v("out",out),id,weighted)],vec![])],64)
}
pub(crate) fn histogram()->Program{
    let x=ptr(Number::U32,Access::ReadOnly);
    let hist=ptr(Number::U32,Access::ReadWrite);
    let id=global_id(0).unwrap();
    let bin=load(v("x",x.clone()),id.clone()).unwrap();
    let bump=atomic_add(v("hist",hist.clone()),v("bin",t(Number::U32)),u(1)).unwrap();
    let body=vec![Statement::If(b(Binary::Lt,id,v("count",t(Number::U32))),
        vec![Statement::Let("bin".into(),t(Number::U32),bin),
             Statement::If(b(Binary::Lt,v("bin",t(Number::U32)),v("bins",t(Number::U32))),vec![bump],vec![])],vec![])];
    finish("histogram_u32",vec![p("x",x),p("hist",hist),p("count",t(Number::U32)),p("bins",t(Number::U32))],body,64)
}
