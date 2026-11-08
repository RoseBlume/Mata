use mata_gpu_intrinsics::{Backend,ir::*,collectives};
#[test]
fn portable_workgroup_tree_reduction_emits_all_three_backends(){
    let ty=DataType::Scalar(Number::U32);
    let mut body=Vec::new();
    let value=variable("input_value",ty.clone()).unwrap();
    let total=collectives::workgroup_sum(&mut body,"scratch",8,Value::Builtin(Builtin::LocalId,0),value).unwrap();
    body.push(Statement::Let("total".into(),ty.clone(),total));
    let mut p=Program::default();
    p.add_kernel(Kernel{name:"sum".into(),args:vec![Parameter{name:"input_value".into(),ty}],body,workgroup:[8,1,1]}).unwrap();
    for b in [Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip]{
        let src=p.emit_for(&TargetProfile::offline(b)).unwrap();
        assert_eq!(src.matches(if b==Backend::IntelOpenCl{"barrier(CLK_LOCAL_MEM_FENCE)"}else{"__syncthreads()"}).count(),4);
        assert!(src.contains("scratch["));
    }
}
#[test]
fn group_reduction_rejects_non_power_of_two_and_invalid_types(){
    let mut body=Vec::new();let u=Value::UInt(1,DataType::Scalar(Number::U32));
    assert!(collectives::workgroup_sum(&mut body,"scratch",7,u.clone(),u.clone()).is_err());
    assert!(collectives::workgroup_sum(&mut body,"scratch",8,u.clone(),Value::Bool(true)).is_err());
    assert!(body.is_empty());
}
#[test]
fn inclusive_scan_is_double_buffered_and_synchronizes_each_stage(){
    let mut body=Vec::new();
    let total=collectives::workgroup_inclusive_scan(&mut body,"read_scratch","write_scratch",8,Value::Builtin(Builtin::LocalId,0),Value::UInt(1,DataType::Scalar(Number::U32))).unwrap();
    body.push(Statement::Let("prefix".into(),DataType::Scalar(Number::U32),total));
    let mut p=Program::default();p.add_kernel(Kernel{name:"scan".into(),args:vec![],body,workgroup:[8,1,1]}).unwrap();
    for b in [Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip]{
        let src=p.emit_for(&TargetProfile::offline(b)).unwrap();
        assert!(src.contains("read_scratch["));
        assert!(src.contains("write_scratch["));
        assert_eq!(src.matches(if b==Backend::IntelOpenCl{"barrier(CLK_LOCAL_MEM_FENCE)"}else{"__syncthreads()"}).count(),4);
    }
}
#[test]
fn fixed_tile_collective_does_not_assume_vendor_wave_size(){
    let mut body=Vec::new();
    let sum=collectives::workgroup_partitioned_sum(&mut body,"tile",32,8,Value::Builtin(Builtin::LocalId,0),Value::UInt(1,DataType::Scalar(Number::U32))).unwrap();
    body.push(Statement::Let("total".into(),DataType::Scalar(Number::U32),sum));
    let mut p=Program::default();p.add_kernel(Kernel{name:"tile_sum".into(),args:vec![],body,workgroup:[32,1,1]}).unwrap();
    for backend in [Backend::IntelOpenCl,Backend::NvidiaCuda,Backend::AmdHip]{
        assert!(p.emit_for(&TargetProfile::offline(backend)).is_ok());
    }
}
