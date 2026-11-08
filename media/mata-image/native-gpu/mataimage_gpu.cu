// Native whole-image GPU decoder bridge. Build with nvcc (CUDA) or hipcc (HIP).
// The exported C ABI matches mata-image/src/gpu_full.rs and deliberately returns
// a failure code rather than allowing an incomplete device output to escape.
#include <cstdint>
#include <cstddef>
#ifdef MATA_HIP
#include <hip/hip_runtime.h>
#define GPU_ERR hipError_t
#define GPU_SUCCESS hipSuccess
#define GPU_MALLOC hipMalloc
#define GPU_FREE hipFree
#define GPU_COPY hipMemcpy
#define GPU_H2D hipMemcpyHostToDevice
#define GPU_D2H hipMemcpyDeviceToHost
#define GPU_SYNC hipDeviceSynchronize
#define GPU_LAST hipGetLastError
#define EXPORT_NAME mataimage_hip_decode
#else
#include <cuda_runtime.h>
#define GPU_ERR cudaError_t
#define GPU_SUCCESS cudaSuccess
#define GPU_MALLOC cudaMalloc
#define GPU_FREE cudaFree
#define GPU_COPY cudaMemcpy
#define GPU_H2D cudaMemcpyHostToDevice
#define GPU_D2H cudaMemcpyDeviceToHost
#define GPU_SYNC cudaDeviceSynchronize
#define GPU_LAST cudaGetLastError
#define EXPORT_NAME mataimage_cuda_decode
#endif

static __device__ __forceinline__ uint32_t pack(uint32_t r,uint32_t g,uint32_t b,uint32_t a){
  return (r<<24)|(g<<16)|(b<<8)|a;
}
static __device__ __forceinline__ uint32_t target(uint32_t i,uint32_t w,uint32_t h,uint32_t mode){
  uint32_t y=i/w,x=i%w;
  if((mode&0x200u)==0)y=h-1u-y;
  if(mode&0x400u)x=w-1u-x;
  return y*w+x;
}
static __device__ __forceinline__ uint32_t rgb(const uint8_t* src,uint32_t p,uint32_t bpp){
  return pack(src[p+2],src[p+1],src[p],bpp==4?src[p+3]:255);
}
__global__ void full_wbmp(const uint8_t* src,uint32_t len,uint32_t* out,
    uint32_t w,uint32_t h,uint32_t off,uint32_t mode,uint32_t* err){
  uint32_t i=blockIdx.x*blockDim.x+threadIdx.x;
  if(i>=w*h)return;
  uint32_t x=i%w,y=i/w;
  uint64_t p=uint64_t(off)+uint64_t(y)*mode+x/8u;
  if(p>=len){atomicOr(err,1u);return;}
  out[i]=(src[p]&(0x80u>>(x&7u)))?0x000000ffu:0xffffffffu;
}
__global__ void full_tga(const uint8_t* src,uint32_t len,uint32_t* out,
    uint32_t w,uint32_t h,uint32_t off,uint32_t mode,uint32_t* err){
  uint32_t i=blockIdx.x*blockDim.x+threadIdx.x;
  uint32_t n=w*h,bpp=mode&255u;
  if((mode&0x100u)==0u){
    if(i>=n)return;
    uint64_t p=uint64_t(off)+uint64_t(i)*bpp;
    if(p+bpp>len){atomicOr(err,1u);return;}
    out[target(i,w,h,mode)]=rgb(src,uint32_t(p),bpp);return;
  }
  if(i!=0u)return;
  uint32_t pos=off,dst=0u;
  while(dst<n){
    if(pos>=len){*err=1u;return;}
    uint32_t tag=src[pos++],run=(tag&127u)+1u;
    if(run>n-dst){*err=1u;return;}
    if(tag&128u){
      if(uint64_t(pos)+bpp>len){*err=1u;return;}
      uint32_t px=rgb(src,pos,bpp);pos+=bpp;
      for(uint32_t j=0;j<run;j++)out[target(dst++,w,h,mode)]=px;
    }else{
      if(uint64_t(pos)+uint64_t(run)*bpp>len){*err=1u;return;}
      for(uint32_t j=0;j<run;j++){out[target(dst++,w,h,mode)]=rgb(src,pos,bpp);pos+=bpp;}
    }
  }
}
__global__ void full_bmp(const uint8_t* src,uint32_t len,uint32_t* out,
    uint32_t w,uint32_t h,uint32_t off,uint32_t mode,uint32_t* err){
  uint32_t i=blockIdx.x*blockDim.x+threadIdx.x;
  if(i>=w*h)return;
  uint32_t x=i%w,y=i/w,bpp=mode&255u;
  uint32_t source_y=(mode&0x200u)?y:h-1u-y;
  uint64_t stride=(uint64_t(w)*bpp+3u)&~uint64_t(3u);
  uint64_t p=uint64_t(off)+uint64_t(source_y)*stride+uint64_t(x)*bpp;
  if(p+bpp>len){atomicOr(err,1u);return;}
  out[i]=pack(src[p+2],src[p+1],src[p],255u);
}
__global__ void full_qoi(const uint8_t* src,uint32_t len,uint32_t* out,
    uint32_t w,uint32_t h,uint32_t off,uint32_t mode,uint32_t* err){
  if(blockIdx.x*blockDim.x+threadIdx.x!=0u)return;
  uint32_t r=0,g=0,b=0,a=255,indices[64];
  for(uint32_t j=0;j<64;j++)indices[j]=0;
  uint32_t n=w*h,dst=0,pos=off;
  while(dst<n){
    if(pos>=len){*err=1;return;}
    uint32_t op=src[pos++];
    if(op==254u){
      if(uint64_t(pos)+3u>len){*err=1;return;}
      r=src[pos++];g=src[pos++];b=src[pos++];
    }else if(op==255u){
      if(uint64_t(pos)+4u>len){*err=1;return;}
      r=src[pos++];g=src[pos++];b=src[pos++];a=src[pos++];
    }else if((op&192u)==0){
      uint32_t px=indices[op&63u];r=px>>24;g=(px>>16)&255u;b=(px>>8)&255u;a=px&255u;
    }else if((op&192u)==64u){
      r=(r+((op>>4)&3u)-2u)&255u;
      g=(g+((op>>2)&3u)-2u)&255u;
      b=(b+(op&3u)-2u)&255u;
    }else if((op&192u)==128u){
      if(pos>=len){*err=1;return;}
      uint32_t second=src[pos++],dg=(op&63u)-32u;
      r=(r+dg+(second>>4)-8u)&255u;
      g=(g+dg)&255u;
      b=(b+dg+(second&15u)-8u)&255u;
    }else{
      uint32_t run=(op&63u)+1u;
      if(run>n-dst){*err=1;return;}
      uint32_t px=pack(r,g,b,a);
      for(uint32_t j=0;j<run;j++)out[dst++]=px;
      indices[(r*3u+g*5u+b*7u+a*11u)&63u]=px;
      continue;
    }
    uint32_t px=pack(r,g,b,a);
    indices[(r*3u+g*5u+b*7u+a*11u)&63u]=px;
    out[dst++]=px;
  }
  if(uint64_t(pos)+8u>len){*err=1;return;}
  for(uint32_t j=0;j<7u;j++)if(src[pos+j]!=0){*err=1;return;}
  if(src[pos+7]!=1){*err=1;return;}
}

