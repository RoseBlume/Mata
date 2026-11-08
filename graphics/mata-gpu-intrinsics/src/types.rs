use alloc::string::String;
use alloc::format;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Shader syntax target. No backend is queried or initialized during Rust compilation.
pub enum Backend { Wgsl, Glsl, Hlsl,
    /// Intel GPU OpenCL C compute kernel source (runtime compilation by Intel OpenCL).
    IntelOpenCl,
    /// NVIDIA CUDA C++ compute kernel source; no CUDA toolkit is used to build this crate.
    NvidiaCuda,
    /// AMD ROCm HIP C++ compute-kernel source; no ROCm libraries linked at build time.
    AmdHip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Shader stage used to reject restricted operations at emission time.
pub enum Stage { Vertex, Fragment, Compute }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// A scalar shader type supported by the expression emitter.
pub enum Scalar { Bool, I32, U32, F32 }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Shader data type; no allocation or runtime GPU resource is implied.
pub enum Type {
    Void,
    /// Unsigned 64-bit device value, used for AMD wave64 ballot masks.
    U64,
    Scalar(Scalar),
    Vector(Scalar, u8),
    Matrix(u8, u8), // float matrix columns, rows (2..4)
    Texture2D,
    Texture2DArray,
    Texture3D,
    StorageTexture2D,
    Sampler,
    ComparisonSampler,
    Atomic(Scalar),
}
impl Type {
    /// A 32-bit float scalar.
    pub const F32: Self = Type::Scalar(Scalar::F32);
    /// A 32-bit signed scalar.
    pub const I32: Self = Type::Scalar(Scalar::I32);
    /// A 32-bit unsigned scalar.
    pub const U32: Self = Type::Scalar(Scalar::U32);
    /// Unsigned 64-bit scalar, including wave64 ballot masks.
    pub const UINT64: Self = Type::U64;
    /// A Boolean scalar.
    pub const BOOL: Self = Type::Scalar(Scalar::Bool);
    /// A two-lane float vector.
    pub const VEC2: Self = Type::Vector(Scalar::F32, 2);
    /// A three-lane float vector.
    pub const VEC3: Self = Type::Vector(Scalar::F32, 3);
    /// A four-lane float vector.
    pub const VEC4: Self = Type::Vector(Scalar::F32, 4);
    /// Return whether the type is a floating scalar or vector.
    pub fn is_float(self) -> bool {
        matches!(self, Type::Scalar(Scalar::F32) | Type::Vector(Scalar::F32, _))
    }
    /// Return whether the type is a signed or unsigned integer scalar/vector.
    pub fn is_integer(self) -> bool {
        matches!(self, Type::Scalar(Scalar::I32 | Scalar::U32) | Type::Vector(Scalar::I32 | Scalar::U32, _))
    }
    /// Return whether the type is a supported numeric scalar/vector.
    pub fn is_numeric(self) -> bool { self.is_float() || self.is_integer() }
    /// Return whether the type is a 2-, 3-, or 4-lane vector.
    pub fn is_vector(self) -> bool { matches!(self, Type::Vector(_,2..=4)) }
    /// Return whether the type is a Boolean scalar/vector.
    pub fn is_bool(self) -> bool { matches!(self, Type::Scalar(Scalar::Bool) | Type::Vector(Scalar::Bool, _)) }
    /// Return whether this is a sampled or storage texture.
    pub fn is_texture(self) -> bool { matches!(self, Type::Texture2D | Type::Texture2DArray | Type::Texture3D | Type::StorageTexture2D) }
    /// Return whether this is an integer atomic type.
    pub fn is_atomic(self) -> bool { matches!(self, Type::Atomic(Scalar::I32 | Scalar::U32)) }
    /// Check the lane dimensions and atomic element-type constraints.
    pub fn valid(self) -> bool { match self {
        Type::Vector(_, n) => (2..=4).contains(&n),
        Type::Matrix(c,r) => (2..=4).contains(&c) && (2..=4).contains(&r),
        Type::Atomic(s) => matches!(s, Scalar::I32|Scalar::U32),
        _ => true,
    }}
    /// Return the target shader-language type spelling; GLSL samplers are combined.
    pub fn source(self, backend: Backend) -> String {
        let scalar = |s: Scalar| match (backend,s) {
            (Backend::Wgsl,Scalar::Bool)=>"bool", (Backend::Wgsl,Scalar::I32)=>"i32",
            (Backend::Wgsl,Scalar::U32)=>"u32", (Backend::Wgsl,Scalar::F32)=>"f32",
            (_,Scalar::Bool)=>"bool", (Backend::Glsl,Scalar::I32)=>"int",
            (Backend::Glsl,Scalar::U32)=>"uint", (Backend::Glsl,Scalar::F32)=>"float",
            (Backend::Hlsl | Backend::IntelOpenCl | Backend::NvidiaCuda | Backend::AmdHip,Scalar::I32)=>"int",
            (Backend::Hlsl | Backend::IntelOpenCl,Scalar::U32)=>"uint",
            (Backend::NvidiaCuda | Backend::AmdHip,Scalar::U32)=>"unsigned int",
            (Backend::Hlsl | Backend::IntelOpenCl | Backend::NvidiaCuda | Backend::AmdHip,Scalar::F32)=>"float",
        };
        match self {
            Type::Void => "void".into(), Type::U64 => "unsigned long long".into(), Type::Scalar(s) => scalar(s).into(),
            Type::Vector(s,n) => match backend {
                Backend::Wgsl => format!("vec{}<{}>",n,scalar(s)),
                Backend::Glsl => {
                    let prefix=match s {Scalar::F32=>"vec", Scalar::I32=>"ivec", Scalar::U32=>"uvec",Scalar::Bool=>"bvec"};
                    format!("{}{}",prefix,n)
                }
                Backend::Hlsl | Backend::IntelOpenCl=>format!("{}{}",scalar(s),n),
                Backend::NvidiaCuda | Backend::AmdHip=>format!("{}{}",match s{Scalar::U32=>"uint",Scalar::I32=>"int",Scalar::F32=>"float",Scalar::Bool=>"bool"},n),
            },
            Type::Matrix(c,r) => match backend {
                Backend::Wgsl => format!("mat{}x{}<f32>",c,r),
                Backend::Glsl=> if c==r {format!("mat{}",c)} else {format!("mat{}x{}",c,r)},
                Backend::Hlsl=>format!("float{}x{}",r,c), // HLSL: rows x cols
                Backend::IntelOpenCl=>"/* OpenCL C has no native matrix type */".into(),
                Backend::NvidiaCuda | Backend::AmdHip=>"/* HIP/CUDA has no native matrix type */".into(),
            },
            Type::Texture2D => match backend { Backend::Wgsl=>"texture_2d<f32>", Backend::Glsl=>"sampler2D", Backend::Hlsl=>"Texture2D<float4>",Backend::IntelOpenCl=>"image2d_t",Backend::NvidiaCuda | Backend::AmdHip=>"/* device texture resource requires explicit API */" }.into(),
            Type::Texture2DArray => match backend {Backend::Wgsl=>"texture_2d_array<f32>",Backend::Glsl=>"sampler2DArray",Backend::Hlsl=>"Texture2DArray<float4>",Backend::IntelOpenCl=>"image2d_array_t",Backend::NvidiaCuda | Backend::AmdHip=>"/* unsupported resource */"}.into(),
            Type::Texture3D => match backend {Backend::Wgsl=>"texture_3d<f32>",Backend::Glsl=>"sampler3D",Backend::Hlsl=>"Texture3D<float4>",Backend::IntelOpenCl=>"image3d_t",Backend::NvidiaCuda | Backend::AmdHip=>"/* unsupported resource */"}.into(),
            Type::StorageTexture2D => match backend {Backend::Wgsl=>"texture_storage_2d<rgba8unorm, read_write>",Backend::Glsl=>"image2D",Backend::Hlsl=>"RWTexture2D<float4>",Backend::IntelOpenCl=>"image2d_t",Backend::NvidiaCuda | Backend::AmdHip=>"/* device texture resource requires explicit API */"}.into(),
            Type::Sampler => match backend {Backend::Wgsl=>"sampler",Backend::Glsl=>"sampler2D",Backend::Hlsl=>"SamplerState",Backend::IntelOpenCl=>"sampler_t",Backend::NvidiaCuda | Backend::AmdHip=>"/* device sampler requires explicit API */"}.into(),
            Type::ComparisonSampler => match backend {Backend::Wgsl=>"sampler_comparison",Backend::Glsl=>"sampler2DShadow",Backend::Hlsl=>"SamplerComparisonState",Backend::IntelOpenCl=>"sampler_t",Backend::NvidiaCuda | Backend::AmdHip=>"/* device sampler requires explicit API */"}.into(),
            Type::Atomic(s) => match backend {
                Backend::Wgsl=>format!("atomic<{}>",scalar(s)),
                Backend::IntelOpenCl=>format!("atomic_{}",scalar(s)),
                Backend::NvidiaCuda | Backend::AmdHip=>format!("{}",scalar(s)),
                _=>format!("{}",scalar(s)),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Optional shader capability requested by the application; this is NOT device detection.
pub enum Capability {
    Core, Derivatives, Textures, Atomics, Workgroup, Subgroups, F16, Packing, StorageTextures, Vendor,
    /// Intel `cl_intel_subgroups` OpenCL extension (must be available on runtime device).
    IntelSubgroups,
    /// Intel `cl_intel_subgroups_short` extension for 16-bit block memory I/O.
    IntelSubgroupsShort,
    /// NVIDIA CUDA warp-vote, shuffle, match and sync intrinsics.
    NvidiaWarp,
    /// NVIDIA CUDA warp reduce operations requiring compute capability SM 8.0+.
    NvidiaSm80,
    /// AMD HIP wavefront shuffle, vote and lane operations. The wave size is
    /// **runtime-dependent**: some devices use 32 lanes and others 64 lanes.
    AmdWave,
    /// AMDGCN-specific compiler builtins; require a matching HIP-Clang target.
    AmdGcn,
}
impl Capability {
    /// Return the capability bit mask; Core is always enabled.
    pub const fn bit(self) -> u32 {match self{
        Self::Core=>0,Self::Derivatives=>1<<0,Self::Textures=>1<<1,Self::Atomics=>1<<2,
        Self::Workgroup=>1<<3,Self::Subgroups=>1<<4,Self::F16=>1<<5,
        Self::Packing=>1<<6,Self::StorageTextures=>1<<7,Self::Vendor=>1<<8,
        Self::IntelSubgroups=>1<<9,Self::IntelSubgroupsShort=>1<<10,
        Self::NvidiaWarp=>1<<11,Self::NvidiaSm80=>1<<12,
        Self::AmdWave=>1<<13,Self::AmdGcn=>1<<14,
    }}
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Enabled shader-capability mask. Supplied explicitly, not detected at build time.
pub struct CapabilitySet(pub u32);
impl CapabilitySet {
    /// Core shader capabilities without optional extensions.
    pub const CORE: Self=Self(0);
    /// Enable all modeled capabilities (does not guarantee GPU/runtime support).
    pub const ALL: Self=Self(u32::MAX);
    /// Return a new capability set with this capability enabled.
    pub const fn with(self, c:Capability)->Self{Self(self.0|c.bit())}
    /// Check whether a capability is present.
    pub const fn contains(self,c:Capability)->bool{(self.0 & c.bit())==c.bit()}
}
