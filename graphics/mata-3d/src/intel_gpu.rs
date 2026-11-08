//! Multi-vendor Direct3D 11 triangle-rasterization and presentation backend, using Win32/DXGI/
//! Direct3D 11 system libraries (no additional Cargo dependencies).
//!
//! Uses the GPU for native-resolution triangle rasterization and depth tests.
//! CPU code retains scene graph traversal, vertex transformation and lighting.
//! The compositor can still present CPU-rendered frames on initialization failures.
//!
//! Full 3D triangle rasterization is performed at native client resolution;
//! the HUD is composed separately in the final GPU pass. No CPU scene upscale.
//!
//! Errors never panic: main.rs switches back to MataWin software presentation.

use mata_win::Framebuffer;
use crate::{GpuFrame,GpuVertex,GpuSphereInstance,GpuBondInstance,shader_dispatch::D3d11Shaders};
use crate::gpu_backend::{GpuPreference,AdapterInfo,ranked_adapters};
use std::{ffi::{c_char,c_void},ptr};

type Raw = *mut c_void;
type Hr = i32;
const DXGI_FORMAT_BGRA8:u32=87;
const D3D11_SDK_VERSION:u32=7;
const D3D11_BIND_SHADER_RESOURCE:u32=8;
const D3D11_BIND_RENDER_TARGET:u32=32;
const D3D11_BIND_DEPTH_STENCIL:u32=64;
const D3D11_BIND_VERTEX_BUFFER:u32=1;
const DXGI_FORMAT_D32_FLOAT:u32=40;
const D3D11_CPU_ACCESS_WRITE:u32=0x10000;
const D3D11_USAGE_DYNAMIC:u32=2;
const D3D11_MAP_WRITE_DISCARD:u32=4;
const D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST:u32=4;
const DXGI_ADAPTER_FLAG_SOFTWARE:u32=2;

#[repr(C)]#[derive(Clone,Copy)]struct Guid{a:u32,b:u16,c:u16,d:[u8;8]}
const IID_DXGI_FACTORY1:Guid=Guid{a:0x770aae78,b:0xf26f,c:0x4dba,d:[0xa8,0x29,0x25,0x3c,0x83,0xd1,0xb3,0x87]};
const IID_D3D11_TEXTURE2D:Guid=Guid{a:0x6f15aaf2,b:0xd208,c:0x4e89,d:[0x9a,0xb4,0x48,0x95,0x35,0xd3,0x4f,0x9c]};
#[repr(C)]#[derive(Default,Clone,Copy)]struct Luid{low:u32,high:i32}
#[repr(C)]struct AdapterDesc{
    name:[u16;128],vendor:u32,device:u32,subsystem:u32,revision:u32,
    dedicated_video:usize,dedicated_system:usize,shared_system:usize,luid:Luid,flags:u32,
}
impl Default for AdapterDesc{fn default()->Self{unsafe{std::mem::zeroed()}}}
#[repr(C)]#[derive(Default,Clone,Copy)]struct Rational{numerator:u32,denominator:u32}
#[repr(C)]#[derive(Default,Clone,Copy)]struct ModeDesc{
    width:u32,height:u32,refresh:Rational,format:u32,scanline_order:u32,scaling:u32,
}
#[repr(C)]#[derive(Default,Clone,Copy)]struct SampleDesc{count:u32,quality:u32}
#[repr(C)]struct SwapChainDesc{
    buffer:ModeDesc,sample:SampleDesc,usage:u32,count:u32,hwnd:Raw,
    windowed:i32,effect:u32,flags:u32,
}
#[repr(C)]struct TextureDesc{
    width:u32,height:u32,mip_levels:u32,array_size:u32,format:u32,sample:SampleDesc,
    usage:u32,bind:u32,cpu_access:u32,misc:u32,
}
#[repr(C)]#[derive(Default)]struct Mapped{data:Raw,row_pitch:u32,depth_pitch:u32}
#[repr(C)]struct SamplerDesc{
    filter:u32,address_u:u32,address_v:u32,address_w:u32,mip_bias:f32,
    max_anisotropy:u32,comparison:u32,border:[f32;4],min_lod:f32,max_lod:f32,
}
#[repr(C)]struct Viewport{x:f32,y:f32,width:f32,height:f32,min_depth:f32,max_depth:f32}

#[link(name="user32")]
unsafe extern "system"{
    fn FindWindowW(class:*const u16,name:*const u16)->Raw;
    fn GetWindowThreadProcessId(hwnd:Raw,pid:*mut u32)->u32;
    fn IsWindow(hwnd:Raw)->i32;
}
#[link(name="kernel32")]
unsafe extern "system"{
    fn GetCurrentProcessId()->u32;
    fn LoadLibraryW(filename:*const u16)->Raw;
    fn FreeLibrary(module:Raw)->i32;
    fn GetProcAddress(module:Raw,name:*const c_char)->Raw;
}
#[link(name="dxgi")]
unsafe extern "system"{fn CreateDXGIFactory1(riid:*const Guid,out:*mut Raw)->Hr;}
#[link(name="d3d11")]
unsafe extern "system"{
    fn D3D11CreateDeviceAndSwapChain(
        adapter:Raw,driver:u32,software:Raw,flags:u32,feature_levels:*const u32,
        levels:u32,sdk:u32,description:*const SwapChainDesc,swap:*mut Raw,
        device:*mut Raw,feature_out:*mut u32,context:*mut Raw,
    )->Hr;
}
// The shader compiler is loaded dynamically, not imported at process start:
// missing `d3dcompiler_47.dll` triggers the normal MataWin fallback instead
// of preventing the entire game executable from loading.
struct CompilerDll(Raw);
impl CompilerDll{
    fn load()->Result<Self,String>{
        let name:Vec<u16>="d3dcompiler_47.dll".encode_utf16().chain(Some(0)).collect();
        let lib=unsafe{LoadLibraryW(name.as_ptr())};
        if lib.is_null(){Err("d3dcompiler_47.dll is unavailable".into())}
        else{Ok(Self(lib))}
    }
    fn compile(&self,source:&str,entry:&[u8],target:&[u8],code:*mut Raw,errors:*mut Raw)->Result<Hr,String>{
        let ptr=unsafe{GetProcAddress(self.0,b"D3DCompile\0".as_ptr().cast())};
        if ptr.is_null(){return Err("D3DCompile export is unavailable".into())}
        let f:unsafe extern "system" fn(*const c_void,usize,*const c_char,Raw,Raw,
            *const c_char,*const c_char,u32,u32,*mut Raw,*mut Raw)->Hr=
            unsafe{std::mem::transmute(ptr)};
        Ok(unsafe{f(source.as_ptr().cast(),source.len(),ptr::null(),
            ptr::null_mut(),ptr::null_mut(),entry.as_ptr().cast(),target.as_ptr().cast(),
            0,0,code,errors)})
    }
}
impl Drop for CompilerDll{fn drop(&mut self){unsafe{FreeLibrary(self.0)};}}

// COM slots are the published Microsoft SDK vtable indices, not the address
// of exported functions. All native COM pointers below are owned by `Com`.
#[derive(Debug)]struct Com(Raw);
impl Com{
    fn from_raw(value:Raw,label:&str)->Result<Self,String>{
        if value.is_null(){Err(format!("{label}: null interface"))}else{Ok(Self(value))}
    }
    fn get(&self)->Raw{self.0}
    unsafe fn method<T:Copy>(&self,index:usize)->T{
        // SAFETY: caller uses the exact COM interface slot/signature.
        unsafe{
            let table=*(self.0 as *const *const *const c_void);
            std::mem::transmute_copy::<*const c_void,T>(&*table.add(index))
        }
    }
}
impl Drop for Com{
    fn drop(&mut self){
        if !self.0.is_null(){unsafe{
            let release:unsafe extern "system" fn(Raw)->u32=self.method(2);
            release(self.0);
        }}
    }
}
fn hr_ok(code:Hr,label:&str)->Result<(),String>{
    if code>=0{Ok(())}else{Err(format!("{label}: HRESULT 0x{:08X}",code as u32))}
}

#[repr(C)]struct BufferDesc{byte_width:u32,usage:u32,bind:u32,cpu_access:u32,misc:u32,stride:u32}
#[repr(C)]struct InputDesc{semantic:*const c_char,index:u32,format:u32,slot:u32,offset:u32,class:u32,step:u32}
#[repr(C)]struct StencilFace{fail:u32,depth_fail:u32,pass:u32,comparison:u32}
#[repr(C)]struct DepthStateDesc{enable:i32,write:u32,compare:u32,stencil:i32,read_mask:u8,write_mask:u8,
    front:StencilFace,back:StencilFace}
