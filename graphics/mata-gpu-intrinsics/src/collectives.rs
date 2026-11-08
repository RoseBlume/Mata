//! Phase 7 portable workgroup collective expansion.
//! Explicit shared-memory tree reduction rather than assuming warp32/wave64.
//! The caller must select a 1D kernel workgroup with exactly `group_size`
//! invocations and use `LocalId` on axis X for the lane identifier.
use alloc::{boxed::Box, string::String, vec::Vec};
use super::ir::{self,DataType,Number,Value,Statement,AddressSpace,Access,Binary,IrError};

/// Expand a workgroup-wide sum into typed shared-memory IR and return a value
/// for the total sum that is valid for every lane after the final barrier.
/// This produces barriers *outside* divergent branches and supports OpenCL,
/// CUDA and HIP without relying on vendor-specific cooperative groups.
pub fn workgroup_sum(body:&mut Vec<Statement>,name:&str,group_size:u32,lane:Value,item:Value)->Result<Value,IrError>{
    if group_size==0 || group_size>1024 || !group_size.is_power_of_two(){return Err(IrError::InvalidWorkgroup)}
    if lane.ty()!=DataType::Scalar(Number::U32){return Err(IrError::TypeMismatch)}
    let number=match item.ty() {DataType::Scalar(n@ (Number::I32|Number::U32|Number::F32|Number::F64))=>n,_=>return Err(IrError::InvalidType)};
    let buf=ir::shared(name,number)?;
    body.push(Statement::Shared{name:String::from(name),element:number,len:group_size as usize});
    body.push(Statement::SetIndex(buf.clone(),lane.clone(),item));
    body.push(Statement::Barrier);
    let mut stride=group_size/2;
    while stride>0 {
        let other=ir::binary(Binary::Add,lane.clone(),Value::UInt(u64::from(stride),DataType::Scalar(Number::U32)))?;
        let a=ir::load(buf.clone(),lane.clone())?;
        let b=ir::load(buf.clone(),other)?;
        let sum=ir::binary(Binary::Add,a,b)?;
        let condition=ir::binary(Binary::Lt,lane.clone(),Value::UInt(u64::from(stride),DataType::Scalar(Number::U32)))?;
        body.push(Statement::If(condition,alloc::vec![Statement::SetIndex(buf.clone(),lane.clone(),sum)],Vec::new()));
        body.push(Statement::Barrier);
        stride/=2;
    }
    ir::load(buf,Value::UInt(0,DataType::Scalar(Number::U32)))
}

/// Expand a workgroup-wide inclusive prefix sum using double-buffered
/// Hillis–Steele steps. Double buffering avoids in-place read/write races.
pub fn workgroup_inclusive_scan(body:&mut Vec<Statement>,name_a:&str,name_b:&str,
    group_size:u32,lane:Value,item:Value)->Result<Value,IrError>{
    if group_size==0||group_size>1024||!group_size.is_power_of_two(){return Err(IrError::InvalidWorkgroup)}
    if name_a==name_b||lane.ty()!=DataType::Scalar(Number::U32){return Err(IrError::InvalidType)}
    let num=match item.ty(){DataType::Scalar(n@(Number::I32|Number::U32|Number::F32|Number::F64))=>n,_=>return Err(IrError::InvalidType)};
    let mut a=ir::shared(name_a,num)?;let mut b=ir::shared(name_b,num)?;
    body.push(Statement::Shared{name:String::from(name_a),element:num,len:group_size as usize});
    body.push(Statement::Shared{name:String::from(name_b),element:num,len:group_size as usize});
    body.push(Statement::SetIndex(a.clone(),lane.clone(),item));
    body.push(Statement::Barrier);
    let mut offset=1;
    while offset<group_size {
        let offset_value=Value::UInt(u64::from(offset),DataType::Scalar(Number::U32));
        let can_read=ir::binary(Binary::Ge,lane.clone(),offset_value.clone())?;
        let previous=ir::load(a.clone(),ir::binary(Binary::Sub,lane.clone(),offset_value)?)?;
        let current=ir::load(a.clone(),lane.clone())?;
        let combined=ir::binary(Binary::Add,current.clone(),previous)?;
        body.push(Statement::If(can_read,
            alloc::vec![Statement::SetIndex(b.clone(),lane.clone(),combined)],
            alloc::vec![Statement::SetIndex(b.clone(),lane.clone(),current)]));
        body.push(Statement::Barrier);
        core::mem::swap(&mut a,&mut b);
        offset*=2;
    }
    ir::load(a,lane)
}

/// Partition a 1D workgroup into fixed-size contiguous tiles, returning the
/// per-tile total to each participating lane. Works with variable GPU wave
/// sizes because it uses workgroup barriers, not assumed hardware subgroups.
pub fn workgroup_partitioned_sum(body:&mut Vec<Statement>,name:&str,workgroup_size:u32,tile_size:u32,
    lane:Value,item:Value)->Result<Value,IrError>{
    if workgroup_size==0||workgroup_size>1024||!workgroup_size.is_power_of_two()||
        tile_size==0||!tile_size.is_power_of_two()||tile_size>workgroup_size ||workgroup_size%tile_size!=0 {
        return Err(IrError::InvalidWorkgroup);
    }
    if lane.ty()!=DataType::Scalar(Number::U32){return Err(IrError::TypeMismatch)}
    let number=match item.ty(){DataType::Scalar(n@(Number::I32|Number::U32|Number::F32|Number::F64))=>n,_=>return Err(IrError::InvalidType)};
    let buf=ir::shared(name,number)?;
    body.push(Statement::Shared{name:String::from(name),element:number,len:workgroup_size as usize});
    body.push(Statement::SetIndex(buf.clone(),lane.clone(),item));
    body.push(Statement::Barrier);
    let u=|n:u32|Value::UInt(u64::from(n),DataType::Scalar(Number::U32));
    let tile_base=ir::binary(Binary::Mul,ir::binary(Binary::Div,lane.clone(),u(tile_size))?,u(tile_size))?;
    let tile_lane=ir::binary(Binary::Rem,lane.clone(),u(tile_size))?;
    let mut stride=tile_size/2;
    while stride>0 {
        let conditional=ir::binary(Binary::Lt,tile_lane.clone(),u(stride))?;
        let value_here=ir::load(buf.clone(),lane.clone())?;
        let value_next=ir::load(buf.clone(),ir::binary(Binary::Add,lane.clone(),u(stride))?)?;
        let sum=ir::binary(Binary::Add,value_here,value_next)?;
        body.push(Statement::If(conditional,alloc::vec![Statement::SetIndex(buf.clone(),lane.clone(),sum)],Vec::new()));
        body.push(Statement::Barrier);
        stride/=2;
    }
    ir::load(buf,tile_base)
}
