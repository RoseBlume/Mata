use mata_gpu_intrinsics::{Backend,ir::*};
fn u(v:u32)->Value{Value::UInt(u64::from(v),DataType::Scalar(Number::U32))}
fn pointer(name:&str,space:AddressSpace)->Value{variable(name,DataType::Pointer(Box::new(DataType::Scalar(Number::U32)),space,Access::ReadWrite)).unwrap()}
fn main(){
    let mut p=Program::default();
    let atomic=atomic_statement(AtomicOp::Add,pointer("counter",AddressSpace::Global),u(0),Some(u(1)),None,
        Some("previous"),AtomicOrder::AcqRel,AtomicScope::Device,None).unwrap();
    p.add_kernel(Kernel{name:"ordered_add".into(),args:vec![Parameter{name:"counter".into(),ty:DataType::Pointer(Box::new(DataType::Scalar(Number::U32)),AddressSpace::Global,Access::ReadWrite)}],body:vec![atomic],workgroup:[8,1,1]}).unwrap();
    let mut profile=TargetProfile::offline(Backend::IntelOpenCl);
    profile.enable(Feature::Int64); // example only, no 64-bit operations required
    print!("{}",p.emit_for(&profile).unwrap());
}
