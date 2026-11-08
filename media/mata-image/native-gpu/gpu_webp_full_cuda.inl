// Same VP8L decoding algorithm as src/gpu_webp_full.cl.
// Whole-device VP8L lossless pixels for the no-transform, single-Huffman-
// group profile. Huffman tables (including complex tables), backreferences,
// literal channels and color caches are decoded on the device.
// Predictor/color-index/transform/meta-group cases deliberately fall back.
#define WEBP_BAD do { status[0] = 1u; return; } while(0)
static __device__ __forceinline__ uint32_t vp8l_bits(const uint8_t *src, uint32_t len, uint32_t *bit,
                     uint32_t count, uint32_t *valid) {
    if (count > 24u || ((uint64_t)*bit + count) > (uint64_t)len * 8ul) {
        *valid = 0u; return 0u;
    }
    uint32_t result = 0u;
    for (uint32_t i = 0; i < count; i++) {
        uint32_t p = (*bit)++;
        result |= ((uint32_t)((src[p >> 3] >> (p & 7u)) & 1u)) << i;
    }
    return result;
}

static __device__ __forceinline__ uint32_t vp8l_rev_code(uint32_t code, uint32_t n) {
    uint32_t reverse = 0u;
    for (uint32_t i = 0u; i < n; i++) { reverse = (reverse << 1u) | (code & 1u); code >>= 1u; }
    return reverse;
}

// Keep Huffman tables in private storage (never in the global image buffer).
// Symbols sorted by canonical-code order, with first-code and offsets for
// each bit-length. Incomplete trees are accepted only when decodable.
static __device__ __forceinline__ uint32_t vp8l_table(const uint8_t *lengths, uint32_t n,
                      uint32_t *counts, uint32_t *first,
                      uint32_t *starts, uint16_t *symbols) {
    for (uint32_t i = 0; i <= 15u; i++) { counts[i] = 0u; first[i] = 0u; starts[i] = 0u; }
    for (uint32_t i = 0; i < n; i++) {
        if (lengths[i] > 15u) return 0u;
        if (lengths[i]) counts[lengths[i]]++;
    }
    int available = 1;
    uint32_t code = 0u, next = 0u;
    for (uint32_t bits = 1; bits <= 15u; bits++) {
        available = (available << 1) - (int)counts[bits];
        if (available < 0) return 0u;
        code = (code + counts[bits - 1u]) << 1u;
        first[bits] = code;
        starts[bits] = next;
        next += counts[bits];
    }
    if (!next) return 0u;
    uint32_t offsets[16];
    for (uint32_t i = 0; i <= 15u; i++) offsets[i] = starts[i];
    for (uint32_t i = 0; i < n; i++) if (lengths[i]) symbols[offsets[lengths[i]]++] = (uint16_t)i;
    return 1u;
}

static __device__ __forceinline__ uint32_t vp8l_symbol(const uint8_t *src, uint32_t len,
                       uint32_t *bit, uint32_t *valid,
                       uint32_t *counts, uint32_t *first,
                       uint32_t *starts, uint16_t *symbols) {
    uint32_t code = 0u;
    for (uint32_t bits = 1u; bits <= 15u; bits++) {
        // Deflate transmits Huffman codes most-significant-bit first.
        code = (code << 1u) | vp8l_bits(src, len, bit, 1u, valid);
        if (!*valid) return 0u;
        if (code >= first[bits] && code - first[bits] < counts[bits])
            return symbols[starts[bits] + code - first[bits]];
    }
    *valid = 0u;
    return 0u;
}