#[repr(C)]#[derive(Clone,Copy)]struct ScissorRect{left:i32,top:i32,right:i32,bottom:i32}
struct GpuSceneTarget{texture:Com,view:Com,rtv:Com,size:(u32,u32)}
struct GpuDepth{_texture:Com,view:Com,size:(u32,u32)}

fn shader(compiler:&CompilerDll,source:&str,entry:&[u8],target:&[u8],pass:&str)->Result<Com,String>{
    let(mut bytes,mut error)=(ptr::null_mut(),ptr::null_mut());
    let code=compiler.compile(source,entry,target,&mut bytes,&mut error)?;
    let message=if !error.is_null(){
        let blob=Com(error);
        let data:unsafe extern "system" fn(Raw)->*const u8=unsafe{blob.method(3)};
        let len:unsafe extern "system" fn(Raw)->usize=unsafe{blob.method(4)};
        let src=unsafe{std::slice::from_raw_parts(data(blob.get()),len(blob.get()))};
        String::from_utf8_lossy(src).to_string()
    }else{String::new()};
    if code<0{
        if !bytes.is_null(){drop(Com(bytes));}
        // Name the failing pass and show exact HLSL line numbers. Diagnostics
        // from D3DCompile otherwise reference an unnamed Shader@ address.
        let numbered = source.lines().enumerate()
            .map(|(index,line)| format!("{:3} | {}",index+1,line))
            .collect::<Vec<_>>().join("\n");
        return Err(format!("D3D shader compile ({pass}) 0x{:08X}: {}\n{}",code as u32,message,numbered));
    }
    Com::from_raw(bytes,"shader code")
}
fn blob_bytes(blob:&Com)->(*const c_void,usize){
    unsafe{
        let ptr_fn:unsafe extern "system" fn(Raw)->*const c_void=blob.method(3);
        let len_fn:unsafe extern "system" fn(Raw)->usize=blob.method(4);
        (ptr_fn(blob.get()),len_fn(blob.get()))
    }
}

struct Texture{resource:Com,view:Com,size:(u32,u32)}

