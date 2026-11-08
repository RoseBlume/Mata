// Complete pixel decoding on the device. Headers are validated on the host.
// QOI and TGA RLE are inherently serial within a single image: this is a
// correctness-oriented GPU path, not a claim of higher throughput.
inline uint pack_rgba(uint r, uint g, uint b, uint a) {
    return (r << 24) | (g << 16) | (b << 8) | a;
}
inline uint tga_target(uint i, uint w, uint h, uint mode) {
    uint y = i / w, x = i % w;
    if ((mode & 0x200u) == 0u) y = h - 1u - y;
    if ((mode & 0x400u) != 0u) x = w - 1u - x;
    return y * w + x;
}
inline uint tga_pixel(__global const uchar *src, uint p, uint bytes_per_pixel) {
    return pack_rgba(src[p + 2], src[p + 1], src[p],
                     bytes_per_pixel == 4u ? src[p + 3] : 255u);
}
__kernel void mata_full_wbmp(__global const uchar *src, uint len,
    __global uint *dst, uint w, uint h, uint off, uint mode, __global uint *err) {
    uint i = get_global_id(0), n = w * h;
    if (i >= n) return;
    uint x = i % w, y = i / w;
    uint p = off + y * mode + x / 8u;
    if (p >= len) { atomic_or((volatile __global uint*)err,1u); return; }
    dst[i] = (src[p] & (0x80u >> (x & 7u))) ? 0x000000ffu : 0xffffffffu;
}
__kernel void mata_full_tga(__global const uchar *src, uint len,
    __global uint *dst, uint w, uint h, uint off, uint mode, __global uint *err) {
    uint n = w * h, i = get_global_id(0);
    uint bpp = (mode & 0xffu);
    if ((mode & 0x100u) == 0u) {
        if (i >= n) return;
        ulong p = (ulong)off + (ulong)i * bpp;
        if (p + bpp > len) { atomic_or((volatile __global uint*)err,1u); return; }
        dst[tga_target(i,w,h,mode)] = tga_pixel(src,(uint)p,bpp);
        return;
    }
    if (i != 0u) return;
    uint p = off, out = 0u;
    while (out < n) {
        if (p >= len) { atomic_or((volatile __global uint*)err,1u); return; }
        uint tag = src[p++], run = (tag & 127u) + 1u;
        if (run > n - out) { err[0] = 1u; return; }
        if (tag & 128u) {
            if ((ulong)p + bpp > len) { err[0] = 1u; return; }
            uint color = tga_pixel(src,p,bpp); p += bpp;
            for (uint k=0u;k<run;k++) dst[tga_target(out++,w,h,mode)] = color;
        } else {
            if ((ulong)p + (ulong)run*bpp > len) { err[0] = 1u; return; }
            for (uint k=0u;k<run;k++) {
                dst[tga_target(out++,w,h,mode)] = tga_pixel(src,p,bpp);
                p += bpp;
            }
        }
    }
}
__kernel void mata_full_qoi(__global const uchar *src, uint len,
    __global uint *dst, uint w, uint h, uint off, uint mode, __global uint *err) {
    if (get_global_id(0) != 0u) return;
    uint r=0u, g=0u, b=0u, a=255u;
    uint idx[64];
    for (uint k=0u;k<64u;k++) idx[k]=0u;
    uint n = w*h, out=0u, pos=off;
    while (out < n) {
        if (pos >= len) { err[0]=1u; return; }
        uint op = src[pos++];
        if (op == 254u) {
            if ((ulong)pos+3u>len) { err[0]=1u; return; }
            r=src[pos++]; g=src[pos++]; b=src[pos++];
        } else if (op == 255u) {
            if ((ulong)pos+4u>len) { err[0]=1u; return; }
            r=src[pos++]; g=src[pos++]; b=src[pos++]; a=src[pos++];
        } else if ((op&192u)==0u) {
            uint pix=idx[op&63u]; r=pix>>24; g=(pix>>16)&255u; b=(pix>>8)&255u; a=pix&255u;
        } else if ((op&192u)==64u) {
            r=(r+((op>>4)&3u)-2u)&255u;
            g=(g+((op>>2)&3u)-2u)&255u;
            b=(b+(op&3u)-2u)&255u;
        } else if ((op&192u)==128u) {
            if (pos>=len) { err[0]=1u; return; }
            uint second=src[pos++];
            uint dg=(op&63u)-32u;
            r=(r+dg+(second>>4)-8u)&255u;
            g=(g+dg)&255u;
            b=(b+dg+(second&15u)-8u)&255u;
        } else {
            uint run=(op&63u)+1u;
            if (run>n-out) { err[0]=1u; return; }
            uint pix=pack_rgba(r,g,b,a);
            for(uint k=0u;k<run;k++) dst[out++]=pix;
            idx[(r*3u+g*5u+b*7u+a*11u)&63u]=pix;
            continue;
        }
        uint pix=pack_rgba(r,g,b,a);
        idx[(r*3u+g*5u+b*7u+a*11u)&63u]=pix;
        dst[out++]=pix;
    }
    if ((ulong)pos+8u>len) { err[0]=1u; return; }
    for(uint k=0u;k<7u;k++) if(src[pos+k]!=0u) { err[0]=1u; return; }
    if(src[pos+7u]!=1u) err[0]=1u;
}

__kernel void mata_full_bmp(__global const uchar *src, uint len,
    __global uint *dst, uint w, uint h, uint off, uint mode, __global uint *err) {
    uint i=get_global_id(0),n=w*h;
    if(i>=n)return;
    uint bpp=mode&255u,x=i%w,y=i/w;
    uint source_y=(mode&0x200u)?y:h-1u-y;
    ulong stride=(((ulong)w*bpp)+3ul)&(~3ul);
    ulong pos=(ulong)off+(ulong)source_y*stride+(ulong)x*bpp;
    if(pos+bpp>len){atomic_or((volatile __global uint*)err,1u);return;}
    dst[i]=pack_rgba(src[pos+2u],src[pos+1u],src[pos],255u);
}
