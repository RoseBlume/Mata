// JPEG baseline dequantized 8x8 inverse DCT, OpenCL 1.2.
// Kept bit-for-bit aligned with decoder/jpeg/mod.rs::idct_row and
// idct_column_into. One GPU work item reconstructs a complete 8x8 block.
// Signed 64-bit calculations are necessary for exact fixed-point arithmetic.
inline uchar mata_jpeg_clamp(long x) {
    return (uchar)(x < 0 ? 0 : (x > 255 ? 255 : x));
}
inline void mata_jpeg_row(__private int *b, int p) {
    long x1 = (long)b[p+4] * 2048L;
    long x2 = (long)b[p+6];
    long x3 = (long)b[p+2];
    long x4 = (long)b[p+1];
    long x5 = (long)b[p+7];
    long x6 = (long)b[p+5];
    long x7 = (long)b[p+3];
    if ((x1 | x2 | x3 | x4 | x5 | x6 | x7) == 0L) {
        int dc = (int)((long)b[p] * 8L);
        for (int i=0; i<8; i++) b[p+i] = dc;
        return;
    }
    long x0 = ((long)b[p] * 2048L) + 128L;
    long x8 = 565L*(x4+x5);
    x4 = x8 + (2841L-565L)*x4;
    x5 = x8 - (2841L+565L)*x5;
    x8 = 2408L*(x6+x7);
    x6 = x8 - (2408L-1609L)*x6;
    x7 = x8 - (2408L+1609L)*x7;
    x8 = x0+x1; x0 -= x1;
    x1 = 1108L*(x3+x2);
    x2 = x1 - (2676L+1108L)*x2;
    x3 = x1 + (2676L-1108L)*x3;
    x1 = x4+x6; x4 -= x6;
    x6 = x5+x7; x5 -= x7;
    x7 = x8+x3; x8 -= x3;
    x3 = x0+x2; x0 -= x2;
    x2 = (181L*(x4+x5)+128L)>>8;
    x4 = (181L*(x4-x5)+128L)>>8;
    b[p+0] = (int)((x7+x1)>>8);
    b[p+1] = (int)((x3+x2)>>8);
    b[p+2] = (int)((x0+x4)>>8);
    b[p+3] = (int)((x8+x6)>>8);
    b[p+4] = (int)((x8-x6)>>8);
    b[p+5] = (int)((x0-x4)>>8);
    b[p+6] = (int)((x3-x2)>>8);
    b[p+7] = (int)((x7-x1)>>8);
}
inline void mata_jpeg_column(__private const int *b, int c, __private uchar *out) {
    long x1 = (long)b[c+32]*256L;
    long x2 = (long)b[c+48];
    long x3 = (long)b[c+16];
    long x4 = (long)b[c+8];
    long x5 = (long)b[c+56];
    long x6 = (long)b[c+40];
    long x7 = (long)b[c+24];
    if ((x1 | x2 | x3 | x4 | x5 | x6 | x7) == 0L) {
        uchar v = mata_jpeg_clamp((((long)b[c]+32L)>>6)+128L);
        for (int i=0;i<8;i++) out[i*8+c] = v;
        return;
    }
    long x0 = ((long)b[c]*256L)+8192L;
    long x8 = 565L*(x4+x5)+4L;
    x4 = (x8+(2841L-565L)*x4)>>3;
    x5 = (x8-(2841L+565L)*x5)>>3;
    x8 = 2408L*(x6+x7)+4L;
    x6 = (x8-(2408L-1609L)*x6)>>3;
    x7 = (x8-(2408L+1609L)*x7)>>3;
    x8 = x0+x1; x0 -= x1;
    x1 = 1108L*(x3+x2)+4L;
    x2 = (x1-(2676L+1108L)*x2)>>3;
    x3 = (x1+(2676L-1108L)*x3)>>3;
    x1 = x4+x6; x4 -= x6;
    x6 = x5+x7; x5 -= x7;
    x7 = x8+x3; x8 -= x3;
    x3 = x0+x2; x0 -= x2;
    x2 = (181L*(x4+x5)+128L)>>8;
    x4 = (181L*(x4-x5)+128L)>>8;
    out[0*8+c] = mata_jpeg_clamp(((x7+x1)>>14)+128L);
    out[1*8+c] = mata_jpeg_clamp(((x3+x2)>>14)+128L);
    out[2*8+c] = mata_jpeg_clamp(((x0+x4)>>14)+128L);
    out[3*8+c] = mata_jpeg_clamp(((x8+x6)>>14)+128L);
    out[4*8+c] = mata_jpeg_clamp(((x8-x6)>>14)+128L);
    out[5*8+c] = mata_jpeg_clamp(((x0-x4)>>14)+128L);
    out[6*8+c] = mata_jpeg_clamp(((x3-x2)>>14)+128L);
    out[7*8+c] = mata_jpeg_clamp(((x7-x1)>>14)+128L);
}
__kernel void mata_jpeg_idct(__global const int *coeffs,
                             __global uchar *pixels, uint count) {
    uint block_id = (uint)get_global_id(0);
    if (block_id >= count) return;
    size_t base = (size_t)block_id*64;
    int block[64];
    uchar spatial[64];
    int any_ac = 0;
    for (int i=0;i<64;i++) {
        block[i] = coeffs[base+(size_t)i];
        if (i > 0) any_ac |= block[i];
    }
    if (any_ac == 0) {
        uchar v = mata_jpeg_clamp((((long)block[0]+4L)>>3)+128L);
        for (int i=0;i<64;i++) pixels[base+(size_t)i] = v;
        return;
    }
    for (int r=0;r<8;r++) mata_jpeg_row(block, r*8);
    for (int c=0;c<8;c++) mata_jpeg_column(block, c, spatial);
    for (int i=0;i<64;i++) pixels[base+(size_t)i] = spatial[i];
}