pub struct IntelGpu{
    hwnd:Raw,
    // Drop order is explicit in the field order: RTV, GPU resources, device,
    // adapter, factory. Swap chain must not outlive its device.
    target:Option<Com>,
    scene:Option<Texture>,hud:Option<Texture>,
    vertex:Com,pixel:Com,pixel_sky:Com,sampler:Com,raster:Com,raster_scissor:Com,
    geo_target:Option<GpuSceneTarget>,geo_depth:Option<GpuDepth>,
    geo_buffer:Option<Com>,geo_capacity:usize,
    // A captured scene is immutable until its generation changes. Keep the
    // D3D11 buffer for UI-only repaints instead of re-uploading megabytes.
    geo_last_upload:Option<(u64,usize)>,
    geo_vertex:Com,geo_pixel:Com,geo_layout:Com,geo_depth_state:Com,
    sphere_vs:Com,sphere_ps:Com,sphere_layout:Com,
    sphere_mesh:Option<Com>,sphere_mesh_count:u32,
    sphere_instances:Option<Com>,sphere_capacity:usize,
    sphere_last_upload:Option<(u64,usize)>,
    bond_vs:Com,bond_ps:Com,bond_layout:Com,
    bond_mesh:Option<Com>,bond_mesh_count:u32,
    bond_instances:Option<Com>,bond_capacity:usize,
    bond_last_upload:Option<(u64,usize)>,
    context:Com,swap:Com,device:Com,adapter:Com,factory:Com,
    size:(u32,u32),
    pub adapter_name:String,
    /// Vendor selected by the successful D3D11 device probe.
    pub vendor_id:u32,
    /// Actual DXGI adapter index that successfully initialized.
    pub adapter_index:u32,
    /// Dedicated video-memory capacity reported by DXGI.
    pub dedicated_video_memory:u64,
    present_interval:u32,
}
/// Backend-neutral name: the same Direct3D 11 renderer supports Intel, AMD,
/// NVIDIA and other hardware DXGI adapters.
pub type D3d11Gpu = IntelGpu;
impl IntelGpu{
    /// Legacy title lookup. Prefer from_hwnd(window.handle(), ...), which uses
    /// the native window handle exposed by Molecule Studio's MataWin.
    pub fn new(window_title:&str,width:u32,height:u32)->Result<Self,String>{
        let program=D3d11Shaders::new()?;
        Self::new_with_shaders(window_title,width,height,&program)
    }
    /// Initialize the legacy Intel-only renderer from a title and HLSL source.
    pub fn new_with_shaders(window_title:&str,width:u32,height:u32,program:&D3d11Shaders)->Result<Self,String>{
        if width==0||height==0{return Err("window has zero size".into())}
        let title:Vec<u16>=window_title.encode_utf16().chain(Some(0)).collect();
        let hwnd=unsafe{FindWindowW(ptr::null(),title.as_ptr())};
        if hwnd.is_null(){return Err("MataWin HWND not found by exact title".into())}
        let mut pid=0;
        unsafe{GetWindowThreadProcessId(hwnd,&mut pid)};
        if pid!=unsafe{GetCurrentProcessId()}{return Err("MataWin HWND belongs to another process".into())}
        let mut f=ptr::null_mut();
        hr_ok(unsafe{CreateDXGIFactory1(&IID_DXGI_FACTORY1,&mut f)},"DXGI factory")?;
        let factory=Com::from_raw(f,"DXGI factory")?;
        Self::try_candidates(hwnd,width,height,factory,program,GpuPreference::Intel)
    }
    /// Legacy Intel-only constructor. Prefer `from_hwnd_auto` or
    /// `from_hwnd_with_preference` for automatic multi-vendor selection.
    pub fn from_hwnd(hwnd:Raw,width:u32,height:u32)->Result<Self,String>{
        let program=D3d11Shaders::new()?;
        Self::from_hwnd_with_shaders(hwnd,width,height,&program)
    }
    /// Legacy Intel-only device constructor, preserved for existing callers.
    pub fn from_hwnd_with_shaders(hwnd:Raw,width:u32,height:u32,program:&D3d11Shaders)->Result<Self,String>{
        Self::from_hwnd_with_preference(hwnd,width,height,program,GpuPreference::Intel)
    }
    /// Automatically select the best available hardware D3D11 adapter.
    /// Selects based on dedicated video memory, then tests device creation.
    pub fn from_hwnd_auto(hwnd:Raw,width:u32,height:u32)->Result<Self,String>{
        let shaders=D3d11Shaders::new()?;
        Self::from_hwnd_with_preference(hwnd,width,height,&shaders,GpuPreference::Auto)
    }
    /// Hardware-independent preference, resolved against actual Windows DXGI
    /// devices at runtime. A failed candidate is skipped, not fatal to search.
    pub fn from_hwnd_with_preference(hwnd:Raw,width:u32,height:u32,
        program:&D3d11Shaders,preference:GpuPreference)->Result<Self,String>{
        if preference==GpuPreference::Software{return Err("software backend explicitly requested".into());}
        if hwnd.is_null()||unsafe{IsWindow(hwnd)}==0{return Err("invalid game HWND".into());}
        if width==0||height==0{return Err("window has zero size".into());}
        let mut pid=0;unsafe{GetWindowThreadProcessId(hwnd,&mut pid)};
        if pid!=unsafe{GetCurrentProcessId()}{return Err("HWND is not owned by this process".into());}
        let mut raw=ptr::null_mut();
        hr_ok(unsafe{CreateDXGIFactory1(&IID_DXGI_FACTORY1,&mut raw)},"DXGI factory")?;
        Self::try_candidates(hwnd,width,height,Com::from_raw(raw,"DXGI factory")?,program,preference)
    }
    fn try_candidates(hwnd:Raw,width:u32,height:u32,factory:Com,
        program:&D3d11Shaders,preference:GpuPreference)->Result<Self,String>{
        let mut available=Vec::new();
        let enumeration:unsafe extern "system" fn(Raw,u32,*mut Raw)->Hr=unsafe{factory.method(12)};
        for index in 0..64u32 {
            let mut raw=ptr::null_mut();
            if unsafe{enumeration(factory.get(),index,&mut raw)}<0{break}
            let adapter=Com::from_raw(raw,"DXGI adapter")?;
            let mut desc=AdapterDesc::default();
            let description:unsafe extern "system" fn(Raw,*mut AdapterDesc)->Hr=unsafe{adapter.method(10)};
            if unsafe{description(adapter.get(),&mut desc)}<0{continue}
            let name=String::from_utf16_lossy(&desc.name[..desc.name.iter()
                .position(|&v|v==0).unwrap_or(desc.name.len())]);
            available.push(AdapterInfo{index,vendor_id:desc.vendor,
                dedicated_video_memory:desc.dedicated_video as u64,
                shared_system_memory:desc.shared_system as u64,
                software:desc.flags&DXGI_ADAPTER_FLAG_SOFTWARE!=0,name});
        }
        let candidates=ranked_adapters(available,preference);
        if candidates.is_empty(){return Err(format!("no usable hardware DXGI adapter for '{}'",preference.label()));}
        let mut failures=Vec::new();
        for candidate in candidates {
            // Re-enumerate by stable index. Ownership of the factory and the
            // adapter is transferred to the initialized renderer, not shared.
            let mut factory_ptr=ptr::null_mut();
            if let Err(e)=hr_ok(unsafe{CreateDXGIFactory1(&IID_DXGI_FACTORY1,&mut factory_ptr)},"DXGI factory"){
                failures.push(e);continue;
            }
            let selected_factory=Com::from_raw(factory_ptr,"DXGI factory")?;
            let enum_selected:unsafe extern "system" fn(Raw,u32,*mut Raw)->Hr=unsafe{selected_factory.method(12)};
            let mut adapter_ptr=ptr::null_mut();
            if let Err(e)=hr_ok(unsafe{enum_selected(selected_factory.get(),candidate.index,&mut adapter_ptr)},"DXGI adapter"){
                failures.push(format!("{}: {e}",candidate.name));continue;
            }
            let selected_adapter=Com::from_raw(adapter_ptr,"DXGI adapter")?;
            match Self::from_adapter(hwnd,width,height,selected_factory,selected_adapter,&candidate,program) {
                Ok(gpu)=>return Ok(gpu),
                Err(e)=>{
                    eprintln!("[Mata GPU] {} (DXGI {}) rejected: {e}",candidate.name,candidate.index);
                    failures.push(format!("{}: {e}",candidate.name));
                }
            }
        }
        Err(format!("all D3D11 hardware adapters failed: {}",failures.join("; ")))
    }
    fn from_adapter(hwnd:Raw,width:u32,height:u32,factory:Com,adapter:Com,info:&AdapterInfo,program:&D3d11Shaders)->Result<Self,String>{
        let swap_desc=SwapChainDesc{
            buffer:ModeDesc{width,height,format:DXGI_FORMAT_BGRA8,..Default::default()},
            sample:SampleDesc{count:1,quality:0},usage:0x20,count:2,hwnd,
            windowed:1,effect:0,flags:0,
        };
        let(mut swap,mut device,mut context)=(ptr::null_mut(),ptr::null_mut(),ptr::null_mut());
        let result=unsafe{D3D11CreateDeviceAndSwapChain(adapter.get(),0,ptr::null_mut(),
            0x20,ptr::null(),0,D3D11_SDK_VERSION,&swap_desc,
            &mut swap,&mut device,ptr::null_mut(),&mut context)};
        if result<0 {
            for p in [context,device,swap] {if !p.is_null(){drop(Com(p));}}
            return Err(format!("Direct3D 11 device: 0x{:08X}",result as u32));
        }
        let device=Com::from_raw(device,"D3D device")?;
        let context=Com::from_raw(context,"D3D immediate context")?;
        let swap=Com::from_raw(swap,"D3D swapchain")?;
        let compiler=CompilerDll::load()?;
        let vertex_blob=shader(&compiler,&program.composite,b"VS\0",b"vs_4_0\0","compositor vertex shader")?;
        let pixel_blob=shader(&compiler,&program.composite,b"PS\0",b"ps_4_0\0","compositor pixel shader")?;
        let sky_blob=shader(&compiler,&program.composite_sky,b"PS\0",b"ps_4_0\0","sky compositor pixel shader")?;
        let geometry_vs=shader(&compiler,&program.geometry,b"GVS\0",b"vs_4_0\0","geometry vertex shader")?;
        let geometry_ps=shader(&compiler,&program.geometry,b"GPS\0",b"ps_4_0\0","geometry pixel shader")?;
        let sphere_vs_blob=shader(&compiler,&program.sphere_instances,b"GSI\0",b"vs_4_0\0","instanced sphere vertex shader")?;
        let sphere_ps_blob=shader(&compiler,&program.sphere_instances,b"PSI\0",b"ps_4_0\0","instanced sphere pixel shader")?;
        let bond_vs_blob=shader(&compiler,&program.bond_instances,b"GBI\0",b"vs_4_0\0","instanced bond vertex shader")?;
        let bond_ps_blob=shader(&compiler,&program.bond_instances,b"PBI\0",b"ps_4_0\0","instanced bond pixel shader")?;
        let(mut vertex,mut pixel)= (ptr::null_mut(),ptr::null_mut());
        let create_vs:unsafe extern "system" fn(Raw,*const c_void,usize,Raw,*mut Raw)->Hr=unsafe{device.method(12)};
        let create_ps:unsafe extern "system" fn(Raw,*const c_void,usize,Raw,*mut Raw)->Hr=unsafe{device.method(15)};
        let (data,len)=blob_bytes(&vertex_blob);
        hr_ok(unsafe{create_vs(device.get(),data,len,ptr::null_mut(),&mut vertex)},"vertex shader")?;
        let (data,len)=blob_bytes(&pixel_blob);
        if let Err(e)=hr_ok(unsafe{create_ps(device.get(),data,len,ptr::null_mut(),&mut pixel)},"pixel shader"){
            drop(Com(vertex));return Err(e);
        }
        let (vertex,pixel)=(Com(vertex),Com(pixel));
        let mut sky_ps=ptr::null_mut();
        let (data,len)=blob_bytes(&sky_blob);
        hr_ok(unsafe{create_ps(device.get(),data,len,ptr::null_mut(),&mut sky_ps)},"sky composition shader")?;
        let pixel_sky=Com::from_raw(sky_ps,"sky composition pixel shader")?;
        let mut geo_vs_raw=ptr::null_mut();
        let mut geo_ps_raw=ptr::null_mut();
        let (data,len)=blob_bytes(&geometry_vs);
        hr_ok(unsafe{create_vs(device.get(),data,len,ptr::null_mut(),&mut geo_vs_raw)},"geometry vertex shader")?;
        let (data,len)=blob_bytes(&geometry_ps);
        hr_ok(unsafe{create_ps(device.get(),data,len,ptr::null_mut(),&mut geo_ps_raw)},"geometry pixel shader")?;
        let (data,len)=blob_bytes(&geometry_vs);
        let attributes=[
            InputDesc{semantic:b"POSITION\0".as_ptr().cast(),index:0,format:6,slot:0,offset:0,class:0,step:0},
            InputDesc{semantic:b"COLOR\0".as_ptr().cast(),index:0,format:28,slot:0,offset:12,class:0,step:0},
        ];
        let mut layout=ptr::null_mut();
        let make_layout:unsafe extern "system" fn(Raw,*const InputDesc,u32,*const c_void,usize,*mut Raw)->Hr=unsafe{device.method(11)};
        hr_ok(unsafe{make_layout(device.get(),attributes.as_ptr(),2,data,len,&mut layout)},"GPU vertex input layout")?;
        let mut sphere_vs_raw=ptr::null_mut();
        let mut sphere_ps_raw=ptr::null_mut();
        let (data,len)=blob_bytes(&sphere_vs_blob);
        hr_ok(unsafe{create_vs(device.get(),data,len,ptr::null_mut(),&mut sphere_vs_raw)},"instanced vertex shader")?;
        let (data,len)=blob_bytes(&sphere_ps_blob);
        hr_ok(unsafe{create_ps(device.get(),data,len,ptr::null_mut(),&mut sphere_ps_raw)},"instanced pixel shader")?;
        let (data,len)=blob_bytes(&sphere_vs_blob);
        let sphere_attributes=[
            InputDesc{semantic:b"POSITION\0".as_ptr().cast(),index:0,format:6,slot:0,offset:0,class:0,step:0},
            InputDesc{semantic:b"TEXCOORD\0".as_ptr().cast(),index:0,format:2,slot:1,offset:0,class:1,step:1},
            InputDesc{semantic:b"TEXCOORD\0".as_ptr().cast(),index:1,format:16,slot:1,offset:16,class:1,step:1},
            InputDesc{semantic:b"TEXCOORD\0".as_ptr().cast(),index:2,format:41,slot:1,offset:24,class:1,step:1},
            InputDesc{semantic:b"COLOR\0".as_ptr().cast(),index:0,format:28,slot:1,offset:28,class:1,step:1},
        ];
        let mut sphere_layout_raw=ptr::null_mut();
        hr_ok(unsafe{make_layout(device.get(),sphere_attributes.as_ptr(),5,data,len,&mut sphere_layout_raw)},"instanced input layout")?;
        let (sphere_vs,sphere_ps,sphere_layout)=(Com(sphere_vs_raw),Com(sphere_ps_raw),Com(sphere_layout_raw));
        let mut bond_vs_raw=ptr::null_mut();
        let mut bond_ps_raw=ptr::null_mut();
        let (data,len)=blob_bytes(&bond_vs_blob);
        hr_ok(unsafe{create_vs(device.get(),data,len,ptr::null_mut(),&mut bond_vs_raw)},"instanced bond vertex shader")?;
        let (data,len)=blob_bytes(&bond_ps_blob);
        hr_ok(unsafe{create_ps(device.get(),data,len,ptr::null_mut(),&mut bond_ps_raw)},"instanced bond pixel shader")?;
        let (data,len)=blob_bytes(&bond_vs_blob);
        let bond_attributes=[
            InputDesc{semantic:b"POSITION\0".as_ptr().cast(),index:0,format:6,slot:0,offset:0,class:0,step:0},
            InputDesc{semantic:b"TEXCOORD\0".as_ptr().cast(),index:0,format:2,slot:1,offset:0,class:1,step:1},
            InputDesc{semantic:b"TEXCOORD\0".as_ptr().cast(),index:1,format:2,slot:1,offset:16,class:1,step:1},
            InputDesc{semantic:b"TEXCOORD\0".as_ptr().cast(),index:2,format:2,slot:1,offset:32,class:1,step:1},
            InputDesc{semantic:b"COLOR\0".as_ptr().cast(),index:0,format:28,slot:1,offset:48,class:1,step:1},
        ];
        let mut bond_layout_raw=ptr::null_mut();
        hr_ok(unsafe{make_layout(device.get(),bond_attributes.as_ptr(),5,data,len,&mut bond_layout_raw)},"instanced bond input layout")?;
        let (bond_vs,bond_ps,bond_layout)=(Com(bond_vs_raw),Com(bond_ps_raw),Com(bond_layout_raw));
        let depth_desc=DepthStateDesc{enable:1,write:1,compare:5,stencil:0,read_mask:0xff,write_mask:0xff,
            front:StencilFace{fail:1,depth_fail:1,pass:1,comparison:8},
            back:StencilFace{fail:1,depth_fail:1,pass:1,comparison:8}};
        let mut ds=ptr::null_mut();
        let make_ds:unsafe extern "system" fn(Raw,*const DepthStateDesc,*mut Raw)->Hr=unsafe{device.method(21)};
        hr_ok(unsafe{make_ds(device.get(),&depth_desc,&mut ds)},"reverse-depth state")?;
        let (geo_vertex,geo_pixel,geo_layout,geo_depth_state)=(Com(geo_vs_raw),Com(geo_ps_raw),Com(layout),Com(ds));
        let sd=SamplerDesc{filter:0,address_u:3,address_v:3,address_w:3,
            mip_bias:0.0,max_anisotropy:1,comparison:8,border:[0.0;4],min_lod:0.0,max_lod:f32::MAX};
        let mut sampler=ptr::null_mut();
        let create_sampler:unsafe extern "system" fn(Raw,*const SamplerDesc,*mut Raw)->Hr=unsafe{device.method(23)};
        hr_ok(unsafe{create_sampler(device.get(),&sd,&mut sampler)},"point sampler")?;
        let sampler=Com::from_raw(sampler,"GPU sampler")?;
        #[repr(C)]struct RasterDesc{fill:u32,cull:u32,front:i32,bias:i32,bias_clamp:f32,slope:f32,
            depth_clip:i32,scissor:i32,multisample:i32,antialiased_line:i32}
        let rs=RasterDesc{fill:3,cull:1,front:0,bias:0,bias_clamp:0.0,slope:0.0,
            depth_clip:1,scissor:0,multisample:0,antialiased_line:0};
        let mut raster=ptr::null_mut();
        let create_rs:unsafe extern "system" fn(Raw,*const RasterDesc,*mut Raw)->Hr=unsafe{device.method(22)};
        hr_ok(unsafe{create_rs(device.get(),&rs,&mut raster)},"rasterizer state")?;
        let raster=Com::from_raw(raster,"GPU rasterizer")?;
        let clipped=RasterDesc{scissor:1,..rs};
        let mut clipped_state=ptr::null_mut();
        hr_ok(unsafe{create_rs(device.get(),&clipped,&mut clipped_state)},"scissor rasterizer")?;
        let raster_scissor=Com::from_raw(clipped_state,"GPU scissor rasterizer")?;
        // VSync is normally the most power- and latency-consistent setting
        // on integrated graphics; opt out only when measuring raw throughput.
        let present_interval=match std::env::var("MATA_GPU_VSYNC") {
            Ok(v) if v.eq_ignore_ascii_case("off") || v=="0" => 0,
            _ => 1,
        };
        let mut gpu=Self{hwnd,target:None,scene:None,hud:None,
            vertex,pixel,pixel_sky,sampler,raster,raster_scissor,
            geo_target:None,geo_depth:None,geo_buffer:None,geo_capacity:0,geo_last_upload:None,
            geo_vertex,geo_pixel,geo_layout,geo_depth_state,
            sphere_vs,sphere_ps,sphere_layout,sphere_mesh:None,sphere_mesh_count:0,
            sphere_instances:None,sphere_capacity:0,sphere_last_upload:None,
            bond_vs,bond_ps,bond_layout,bond_mesh:None,bond_mesh_count:0,
            bond_instances:None,bond_capacity:0,bond_last_upload:None,
            context,swap,device,adapter,factory,size:(width,height),adapter_name:info.name.clone(),vendor_id:info.vendor_id,
            adapter_index:info.index,dedicated_video_memory:info.dedicated_video_memory,
            present_interval};
        gpu.rebuild_target()?;
        Ok(gpu)
    }
    fn rebuild_target(&mut self)->Result<(),String>{
        let mut texture=ptr::null_mut();
        let get_buffer:unsafe extern "system" fn(Raw,u32,*const Guid,*mut Raw)->Hr=unsafe{self.swap.method(9)};
        hr_ok(unsafe{get_buffer(self.swap.get(),0,&IID_D3D11_TEXTURE2D,&mut texture)},"swapchain buffer")?;
        let backbuffer=Com::from_raw(texture,"backbuffer")?;
        let mut target=ptr::null_mut();
        let make_rtv:unsafe extern "system" fn(Raw,Raw,Raw,*mut Raw)->Hr=unsafe{self.device.method(9)};
        hr_ok(unsafe{make_rtv(self.device.get(),backbuffer.get(),ptr::null_mut(),&mut target)},"render target")?;
        self.target=Some(Com(target));
        Ok(())
    }
    fn resize(&mut self,width:u32,height:u32)->Result<(),String>{
        if self.size==(width,height){return Ok(())}
        if width==0||height==0{return Ok(())}
        // Release *all* direct and context-held references to the backbuffer.
        unsafe{
            let unbind:unsafe extern "system" fn(Raw,u32,*const Raw,Raw)=self.context.method(33);
            unbind(self.context.get(),0,ptr::null(),ptr::null_mut());
        }
        self.target.take();
        let resize:unsafe extern "system" fn(Raw,u32,u32,u32,u32,u32)->Hr=unsafe{self.swap.method(13)};
        hr_ok(unsafe{resize(self.swap.get(),0,width,height,DXGI_FORMAT_BGRA8,0)},"resize swapchain")?;
        self.size=(width,height);
        self.rebuild_target()
    }
    fn make_texture(&self,width:u32,height:u32)->Result<Texture,String>{
        let desc=TextureDesc{width,height,mip_levels:1,array_size:1,format:DXGI_FORMAT_BGRA8,
            sample:SampleDesc{count:1,quality:0},usage:D3D11_USAGE_DYNAMIC,
            bind:D3D11_BIND_SHADER_RESOURCE,cpu_access:D3D11_CPU_ACCESS_WRITE,misc:0};
        let mut raw=ptr::null_mut();
        let make:unsafe extern "system" fn(Raw,*const TextureDesc,Raw,*mut Raw)->Hr=unsafe{self.device.method(5)};
        hr_ok(unsafe{make(self.device.get(),&desc,ptr::null_mut(),&mut raw)},"create dynamic texture")?;
        let resource=Com::from_raw(raw,"upload texture")?;
        let mut view=ptr::null_mut();
        let srv:unsafe extern "system" fn(Raw,Raw,Raw,*mut Raw)->Hr=unsafe{self.device.method(7)};
        hr_ok(unsafe{srv(self.device.get(),resource.get(),ptr::null_mut(),&mut view)},"create shader view")?;
        Ok(Texture{resource,view:Com(view),size:(width,height)})
    }
    fn upload(context:&Com,texture:&Texture,frame:&Framebuffer)->Result<(),String>{
        let map:unsafe extern "system" fn(Raw,Raw,u32,u32,u32,*mut Mapped)->Hr=unsafe{context.method(14)};
        let unmap:unsafe extern "system" fn(Raw,Raw,u32)=unsafe{context.method(15)};
        let mut dest=Mapped::default();
        hr_ok(unsafe{map(context.get(),texture.resource.get(),0,D3D11_MAP_WRITE_DISCARD,0,&mut dest)},"map GPU upload")?;
        // BGRA byte ordering is identical for little-endian u32 ARGB and DXGI BGRA8.
        let row_bytes=frame.width as usize*4;
        if dest.data.is_null() || (dest.row_pitch as usize)<row_bytes {
            unsafe{unmap(context.get(),texture.resource.get(),0)};
            return Err("invalid mapped texture row pitch".into());
        }
        unsafe{
            let source=frame.pixels.as_ptr().cast::<u8>();
            let target=dest.data.cast::<u8>();
            for y in 0..frame.height as usize{
                ptr::copy_nonoverlapping(source.add(y*row_bytes),
                    target.add(y*dest.row_pitch as usize),row_bytes);
            }
            unmap(context.get(),texture.resource.get(),0);
        }
        Ok(())
    }
    pub fn present(&mut self,scene:&Framebuffer,hud:&Framebuffer)->Result<(),String>{
        if scene.width==0||scene.height==0||hud.width==0||hud.height==0{return Ok(())}
        if unsafe{IsWindow(self.hwnd)}==0{return Err("game HWND is no longer valid".into())}
        self.resize(hud.width,hud.height)?;
        if self.scene.as_ref().is_none_or(|t|t.size!=(scene.width,scene.height)){
            self.scene=Some(self.make_texture(scene.width,scene.height)?);
        }
        if self.hud.as_ref().is_none_or(|t|t.size!=(hud.width,hud.height)){
            self.hud=Some(self.make_texture(hud.width,hud.height)?);
        }
        Self::upload(&self.context,self.scene.as_ref().unwrap(),scene)?;
        Self::upload(&self.context,self.hud.as_ref().unwrap(),hud)?;
        self.compose_and_present(self.scene.as_ref().unwrap().view.get(),hud,false)
    }
    /// GPU scene geometry, including the first-person depth-clear boundary,
    /// is rendered into a GPU-owned render-target texture. Only the sky
    /// background and HUD are uploaded; no pixel triangles are computed on CPU.
    pub fn present_geometry(&mut self,background:&Framebuffer,frame:&GpuFrame,hud:&Framebuffer)->Result<(),String>{
        if background.width==0||background.height==0||hud.width==0||hud.height==0{return Ok(());}
        if background.width!=hud.width||background.height!=hud.height{
            return Err("GPU scene/background/HUD dimensions must match".into());
        }
        self.validate_geometry_frame(frame)?;
        if frame.vertices.len()>u32::MAX as usize{return Err("too many geometry vertices".into());}
        if !frame.depth_clears.windows(2).all(|w|w[0]<=w[1])
            || frame.depth_clears.iter().any(|&n|n>frame.vertices.len()||n%3!=0)
            || frame.vertices.len()%3!=0{return Err("invalid GPU triangle boundaries".into());}
        if frame.vertices.iter().any(|v|v.position.iter().any(|x|!x.is_finite())){
            return Err("nonfinite GPU vertex".into());
        }
        if unsafe{IsWindow(self.hwnd)}==0{return Err("game HWND is no longer valid".into());}
        self.resize(hud.width,hud.height)?;
        let size=(background.width,background.height);
        if self.scene.as_ref().is_none_or(|t|t.size!=size){
            self.scene=Some(self.make_texture(size.0,size.1)?);
        }
        if self.hud.as_ref().is_none_or(|t|t.size!=size){self.hud=Some(self.make_texture(size.0,size.1)?);}
        if self.geo_target.as_ref().is_none_or(|t|t.size!=size){self.geo_target=Some(self.make_geo_target(size.0,size.1)?);}
        if self.geo_depth.as_ref().is_none_or(|t|t.size!=size){self.geo_depth=Some(self.make_geo_depth(size.0,size.1)?);}
        Self::upload(&self.context,self.scene.as_ref().unwrap(),background)?;
        Self::upload(&self.context,self.hud.as_ref().unwrap(),hud)?;
        self.draw_geometry(frame,false,None)?;
        self.compose_and_present(self.geo_target.as_ref().unwrap().view.get(),hud,false)
    }
    /// Present a molecule's GPU-triangle stream within its UI viewport.
    /// The scissor also clips triangles which straddle the viewport border;
    /// panes, toolbars and text outside that rectangle cannot be overwritten.
    /// `hud` contains alpha-masked foreground UI painted after the 3D scene.
    pub fn present_geometry_clipped(&mut self,background:&Framebuffer,frame:&GpuFrame,
        hud:&Framebuffer,clip:(i32,i32,i32,i32))->Result<(),String>{
        if background.width==0||background.height==0{return Ok(());}
        let (x,y,width,height)=clip;
        let max_x=background.width.min(i32::MAX as u32) as i32;
        let max_y=background.height.min(i32::MAX as u32) as i32;
        let bounds=ScissorRect{
            left:x.clamp(0,max_x),top:y.clamp(0,max_y),
            right:x.saturating_add(width.max(0)).clamp(0,max_x),
            bottom:y.saturating_add(height.max(0)).clamp(0,max_y),
        };
        if bounds.left>=bounds.right || bounds.top>=bounds.bottom{
            return self.present(background,hud);
        }
        self.validate_geometry_frame(frame)?;
        if hud.width!=background.width || hud.height!=background.height {
            return Err("GPU background and overlay dimensions must match".into());
        }
        if unsafe{IsWindow(self.hwnd)}==0 {return Err("MataWin HWND is no longer valid".into());}
        self.resize(background.width,background.height)?;
        let size=(background.width,background.height);
        if self.scene.as_ref().is_none_or(|t|t.size!=size) {self.scene=Some(self.make_texture(size.0,size.1)?);}
        if self.hud.as_ref().is_none_or(|t|t.size!=size) {self.hud=Some(self.make_texture(size.0,size.1)?);}
        if self.geo_target.as_ref().is_none_or(|t|t.size!=size) {self.geo_target=Some(self.make_geo_target(size.0,size.1)?);}
        if self.geo_depth.as_ref().is_none_or(|t|t.size!=size) {self.geo_depth=Some(self.make_geo_depth(size.0,size.1)?);}
        Self::upload(&self.context,self.scene.as_ref().unwrap(),background)?;
        Self::upload(&self.context,self.hud.as_ref().unwrap(),hud)?;
        self.draw_geometry(frame,false,Some(bounds))?;
        self.compose_and_present(self.geo_target.as_ref().unwrap().view.get(),hud,false)
    }
    /// Check command stream consistency before issuing native D3D11 calls.
    fn validate_geometry_frame(&self,frame:&GpuFrame)->Result<(),String>{
        if frame.bonds.len()>u32::MAX as usize || frame.bonds.iter().any(|b|{
            b.start_radius.iter().chain(b.end_near.iter()).chain(b.projection.iter())
                .any(|v|!v.is_finite()) || b.start_radius[3]<=0.0 || b.end_near[3]<=0.0
                || b.start_radius[2]<=b.end_near[3]+b.start_radius[3]
                || b.end_near[2]<=b.end_near[3]+b.start_radius[3]
        }) {return Err("invalid GPU bond instance".into());}
        if frame.spheres.len()>u32::MAX as usize || frame.spheres.iter().any(|s|{
            s.center_depth.iter().chain(s.scale.iter()).chain(std::iter::once(&s.near))
                .any(|v|!v.is_finite()) || s.center_depth[2]<=0.0 || s.center_depth[3]<=0.0
                || s.near<=0.0 || s.center_depth[2]-s.center_depth[3]<=s.near
        }) {return Err("invalid GPU sphere instance".into());}
        // Native vertex data was validated when uploaded. The generation ID
        // is global to all Rasterizer instances, including tab switches.
        if frame.generation!=0 && self.geo_last_upload==Some((frame.generation,frame.vertices.len())) {
            return Ok(());
        }
        if frame.vertices.len()>u32::MAX as usize || frame.vertices.len()%3!=0 {
            return Err("invalid GPU triangle count".into());
        }
        if !frame.depth_clears.windows(2).all(|w|w[0]<=w[1]) ||
            frame.depth_clears.iter().any(|&n|n>frame.vertices.len()||n%3!=0){
            return Err("invalid GPU depth-clear boundaries".into());
        }
        if frame.vertices.iter().any(|v|v.position.iter().any(|x|!x.is_finite())) {
            return Err("nonfinite GPU clip coordinate".into());
        }
        Ok(())
    }
    /// Bandwidth-saving geometry path for Intel integrated GPUs. Builds the
    /// original Neon Run sky gradient in the GPU compositor; the CPU does not
    /// generate or upload a full 32-bit 4K background texture each frame.
    /// Geometry is still drawn in native resolution with the same depth tests.
    pub fn present_geometry_procedural(&mut self,frame:&GpuFrame,hud:&Framebuffer)->Result<(),String>{
        if hud.width==0||hud.height==0{return Ok(())}
        self.validate_geometry_frame(frame)?;
        if frame.vertices.len()>u32::MAX as usize{return Err("too many geometry vertices".into());}
        if !frame.depth_clears.windows(2).all(|w|w[0]<=w[1])
            ||frame.depth_clears.iter().any(|&n|n>frame.vertices.len()||n%3!=0)
            ||frame.vertices.len()%3!=0 {return Err("invalid GPU triangle boundaries".into());}
        if frame.vertices.iter().any(|v|v.position.iter().any(|x|!x.is_finite())){
            return Err("nonfinite GPU vertex".into());
        }
        if unsafe{IsWindow(self.hwnd)}==0{return Err("game HWND is no longer valid".into());}
        self.resize(hud.width,hud.height)?;
        let size=(hud.width,hud.height);
        if self.hud.as_ref().is_none_or(|t|t.size!=size){self.hud=Some(self.make_texture(size.0,size.1)?);}
        if self.geo_target.as_ref().is_none_or(|t|t.size!=size){self.geo_target=Some(self.make_geo_target(size.0,size.1)?);}
        if self.geo_depth.as_ref().is_none_or(|t|t.size!=size){self.geo_depth=Some(self.make_geo_depth(size.0,size.1)?);}
        Self::upload(&self.context,self.hud.as_ref().unwrap(),hud)?;
        self.draw_geometry(frame,true,None)?;
        self.compose_and_present(self.geo_target.as_ref().unwrap().view.get(),hud,true)
    }
    fn make_geo_target(&self,width:u32,height:u32)->Result<GpuSceneTarget,String>{
        let desc=TextureDesc{width,height,mip_levels:1,array_size:1,format:DXGI_FORMAT_BGRA8,
            sample:SampleDesc{count:1,quality:0},usage:0,
            bind:D3D11_BIND_SHADER_RESOURCE|D3D11_BIND_RENDER_TARGET,cpu_access:0,misc:0};
        let mut raw=ptr::null_mut();
        let make:unsafe extern "system" fn(Raw,*const TextureDesc,Raw,*mut Raw)->Hr=unsafe{self.device.method(5)};
        hr_ok(unsafe{make(self.device.get(),&desc,ptr::null_mut(),&mut raw)},"create GPU scene target")?;
        let texture=Com::from_raw(raw,"GPU scene texture")?;
        let mut rtv=ptr::null_mut();let mut srv=ptr::null_mut();
        let make_rtv:unsafe extern "system" fn(Raw,Raw,Raw,*mut Raw)->Hr=unsafe{self.device.method(9)};
        let make_srv:unsafe extern "system" fn(Raw,Raw,Raw,*mut Raw)->Hr=unsafe{self.device.method(7)};
        hr_ok(unsafe{make_rtv(self.device.get(),texture.get(),ptr::null_mut(),&mut rtv)},"GPU scene RTV")?;
        hr_ok(unsafe{make_srv(self.device.get(),texture.get(),ptr::null_mut(),&mut srv)},"GPU scene SRV")?;
        Ok(GpuSceneTarget{texture,rtv:Com(rtv),view:Com(srv),size:(width,height)})
    }
    fn make_geo_depth(&self,width:u32,height:u32)->Result<GpuDepth,String>{
        let desc=TextureDesc{width,height,mip_levels:1,array_size:1,format:DXGI_FORMAT_D32_FLOAT,
            sample:SampleDesc{count:1,quality:0},usage:0,bind:D3D11_BIND_DEPTH_STENCIL,cpu_access:0,misc:0};
        let mut raw=ptr::null_mut();
        let make:unsafe extern "system" fn(Raw,*const TextureDesc,Raw,*mut Raw)->Hr=unsafe{self.device.method(5)};
        hr_ok(unsafe{make(self.device.get(),&desc,ptr::null_mut(),&mut raw)},"GPU depth texture")?;
        let texture=Com::from_raw(raw,"GPU depth texture")?;
        let mut dsv=ptr::null_mut();
        let make_dsv:unsafe extern "system" fn(Raw,Raw,Raw,*mut Raw)->Hr=unsafe{self.device.method(10)};
        hr_ok(unsafe{make_dsv(self.device.get(),texture.get(),ptr::null_mut(),&mut dsv)},"GPU depth view")?;
        Ok(GpuDepth{_texture:texture,view:Com(dsv),size:(width,height)})
    }
    fn ensure_gpu_vertex_buffer(&mut self,count:usize)->Result<(),String>{
        if count<=self.geo_capacity{return Ok(())}
        let capacity=count.next_power_of_two().max(3072);
        let bytes=capacity.checked_mul(std::mem::size_of::<GpuVertex>())
            .ok_or("GPU vertex capacity overflow")?;
        if bytes>u32::MAX as usize{return Err("GPU vertex buffer exceeds D3D11 limit".into())}
        let desc=BufferDesc{byte_width:bytes as u32,usage:D3D11_USAGE_DYNAMIC,
            bind:D3D11_BIND_VERTEX_BUFFER,cpu_access:D3D11_CPU_ACCESS_WRITE,misc:0,stride:0};
        let mut raw=ptr::null_mut();
        let make:unsafe extern "system" fn(Raw,*const BufferDesc,Raw,*mut Raw)->Hr=unsafe{self.device.method(3)};
        hr_ok(unsafe{make(self.device.get(),&desc,ptr::null_mut(),&mut raw)},"dynamic GPU triangle buffer")?;
        self.geo_buffer=Some(Com::from_raw(raw,"GPU vertex buffer")?);
        self.geo_capacity=capacity;
        self.geo_last_upload=None;
        Ok(())
    }
    fn make_dynamic_vertex_buffer(&self,bytes:usize,label:&str)->Result<Com,String>{
        if bytes==0 || bytes>u32::MAX as usize{return Err(format!("{label}: invalid buffer size"));}
        let desc=BufferDesc{byte_width:bytes as u32,usage:D3D11_USAGE_DYNAMIC,
            bind:D3D11_BIND_VERTEX_BUFFER,cpu_access:D3D11_CPU_ACCESS_WRITE,misc:0,stride:0};
        let mut raw=ptr::null_mut();
        let make:unsafe extern "system" fn(Raw,*const BufferDesc,Raw,*mut Raw)->Hr=unsafe{self.device.method(3)};
        hr_ok(unsafe{make(self.device.get(),&desc,ptr::null_mut(),&mut raw)},label)?;
        Com::from_raw(raw,label)
    }
    fn upload_dynamic<T>(&self,buffer:&Com,vertices:&[T],label:&str)->Result<(),String>{
        unsafe{
            let map:unsafe extern "system" fn(Raw,Raw,u32,u32,u32,*mut Mapped)->Hr=self.context.method(14);
            let unmap:unsafe extern "system" fn(Raw,Raw,u32)=self.context.method(15);
            let mut mapped=Mapped::default();
            hr_ok(map(self.context.get(),buffer.get(),0,D3D11_MAP_WRITE_DISCARD,0,&mut mapped),label)?;
            if mapped.data.is_null(){unmap(self.context.get(),buffer.get(),0);return Err(format!("{label}: null map"));}
            ptr::copy_nonoverlapping(vertices.as_ptr().cast::<u8>(),mapped.data.cast::<u8>(),
                std::mem::size_of_val(vertices));
            unmap(self.context.get(),buffer.get(),0);
        }
        Ok(())
    }
    fn ensure_sphere_resources(&mut self,count:usize)->Result<(),String>{
        if self.sphere_mesh.is_none(){
            // Cached unit mesh: 6 latitude bands x 8 longitude segments,
            // 96 triangles shared by every atom in every frame.
            let rows=6usize;let cols=8usize;
            let mut vertices=Vec::<[f32;3]>::with_capacity(rows*cols*6);
            let xyz=|row:usize,col:usize|{
                let phi=row as f32*std::f32::consts::PI/rows as f32;
                let theta=col as f32*std::f32::consts::TAU/cols as f32;
                [phi.sin()*theta.cos(),phi.sin()*theta.sin(),phi.cos()]
            };
            for row in 0..rows {for col in 0..cols {
                let a=xyz(row,col);let b=xyz(row+1,col);
                let c=xyz(row,col+1);let d=xyz(row+1,col+1);
                vertices.extend([a,b,c,c,b,d]);
            }}
            let size=vertices.len()*std::mem::size_of::<[f32;3]>();
            let buffer=self.make_dynamic_vertex_buffer(size,"unit sphere vertex buffer")?;
            self.upload_dynamic(&buffer,&vertices,"upload unit sphere")?;
            self.sphere_mesh_count=vertices.len() as u32;
            self.sphere_mesh=Some(buffer);
        }
        if count>self.sphere_capacity {
            let capacity=count.checked_next_power_of_two().ok_or("atom GPU instance count overflow")?.max(128);
            let bytes=capacity.checked_mul(std::mem::size_of::<GpuSphereInstance>())
                .ok_or("atom GPU instance allocation overflow")?;
            let buffer=self.make_dynamic_vertex_buffer(bytes,"GPU atom instance buffer")?;
            self.sphere_instances=Some(buffer);
            self.sphere_capacity=capacity;
            self.sphere_last_upload=None;
        }
        Ok(())
    }
    fn draw_sphere_instances(&mut self,frame:&GpuFrame)->Result<(),String>{
        let count=frame.spheres.len();
        if count==0{return Ok(());}
        self.ensure_sphere_resources(count)?;
        let instance=self.sphere_instances.as_ref().ok_or("no instance buffer")?;
        if frame.generation==0 || self.sphere_last_upload!=Some((frame.generation,count)){
            self.upload_dynamic(instance,&frame.spheres,"upload atom instances")?;
            self.sphere_last_upload=if frame.generation==0{None}else{Some((frame.generation,count))};
        }
        unsafe{
            let ctx=self.context.get();
            let set_layout:unsafe extern "system" fn(Raw,Raw)=self.context.method(17);
            set_layout(ctx,self.sphere_layout.get());
            let vs:unsafe extern "system" fn(Raw,Raw,*const Raw,u32)=self.context.method(11);
            let ps:unsafe extern "system" fn(Raw,Raw,*const Raw,u32)=self.context.method(9);
            vs(ctx,self.sphere_vs.get(),ptr::null(),0);
            ps(ctx,self.sphere_ps.get(),ptr::null(),0);
            let buffers=[self.sphere_mesh.as_ref().unwrap().get(),instance.get()];
            let strides=[std::mem::size_of::<[f32;3]>() as u32,std::mem::size_of::<GpuSphereInstance>() as u32];
            let offsets=[0u32,0u32];
            let vb:unsafe extern "system" fn(Raw,u32,u32,*const Raw,*const u32,*const u32)=self.context.method(18);
            vb(ctx,0,2,buffers.as_ptr(),strides.as_ptr(),offsets.as_ptr());
            let draw:unsafe extern "system" fn(Raw,u32,u32,u32,u32)=self.context.method(21);
            draw(ctx,self.sphere_mesh_count,count as u32,0,0);
            vb(ctx,0,2,[ptr::null_mut(),ptr::null_mut()].as_ptr(),[0u32;2].as_ptr(),[0u32;2].as_ptr());
        }
        Ok(())
    }
    fn ensure_bond_resources(&mut self,count:usize)->Result<(),String>{
        if self.bond_mesh.is_none(){
            // One open eight-sided cylinder, reused for every bond.
            let sides=8usize;
            let mut vertices=Vec::<[f32;3]>::with_capacity(sides*6);
            for i in 0..sides {
                let a=i as f32*std::f32::consts::TAU/sides as f32;
                let b=(i+1) as f32*std::f32::consts::TAU/sides as f32;
                let (sa,ca)=a.sin_cos();let(sb,cb)=b.sin_cos();
                vertices.extend([[ca,sa,0.0],[ca,sa,1.0],[cb,sb,0.0],
                                 [cb,sb,0.0],[ca,sa,1.0],[cb,sb,1.0]]);
            }
            let size=vertices.len()*std::mem::size_of::<[f32;3]>();
            let buffer=self.make_dynamic_vertex_buffer(size,"unit bond cylinder")?;
            self.upload_dynamic(&buffer,&vertices,"upload unit bond cylinder")?;
            self.bond_mesh_count=vertices.len() as u32;
            self.bond_mesh=Some(buffer);
        }
        if count>self.bond_capacity {
            let capacity=count.checked_next_power_of_two().ok_or("bond instance overflow")?.max(128);
            let bytes=capacity.checked_mul(std::mem::size_of::<GpuBondInstance>())
                .ok_or("bond instance allocation overflow")?;
            let buffer=self.make_dynamic_vertex_buffer(bytes,"GPU bond instance buffer")?;
            self.bond_instances=Some(buffer);
            self.bond_capacity=capacity;
            self.bond_last_upload=None;
        }
        Ok(())
    }
    fn draw_bond_instances(&mut self,frame:&GpuFrame)->Result<(),String>{
        let count=frame.bonds.len();
        if count==0{return Ok(());}
        self.ensure_bond_resources(count)?;
        let instance=self.bond_instances.as_ref().ok_or("missing bond buffer")?;
        if frame.generation==0 || self.bond_last_upload!=Some((frame.generation,count)){
            self.upload_dynamic(instance,&frame.bonds,"upload bond instances")?;
            self.bond_last_upload=if frame.generation==0{None}else{Some((frame.generation,count))};
        }
        unsafe{
            let ctx=self.context.get();
            let layout:unsafe extern "system" fn(Raw,Raw)=self.context.method(17);
            layout(ctx,self.bond_layout.get());
            let vs:unsafe extern "system" fn(Raw,Raw,*const Raw,u32)=self.context.method(11);
            let ps:unsafe extern "system" fn(Raw,Raw,*const Raw,u32)=self.context.method(9);
            vs(ctx,self.bond_vs.get(),ptr::null(),0);
            ps(ctx,self.bond_ps.get(),ptr::null(),0);
            let buffers=[self.bond_mesh.as_ref().unwrap().get(),instance.get()];
            let strides=[std::mem::size_of::<[f32;3]>() as u32,std::mem::size_of::<GpuBondInstance>() as u32];
            let offsets=[0u32,0u32];
            let vb:unsafe extern "system" fn(Raw,u32,u32,*const Raw,*const u32,*const u32)=self.context.method(18);
            vb(ctx,0,2,buffers.as_ptr(),strides.as_ptr(),offsets.as_ptr());
            let draw:unsafe extern "system" fn(Raw,u32,u32,u32,u32)=self.context.method(21);
            draw(ctx,self.bond_mesh_count,count as u32,0,0);
            vb(ctx,0,2,[ptr::null_mut(),ptr::null_mut()].as_ptr(),[0u32;2].as_ptr(),[0u32;2].as_ptr());
        }
        Ok(())
    }
    fn draw_geometry(&mut self,frame:&GpuFrame,procedural_sky:bool,scissor:Option<ScissorRect>)->Result<(),String>{
        self.ensure_gpu_vertex_buffer(frame.vertices.len())?;
        unsafe{
            let ctx=self.context.get();
            let color=self.geo_target.as_ref().ok_or("missing GPU scene")?;
            let depth=self.geo_depth.as_ref().ok_or("missing GPU depth")?;
            if procedural_sky {
                // Transparent pixels mean sky in the final composition pass.
                // Avoids CPU sky generation, Map/Unmap and 4K texture upload.
                let clear:unsafe extern "system" fn(Raw,Raw,*const f32)=self.context.method(50);
                clear(ctx,color.rtv.get(),[0.0f32,0.0,0.0,0.0].as_ptr());
            }else{
                let copy:unsafe extern "system" fn(Raw,Raw,Raw)=self.context.method(47);
                copy(ctx,color.texture.get(),self.scene.as_ref().unwrap().resource.get());
            }
            let bind:unsafe extern "system" fn(Raw,u32,*const Raw,Raw)=self.context.method(33);
            let rt=color.rtv.get();
            bind(ctx,1,&rt,depth.view.get());
            let clear_depth:unsafe extern "system" fn(Raw,Raw,u32,f32,u8)=self.context.method(53);
            clear_depth(ctx,depth.view.get(),1,0.0,0);
            let ds:unsafe extern "system" fn(Raw,Raw,u32)=self.context.method(36);
            ds(ctx,self.geo_depth_state.get(),0);
            let viewport=Viewport{x:0.0,y:0.0,width:color.size.0 as f32,height:color.size.1 as f32,min_depth:0.0,max_depth:1.0};
            let set_viewport:unsafe extern "system" fn(Raw,u32,*const Viewport)=self.context.method(44);
            set_viewport(ctx,1,&viewport);
            let rs:unsafe extern "system" fn(Raw,Raw)=self.context.method(43);
            rs(ctx,if scissor.is_some(){self.raster_scissor.get()}else{self.raster.get()});
            if let Some(bounds)=scissor {
                // ID3D11DeviceContext::RSSetScissorRects, COM vtable slot 45.
                let set_scissor:unsafe extern "system" fn(Raw,u32,*const ScissorRect)=self.context.method(45);
                set_scissor(ctx,1,&bounds);
            }
            let layout:unsafe extern "system" fn(Raw,Raw)=self.context.method(17);
            layout(ctx,self.geo_layout.get());
            let set_vs:unsafe extern "system" fn(Raw,Raw,*const Raw,u32)=self.context.method(11);
            let set_ps:unsafe extern "system" fn(Raw,Raw,*const Raw,u32)=self.context.method(9);
            set_vs(ctx,self.geo_vertex.get(),ptr::null(),0);
            set_ps(ctx,self.geo_pixel.get(),ptr::null(),0);
            let topology:unsafe extern "system" fn(Raw,u32)=self.context.method(24);
            topology(ctx,D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            if let Some(buffer)=self.geo_buffer.as_ref(){
                if !frame.vertices.is_empty(){
                    if frame.generation==0 || self.geo_last_upload!=Some((frame.generation,frame.vertices.len())) {
                        let map:unsafe extern "system" fn(Raw,Raw,u32,u32,u32,*mut Mapped)->Hr=self.context.method(14);
                        let unmap:unsafe extern "system" fn(Raw,Raw,u32)=self.context.method(15);
                        let mut mapped=Mapped::default();
                        hr_ok(map(ctx,buffer.get(),0,D3D11_MAP_WRITE_DISCARD,0,&mut mapped),"map GPU vertices")?;
                        if mapped.data.is_null() {unmap(ctx,buffer.get(),0);return Err("GPU vertex map returned null".into());}
                        ptr::copy_nonoverlapping(frame.vertices.as_ptr().cast::<u8>(),mapped.data.cast::<u8>(),
                            frame.vertices.len()*std::mem::size_of::<GpuVertex>());
                        unmap(ctx,buffer.get(),0);
                        self.geo_last_upload=if frame.generation==0 {None}
                            else{Some((frame.generation,frame.vertices.len()))};
                    }
                    let vertex=buffer.get();let stride=std::mem::size_of::<GpuVertex>() as u32;let offset=0u32;
                    let vb:unsafe extern "system" fn(Raw,u32,u32,*const Raw,*const u32,*const u32)=self.context.method(18);
                    vb(ctx,0,1,&vertex,&stride,&offset);
                    let draw:unsafe extern "system" fn(Raw,u32,u32)=self.context.method(13);
                    let mut start=0usize;
                    for end in frame.depth_clears.iter().copied().chain(std::iter::once(frame.vertices.len())){
                        if end>start{draw(ctx,(end-start) as u32,start as u32)}
                        if end!=frame.vertices.len(){clear_depth(ctx,depth.view.get(),1,0.0,0)}
                        start=end;
                    }
                    let no_vertex:Raw=ptr::null_mut();
                    vb(ctx,0,1,&no_vertex,&0u32,&0u32);
                }
            }
            // The shared sphere/bond vertex meshes are retained on GPU.
            // The per-instance vertex streams feed native DrawInstanced calls
            // instead of repeatedly expanding triangles on the CPU.
        }
        self.draw_bond_instances(frame)?;
        self.draw_sphere_instances(frame)?;
        unsafe{
            let ctx=self.context.get();
            let bind:unsafe extern "system" fn(Raw,u32,*const Raw,Raw)=self.context.method(33);
            let ds:unsafe extern "system" fn(Raw,Raw,u32)=self.context.method(36);
            bind(ctx,0,ptr::null(),ptr::null_mut());
            ds(ctx,ptr::null_mut(),0);
        }
        Ok(())
    }
    fn compose_and_present(&mut self,scene_view:Raw,hud:&Framebuffer,procedural_sky:bool)->Result<(),String>{
        unsafe{
            let ctx=self.context.get();
            let rtv=self.target.as_ref().ok_or("missing backbuffer view")?.get();
            let bind_target:unsafe extern "system" fn(Raw,u32,*const Raw,Raw)=self.context.method(33);
            let clear:unsafe extern "system" fn(Raw,Raw,*const f32)=self.context.method(50);
            bind_target(ctx,1,&rtv,ptr::null_mut());
            clear(ctx,rtv,[0.0f32,0.0,0.0,1.0].as_ptr());
            let viewport=Viewport{x:0.0,y:0.0,width:hud.width as f32,height:hud.height as f32,
                min_depth:0.0,max_depth:1.0};
            let set_viewport:unsafe extern "system" fn(Raw,u32,*const Viewport)=self.context.method(44);
            set_viewport(ctx,1,&viewport);
            let set_raster:unsafe extern "system" fn(Raw,Raw)=self.context.method(43);
            set_raster(ctx,self.raster.get());
            let clear_layout:unsafe extern "system" fn(Raw,Raw)=self.context.method(17);
            clear_layout(ctx,ptr::null_mut());
            let depth_state:unsafe extern "system" fn(Raw,Raw,u32)=self.context.method(36);
            depth_state(ctx,ptr::null_mut(),0);
            let set_vs:unsafe extern "system" fn(Raw,Raw,*const Raw,u32)=self.context.method(11);
            let set_ps:unsafe extern "system" fn(Raw,Raw,*const Raw,u32)=self.context.method(9);
            set_vs(ctx,self.vertex.get(),ptr::null(),0);
            set_ps(ctx,if procedural_sky{self.pixel_sky.get()}else{self.pixel.get()},ptr::null(),0);
            let views=[scene_view,self.hud.as_ref().unwrap().view.get()];
            let set_views:unsafe extern "system" fn(Raw,u32,u32,*const Raw)=self.context.method(8);
            set_views(ctx,0,2,views.as_ptr());
            let sampler=self.sampler.get();
            let set_samplers:unsafe extern "system" fn(Raw,u32,u32,*const Raw)=self.context.method(10);
            set_samplers(ctx,0,1,&sampler);
            let primitive:unsafe extern "system" fn(Raw,u32)=self.context.method(24);
            primitive(ctx,D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            let draw:unsafe extern "system" fn(Raw,u32,u32)=self.context.method(13);
            draw(ctx,3,0);
            // Release context references between frames / after resize.
            set_views(ctx,0,2,[ptr::null_mut(),ptr::null_mut()].as_ptr());
            let present:unsafe extern "system" fn(Raw,u32,u32)->Hr=self.swap.method(8);
            hr_ok(present(self.swap.get(),self.present_interval,0),"GPU present")
        }
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn intel_vendor_is_explicit(){assert_eq!(0x8086,32902);}
    #[test]fn texture_formats_are_bgra(){assert_eq!(DXGI_FORMAT_BGRA8,87);}
    #[test]fn pixel_upload_layout(){assert_eq!(0xff12_3456_u32.to_le_bytes(),[0x56,0x34,0x12,0xff]);}
    #[test]fn packed_vertex_layout(){
        assert_eq!(std::mem::size_of::<GpuVertex>(),16);
        assert_eq!(std::mem::size_of::<GpuSphereInstance>(),32);
        assert_eq!(std::mem::size_of::<GpuBondInstance>(),52);
        assert_eq!(28u32,28); // DXGI_FORMAT_R8G8B8A8_UNORM
    }
}

impl Drop for IntelGpu{
    fn drop(&mut self){
        // In particular, unbind the backbuffer before releasing the swapchain;
        // DXGI cannot resize/recreate a swapchain with outstanding RTV refs.
        unsafe{
            let ctx=self.context.get();
            let unbind_render:unsafe extern "system" fn(Raw,u32,*const Raw,Raw)=self.context.method(33);
            unbind_render(ctx,0,ptr::null(),ptr::null_mut());
            let clear_views:unsafe extern "system" fn(Raw,u32,u32,*const Raw)=self.context.method(8);
            clear_views(ctx,0,2,[ptr::null_mut(),ptr::null_mut()].as_ptr());
            let shader:unsafe extern "system" fn(Raw,Raw,*const Raw,u32)=self.context.method(9);
            shader(ctx,ptr::null_mut(),ptr::null(),0);
            let shader_vs:unsafe extern "system" fn(Raw,Raw,*const Raw,u32)=self.context.method(11);
            shader_vs(ctx,ptr::null_mut(),ptr::null(),0);
        }
    }
}
