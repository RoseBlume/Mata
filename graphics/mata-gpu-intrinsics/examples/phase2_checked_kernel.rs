//! Offline, hardware-independent profile validation example.
use mata_gpu_intrinsics::{Backend, ir::{Program, Kernel, Statement, Value, TargetProfile, DeviceLimits}};
fn main() {
    let mut program=Program::default();
    program.add_kernel(Kernel{name:"safe".into(),args:vec![],body:vec![Statement::While(Value::Bool(false),vec![])],workgroup:[8,1,1]}).unwrap();
    let mut target=TargetProfile::offline(Backend::IntelOpenCl);
    // Populate these properties from a real device query before dispatch.
    target.strict_limits=true;
    target.limits=DeviceLimits {
        max_workgroup_size:Some(256),max_workgroup_dimensions:[Some(256),Some(256),Some(64)],
        max_storage_bindings:Some(8),max_uniform_bytes:Some(16384),max_buffer_element_bytes:Some(1048576),
    };
    println!("{}",program.emit_for(&target).expect("kernel must validate"));
}
