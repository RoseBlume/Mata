// Equivalent to `cargo run --example intel_opencl`; an offline syntax fixture.
__kernel void scale_and_bias(const __global float* input, __global float* output, uint count, float scale) {
    uint i = (uint)get_global_id(0);
    if (i >= count) return;
    output[i] = ((input[i] * scale) + 0.25);
}
__kernel void vector_test(const __global float4* input, __global float4* output, uint count) {
    uint i = (uint)get_global_id(0);
    if (i >= count) return;
    float4 x = normalize(input[i]);
    float4 v = (float4)(0.5f, 1.0f, 0.0f, 2.0f);
    output[i] = x * v;
}
