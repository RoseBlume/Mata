// OpenCL 1.2. Pixel reconstruction only; compressed entropy streams and
// PNG scanline filters remain in the CPU decoder.
__kernel void mata_png_unpack(__global const uchar *src, __global uint *dst,
                               uint count, uint channels) {
    const uint i = (uint)get_global_id(0);
    if (i >= count) return;
    const uint j = i * channels;
    uchar r = src[j], g = r, b = r, a = (uchar)255;
    if (channels == 2u) a = src[j + 1u];
    if (channels >= 3u) {
        g = src[j + 1u]; b = src[j + 2u];
        if (channels == 4u) a = src[j + 3u];
    }
    dst[i] = ((uint)r << 24) | ((uint)g << 16) | ((uint)b << 8) | (uint)a;
}

inline uint mata_clamp8(int a) { return (uint)clamp(a, 0, 255); }
__kernel void mata_vp8_yuv420(__global const uchar *y_plane,
                               __global const uchar *u_plane,
                               __global const uchar *v_plane,
                               __global const uchar *a_plane,
                               __global uint *dst,
                               uint width, uint y_stride, uint uv_stride,
                               uint has_alpha, uint count) {
    const uint i = (uint)get_global_id(0);
    if (i >= count) return;
    const uint px = i % width, py = i / width;
    const uint iy = py * y_stride + px;
    const uint iu = (py / 2u) * uv_stride + px / 2u;
    const int yv = (int)y_plane[iy];
    const int uv = (int)u_plane[iu] - 128;
    const int vv = (int)v_plane[iu] - 128;
    const int ys = 298 * max(yv - 16, 0);
    // Match the existing CPU lookup-table conversion exactly, including
    // the +128 rounding step and signed arithmetic shifts.
    const uint r = mata_clamp8((ys + 409 * vv + 128) >> 8);
    const uint g = mata_clamp8((ys - 100 * uv - 208 * vv + 128) >> 8);
    const uint b = mata_clamp8((ys + 516 * uv + 128) >> 8);
    const uint a = has_alpha ? (uint)a_plane[i] : 255u;
    dst[i] = (r << 24) | (g << 16) | (b << 8) | a;
}

__kernel void mata_vp8l_subgreen(__global const uint *src,
                                  __global uint *dst, uint count) {
    const uint i = (uint)get_global_id(0);
    if (i >= count) return;
    const uint p = src[i]; // VP8L internal AARRGGBB
    const uint g = (p >> 8) & 255u;
    const uint r = (((p >> 16) & 255u) + g) & 255u;
    const uint b = ((p & 255u) + g) & 255u;
    dst[i] = (r << 24) | (g << 16) | (b << 8) | (p >> 24);
}