#include "gpu_png_full_cuda.inl"
#include "gpu_webp_full_cuda.inl"

#ifdef _WIN32
#define API_EXPORT extern "C" __declspec(dllexport)
#else
#define API_EXPORT extern "C" __attribute__((visibility("default")))
#endif
API_EXPORT int EXPORT_NAME(uint32_t format,const uint8_t* source,size_t length,
        uint32_t width,uint32_t height,uint32_t offset,uint32_t mode,uint32_t* output){
  if(!source||!output||width==0||height==0||length==0||length>UINT32_MAX)return 1;
  uint64_t count=uint64_t(width)*height;
  if(count==0||count>UINT32_MAX||count>SIZE_MAX/sizeof(uint32_t))return 1;
  if(format<1||format>4)return 1;
  uint8_t* in=nullptr;uint32_t* out=nullptr;uint32_t* status=nullptr;
  int result=1;uint32_t host_status=0;
  do{
    if(GPU_MALLOC(reinterpret_cast<void**>(&in),length)!=GPU_SUCCESS)break;
    if(GPU_MALLOC(reinterpret_cast<void**>(&out),size_t(count)*4)!=GPU_SUCCESS)break;
    if(GPU_MALLOC(reinterpret_cast<void**>(&status),4)!=GPU_SUCCESS)break;
    if(GPU_COPY(in,source,length,GPU_H2D)!=GPU_SUCCESS)break;
    if(GPU_COPY(status,&host_status,4,GPU_H2D)!=GPU_SUCCESS)break;
    uint32_t threads=256u;
    uint32_t blocks=uint32_t((count+threads-1)/threads);
    if(format==1u||(format==2u&&(mode&0x100u)))blocks=1;
    if(format==1u){full_qoi<<<1,1>>>(in,uint32_t(length),out,width,height,offset,mode,status);}
    else if(format==2u){full_tga<<<blocks,threads>>>(in,uint32_t(length),out,width,height,offset,mode,status);}
    else if(format==3u){full_wbmp<<<blocks,threads>>>(in,uint32_t(length),out,width,height,offset,mode,status);}
    else {full_bmp<<<blocks,threads>>>(in,uint32_t(length),out,width,height,offset,mode,status);}
    if(GPU_LAST()!=GPU_SUCCESS||GPU_SYNC()!=GPU_SUCCESS)break;
    if(GPU_COPY(&host_status,status,4,GPU_D2H)!=GPU_SUCCESS||host_status)break;
    if(GPU_COPY(output,out,size_t(count)*4,GPU_D2H)!=GPU_SUCCESS)break;
    result=0;
  }while(false);
  if(status)GPU_FREE(status);
  if(out)GPU_FREE(out);
  if(in)GPU_FREE(in);
  return result;
}