static __device__ __forceinline__ uint32_t webp_symbol(const uint8_t *src,uint32_t len,uint32_t *bit,
    uint32_t *valid,uint32_t *cnt,uint32_t *first,
    uint32_t *start,uint16_t *symbols) {
    uint32_t total=0u; for(uint32_t i=1u;i<=15u;i++) total+=cnt[i];
    if(total==1u) return symbols[0]; // a single-code VP8L Huffman consumes 0 bits
    return vp8l_symbol(src,len,bit,valid,cnt,first,start,symbols);
}
static __device__ __forceinline__ uint32_t webp_read_table(const uint8_t *src,uint32_t len,uint32_t *bit,
    uint32_t *valid,uint32_t alphabet,uint32_t *cnt,uint32_t *first,
    uint32_t *start,uint16_t *symbols) {
    uint8_t lengths[540];
    for(uint32_t i=0u;i<alphabet;i++)lengths[i]=0u;
    uint32_t simple=vp8l_bits(src,len,bit,1u,valid);
    if(!*valid)return 0u;
    if(simple) {
        uint32_t n=vp8l_bits(src,len,bit,1u,valid)+1u;
        uint32_t wide=vp8l_bits(src,len,bit,1u,valid);
        uint32_t sym=vp8l_bits(src,len,bit,wide?8u:1u,valid);
        if(!*valid||sym>=alphabet)return 0u;
        lengths[sym]=1u;
        if(n==2u) {
            uint32_t sym2=vp8l_bits(src,len,bit,8u,valid);
            if(!*valid||sym2>=alphabet||sym2==sym)return 0u;
            lengths[sym2]=1u;
        }
    } else {
        const uint8_t order[19]={17,18,0,1,2,3,4,5,16,6,7,8,9,10,11,12,13,14,15};
        uint8_t cl[19]; for(uint32_t i=0u;i<19u;i++)cl[i]=0u;
        uint32_t n=vp8l_bits(src,len,bit,4u,valid)+4u;
        if(!*valid||n>19u)return 0u;
        for(uint32_t i=0u;i<n;i++) cl[order[i]]=(uint8_t)vp8l_bits(src,len,bit,3u,valid);
        if(!*valid)return 0u;
        uint32_t cc[16],cf[16],cs[16];uint16_t cy[19];
        if(!vp8l_table(cl,19u,cc,cf,cs,cy))return 0u;
        uint32_t constrained=vp8l_bits(src,len,bit,1u,valid);
        uint32_t maximum=alphabet;
        if(constrained) {
            uint32_t nb=2u+2u*vp8l_bits(src,len,bit,3u,valid);
            maximum=2u+vp8l_bits(src,len,bit,nb,valid);
        }
        if(!*valid||maximum>alphabet)return 0u;
        uint32_t i=0u, left=maximum, previous=8u;
        while(i<alphabet && left) {
            left--;
            uint32_t code=webp_symbol(src,len,bit,valid,cc,cf,cs,cy);
            if(!*valid)return 0u;
            if(code<=15u) {
                lengths[i++]=(uint8_t)code;
                if(code)previous=code;
            } else {
                uint32_t repeat=0u, value=0u;
                if(code==16u){repeat=3u+vp8l_bits(src,len,bit,2u,valid);value=previous;}
                else if(code==17u){repeat=3u+vp8l_bits(src,len,bit,3u,valid);}
                else if(code==18u){repeat=11u+vp8l_bits(src,len,bit,7u,valid);}
                else return 0u;
                if(!*valid||repeat>alphabet-i)return 0u;
                for(uint32_t k=0u;k<repeat;k++)lengths[i++]=(uint8_t)value;
            }
        }
    }
    if(!vp8l_table(lengths,alphabet,cnt,first,start,symbols))return 0u;
    uint32_t total_codes=0u;
    int remaining=1;
    for(uint32_t depth=1u;depth<=15u;depth++){
        total_codes+=cnt[depth];
        remaining=(remaining<<1)-(int)cnt[depth];
    }
    return total_codes==1u || remaining==0;

}
static __device__ __forceinline__ uint32_t webp_prefix(const uint8_t *src,uint32_t len,uint32_t *bit,
                        uint32_t *valid,uint32_t prefix) {
    if(prefix<4u)return prefix+1u;
    uint32_t extra=(prefix-2u)>>1u;
    return ((2u+(prefix&1u))<<extra)+vp8l_bits(src,len,bit,extra,valid)+1u;
}
static __device__ __constant__ int webp_dist_x[120]={0,1,1,-1,0,2,1,-1,2,-2,2,-2,0,3,1,-1,3,-3,2,-2,3,-3,0,4,1,-1,4,-4,3,-3,2,-2,4,-4,0,3,-3,4,-4,5,1,-1,5,-5,2,-2,5,-5,4,-4,3,-3,5,-5,0,6,1,-1,6,-6,2,-2,6,-6,4,-4,5,-5,3,-3,6,-6,0,7,1,-1,5,-5,7,-7,4,-4,6,-6,2,-2,7,-7,3,-3,7,-7,5,-5,6,-6,8,4,-4,7,-7,8,8,6,-6,8,5,-5,7,-7,8,6,-6,7,-7,8,7,-7,8,8};
static __device__ __constant__ int webp_dist_y[120]={1,0,1,1,2,0,2,2,1,1,2,2,3,0,3,3,1,1,3,3,2,2,4,0,4,4,1,1,3,3,4,4,2,2,5,4,4,3,3,0,5,5,1,1,5,5,2,2,4,4,5,5,3,3,6,0,6,6,1,1,6,6,2,2,5,5,4,4,6,6,3,3,7,0,7,7,5,5,1,1,6,6,4,4,7,7,2,2,7,7,3,3,6,6,5,5,0,7,7,4,4,1,2,6,6,3,7,7,5,5,4,7,7,6,6,5,7,7,6,7};

