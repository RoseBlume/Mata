// C++-only syntax smoke test using mock HIP declarations.
// This does NOT verify HIP intrinsics, device ISA, or kernel execution.
#define __global__
struct SimDim { unsigned int x; };
extern SimDim blockIdx, blockDim, threadIdx;
extern "C" __global__ void transform(const float* __restrict__ input, float* __restrict__ output, unsigned int count) {
    unsigned int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= count) return;
    output[idx] = (input[idx] * 2.0);
}