// Entire PNG DEFLATE/filter/8-bit pixel decode on the selected native device.
#ifdef MATA_HIP
#define PNG_EXPORT_NAME mataimage_hip_png_decode
#else
#define PNG_EXPORT_NAME mataimage_cuda_png_decode
#endif
API_EXPORT int PNG_EXPORT_NAME(const uint8_t* zlib,size_t length,
        uint32_t width,uint32_t height,uint32_t channels,uint32_t* output) {
  if(!zlib||!output||width==0||height==0||channels<1||channels>4||
     length<6||length>UINT32_MAX)return 1;
  uint64_t pixels=uint64_t(width)*height;
  uint64_t stride=uint64_t(width)*channels+1;
  uint64_t raw_len=stride*height;
  if(pixels>UINT32_MAX||pixels>SIZE_MAX/4||raw_len>UINT32_MAX||raw_len==0)return 1;
  uint8_t* input=nullptr; uint8_t* scratch=nullptr;
  uint32_t* result_buffer=nullptr; uint32_t* error_buffer=nullptr;
  uint32_t error=0; int result=1;
  do {
    if(GPU_MALLOC(reinterpret_cast<void**>(&input),length)!=GPU_SUCCESS)break;
    if(GPU_MALLOC(reinterpret_cast<void**>(&scratch),size_t(raw_len))!=GPU_SUCCESS)break;
    if(GPU_MALLOC(reinterpret_cast<void**>(&result_buffer),size_t(pixels)*4)!=GPU_SUCCESS)break;
    if(GPU_MALLOC(reinterpret_cast<void**>(&error_buffer),4)!=GPU_SUCCESS)break;
    if(GPU_COPY(input,zlib,length,GPU_H2D)!=GPU_SUCCESS)break;
    if(GPU_COPY(error_buffer,&error,4,GPU_H2D)!=GPU_SUCCESS)break;
    mata_png_full<<<1,1>>>(input,uint32_t(length),result_buffer,scratch,width,height,channels,error_buffer);
    if(GPU_LAST()!=GPU_SUCCESS||GPU_SYNC()!=GPU_SUCCESS)break;
    if(GPU_COPY(&error,error_buffer,4,GPU_D2H)!=GPU_SUCCESS||error!=0)break;
    if(GPU_COPY(output,result_buffer,size_t(pixels)*4,GPU_D2H)!=GPU_SUCCESS)break;
    result=0;
  } while(false);
  if(error_buffer)GPU_FREE(error_buffer);
  if(result_buffer)GPU_FREE(result_buffer);
  if(scratch)GPU_FREE(scratch);
  if(input)GPU_FREE(input);
  return result;
}

#ifdef MATA_HIP
#define WEBP_EXPORT_NAME mataimage_hip_webp_decode
#else
#define WEBP_EXPORT_NAME mataimage_cuda_webp_decode
#endif
API_EXPORT int WEBP_EXPORT_NAME(const uint8_t* vp8l,size_t length,
        uint32_t width,uint32_t height,uint32_t* output) {
  if(!vp8l||!output||!width||!height||length<6||length>UINT32_MAX)return 1;
  uint64_t pixels=uint64_t(width)*height;
  if(!pixels||pixels>UINT32_MAX||pixels>SIZE_MAX/4)return 1;
  uint8_t* input=nullptr;uint32_t* device_output=nullptr;uint32_t* error_buffer=nullptr;
  int result=1;uint32_t error=0;
  do {
    if(GPU_MALLOC(reinterpret_cast<void**>(&input),length)!=GPU_SUCCESS)break;
    if(GPU_MALLOC(reinterpret_cast<void**>(&device_output),size_t(pixels)*4)!=GPU_SUCCESS)break;
    if(GPU_MALLOC(reinterpret_cast<void**>(&error_buffer),4)!=GPU_SUCCESS)break;
    if(GPU_COPY(input,vp8l,length,GPU_H2D)!=GPU_SUCCESS)break;
    if(GPU_COPY(error_buffer,&error,4,GPU_H2D)!=GPU_SUCCESS)break;
    mata_webp_lossless_full<<<1,1>>>(input,uint32_t(length),device_output,width,height,error_buffer);
    if(GPU_LAST()!=GPU_SUCCESS||GPU_SYNC()!=GPU_SUCCESS)break;
    if(GPU_COPY(&error,error_buffer,4,GPU_D2H)!=GPU_SUCCESS||error)break;
    if(GPU_COPY(output,device_output,size_t(pixels)*4,GPU_D2H)!=GPU_SUCCESS)break;
    result=0;
  } while(false);
  if(error_buffer)GPU_FREE(error_buffer);
  if(device_output)GPU_FREE(device_output);
  if(input)GPU_FREE(input);
  return result;
}
