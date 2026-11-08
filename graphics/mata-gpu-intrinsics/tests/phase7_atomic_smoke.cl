__kernel void atomic_phase7(__global uint* counter, __global uint* old_out) {
    uint i=(uint)get_global_id(0);
    uint old = atomic_fetch_add_explicit((volatile __global atomic_uint*)(&counter[i]), 1u, memory_order_acq_rel, memory_scope_device);
    atomic_work_item_fence(CLK_GLOBAL_MEM_FENCE, memory_order_release, memory_scope_device);
    old_out[i] = old;
}
__kernel void coop_copy(__global uint* input, __global uint* output){
    __local uint scratch[8];
    event_t copy_done = async_work_group_copy(scratch,input,8u,0);
    wait_group_events(1, &copy_done);
    barrier(CLK_LOCAL_MEM_FENCE);
    output[get_global_id(0)] = scratch[get_local_id(0)];
}
