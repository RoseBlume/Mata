__kernel void mata_phase5_math(__global const float *x, __global float *out) {
    const uint i = (uint)get_global_id(0);
    const float v = x[i];
    out[i] = log1p(v) + expm1(v) + erf(v) + cbrt(v) + hypot(v, v)
           + sinpi(v) + cospi(v) + tanpi(v) + nextafter(v, 1.0f)
           + atan2(v, 2.0f) + rsqrt(v + 1.0f);
}
__kernel void mata_phase5_integer(__global const uint *a, __global uint *out) {
    const uint i = (uint)get_global_id(0);
    const uint x = a[i];
    out[i] = add_sat(x, 5u) + sub_sat(x, 2u) + abs_diff(x, 3u)
           + rotate(x, 1u) + popcount(x) + clz(x) + ctz(x) + mul_hi(x, 7u);
}
