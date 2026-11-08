// Whole-device VP8L lossless pixels for the no-transform, single-Huffman-
// group profile. Huffman tables (including complex tables), backreferences,
// literal channels and color caches are decoded on the device.
// Predictor/color-index/transform/meta-group cases deliberately fall back.
#define WEBP_BAD do { status[0] = 1u; return; } while(0)
inline uint png_bits(__global const uchar *src, uint len, __private uint *bit,
                     uint count, __private uint *valid) {
    if (count > 24u || ((ulong)*bit + count) > (ulong)len * 8ul) {
        *valid = 0u; return 0u;
    }
    uint result = 0u;
    for (uint i = 0; i < count; i++) {
        uint p = (*bit)++;
        result |= ((uint)((src[p >> 3] >> (p & 7u)) & 1u)) << i;
    }
    return result;
}

inline uint png_rev_code(uint code, uint n) {
    uint reverse = 0u;
    for (uint i = 0u; i < n; i++) { reverse = (reverse << 1u) | (code & 1u); code >>= 1u; }
    return reverse;
}

// Keep Huffman tables in private storage (never in the global image buffer).
// Symbols sorted by canonical-code order, with first-code and offsets for
// each bit-length. Incomplete trees are accepted only when decodable.
inline uint png_table(__private const uchar *lengths, uint n,
                      __private uint *counts, __private uint *first,
                      __private uint *starts, __private ushort *symbols) {
    for (uint i = 0; i <= 15u; i++) { counts[i] = 0u; first[i] = 0u; starts[i] = 0u; }
    for (uint i = 0; i < n; i++) {
        if (lengths[i] > 15u) return 0u;
        if (lengths[i]) counts[lengths[i]]++;
    }
    int available = 1;
    uint code = 0u, next = 0u;
    for (uint bits = 1; bits <= 15u; bits++) {
        available = (available << 1) - (int)counts[bits];
        if (available < 0) return 0u;
        code = (code + counts[bits - 1u]) << 1u;
        first[bits] = code;
        starts[bits] = next;
        next += counts[bits];
    }
    if (!next) return 0u;
    uint offsets[16];
    for (uint i = 0; i <= 15u; i++) offsets[i] = starts[i];
    for (uint i = 0; i < n; i++) if (lengths[i]) symbols[offsets[lengths[i]]++] = (ushort)i;
    return 1u;
}

inline uint png_symbol(__global const uchar *src, uint len,
                       __private uint *bit, __private uint *valid,
                       __private uint *counts, __private uint *first,
                       __private uint *starts, __private ushort *symbols) {
    uint code = 0u;
    for (uint bits = 1u; bits <= 15u; bits++) {
        // Deflate transmits Huffman codes most-significant-bit first.
        code = (code << 1u) | png_bits(src, len, bit, 1u, valid);
        if (!*valid) return 0u;
        if (code >= first[bits] && code - first[bits] < counts[bits])
            return symbols[starts[bits] + code - first[bits]];
    }
    *valid = 0u;
    return 0u;
}