__global__ void mata_webp_lossless_full(const uint8_t *src,uint32_t len,
    uint32_t *pixels,uint32_t width,uint32_t height,uint32_t *status) {
    if((blockIdx.x*blockDim.x+threadIdx.x)!=0u)return;
    if(len<6u || src[0]!=47u || width==0u || height==0u) WEBP_BAD;
    uint32_t header=((uint32_t)src[1])|((uint32_t)src[2]<<8u)|((uint32_t)src[3]<<16u)|((uint32_t)src[4]<<24u);
    if(((header&16383u)+1u)!=width || (((header>>14u)&16383u)+1u)!=height || (header>>29u))WEBP_BAD;
    uint32_t bit=40u,valid=1u,has_transform=vp8l_bits(src,len,&bit,1u,&valid);
    if(!valid||has_transform)WEBP_BAD;
    uint32_t cache_enable=vp8l_bits(src,len,&bit,1u,&valid);
    uint32_t cache_bits=cache_enable?vp8l_bits(src,len,&bit,4u,&valid):0u;
    if(!valid || (cache_enable && (cache_bits<1u||cache_bits>8u)))WEBP_BAD;
    if(vp8l_bits(src,len,&bit,1u,&valid)!=0u || !valid) WEBP_BAD; // no meta groups
    uint32_t alphabets[5]={280u+(cache_bits?1u<<cache_bits:0u),256u,256u,256u,40u};
    uint32_t count[5][16],first[5][16],start[5][16];
    uint16_t symbols[5][540];
    for(uint32_t g=0u;g<5u;g++){
        if(!webp_read_table(src,len,&bit,&valid,alphabets[g],
            count[g],first[g],start[g],symbols[g]))WEBP_BAD;
    }
    uint32_t cache[256];
    for(uint32_t i=0u;i<(cache_bits?1u<<cache_bits:0u);i++)cache[i]=0u;
    uint32_t n=width*height, pos=0u;
    if(n/width!=height)WEBP_BAD;
    while(pos<n && valid) {
        uint32_t s=webp_symbol(src,len,&bit,&valid,count[0],first[0],start[0],symbols[0]);
        if(!valid)WEBP_BAD;
        if(s<256u) {
            uint32_t red=webp_symbol(src,len,&bit,&valid,count[1],first[1],start[1],symbols[1]);
            uint32_t blue=webp_symbol(src,len,&bit,&valid,count[2],first[2],start[2],symbols[2]);
            uint32_t alpha=webp_symbol(src,len,&bit,&valid,count[3],first[3],start[3],symbols[3]);
            if(!valid)WEBP_BAD;
            uint32_t c=(red<<24u)|(s<<16u)|(blue<<8u)|alpha;
            pixels[pos++]=c;
            if(cache_bits) { uint32_t argb=(alpha<<24u)|(red<<16u)|(s<<8u)|blue;
                cache[(0x1e35a7bdu*argb)>>(32u-cache_bits)]=c; }
        } else if(s<280u) {
            uint32_t length=webp_prefix(src,len,&bit,&valid,s-256u);
            uint32_t dp=webp_symbol(src,len,&bit,&valid,count[4],first[4],start[4],symbols[4]);
            uint32_t dc=webp_prefix(src,len,&bit,&valid,dp);
            if(!valid||dc==0u)WEBP_BAD;
            uint32_t distance=dc>120u?dc-120u:0u;
            if(dc<=120u) {
                int candidate=webp_dist_x[dc-1u]+webp_dist_y[dc-1u]*(int)width;
                distance=(uint32_t)(candidate>1?candidate:1);
            }
            if(!distance || distance>pos || length>n-pos)WEBP_BAD;
            for(uint32_t k=0u;k<length;k++){
                uint32_t c=pixels[pos-distance];pixels[pos++]=c;
                if(cache_bits) {
                    uint32_t r=c>>24u,g=(c>>16u)&255u,b=(c>>8u)&255u,a=c&255u;
                    uint32_t argb=(a<<24u)|(r<<16u)|(g<<8u)|b;
                    cache[(0x1e35a7bdu*argb)>>(32u-cache_bits)]=c;
                }
            }
        } else {
            uint32_t idx=s-280u;
            if(!cache_bits || idx>=(1u<<cache_bits))WEBP_BAD;
            uint32_t c=cache[idx];pixels[pos++]=c;
            uint32_t r=c>>24u,g=(c>>16u)&255u,b=(c>>8u)&255u,a=c&255u;
            uint32_t argb=(a<<24u)|(r<<16u)|(g<<8u)|b;
            cache[(0x1e35a7bdu*argb)>>(32u-cache_bits)]=c;
        }
    }
    if(!valid || pos!=n)WEBP_BAD;
}
