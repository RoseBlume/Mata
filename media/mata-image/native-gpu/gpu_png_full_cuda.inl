// Generated equivalent of src/gpu_png_full.cl for HIP/CUDA.
// Device-side PNG zlib/DEFLATE + scanline filters + 8-bit pixel packing.
// Deliberately one work-item per image while Huffman/LZ77 dependencies are
// decoded; correct GPU execution is the initial milestone, not throughput.
// Arguments: zlib, length, output, scratch, width, height, channels, status.
// Unsupported images (interlace, palette, 16-bit, ancillary transparency) are
// rejected by the Rust entry point before launch.
#define PNG_BAD do { status[0] = 1u; return; } while (0)

static __device__ __forceinline__ uint32_t png_bits(const uint8_t *src, uint32_t len, uint32_t *bit,
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

static __device__ __forceinline__ uint32_t png_rev_code(uint32_t code, uint32_t n) {
    uint32_t reverse = 0u;
    for (uint32_t i = 0u; i < n; i++) { reverse = (reverse << 1u) | (code & 1u); code >>= 1u; }
    return reverse;
}

// Keep Huffman tables in private storage (never in the global image buffer).
// Symbols sorted by canonical-code order, with first-code and offsets for
// each bit-length. Incomplete trees are accepted only when decodable.
static __device__ __forceinline__ uint32_t png_table(const uint8_t *lengths, uint32_t n,
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

static __device__ __forceinline__ uint32_t png_symbol(const uint8_t *src, uint32_t len,
                       uint32_t *bit, uint32_t *valid,
                       uint32_t *counts, uint32_t *first,
                       uint32_t *starts, uint16_t *symbols) {
    uint32_t code = 0u;
    for (uint32_t bits = 1u; bits <= 15u; bits++) {
        // Deflate transmits Huffman codes most-significant-bit first.
        code = (code << 1u) | png_bits(src, len, bit, 1u, valid);
        if (!*valid) return 0u;
        if (code >= first[bits] && code - first[bits] < counts[bits])
            return symbols[starts[bits] + code - first[bits]];
    }
    *valid = 0u;
    return 0u;
}

static __device__ __forceinline__ uint32_t png_paeth(uint32_t a, uint32_t b, uint32_t c) {
    int p=(int)a+(int)b-(int)c;
    int da=abs(p-(int)a), db=abs(p-(int)b), dc=abs(p-(int)c);
    return (da<=db && da<=dc) ? a : ((db<=dc) ? b : c);
}

__global__ void mata_png_full(const uint8_t *src, uint32_t len,
    uint32_t *pixels, uint8_t *raw,
    uint32_t width, uint32_t height, uint32_t channels, uint32_t *status) {
    if ((blockIdx.x*blockDim.x+threadIdx.x) != 0u) return;
    if (len < 6u || width == 0u || height == 0u || channels < 1u || channels > 4u) PNG_BAD;
    uint32_t row = width * channels;
    uint32_t total = (row + 1u) * height;
    if (row / channels != width || total / height != row + 1u) PNG_BAD;
    uint32_t cmf = src[0], flg = src[1];
    if ((cmf & 15u) != 8u || (cmf >> 4u) > 7u ||
        ((cmf * 256u + flg) % 31u) || (flg & 32u)) PNG_BAD;

    uint32_t bit = 16u, valid = 1u, out = 0u, done = 0u;
    uint8_t ll[288], dd[32], cl[19];
    uint32_t lcnt[16], lfirst[16], lstart[16], dcnt[16], dfirst[16], dstart[16];
    uint32_t ccnt[16], cfirst[16], cstart[16];
    uint16_t lsym[288], dsym[32], csym[19];
    const uint8_t order[19] = {16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15};
    const uint16_t lenbase[29] = {3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,
        35,43,51,59,67,83,99,115,131,163,195,227,258};
    const uint8_t lenextra[29] = {0,0,0,0,0,0,0,0,1,1,1,1,2,2,2,2,
        3,3,3,3,4,4,4,4,5,5,5,5,0};
    const uint16_t distbase[30] = {1,2,3,4,5,7,9,13,17,25,33,49,65,97,
        129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577};
    const uint8_t distextra[30] = {0,0,0,0,1,1,2,2,3,3,4,4,5,5,6,6,
        7,7,8,8,9,9,10,10,11,11,12,12,13,13};

    while (!done && valid) {
        done = png_bits(src, len, &bit, 1u, &valid);
        uint32_t btype = png_bits(src, len, &bit, 2u, &valid);
        if (!valid || btype == 3u) PNG_BAD;
        if (btype == 0u) {
            bit = (bit + 7u) & ~7u;
            uint32_t n = png_bits(src,len,&bit,16u,&valid);
            uint32_t inv = png_bits(src,len,&bit,16u,&valid);
            if (!valid || (n ^ inv) != 65535u || n > total - out) PNG_BAD;
            for (uint32_t i=0u;i<n;i++) raw[out++] = (uint8_t)png_bits(src,len,&bit,8u,&valid);
            if (!valid) PNG_BAD;
            continue;
        }
        for (uint32_t i=0u;i<288u;i++) ll[i]=0;
        for (uint32_t i=0u;i<32u;i++) dd[i]=0;
        if (btype == 1u) {
            for (uint32_t i=0u;i<144u;i++) ll[i]=8;
            for (uint32_t i=144u;i<256u;i++) ll[i]=9;
            for (uint32_t i=256u;i<280u;i++) ll[i]=7;
            for (uint32_t i=280u;i<288u;i++) ll[i]=8;
            for (uint32_t i=0u;i<32u;i++) dd[i]=5;
        } else {
            uint32_t nl = png_bits(src,len,&bit,5u,&valid)+257u;
            uint32_t nd = png_bits(src,len,&bit,5u,&valid)+1u;
            uint32_t nc = png_bits(src,len,&bit,4u,&valid)+4u;
            if (!valid || nl>286u || nd>32u || nc>19u) PNG_BAD;
            for (uint32_t i=0u;i<19u;i++) cl[i]=0;
            for (uint32_t i=0u;i<nc;i++) cl[order[i]] = (uint8_t)png_bits(src,len,&bit,3u,&valid);
            if (!valid || !png_table(cl,19u,ccnt,cfirst,cstart,csym)) PNG_BAD;
            uint32_t idx=0u, end=nl+nd, previous=0u;
            while (idx<end && valid) {
                uint32_t s=png_symbol(src,len,&bit,&valid,ccnt,cfirst,cstart,csym);
                uint32_t copies=1u,v=s;
                if (s==16u) { if (!idx) PNG_BAD; copies=png_bits(src,len,&bit,2u,&valid)+3u;v=previous; }
                else if (s==17u) {copies=png_bits(src,len,&bit,3u,&valid)+3u;v=0u;}
                else if (s==18u) {copies=png_bits(src,len,&bit,7u,&valid)+11u;v=0u;}
                else if (s>18u) PNG_BAD;
                if (!valid || copies>end-idx) PNG_BAD;
                for (uint32_t j=0u;j<copies;j++,idx++) {
                    if (idx<nl) ll[idx]=(uint8_t)v; else dd[idx-nl]=(uint8_t)v;
                }
                previous=v;
            }
            if (!valid) PNG_BAD;
        }
        if (!ll[256] || !png_table(ll,288u,lcnt,lfirst,lstart,lsym)) PNG_BAD;
        // A distance table with no codes is legal only when no matches occur.
        uint32_t distances=png_table(dd,32u,dcnt,dfirst,dstart,dsym);
        while (valid) {
            uint32_t s=png_symbol(src,len,&bit,&valid,lcnt,lfirst,lstart,lsym);
            if (!valid) PNG_BAD;
            if (s<256u) {
                if (out>=total) PNG_BAD;
                raw[out++]=(uint8_t)s;
            } else if (s==256u) break;
            else {
                if (s>285u || !distances) PNG_BAD;
                uint32_t length=lenbase[s-257u]+png_bits(src,len,&bit,lenextra[s-257u],&valid);
                uint32_t d=png_symbol(src,len,&bit,&valid,dcnt,dfirst,dstart,dsym);
                if (!valid || d>=30u) PNG_BAD;
                uint32_t distance=distbase[d]+png_bits(src,len,&bit,distextra[d],&valid);
                if (!valid || distance>out || distance>32768u || length>total-out) PNG_BAD;
                for (uint32_t i=0u;i<length;i++) {raw[out]=raw[out-distance];out++;}
            }
        }
    }
    if (!valid || out!=total || (bit+7u)/8u+4u>len) PNG_BAD;
    // Adler-32 is checked on the GPU before exposing any decoded result.
    uint32_t a=1u,b=0u;
    for (uint32_t i=0u;i<total;i++) {
        a=(a+raw[i])%65521u; b=(b+a)%65521u;
    }
    uint32_t z=(bit+7u)/8u;
    uint32_t expected=((uint32_t)src[z]<<24u)|((uint32_t)src[z+1u]<<16u)|((uint32_t)src[z+2u]<<8u)|src[z+3u];
    if (expected!=((b<<16u)|a)) PNG_BAD;

    // PNG filters use reconstructed values on both the same and prior rows.
    for (uint32_t y=0u;y<height;y++) {
        uint32_t pos=y*(row+1u), filter=raw[pos];
        if (filter>4u) PNG_BAD;
        for (uint32_t x=0u;x<row;x++) {
            uint32_t a0=x>=channels?raw[pos+1u+x-channels]:0u;
            uint32_t b0=y?raw[pos-row+x]:0u;
            uint32_t c0=y&&x>=channels?raw[pos-row+x-channels]:0u;
            uint32_t predictor=filter==1u?a0:(filter==2u?b0:(filter==3u?(a0+b0)/2u:(filter==4u?png_paeth(a0,b0,c0):0u)));
            raw[pos+1u+x]=(uint8_t)((raw[pos+1u+x]+predictor)&255u);
        }
    }
    for (uint32_t y=0u;y<height;y++) for (uint32_t x=0u;x<width;x++) {
        uint32_t i=y*width+x, pos=y*(row+1u)+1u+x*channels;
        uint32_t r,g,b,alpha;
        if (channels==1u || channels==2u) {
            r=g=b=raw[pos];alpha=channels==2u?raw[pos+1u]:255u;
        } else {r=raw[pos];g=raw[pos+1u];b=raw[pos+2u];alpha=channels==4u?raw[pos+3u]:255u;}
        pixels[i]=(r<<24u)|(g<<16u)|(b<<8u)|alpha;
    }
}