inline uint webp_symbol(__global const uchar *src,uint len,__private uint *bit,
    __private uint *valid,__private uint *cnt,__private uint *first,
    __private uint *start,__private ushort *symbols) {
    uint total=0u; for(uint i=1u;i<=15u;i++) total+=cnt[i];
    if(total==1u) return symbols[0]; // a single-code VP8L Huffman consumes 0 bits
    return png_symbol(src,len,bit,valid,cnt,first,start,symbols);
}
inline uint webp_read_table(__global const uchar *src,uint len,__private uint *bit,
    __private uint *valid,uint alphabet,__private uint *cnt,__private uint *first,
    __private uint *start,__private ushort *symbols) {
    uchar lengths[540];
    for(uint i=0u;i<alphabet;i++)lengths[i]=0u;
    uint simple=png_bits(src,len,bit,1u,valid);
    if(!*valid)return 0u;
    if(simple) {
        uint n=png_bits(src,len,bit,1u,valid)+1u;
        uint wide=png_bits(src,len,bit,1u,valid);
        uint sym=png_bits(src,len,bit,wide?8u:1u,valid);
        if(!*valid||sym>=alphabet)return 0u;
        lengths[sym]=1u;
        if(n==2u) {
            uint sym2=png_bits(src,len,bit,8u,valid);
            if(!*valid||sym2>=alphabet||sym2==sym)return 0u;
            lengths[sym2]=1u;
        }
    } else {
        const uchar order[19]={17,18,0,1,2,3,4,5,16,6,7,8,9,10,11,12,13,14,15};
        uchar cl[19]; for(uint i=0u;i<19u;i++)cl[i]=0u;
        uint n=png_bits(src,len,bit,4u,valid)+4u;
        if(!*valid||n>19u)return 0u;
        for(uint i=0u;i<n;i++) cl[order[i]]=(uchar)png_bits(src,len,bit,3u,valid);
        if(!*valid)return 0u;
        uint cc[16],cf[16],cs[16];ushort cy[19];
        if(!png_table(cl,19u,cc,cf,cs,cy))return 0u;
        uint constrained=png_bits(src,len,bit,1u,valid);
        uint maximum=alphabet;
        if(constrained) {
            uint nb=2u+2u*png_bits(src,len,bit,3u,valid);
            maximum=2u+png_bits(src,len,bit,nb,valid);
        }
        if(!*valid||maximum>alphabet)return 0u;
        uint i=0u, left=maximum, previous=8u;
        while(i<alphabet && left) {
            left--;
            uint code=webp_symbol(src,len,bit,valid,cc,cf,cs,cy);
            if(!*valid)return 0u;
            if(code<=15u) {
                lengths[i++]=(uchar)code;
                if(code)previous=code;
            } else {
                uint repeat=0u, value=0u;
                if(code==16u){repeat=3u+png_bits(src,len,bit,2u,valid);value=previous;}
                else if(code==17u){repeat=3u+png_bits(src,len,bit,3u,valid);}
                else if(code==18u){repeat=11u+png_bits(src,len,bit,7u,valid);}
                else return 0u;
                if(!*valid||repeat>alphabet-i)return 0u;
                for(uint k=0u;k<repeat;k++)lengths[i++]=(uchar)value;
            }
        }
    }
    if(!png_table(lengths,alphabet,cnt,first,start,symbols))return 0u;
    uint total_codes=0u;
    int remaining=1;
    for(uint depth=1u;depth<=15u;depth++){
        total_codes+=cnt[depth];
        remaining=(remaining<<1)-(int)cnt[depth];
    }
    return total_codes==1u || remaining==0;

}
inline uint webp_prefix(__global const uchar *src,uint len,__private uint *bit,
                        __private uint *valid,uint prefix) {
    if(prefix<4u)return prefix+1u;
    uint extra=(prefix-2u)>>1u;
    return ((2u+(prefix&1u))<<extra)+png_bits(src,len,bit,extra,valid)+1u;
}
__constant int webp_dist_x[120]={0,1,1,-1,0,2,1,-1,2,-2,2,-2,0,3,1,-1,3,-3,2,-2,3,-3,0,4,1,-1,4,-4,3,-3,2,-2,4,-4,0,3,-3,4,-4,5,1,-1,5,-5,2,-2,5,-5,4,-4,3,-3,5,-5,0,6,1,-1,6,-6,2,-2,6,-6,4,-4,5,-5,3,-3,6,-6,0,7,1,-1,5,-5,7,-7,4,-4,6,-6,2,-2,7,-7,3,-3,7,-7,5,-5,6,-6,8,4,-4,7,-7,8,8,6,-6,8,5,-5,7,-7,8,6,-6,7,-7,8,7,-7,8,8};
__constant int webp_dist_y[120]={1,0,1,1,2,0,2,2,1,1,2,2,3,0,3,3,1,1,3,3,2,2,4,0,4,4,1,1,3,3,4,4,2,2,5,4,4,3,3,0,5,5,1,1,5,5,2,2,4,4,5,5,3,3,6,0,6,6,1,1,6,6,2,2,5,5,4,4,6,6,3,3,7,0,7,7,5,5,1,1,6,6,4,4,7,7,2,2,7,7,3,3,6,6,5,5,0,7,7,4,4,1,2,6,6,3,7,7,5,5,4,7,7,6,6,5,7,7,6,7};

__kernel void mata_webp_lossless_full(__global const uchar *src,uint len,
    __global uint *pixels,uint width,uint height,__global uint *status) {
    if(get_global_id(0)!=0u)return;
    if(len<6u || src[0]!=47u || width==0u || height==0u) WEBP_BAD;
    uint header=((uint)src[1])|((uint)src[2]<<8u)|((uint)src[3]<<16u)|((uint)src[4]<<24u);
    if(((header&16383u)+1u)!=width || (((header>>14u)&16383u)+1u)!=height || (header>>29u))WEBP_BAD;
    uint bit=40u,valid=1u,has_transform=png_bits(src,len,&bit,1u,&valid);
    if(!valid||has_transform)WEBP_BAD;
    uint cache_enable=png_bits(src,len,&bit,1u,&valid);
    uint cache_bits=cache_enable?png_bits(src,len,&bit,4u,&valid):0u;
    if(!valid || (cache_enable && (cache_bits<1u||cache_bits>8u)))WEBP_BAD;
    if(png_bits(src,len,&bit,1u,&valid)!=0u || !valid) WEBP_BAD; // no meta groups
    uint alphabets[5]={280u+(cache_bits?1u<<cache_bits:0u),256u,256u,256u,40u};
    uint count[5][16],first[5][16],start[5][16];
    ushort symbols[5][540];
    for(uint g=0u;g<5u;g++){
        if(!webp_read_table(src,len,&bit,&valid,alphabets[g],
            count[g],first[g],start[g],symbols[g]))WEBP_BAD;
    }
    uint cache[256];
    for(uint i=0u;i<(cache_bits?1u<<cache_bits:0u);i++)cache[i]=0u;
    uint n=width*height, pos=0u;
    if(n/width!=height)WEBP_BAD;
    while(pos<n && valid) {
        uint s=webp_symbol(src,len,&bit,&valid,count[0],first[0],start[0],symbols[0]);
        if(!valid)WEBP_BAD;
        if(s<256u) {
            uint red=webp_symbol(src,len,&bit,&valid,count[1],first[1],start[1],symbols[1]);
            uint blue=webp_symbol(src,len,&bit,&valid,count[2],first[2],start[2],symbols[2]);
            uint alpha=webp_symbol(src,len,&bit,&valid,count[3],first[3],start[3],symbols[3]);
            if(!valid)WEBP_BAD;
            uint c=(red<<24u)|(s<<16u)|(blue<<8u)|alpha;
            pixels[pos++]=c;
            if(cache_bits) { uint argb=(alpha<<24u)|(red<<16u)|(s<<8u)|blue;
                cache[(0x1e35a7bdu*argb)>>(32u-cache_bits)]=c; }
        } else if(s<280u) {
            uint length=webp_prefix(src,len,&bit,&valid,s-256u);
            uint dp=webp_symbol(src,len,&bit,&valid,count[4],first[4],start[4],symbols[4]);
            uint dc=webp_prefix(src,len,&bit,&valid,dp);
            if(!valid||dc==0u)WEBP_BAD;
            uint distance=dc>120u?dc-120u:0u;
            if(dc<=120u) {
                int candidate=webp_dist_x[dc-1u]+webp_dist_y[dc-1u]*(int)width;
                distance=(uint)(candidate>1?candidate:1);
            }
            if(!distance || distance>pos || length>n-pos)WEBP_BAD;
            for(uint k=0u;k<length;k++){
                uint c=pixels[pos-distance];pixels[pos++]=c;
                if(cache_bits) {
                    uint r=c>>24u,g=(c>>16u)&255u,b=(c>>8u)&255u,a=c&255u;
                    uint argb=(a<<24u)|(r<<16u)|(g<<8u)|b;
                    cache[(0x1e35a7bdu*argb)>>(32u-cache_bits)]=c;
                }
            }
        } else {
            uint idx=s-280u;
            if(!cache_bits || idx>=(1u<<cache_bits))WEBP_BAD;
            uint c=cache[idx];pixels[pos++]=c;
            uint r=c>>24u,g=(c>>16u)&255u,b=(c>>8u)&255u,a=c&255u;
            uint argb=(a<<24u)|(r<<16u)|(g<<8u)|b;
            cache[(0x1e35a7bdu*argb)>>(32u-cache_bits)]=c;
        }
    }
    if(!valid || pos!=n)WEBP_BAD;
}
