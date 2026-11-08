//! Generated intrinsic registry; update `tools/generate_catalog.py` and regenerate.
use crate::types::{Capability, Stage};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Logical operation group (not necessarily a hardware instruction).
pub enum Family { Math, Geometry, Matrix, Operator, Selection, Comparison, Bits, Packing, Derivative, Texture, Atomic, Barrier, Subgroup, Vendor, Utility }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Type-validation strategy used when emitting an operation.
pub enum Signature { UnaryFloat, UnaryNumber, UnaryInteger, BinaryFloat, BinaryNumber, BinaryInteger, TernaryFloat, TernaryNumber, UnaryFloatVector, BinaryFloatVector, TernaryFloatVector, Length, Distance, Dot, Cross, Refract, Matrix, MatrixScalar, Equality, Comparison, BoolBinary, BoolUnary, ReductionBool, Pack, Unpack, Barrier, Special }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Inclusive number-of-arguments constraints.
pub struct Arity { pub min:u8, pub max:u8 }
#[derive(Clone, Copy, Debug)]
/// Metadata: spellings of native calls (None means unavailable), signature, stage and capability.
pub struct IntrinsicInfo { pub op:Intrinsic, pub name:&'static str, pub family:Family, pub signature:Signature, pub arity:Arity, pub capability:Capability, pub stage:Option<Stage>, pub wgsl:Option<&'static str>, pub glsl:Option<&'static str>, pub hlsl:Option<&'static str>, pub intel_opencl:Option<&'static str> }
impl IntrinsicInfo {
    /// Return the target builtin spelling or `None` when unsupported.
    pub fn spelling(&self, backend:crate::Backend)->Option<&'static str> {match backend {crate::Backend::Wgsl=>self.wgsl,crate::Backend::Glsl=>self.glsl,crate::Backend::Hlsl=>self.hlsl,crate::Backend::IntelOpenCl=>self.intel_opencl,crate::Backend::NvidiaCuda=>crate::nvidia::spelling(self.op),crate::Backend::AmdHip=>crate::amd::spelling(self.op)}}
}
/// Operation identifier. Some variants are catalog-only and deliberately cannot be emitted.
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intrinsic {
    /// Abs: math operation; 1–1 arguments; capability Core; stage Any.
    Abs,
    /// Sign: math operation; 1–1 arguments; capability Core; stage Any.
    Sign,
    /// Sin: math operation; 1–1 arguments; capability Core; stage Any.
    Sin,
    /// Cos: math operation; 1–1 arguments; capability Core; stage Any.
    Cos,
    /// Tan: math operation; 1–1 arguments; capability Core; stage Any.
    Tan,
    /// Asin: math operation; 1–1 arguments; capability Core; stage Any.
    Asin,
    /// Acos: math operation; 1–1 arguments; capability Core; stage Any.
    Acos,
    /// Atan: math operation; 1–1 arguments; capability Core; stage Any.
    Atan,
    /// Sinh: math operation; 1–1 arguments; capability Core; stage Any.
    Sinh,
    /// Cosh: math operation; 1–1 arguments; capability Core; stage Any.
    Cosh,
    /// Tanh: math operation; 1–1 arguments; capability Core; stage Any.
    Tanh,
    /// Asinh: math operation; 1–1 arguments; capability Core; stage Any.
    Asinh,
    /// Acosh: math operation; 1–1 arguments; capability Core; stage Any.
    Acosh,
    /// Atanh: math operation; 1–1 arguments; capability Core; stage Any.
    Atanh,
    /// Exp: math operation; 1–1 arguments; capability Core; stage Any.
    Exp,
    /// Exp2: math operation; 1–1 arguments; capability Core; stage Any.
    Exp2,
    /// Log: math operation; 1–1 arguments; capability Core; stage Any.
    Log,
    /// Log2: math operation; 1–1 arguments; capability Core; stage Any.
    Log2,
    /// Sqrt: math operation; 1–1 arguments; capability Core; stage Any.
    Sqrt,
    /// InverseSqrt: math operation; 1–1 arguments; capability Core; stage Any.
    InverseSqrt,
    /// Floor: math operation; 1–1 arguments; capability Core; stage Any.
    Floor,
    /// Ceil: math operation; 1–1 arguments; capability Core; stage Any.
    Ceil,
    /// Trunc: math operation; 1–1 arguments; capability Core; stage Any.
    Trunc,
    /// Round: math operation; 1–1 arguments; capability Core; stage Any.
    Round,
    /// Fract: math operation; 1–1 arguments; capability Core; stage Any.
    Fract,
    /// Degrees: math operation; 1–1 arguments; capability Core; stage Any.
    Degrees,
    /// Radians: math operation; 1–1 arguments; capability Core; stage Any.
    Radians,
    /// Pow: math operation; 2–2 arguments; capability Core; stage Any.
    Pow,
    /// Atan2: math operation; 2–2 arguments; capability Core; stage Any.
    Atan2,
    /// Min: math operation; 2–2 arguments; capability Core; stage Any.
    Min,
    /// Max: math operation; 2–2 arguments; capability Core; stage Any.
    Max,
    /// Clamp: math operation; 3–3 arguments; capability Core; stage Any.
    Clamp,
    /// Fma: math operation; 3–3 arguments; capability Core; stage Any.
    Fma,
    /// Saturate: math operation; 1–1 arguments; capability Core; stage Any.
    Saturate,
    /// Mix: math operation; 3–3 arguments; capability Core; stage Any.
    Mix,
    /// Step: math operation; 2–2 arguments; capability Core; stage Any.
    Step,
    /// Smoothstep: math operation; 3–3 arguments; capability Core; stage Any.
    Smoothstep,
    /// Modf: math operation; 1–1 arguments; capability Core; stage Any.
    Modf,
    /// Frexp: math operation; 1–1 arguments; capability Core; stage Any.
    Frexp,
    /// Ldexp: math operation; 2–2 arguments; capability Core; stage Any.
    Ldexp,
    /// QuantizeToF16: math operation; 1–1 arguments; capability F16; stage Any.
    QuantizeToF16,
    /// Length: geometry operation; 1–1 arguments; capability Core; stage Any.
    Length,
    /// Distance: geometry operation; 2–2 arguments; capability Core; stage Any.
    Distance,
    /// Dot: geometry operation; 2–2 arguments; capability Core; stage Any.
    Dot,
    /// Cross: geometry operation; 2–2 arguments; capability Core; stage Any.
    Cross,
    /// Normalize: geometry operation; 1–1 arguments; capability Core; stage Any.
    Normalize,
    /// Reflect: geometry operation; 2–2 arguments; capability Core; stage Any.
    Reflect,
    /// Refract: geometry operation; 3–3 arguments; capability Core; stage Any.
    Refract,
    /// FaceForward: geometry operation; 3–3 arguments; capability Core; stage Any.
    FaceForward,
    /// Transpose: matrix operation; 1–1 arguments; capability Core; stage Any.
    Transpose,
    /// Determinant: matrix operation; 1–1 arguments; capability Core; stage Any.
    Determinant,
    /// Inverse: matrix operation; 1–1 arguments; capability Core; stage Any.
    Inverse,
    /// Add: operator operation; 2–2 arguments; capability Core; stage Any.
    Add,
    /// Sub: operator operation; 2–2 arguments; capability Core; stage Any.
    Sub,
    /// Multiply: operator operation; 2–2 arguments; capability Core; stage Any.
    Multiply,
    /// Divide: operator operation; 2–2 arguments; capability Core; stage Any.
    Divide,
    /// Modulo: operator operation; 2–2 arguments; capability Core; stage Any.
    Modulo,
    /// Negate: operator operation; 1–1 arguments; capability Core; stage Any.
    Negate,
    /// Equal: operator operation; 2–2 arguments; capability Core; stage Any.
    Equal,
    /// NotEqual: operator operation; 2–2 arguments; capability Core; stage Any.
    NotEqual,
    /// Less: operator operation; 2–2 arguments; capability Core; stage Any.
    Less,
    /// LessEqual: operator operation; 2–2 arguments; capability Core; stage Any.
    LessEqual,
    /// Greater: operator operation; 2–2 arguments; capability Core; stage Any.
    Greater,
    /// GreaterEqual: operator operation; 2–2 arguments; capability Core; stage Any.
    GreaterEqual,
    /// LogicalAnd: operator operation; 2–2 arguments; capability Core; stage Any.
    LogicalAnd,
    /// LogicalOr: operator operation; 2–2 arguments; capability Core; stage Any.
    LogicalOr,
    /// LogicalNot: operator operation; 1–1 arguments; capability Core; stage Any.
    LogicalNot,
    /// BitAnd: operator operation; 2–2 arguments; capability Core; stage Any.
    BitAnd,
    /// BitOr: operator operation; 2–2 arguments; capability Core; stage Any.
    BitOr,
    /// BitXor: operator operation; 2–2 arguments; capability Core; stage Any.
    BitXor,
    /// ShiftLeft: operator operation; 2–2 arguments; capability Core; stage Any.
    ShiftLeft,
    /// ShiftRight: operator operation; 2–2 arguments; capability Core; stage Any.
    ShiftRight,
    /// BitNot: operator operation; 1–1 arguments; capability Core; stage Any.
    BitNot,
    /// Select: selection operation; 3–3 arguments; capability Core; stage Any.
    Select,
    /// All: comparison operation; 1–1 arguments; capability Core; stage Any.
    All,
    /// Any: comparison operation; 1–1 arguments; capability Core; stage Any.
    Any,
    /// CountOneBits: bits operation; 1–1 arguments; capability Core; stage Any.
    CountOneBits,
    /// ReverseBits: bits operation; 1–1 arguments; capability Core; stage Any.
    ReverseBits,
    /// FirstLeadingBit: bits operation; 1–1 arguments; capability Core; stage Any.
    FirstLeadingBit,
    /// FirstTrailingBit: bits operation; 1–1 arguments; capability Core; stage Any.
    FirstTrailingBit,
    /// CountLeadingZeros: bits operation; 1–1 arguments; capability Core; stage Any.
    CountLeadingZeros,
    /// CountTrailingZeros: bits operation; 1–1 arguments; capability Core; stage Any.
    CountTrailingZeros,
    /// FindLsb: bits operation; 1–1 arguments; capability Core; stage Any.
    FindLsb,
    /// FindMsb: bits operation; 1–1 arguments; capability Core; stage Any.
    FindMsb,
    /// ExtractBits: bits operation; 3–3 arguments; capability Core; stage Any.
    ExtractBits,
    /// InsertBits: bits operation; 4–4 arguments; capability Core; stage Any.
    InsertBits,
    /// RotateLeft: bits operation; 2–2 arguments; capability Core; stage Any.
    RotateLeft,
    /// RotateRight: bits operation; 2–2 arguments; capability Core; stage Any.
    RotateRight,
    /// BitfieldMask: bits operation; 2–2 arguments; capability Core; stage Any.
    BitfieldMask,
    /// BitfieldReverse: bits operation; 1–1 arguments; capability Core; stage Any.
    BitfieldReverse,
    /// BitfieldExtractSigned: bits operation; 3–4 arguments; capability Core; stage Any.
    BitfieldExtractSigned,
    /// BitfieldExtractUnsigned: bits operation; 3–4 arguments; capability Core; stage Any.
    BitfieldExtractUnsigned,
    /// BitfieldInsertSigned: bits operation; 3–4 arguments; capability Core; stage Any.
    BitfieldInsertSigned,
    /// BitfieldCount: bits operation; 1–1 arguments; capability Core; stage Any.
    BitfieldCount,
    /// FindFirstSet: bits operation; 1–1 arguments; capability Core; stage Any.
    FindFirstSet,
    /// FindLastSet: bits operation; 1–1 arguments; capability Core; stage Any.
    FindLastSet,
    /// Pack4x8Unorm: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack4x8Unorm,
    /// Pack4x8Snorm: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack4x8Snorm,
    /// Pack2x16Unorm: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack2x16Unorm,
    /// Pack2x16Snorm: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack2x16Snorm,
    /// Pack2x16Float: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack2x16Float,
    /// Pack4xI8: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack4xI8,
    /// Pack4xU8: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack4xU8,
    /// Pack4xI8Clamp: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack4xI8Clamp,
    /// Pack4xU8Clamp: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack4xU8Clamp,
    /// Pack4xI8Saturate: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack4xI8Saturate,
    /// Pack4xU8Saturate: packing operation; 1–1 arguments; capability Packing; stage Any.
    Pack4xU8Saturate,
    /// Unpack4x8Unorm: packing operation; 1–1 arguments; capability Packing; stage Any.
    Unpack4x8Unorm,
    /// Unpack4x8Snorm: packing operation; 1–1 arguments; capability Packing; stage Any.
    Unpack4x8Snorm,
    /// Unpack2x16Unorm: packing operation; 1–1 arguments; capability Packing; stage Any.
    Unpack2x16Unorm,
    /// Unpack2x16Snorm: packing operation; 1–1 arguments; capability Packing; stage Any.
    Unpack2x16Snorm,
    /// Unpack2x16Float: packing operation; 1–1 arguments; capability Packing; stage Any.
    Unpack2x16Float,
    /// Unpack4xI8: packing operation; 1–1 arguments; capability Packing; stage Any.
    Unpack4xI8,
    /// Unpack4xU8: packing operation; 1–1 arguments; capability Packing; stage Any.
    Unpack4xU8,
    /// PackSnorm2x16: packing operation; 1–1 arguments; capability Packing; stage Any.
    PackSnorm2x16,
    /// PackUnorm2x16: packing operation; 1–1 arguments; capability Packing; stage Any.
    PackUnorm2x16,
    /// PackHalf2x16: packing operation; 1–1 arguments; capability Packing; stage Any.
    PackHalf2x16,
    /// PackSnorm4x8: packing operation; 1–1 arguments; capability Packing; stage Any.
    PackSnorm4x8,
    /// PackUnorm4x8: packing operation; 1–1 arguments; capability Packing; stage Any.
    PackUnorm4x8,
    /// UnpackSnorm2x16: packing operation; 1–1 arguments; capability Packing; stage Any.
    UnpackSnorm2x16,
    /// UnpackUnorm2x16: packing operation; 1–1 arguments; capability Packing; stage Any.
    UnpackUnorm2x16,
    /// UnpackHalf2x16: packing operation; 1–1 arguments; capability Packing; stage Any.
    UnpackHalf2x16,
    /// UnpackSnorm4x8: packing operation; 1–1 arguments; capability Packing; stage Any.
    UnpackSnorm4x8,
    /// UnpackUnorm4x8: packing operation; 1–1 arguments; capability Packing; stage Any.
    UnpackUnorm4x8,
    /// Dpdx: derivative operation; 1–1 arguments; capability Derivatives; stage Fragment.
    Dpdx,
    /// Dpdy: derivative operation; 1–1 arguments; capability Derivatives; stage Fragment.
    Dpdy,
    /// Fwidth: derivative operation; 1–1 arguments; capability Derivatives; stage Fragment.
    Fwidth,
    /// DpdxFine: derivative operation; 1–1 arguments; capability Derivatives; stage Fragment.
    DpdxFine,
    /// DpdyFine: derivative operation; 1–1 arguments; capability Derivatives; stage Fragment.
    DpdyFine,
    /// FwidthFine: derivative operation; 1–1 arguments; capability Derivatives; stage Fragment.
    FwidthFine,
    /// DpdxCoarse: derivative operation; 1–1 arguments; capability Derivatives; stage Fragment.
    DpdxCoarse,
    /// DpdyCoarse: derivative operation; 1–1 arguments; capability Derivatives; stage Fragment.
    DpdyCoarse,
    /// FwidthCoarse: derivative operation; 1–1 arguments; capability Derivatives; stage Fragment.
    FwidthCoarse,
    /// InterpolateAtCentroid: derivative operation; 1–2 arguments; capability Derivatives; stage Fragment.
    InterpolateAtCentroid,
    /// InterpolateAtSample: derivative operation; 1–2 arguments; capability Derivatives; stage Fragment.
    InterpolateAtSample,
    /// InterpolateAtOffset: derivative operation; 1–2 arguments; capability Derivatives; stage Fragment.
    InterpolateAtOffset,
    /// TextureDimensions: texture operation; 1–2 arguments; capability Textures; stage Any.
    TextureDimensions,
    /// TextureNumLayers: texture operation; 1–1 arguments; capability Textures; stage Any.
    TextureNumLayers,
    /// TextureNumLevels: texture operation; 1–1 arguments; capability Textures; stage Any.
    TextureNumLevels,
    /// TextureNumSamples: texture operation; 1–1 arguments; capability Textures; stage Any.
    TextureNumSamples,
    /// TextureLoad: texture operation; 2–4 arguments; capability Textures; stage Any.
    TextureLoad,
    /// TextureStore: texture operation; 3–4 arguments; capability StorageTextures; stage Any.
    TextureStore,
    /// TextureSample: texture operation; 3–5 arguments; capability Textures; stage Fragment.
    TextureSample,
    /// TextureSampleBias: texture operation; 4–6 arguments; capability Textures; stage Fragment.
    TextureSampleBias,
    /// TextureSampleLevel: texture operation; 4–6 arguments; capability Textures; stage Any.
    TextureSampleLevel,
    /// TextureSampleGrad: texture operation; 5–7 arguments; capability Textures; stage Any.
    TextureSampleGrad,
    /// TextureSampleCompare: texture operation; 4–6 arguments; capability Textures; stage Fragment.
    TextureSampleCompare,
    /// TextureSampleCompareLevel: texture operation; 4–6 arguments; capability Textures; stage Any.
    TextureSampleCompareLevel,
    /// TextureGather: texture operation; 3–5 arguments; capability Textures; stage Fragment.
    TextureGather,
    /// TextureGatherCompare: texture operation; 4–6 arguments; capability Textures; stage Fragment.
    TextureGatherCompare,
    /// TextureSampleBaseClampToEdge: texture operation; 3–3 arguments; capability Textures; stage Fragment.
    TextureSampleBaseClampToEdge,
    /// TextureSampleCompareBias: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureSampleCompareBias,
    /// TextureSampleCompareGrad: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureSampleCompareGrad,
    /// TextureGatherRed: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureGatherRed,
    /// TextureGatherGreen: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureGatherGreen,
    /// TextureGatherBlue: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureGatherBlue,
    /// TextureGatherAlpha: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureGatherAlpha,
    /// TextureFetch: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureFetch,
    /// TextureFetchOffset: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureFetchOffset,
    /// TextureFetchLod: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureFetchLod,
    /// TextureFetchLodOffset: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureFetchLodOffset,
    /// TextureQueryLod: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureQueryLod,
    /// TextureQueryLevels: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureQueryLevels,
    /// TextureQuerySamples: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureQuerySamples,
    /// TextureSize: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureSize,
    /// TextureSamples: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureSamples,
    /// TextureOffset: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureOffset,
    /// TextureProj: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureProj,
    /// TextureProjLod: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureProjLod,
    /// TextureProjGrad: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureProjGrad,
    /// TextureGrad: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureGrad,
    /// TextureLod: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureLod,
    /// TextureGatherOffset: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureGatherOffset,
    /// TextureGatherOffsets: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureGatherOffsets,
    /// TextureGatherCompareOffset: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureGatherCompareOffset,
    /// TextureGatherCompareOffsets: texture operation; 1–6 arguments; capability Textures; stage Any.
    TextureGatherCompareOffsets,
    /// AtomicLoad: atomic operation; 1–1 arguments; capability Atomics; stage Compute.
    AtomicLoad,
    /// AtomicStore: atomic operation; 2–2 arguments; capability Atomics; stage Compute.
    AtomicStore,
    /// AtomicAdd: atomic operation; 2–2 arguments; capability Atomics; stage Compute.
    AtomicAdd,
    /// AtomicSub: atomic operation; 2–2 arguments; capability Atomics; stage Compute.
    AtomicSub,
    /// AtomicMax: atomic operation; 2–2 arguments; capability Atomics; stage Compute.
    AtomicMax,
    /// AtomicMin: atomic operation; 2–2 arguments; capability Atomics; stage Compute.
    AtomicMin,
    /// AtomicAnd: atomic operation; 2–2 arguments; capability Atomics; stage Compute.
    AtomicAnd,
    /// AtomicOr: atomic operation; 2–2 arguments; capability Atomics; stage Compute.
    AtomicOr,
    /// AtomicXor: atomic operation; 2–2 arguments; capability Atomics; stage Compute.
    AtomicXor,
    /// AtomicExchange: atomic operation; 2–2 arguments; capability Atomics; stage Compute.
    AtomicExchange,
    /// AtomicCompareExchangeWeak: atomic operation; 3–3 arguments; capability Atomics; stage Compute.
    AtomicCompareExchangeWeak,
    /// AtomicCompSwap: atomic operation; 1–4 arguments; capability Atomics; stage Compute.
    AtomicCompSwap,
    /// AtomicInc: atomic operation; 1–4 arguments; capability Atomics; stage Compute.
    AtomicInc,
    /// AtomicDec: atomic operation; 1–4 arguments; capability Atomics; stage Compute.
    AtomicDec,
    /// AtomicCounter: atomic operation; 1–4 arguments; capability Atomics; stage Compute.
    AtomicCounter,
    /// AtomicCounterIncrement: atomic operation; 1–4 arguments; capability Atomics; stage Compute.
    AtomicCounterIncrement,
    /// AtomicCounterDecrement: atomic operation; 1–4 arguments; capability Atomics; stage Compute.
    AtomicCounterDecrement,
    /// WorkgroupBarrier: barrier operation; 0–0 arguments; capability Workgroup; stage Compute.
    WorkgroupBarrier,
    /// StorageBarrier: barrier operation; 0–0 arguments; capability Workgroup; stage Compute.
    StorageBarrier,
    /// TextureBarrier: barrier operation; 0–0 arguments; capability Workgroup; stage Compute.
    TextureBarrier,
    /// Barrier: barrier operation; 0–0 arguments; capability Workgroup; stage Compute.
    Barrier,
    /// MemoryBarrier: barrier operation; 0–0 arguments; capability Workgroup; stage Compute.
    MemoryBarrier,
    /// MemoryBarrierBuffer: barrier operation; 0–0 arguments; capability Workgroup; stage Compute.
    MemoryBarrierBuffer,
    /// MemoryBarrierImage: barrier operation; 0–0 arguments; capability Workgroup; stage Compute.
    MemoryBarrierImage,
    /// GroupMemoryBarrier: barrier operation; 0–0 arguments; capability Workgroup; stage Compute.
    GroupMemoryBarrier,
    /// DeviceMemoryBarrier: barrier operation; 0–0 arguments; capability Workgroup; stage Compute.
    DeviceMemoryBarrier,
    /// AllMemoryBarrier: barrier operation; 0–0 arguments; capability Workgroup; stage Compute.
    AllMemoryBarrier,
    /// SubgroupSize: subgroup operation; 0–0 arguments; capability Subgroups; stage Any.
    SubgroupSize,
    /// SubgroupInvocationId: subgroup operation; 0–0 arguments; capability Subgroups; stage Any.
    SubgroupInvocationId,
    /// SubgroupElect: subgroup operation; 0–0 arguments; capability Subgroups; stage Any.
    SubgroupElect,
    /// SubgroupBroadcast: subgroup operation; 2–2 arguments; capability Subgroups; stage Any.
    SubgroupBroadcast,
    /// SubgroupBroadcastFirst: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupBroadcastFirst,
    /// SubgroupShuffle: subgroup operation; 0–2 arguments; capability IntelSubgroups; stage Any.
    SubgroupShuffle,
    /// SubgroupShuffleXor: subgroup operation; 0–2 arguments; capability IntelSubgroups; stage Any.
    SubgroupShuffleXor,
    /// SubgroupShuffleUp: subgroup operation; 3–3 arguments; capability IntelSubgroups; stage Any.
    SubgroupShuffleUp,
    /// SubgroupShuffleDown: subgroup operation; 3–3 arguments; capability IntelSubgroups; stage Any.
    SubgroupShuffleDown,
    /// SubgroupAll: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupAll,
    /// SubgroupAny: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupAny,
    /// SubgroupBallot: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupBallot,
    /// SubgroupAllEqual: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupAllEqual,
    /// SubgroupAdd: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupAdd,
    /// SubgroupMul: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupMul,
    /// SubgroupMin: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupMin,
    /// SubgroupMax: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupMax,
    /// SubgroupAnd: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupAnd,
    /// SubgroupOr: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupOr,
    /// SubgroupXor: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupXor,
    /// SubgroupInclusiveAdd: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupInclusiveAdd,
    /// SubgroupInclusiveMul: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupInclusiveMul,
    /// SubgroupInclusiveMin: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupInclusiveMin,
    /// SubgroupInclusiveMax: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupInclusiveMax,
    /// SubgroupExclusiveAdd: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupExclusiveAdd,
    /// SubgroupExclusiveMul: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupExclusiveMul,
    /// SubgroupExclusiveMin: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupExclusiveMin,
    /// SubgroupExclusiveMax: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupExclusiveMax,
    /// SubgroupQuadBroadcast: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupQuadBroadcast,
    /// SubgroupQuadSwapX: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupQuadSwapX,
    /// SubgroupQuadSwapY: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupQuadSwapY,
    /// SubgroupQuadSwapDiagonal: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupQuadSwapDiagonal,
    /// SubgroupClusteredAdd: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupClusteredAdd,
    /// SubgroupClusteredMul: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupClusteredMul,
    /// SubgroupClusteredMin: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupClusteredMin,
    /// SubgroupClusteredMax: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupClusteredMax,
    /// SubgroupPartitionedAdd: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupPartitionedAdd,
    /// SubgroupPartitionedInclusiveAdd: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupPartitionedInclusiveAdd,
    /// SubgroupPartitionedExclusiveAdd: subgroup operation; 0–2 arguments; capability Subgroups; stage Any.
    SubgroupPartitionedExclusiveAdd,
    /// Discard: utility operation; 1–4 arguments; capability Core; stage Fragment.
    Discard,
    /// Clip: utility operation; 1–1 arguments; capability Core; stage Fragment.
    Clip,
    /// SinCos: utility operation; 1–4 arguments; capability Core; stage Any.
    SinCos,
    /// MsAd4: vendor operation; 1–4 arguments; capability Vendor; stage Any.
    MsAd4,
    /// Dot4AddI8Packed: vendor operation; 3–3 arguments; capability Vendor; stage Any.
    Dot4AddI8Packed,
    /// Dot4AddU8Packed: vendor operation; 3–3 arguments; capability Vendor; stage Any.
    Dot4AddU8Packed,
    /// NonUniformIndex: utility operation; 1–1 arguments; capability Core; stage Any.
    NonUniformIndex,
    /// AsFloat: utility operation; 1–1 arguments; capability Core; stage Any.
    AsFloat,
    /// AsUInt: utility operation; 1–1 arguments; capability Core; stage Any.
    AsUInt,
    /// AsInt: utility operation; 1–1 arguments; capability Core; stage Any.
    AsInt,
    /// IsNan: utility operation; 1–1 arguments; capability Core; stage Any.
    IsNan,
    /// IsInf: utility operation; 1–1 arguments; capability Core; stage Any.
    IsInf,
    /// IsFinite: utility operation; 1–1 arguments; capability Core; stage Any.
    IsFinite,
    /// IntelSubGroupBlockRead: vendor operation; 1–1 arguments; capability IntelSubgroups; stage Compute.
    IntelSubGroupBlockRead,
    /// IntelSubGroupBlockRead2: vendor operation; 1–1 arguments; capability IntelSubgroups; stage Compute.
    IntelSubGroupBlockRead2,
    /// IntelSubGroupBlockRead4: vendor operation; 1–1 arguments; capability IntelSubgroups; stage Compute.
    IntelSubGroupBlockRead4,
    /// IntelSubGroupBlockRead8: vendor operation; 1–1 arguments; capability IntelSubgroups; stage Compute.
    IntelSubGroupBlockRead8,
    /// IntelSubGroupBlockWrite: vendor operation; 2–2 arguments; capability IntelSubgroups; stage Compute.
    IntelSubGroupBlockWrite,
    /// IntelSubGroupBlockWrite2: vendor operation; 2–2 arguments; capability IntelSubgroups; stage Compute.
    IntelSubGroupBlockWrite2,
    /// IntelSubGroupBlockWrite4: vendor operation; 2–2 arguments; capability IntelSubgroups; stage Compute.
    IntelSubGroupBlockWrite4,
    /// IntelSubGroupBlockWrite8: vendor operation; 2–2 arguments; capability IntelSubgroups; stage Compute.
    IntelSubGroupBlockWrite8,
    /// IntelSubGroupBlockReadUs: vendor operation; 1–1 arguments; capability IntelSubgroupsShort; stage Compute.
    IntelSubGroupBlockReadUs,
    /// IntelSubGroupBlockReadUs2: vendor operation; 1–1 arguments; capability IntelSubgroupsShort; stage Compute.
    IntelSubGroupBlockReadUs2,
    /// IntelSubGroupBlockReadUs4: vendor operation; 1–1 arguments; capability IntelSubgroupsShort; stage Compute.
    IntelSubGroupBlockReadUs4,
    /// IntelSubGroupBlockReadUs8: vendor operation; 1–1 arguments; capability IntelSubgroupsShort; stage Compute.
    IntelSubGroupBlockReadUs8,
    /// IntelSubGroupBlockWriteUs: vendor operation; 2–2 arguments; capability IntelSubgroupsShort; stage Compute.
    IntelSubGroupBlockWriteUs,
    /// IntelSubGroupBlockWriteUs2: vendor operation; 2–2 arguments; capability IntelSubgroupsShort; stage Compute.
    IntelSubGroupBlockWriteUs2,
    /// IntelSubGroupBlockWriteUs4: vendor operation; 2–2 arguments; capability IntelSubgroupsShort; stage Compute.
    IntelSubGroupBlockWriteUs4,
    /// IntelSubGroupBlockWriteUs8: vendor operation; 2–2 arguments; capability IntelSubgroupsShort; stage Compute.
    IntelSubGroupBlockWriteUs8,
    /// NvidiaActiveMask: vendor operation; 0–0 arguments; capability NvidiaWarp; stage Compute.
    NvidiaActiveMask,
    /// NvidiaBallotSync: vendor operation; 2–2 arguments; capability NvidiaWarp; stage Compute.
    NvidiaBallotSync,
    /// NvidiaAnySync: vendor operation; 2–2 arguments; capability NvidiaWarp; stage Compute.
    NvidiaAnySync,
    /// NvidiaAllSync: vendor operation; 2–2 arguments; capability NvidiaWarp; stage Compute.
    NvidiaAllSync,
    /// NvidiaShuffleSync: vendor operation; 3–4 arguments; capability NvidiaWarp; stage Compute.
    NvidiaShuffleSync,
    /// NvidiaShuffleUpSync: vendor operation; 3–4 arguments; capability NvidiaWarp; stage Compute.
    NvidiaShuffleUpSync,
    /// NvidiaShuffleDownSync: vendor operation; 3–4 arguments; capability NvidiaWarp; stage Compute.
    NvidiaShuffleDownSync,
    /// NvidiaShuffleXorSync: vendor operation; 3–4 arguments; capability NvidiaWarp; stage Compute.
    NvidiaShuffleXorSync,
    /// NvidiaSyncWarp: vendor operation; 1–1 arguments; capability NvidiaWarp; stage Compute.
    NvidiaSyncWarp,
    /// NvidiaMatchAnySync: vendor operation; 2–2 arguments; capability NvidiaWarp; stage Compute.
    NvidiaMatchAnySync,
    /// NvidiaReduceAddSync: vendor operation; 2–2 arguments; capability NvidiaSm80; stage Compute.
    NvidiaReduceAddSync,
    /// NvidiaReduceMinSync: vendor operation; 2–2 arguments; capability NvidiaSm80; stage Compute.
    NvidiaReduceMinSync,
    /// NvidiaReduceMaxSync: vendor operation; 2–2 arguments; capability NvidiaSm80; stage Compute.
    NvidiaReduceMaxSync,
    /// NvidiaReduceAndSync: vendor operation; 2–2 arguments; capability NvidiaSm80; stage Compute.
    NvidiaReduceAndSync,
    /// NvidiaReduceOrSync: vendor operation; 2–2 arguments; capability NvidiaSm80; stage Compute.
    NvidiaReduceOrSync,
    /// NvidiaReduceXorSync: vendor operation; 2–2 arguments; capability NvidiaSm80; stage Compute.
    NvidiaReduceXorSync,
    /// AmdWaveSize: vendor operation; 0–0 arguments; capability AmdWave; stage Compute.
    AmdWaveSize,
    /// AmdLaneId: vendor operation; 0–0 arguments; capability AmdWave; stage Compute.
    AmdLaneId,
    /// AmdShuffle: vendor operation; 2–2 arguments; capability AmdWave; stage Compute.
    AmdShuffle,
    /// AmdShuffleXor: vendor operation; 2–2 arguments; capability AmdWave; stage Compute.
    AmdShuffleXor,
    /// AmdShuffleUp: vendor operation; 2–2 arguments; capability AmdWave; stage Compute.
    AmdShuffleUp,
    /// AmdShuffleDown: vendor operation; 2–2 arguments; capability AmdWave; stage Compute.
    AmdShuffleDown,
    /// AmdBallot: vendor operation; 1–1 arguments; capability AmdWave; stage Compute.
    AmdBallot,
    /// AmdActiveMask: vendor operation; 0–0 arguments; capability AmdWave; stage Compute.
    AmdActiveMask,
    /// AmdAny: vendor operation; 1–1 arguments; capability AmdWave; stage Compute.
    AmdAny,
    /// AmdAll: vendor operation; 1–1 arguments; capability AmdWave; stage Compute.
    AmdAll,
    /// AmdWaveSync: vendor operation; 0–0 arguments; capability AmdWave; stage Compute.
    AmdWaveSync,
    /// AmdReadFirstLane: vendor operation; 1–1 arguments; capability AmdGcn; stage Compute.
    AmdReadFirstLane,
    /// AmdReadLane: vendor operation; 2–2 arguments; capability AmdGcn; stage Compute.
    AmdReadLane,
    /// AmdWriteLane: vendor operation; 3–3 arguments; capability AmdGcn; stage Compute.
    AmdWriteLane,
    /// AmdDsBpermute: vendor operation; 2–2 arguments; capability AmdGcn; stage Compute.
    AmdDsBpermute,
    /// AmdDsPermute: vendor operation; 2–2 arguments; capability AmdGcn; stage Compute.
    AmdDsPermute,
    /// AmdFastSin: vendor operation; 1–1 arguments; capability Core; stage Compute.
    AmdFastSin,
    /// AmdFastCos: vendor operation; 1–1 arguments; capability Core; stage Compute.
    AmdFastCos,
    /// AmdFastExp: vendor operation; 1–1 arguments; capability Core; stage Compute.
    AmdFastExp,
    /// AmdFastLog: vendor operation; 1–1 arguments; capability Core; stage Compute.
    AmdFastLog,
    /// AmdFastLog2: vendor operation; 1–1 arguments; capability Core; stage Compute.
    AmdFastLog2,
    /// AmdFastPow: vendor operation; 2–2 arguments; capability Core; stage Compute.
    AmdFastPow,
    /// AmdFastRcp: vendor operation; 1–1 arguments; capability Core; stage Compute.
    AmdFastRcp,
    /// AmdBytePerm: vendor operation; 3–3 arguments; capability Core; stage Compute.
    AmdBytePerm,
    /// AmdFunnelShiftLeft: vendor operation; 3–3 arguments; capability Core; stage Compute.
    AmdFunnelShiftLeft,
    /// AmdFunnelShiftRight: vendor operation; 3–3 arguments; capability Core; stage Compute.
    AmdFunnelShiftRight,
    /// AmdMul24: vendor operation; 2–2 arguments; capability Core; stage Compute.
    AmdMul24,
    /// AmdUMul24: vendor operation; 2–2 arguments; capability Core; stage Compute.
    AmdUMul24,
    /// AmdSad: vendor operation; 3–3 arguments; capability Core; stage Compute.
    AmdSad,
    /// AmdUSad: vendor operation; 3–3 arguments; capability Core; stage Compute.
    AmdUSad,
    /// AmdPopcount64: vendor operation; 1–1 arguments; capability Core; stage Compute.
    AmdPopcount64,
    /// AmdReverseBits64: vendor operation; 1–1 arguments; capability Core; stage Compute.
    AmdReverseBits64,
    /// AmdThreadFenceBlock: vendor operation; 0–0 arguments; capability Core; stage Compute.
    AmdThreadFenceBlock,
    /// AmdThreadFenceDevice: vendor operation; 0–0 arguments; capability Core; stage Compute.
    AmdThreadFenceDevice,
    /// AmdThreadFenceSystem: vendor operation; 0–0 arguments; capability Core; stage Compute.
    AmdThreadFenceSystem,
}
pub static ALL_INTRINSICS:&[Intrinsic]=&[
    Intrinsic::Abs,
    Intrinsic::Sign,
    Intrinsic::Sin,
    Intrinsic::Cos,
    Intrinsic::Tan,
    Intrinsic::Asin,
    Intrinsic::Acos,
    Intrinsic::Atan,
    Intrinsic::Sinh,
    Intrinsic::Cosh,
    Intrinsic::Tanh,
    Intrinsic::Asinh,
    Intrinsic::Acosh,
    Intrinsic::Atanh,
    Intrinsic::Exp,
    Intrinsic::Exp2,
    Intrinsic::Log,
    Intrinsic::Log2,
    Intrinsic::Sqrt,
    Intrinsic::InverseSqrt,
    Intrinsic::Floor,
    Intrinsic::Ceil,
    Intrinsic::Trunc,
    Intrinsic::Round,
    Intrinsic::Fract,
    Intrinsic::Degrees,
    Intrinsic::Radians,
    Intrinsic::Pow,
    Intrinsic::Atan2,
    Intrinsic::Min,
    Intrinsic::Max,
    Intrinsic::Clamp,
    Intrinsic::Fma,
    Intrinsic::Saturate,
    Intrinsic::Mix,
    Intrinsic::Step,
    Intrinsic::Smoothstep,
    Intrinsic::Modf,
    Intrinsic::Frexp,
    Intrinsic::Ldexp,
    Intrinsic::QuantizeToF16,
    Intrinsic::Length,
    Intrinsic::Distance,
    Intrinsic::Dot,
    Intrinsic::Cross,
    Intrinsic::Normalize,
    Intrinsic::Reflect,
    Intrinsic::Refract,
    Intrinsic::FaceForward,
    Intrinsic::Transpose,
    Intrinsic::Determinant,
    Intrinsic::Inverse,
    Intrinsic::Add,
    Intrinsic::Sub,
    Intrinsic::Multiply,
    Intrinsic::Divide,
    Intrinsic::Modulo,
    Intrinsic::Negate,
    Intrinsic::Equal,
    Intrinsic::NotEqual,
    Intrinsic::Less,
    Intrinsic::LessEqual,
    Intrinsic::Greater,
    Intrinsic::GreaterEqual,
    Intrinsic::LogicalAnd,
    Intrinsic::LogicalOr,
    Intrinsic::LogicalNot,
    Intrinsic::BitAnd,
    Intrinsic::BitOr,
    Intrinsic::BitXor,
    Intrinsic::ShiftLeft,
    Intrinsic::ShiftRight,
    Intrinsic::BitNot,
    Intrinsic::Select,
    Intrinsic::All,
    Intrinsic::Any,
    Intrinsic::CountOneBits,
    Intrinsic::ReverseBits,
    Intrinsic::FirstLeadingBit,
    Intrinsic::FirstTrailingBit,
    Intrinsic::CountLeadingZeros,
    Intrinsic::CountTrailingZeros,
    Intrinsic::FindLsb,
    Intrinsic::FindMsb,
    Intrinsic::ExtractBits,
    Intrinsic::InsertBits,
    Intrinsic::RotateLeft,
    Intrinsic::RotateRight,
    Intrinsic::BitfieldMask,
    Intrinsic::BitfieldReverse,
    Intrinsic::BitfieldExtractSigned,
    Intrinsic::BitfieldExtractUnsigned,
    Intrinsic::BitfieldInsertSigned,
    Intrinsic::BitfieldCount,
    Intrinsic::FindFirstSet,
    Intrinsic::FindLastSet,
    Intrinsic::Pack4x8Unorm,
    Intrinsic::Pack4x8Snorm,
    Intrinsic::Pack2x16Unorm,
    Intrinsic::Pack2x16Snorm,
    Intrinsic::Pack2x16Float,
    Intrinsic::Pack4xI8,
    Intrinsic::Pack4xU8,
    Intrinsic::Pack4xI8Clamp,
    Intrinsic::Pack4xU8Clamp,
    Intrinsic::Pack4xI8Saturate,
    Intrinsic::Pack4xU8Saturate,
    Intrinsic::Unpack4x8Unorm,
    Intrinsic::Unpack4x8Snorm,
    Intrinsic::Unpack2x16Unorm,
    Intrinsic::Unpack2x16Snorm,
    Intrinsic::Unpack2x16Float,
    Intrinsic::Unpack4xI8,
    Intrinsic::Unpack4xU8,
    Intrinsic::PackSnorm2x16,
    Intrinsic::PackUnorm2x16,
    Intrinsic::PackHalf2x16,
    Intrinsic::PackSnorm4x8,
    Intrinsic::PackUnorm4x8,
    Intrinsic::UnpackSnorm2x16,
    Intrinsic::UnpackUnorm2x16,
    Intrinsic::UnpackHalf2x16,
    Intrinsic::UnpackSnorm4x8,
    Intrinsic::UnpackUnorm4x8,
    Intrinsic::Dpdx,
    Intrinsic::Dpdy,
    Intrinsic::Fwidth,
    Intrinsic::DpdxFine,
    Intrinsic::DpdyFine,
    Intrinsic::FwidthFine,
    Intrinsic::DpdxCoarse,
    Intrinsic::DpdyCoarse,
    Intrinsic::FwidthCoarse,
    Intrinsic::InterpolateAtCentroid,
    Intrinsic::InterpolateAtSample,
    Intrinsic::InterpolateAtOffset,
    Intrinsic::TextureDimensions,
    Intrinsic::TextureNumLayers,
    Intrinsic::TextureNumLevels,
    Intrinsic::TextureNumSamples,
    Intrinsic::TextureLoad,
    Intrinsic::TextureStore,
    Intrinsic::TextureSample,
    Intrinsic::TextureSampleBias,
    Intrinsic::TextureSampleLevel,
    Intrinsic::TextureSampleGrad,
    Intrinsic::TextureSampleCompare,
    Intrinsic::TextureSampleCompareLevel,
    Intrinsic::TextureGather,
    Intrinsic::TextureGatherCompare,
    Intrinsic::TextureSampleBaseClampToEdge,
    Intrinsic::TextureSampleCompareBias,
    Intrinsic::TextureSampleCompareGrad,
    Intrinsic::TextureGatherRed,
    Intrinsic::TextureGatherGreen,
    Intrinsic::TextureGatherBlue,
    Intrinsic::TextureGatherAlpha,
    Intrinsic::TextureFetch,
    Intrinsic::TextureFetchOffset,
    Intrinsic::TextureFetchLod,
    Intrinsic::TextureFetchLodOffset,
    Intrinsic::TextureQueryLod,
    Intrinsic::TextureQueryLevels,
    Intrinsic::TextureQuerySamples,
    Intrinsic::TextureSize,
    Intrinsic::TextureSamples,
    Intrinsic::TextureOffset,
    Intrinsic::TextureProj,
    Intrinsic::TextureProjLod,
    Intrinsic::TextureProjGrad,
    Intrinsic::TextureGrad,
    Intrinsic::TextureLod,
    Intrinsic::TextureGatherOffset,
    Intrinsic::TextureGatherOffsets,
    Intrinsic::TextureGatherCompareOffset,
    Intrinsic::TextureGatherCompareOffsets,
    Intrinsic::AtomicLoad,
    Intrinsic::AtomicStore,
    Intrinsic::AtomicAdd,
    Intrinsic::AtomicSub,
    Intrinsic::AtomicMax,
    Intrinsic::AtomicMin,
    Intrinsic::AtomicAnd,
    Intrinsic::AtomicOr,
    Intrinsic::AtomicXor,
    Intrinsic::AtomicExchange,
    Intrinsic::AtomicCompareExchangeWeak,
    Intrinsic::AtomicCompSwap,
    Intrinsic::AtomicInc,
    Intrinsic::AtomicDec,
    Intrinsic::AtomicCounter,
    Intrinsic::AtomicCounterIncrement,
    Intrinsic::AtomicCounterDecrement,
    Intrinsic::WorkgroupBarrier,
    Intrinsic::StorageBarrier,
    Intrinsic::TextureBarrier,
    Intrinsic::Barrier,
    Intrinsic::MemoryBarrier,
    Intrinsic::MemoryBarrierBuffer,
    Intrinsic::MemoryBarrierImage,
    Intrinsic::GroupMemoryBarrier,
    Intrinsic::DeviceMemoryBarrier,
    Intrinsic::AllMemoryBarrier,
    Intrinsic::SubgroupSize,
    Intrinsic::SubgroupInvocationId,
    Intrinsic::SubgroupElect,
    Intrinsic::SubgroupBroadcast,
    Intrinsic::SubgroupBroadcastFirst,
    Intrinsic::SubgroupShuffle,
    Intrinsic::SubgroupShuffleXor,
    Intrinsic::SubgroupShuffleUp,
    Intrinsic::SubgroupShuffleDown,
    Intrinsic::SubgroupAll,
    Intrinsic::SubgroupAny,
    Intrinsic::SubgroupBallot,
    Intrinsic::SubgroupAllEqual,
    Intrinsic::SubgroupAdd,
    Intrinsic::SubgroupMul,
    Intrinsic::SubgroupMin,
    Intrinsic::SubgroupMax,
    Intrinsic::SubgroupAnd,
    Intrinsic::SubgroupOr,
    Intrinsic::SubgroupXor,
    Intrinsic::SubgroupInclusiveAdd,
    Intrinsic::SubgroupInclusiveMul,
    Intrinsic::SubgroupInclusiveMin,
    Intrinsic::SubgroupInclusiveMax,
    Intrinsic::SubgroupExclusiveAdd,
    Intrinsic::SubgroupExclusiveMul,
    Intrinsic::SubgroupExclusiveMin,
    Intrinsic::SubgroupExclusiveMax,
    Intrinsic::SubgroupQuadBroadcast,
    Intrinsic::SubgroupQuadSwapX,
    Intrinsic::SubgroupQuadSwapY,
    Intrinsic::SubgroupQuadSwapDiagonal,
    Intrinsic::SubgroupClusteredAdd,
    Intrinsic::SubgroupClusteredMul,
    Intrinsic::SubgroupClusteredMin,
    Intrinsic::SubgroupClusteredMax,
    Intrinsic::SubgroupPartitionedAdd,
    Intrinsic::SubgroupPartitionedInclusiveAdd,
    Intrinsic::SubgroupPartitionedExclusiveAdd,
    Intrinsic::Discard,
    Intrinsic::Clip,
    Intrinsic::SinCos,
    Intrinsic::MsAd4,
    Intrinsic::Dot4AddI8Packed,
    Intrinsic::Dot4AddU8Packed,
    Intrinsic::NonUniformIndex,
    Intrinsic::AsFloat,
    Intrinsic::AsUInt,
    Intrinsic::AsInt,
    Intrinsic::IsNan,
    Intrinsic::IsInf,
    Intrinsic::IsFinite,
    Intrinsic::IntelSubGroupBlockRead,
    Intrinsic::IntelSubGroupBlockRead2,
    Intrinsic::IntelSubGroupBlockRead4,
    Intrinsic::IntelSubGroupBlockRead8,
    Intrinsic::IntelSubGroupBlockWrite,
    Intrinsic::IntelSubGroupBlockWrite2,
    Intrinsic::IntelSubGroupBlockWrite4,
    Intrinsic::IntelSubGroupBlockWrite8,
    Intrinsic::IntelSubGroupBlockReadUs,
    Intrinsic::IntelSubGroupBlockReadUs2,
    Intrinsic::IntelSubGroupBlockReadUs4,
    Intrinsic::IntelSubGroupBlockReadUs8,
    Intrinsic::IntelSubGroupBlockWriteUs,
    Intrinsic::IntelSubGroupBlockWriteUs2,
    Intrinsic::IntelSubGroupBlockWriteUs4,
    Intrinsic::IntelSubGroupBlockWriteUs8,
    Intrinsic::NvidiaActiveMask,
    Intrinsic::NvidiaBallotSync,
    Intrinsic::NvidiaAnySync,
    Intrinsic::NvidiaAllSync,
    Intrinsic::NvidiaShuffleSync,
    Intrinsic::NvidiaShuffleUpSync,
    Intrinsic::NvidiaShuffleDownSync,
    Intrinsic::NvidiaShuffleXorSync,
    Intrinsic::NvidiaSyncWarp,
    Intrinsic::NvidiaMatchAnySync,
    Intrinsic::NvidiaReduceAddSync,
    Intrinsic::NvidiaReduceMinSync,
    Intrinsic::NvidiaReduceMaxSync,
    Intrinsic::NvidiaReduceAndSync,
    Intrinsic::NvidiaReduceOrSync,
    Intrinsic::NvidiaReduceXorSync,
    Intrinsic::AmdWaveSize,
    Intrinsic::AmdLaneId,
    Intrinsic::AmdShuffle,
    Intrinsic::AmdShuffleXor,
    Intrinsic::AmdShuffleUp,
    Intrinsic::AmdShuffleDown,
    Intrinsic::AmdBallot,
    Intrinsic::AmdActiveMask,
    Intrinsic::AmdAny,
    Intrinsic::AmdAll,
    Intrinsic::AmdWaveSync,
    Intrinsic::AmdReadFirstLane,
    Intrinsic::AmdReadLane,
    Intrinsic::AmdWriteLane,
    Intrinsic::AmdDsBpermute,
    Intrinsic::AmdDsPermute,
    Intrinsic::AmdFastSin,
    Intrinsic::AmdFastCos,
    Intrinsic::AmdFastExp,
    Intrinsic::AmdFastLog,
    Intrinsic::AmdFastLog2,
    Intrinsic::AmdFastPow,
    Intrinsic::AmdFastRcp,
    Intrinsic::AmdBytePerm,
    Intrinsic::AmdFunnelShiftLeft,
    Intrinsic::AmdFunnelShiftRight,
    Intrinsic::AmdMul24,
    Intrinsic::AmdUMul24,
    Intrinsic::AmdSad,
    Intrinsic::AmdUSad,
    Intrinsic::AmdPopcount64,
    Intrinsic::AmdReverseBits64,
    Intrinsic::AmdThreadFenceBlock,
    Intrinsic::AmdThreadFenceDevice,
    Intrinsic::AmdThreadFenceSystem,

    ];
/// Number of catalogued shader operations.
pub const INTRINSIC_COUNT:usize=ALL_INTRINSICS.len();
static INFOS:&[IntrinsicInfo]=&[
    IntrinsicInfo {op:Intrinsic::Abs, name:"abs",family:Family::Math,signature:Signature::UnaryNumber,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("abs"),glsl:Some("abs"),hlsl:Some("abs"),intel_opencl:Some("fabs")},
    IntrinsicInfo {op:Intrinsic::Sign, name:"sign",family:Family::Math,signature:Signature::UnaryNumber,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("sign"),glsl:Some("sign"),hlsl:Some("sign"),intel_opencl:Some("sign")},
    IntrinsicInfo {op:Intrinsic::Sin, name:"sin",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("sin"),glsl:Some("sin"),hlsl:Some("sin"),intel_opencl:Some("sin")},
    IntrinsicInfo {op:Intrinsic::Cos, name:"cos",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("cos"),glsl:Some("cos"),hlsl:Some("cos"),intel_opencl:Some("cos")},
    IntrinsicInfo {op:Intrinsic::Tan, name:"tan",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("tan"),glsl:Some("tan"),hlsl:Some("tan"),intel_opencl:Some("tan")},
    IntrinsicInfo {op:Intrinsic::Asin, name:"asin",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("asin"),glsl:Some("asin"),hlsl:Some("asin"),intel_opencl:Some("asin")},
    IntrinsicInfo {op:Intrinsic::Acos, name:"acos",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("acos"),glsl:Some("acos"),hlsl:Some("acos"),intel_opencl:Some("acos")},
    IntrinsicInfo {op:Intrinsic::Atan, name:"atan",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("atan"),glsl:Some("atan"),hlsl:Some("atan"),intel_opencl:Some("atan")},
    IntrinsicInfo {op:Intrinsic::Sinh, name:"sinh",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("sinh"),glsl:Some("sinh"),hlsl:Some("sinh"),intel_opencl:Some("sinh")},
    IntrinsicInfo {op:Intrinsic::Cosh, name:"cosh",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("cosh"),glsl:Some("cosh"),hlsl:Some("cosh"),intel_opencl:Some("cosh")},
    IntrinsicInfo {op:Intrinsic::Tanh, name:"tanh",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("tanh"),glsl:Some("tanh"),hlsl:Some("tanh"),intel_opencl:Some("tanh")},
    IntrinsicInfo {op:Intrinsic::Asinh, name:"asinh",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("asinh"),glsl:Some("asinh"),hlsl:Some("asinh"),intel_opencl:Some("asinh")},
    IntrinsicInfo {op:Intrinsic::Acosh, name:"acosh",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("acosh"),glsl:Some("acosh"),hlsl:Some("acosh"),intel_opencl:Some("acosh")},
    IntrinsicInfo {op:Intrinsic::Atanh, name:"atanh",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("atanh"),glsl:Some("atanh"),hlsl:Some("atanh"),intel_opencl:Some("atanh")},
    IntrinsicInfo {op:Intrinsic::Exp, name:"exp",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("exp"),glsl:Some("exp"),hlsl:Some("exp"),intel_opencl:Some("exp")},
    IntrinsicInfo {op:Intrinsic::Exp2, name:"exp2",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("exp2"),glsl:Some("exp2"),hlsl:Some("exp2"),intel_opencl:Some("exp2")},
    IntrinsicInfo {op:Intrinsic::Log, name:"log",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("log"),glsl:Some("log"),hlsl:Some("log"),intel_opencl:Some("log")},
    IntrinsicInfo {op:Intrinsic::Log2, name:"log2",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("log2"),glsl:Some("log2"),hlsl:Some("log2"),intel_opencl:Some("log2")},
    IntrinsicInfo {op:Intrinsic::Sqrt, name:"sqrt",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("sqrt"),glsl:Some("sqrt"),hlsl:Some("sqrt"),intel_opencl:Some("sqrt")},
    IntrinsicInfo {op:Intrinsic::InverseSqrt, name:"inverseSqrt",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("inverseSqrt"),glsl:Some("inversesqrt"),hlsl:Some("rsqrt"),intel_opencl:Some("rsqrt")},
    IntrinsicInfo {op:Intrinsic::Floor, name:"floor",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("floor"),glsl:Some("floor"),hlsl:Some("floor"),intel_opencl:Some("floor")},
    IntrinsicInfo {op:Intrinsic::Ceil, name:"ceil",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("ceil"),glsl:Some("ceil"),hlsl:Some("ceil"),intel_opencl:Some("ceil")},
    IntrinsicInfo {op:Intrinsic::Trunc, name:"trunc",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("trunc"),glsl:Some("trunc"),hlsl:Some("trunc"),intel_opencl:Some("trunc")},
    IntrinsicInfo {op:Intrinsic::Round, name:"round",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("round"),glsl:Some("round"),hlsl:Some("round"),intel_opencl:Some("round")},
    IntrinsicInfo {op:Intrinsic::Fract, name:"fract",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("fract"),glsl:Some("fract"),hlsl:Some("frac"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Degrees, name:"degrees",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("degrees"),glsl:Some("degrees"),hlsl:Some("degrees"),intel_opencl:Some("degrees")},
    IntrinsicInfo {op:Intrinsic::Radians, name:"radians",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("radians"),glsl:Some("radians"),hlsl:Some("radians"),intel_opencl:Some("radians")},
    IntrinsicInfo {op:Intrinsic::Pow, name:"pow",family:Family::Math,signature:Signature::BinaryFloat,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("pow"),glsl:Some("pow"),hlsl:Some("pow"),intel_opencl:Some("pow")},
    IntrinsicInfo {op:Intrinsic::Atan2, name:"atan2",family:Family::Math,signature:Signature::BinaryFloat,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("atan2"),glsl:Some("atan"),hlsl:Some("atan2"),intel_opencl:Some("atan2")},
    IntrinsicInfo {op:Intrinsic::Min, name:"min",family:Family::Math,signature:Signature::BinaryNumber,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("min"),glsl:Some("min"),hlsl:Some("min"),intel_opencl:Some("min")},
    IntrinsicInfo {op:Intrinsic::Max, name:"max",family:Family::Math,signature:Signature::BinaryNumber,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("max"),glsl:Some("max"),hlsl:Some("max"),intel_opencl:Some("max")},
    IntrinsicInfo {op:Intrinsic::Clamp, name:"clamp",family:Family::Math,signature:Signature::TernaryNumber,arity:Arity{min:3,max:3},capability:Capability::Core,stage:None,wgsl:Some("clamp"),glsl:Some("clamp"),hlsl:Some("clamp"),intel_opencl:Some("clamp")},
    IntrinsicInfo {op:Intrinsic::Fma, name:"fma",family:Family::Math,signature:Signature::TernaryFloat,arity:Arity{min:3,max:3},capability:Capability::Core,stage:None,wgsl:Some("fma"),glsl:Some("fma"),hlsl:Some("mad"),intel_opencl:Some("fma")},
    IntrinsicInfo {op:Intrinsic::Saturate, name:"saturate",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:None,hlsl:Some("saturate"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Mix, name:"mix",family:Family::Math,signature:Signature::TernaryFloat,arity:Arity{min:3,max:3},capability:Capability::Core,stage:None,wgsl:Some("mix"),glsl:Some("mix"),hlsl:Some("lerp"),intel_opencl:Some("mix")},
    IntrinsicInfo {op:Intrinsic::Step, name:"step",family:Family::Math,signature:Signature::BinaryFloat,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("step"),glsl:Some("step"),hlsl:Some("step"),intel_opencl:Some("step")},
    IntrinsicInfo {op:Intrinsic::Smoothstep, name:"smoothstep",family:Family::Math,signature:Signature::TernaryFloat,arity:Arity{min:3,max:3},capability:Capability::Core,stage:None,wgsl:Some("smoothstep"),glsl:Some("smoothstep"),hlsl:Some("smoothstep"),intel_opencl:Some("smoothstep")},
    IntrinsicInfo {op:Intrinsic::Modf, name:"modf",family:Family::Math,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("modf"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Frexp, name:"frexp",family:Family::Math,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("frexp"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Ldexp, name:"ldexp",family:Family::Math,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("ldexp"),glsl:Some("ldexp"),hlsl:Some("ldexp"),intel_opencl:Some("ldexp")},
    IntrinsicInfo {op:Intrinsic::QuantizeToF16, name:"quantizeToF16",family:Family::Math,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::F16,stage:None,wgsl:Some("quantizeToF16"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Length, name:"length",family:Family::Geometry,signature:Signature::Length,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("length"),glsl:Some("length"),hlsl:Some("length"),intel_opencl:Some("length")},
    IntrinsicInfo {op:Intrinsic::Distance, name:"distance",family:Family::Geometry,signature:Signature::Distance,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("distance"),glsl:Some("distance"),hlsl:Some("distance"),intel_opencl:Some("distance")},
    IntrinsicInfo {op:Intrinsic::Dot, name:"dot",family:Family::Geometry,signature:Signature::Dot,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("dot"),glsl:Some("dot"),hlsl:Some("dot"),intel_opencl:Some("dot")},
    IntrinsicInfo {op:Intrinsic::Cross, name:"cross",family:Family::Geometry,signature:Signature::Cross,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("cross"),glsl:Some("cross"),hlsl:Some("cross"),intel_opencl:Some("cross")},
    IntrinsicInfo {op:Intrinsic::Normalize, name:"normalize",family:Family::Geometry,signature:Signature::UnaryFloatVector,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("normalize"),glsl:Some("normalize"),hlsl:Some("normalize"),intel_opencl:Some("normalize")},
    IntrinsicInfo {op:Intrinsic::Reflect, name:"reflect",family:Family::Geometry,signature:Signature::BinaryFloatVector,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("reflect"),glsl:Some("reflect"),hlsl:Some("reflect"),intel_opencl:Some("reflect")},
    IntrinsicInfo {op:Intrinsic::Refract, name:"refract",family:Family::Geometry,signature:Signature::Refract,arity:Arity{min:3,max:3},capability:Capability::Core,stage:None,wgsl:Some("refract"),glsl:Some("refract"),hlsl:Some("refract"),intel_opencl:Some("refract")},
    IntrinsicInfo {op:Intrinsic::FaceForward, name:"faceForward",family:Family::Geometry,signature:Signature::TernaryFloatVector,arity:Arity{min:3,max:3},capability:Capability::Core,stage:None,wgsl:Some("faceForward"),glsl:Some("faceforward"),hlsl:Some("faceforward"),intel_opencl:Some("faceforward")},
    IntrinsicInfo {op:Intrinsic::Transpose, name:"transpose",family:Family::Matrix,signature:Signature::Matrix,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("transpose"),glsl:Some("transpose"),hlsl:Some("transpose"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Determinant, name:"determinant",family:Family::Matrix,signature:Signature::MatrixScalar,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("determinant"),glsl:Some("determinant"),hlsl:Some("determinant"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Inverse, name:"inverse",family:Family::Matrix,signature:Signature::Matrix,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:Some("inverse"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Add, name:"add",family:Family::Operator,signature:Signature::BinaryNumber,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("Add"),glsl:Some("Add"),hlsl:Some("Add"),intel_opencl:Some("Add")},
    IntrinsicInfo {op:Intrinsic::Sub, name:"sub",family:Family::Operator,signature:Signature::BinaryNumber,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("Sub"),glsl:Some("Sub"),hlsl:Some("Sub"),intel_opencl:Some("Sub")},
    IntrinsicInfo {op:Intrinsic::Multiply, name:"multiply",family:Family::Operator,signature:Signature::BinaryNumber,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("Multiply"),glsl:Some("Multiply"),hlsl:Some("Multiply"),intel_opencl:Some("Multiply")},
    IntrinsicInfo {op:Intrinsic::Divide, name:"divide",family:Family::Operator,signature:Signature::BinaryNumber,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("Divide"),glsl:Some("Divide"),hlsl:Some("Divide"),intel_opencl:Some("Divide")},
    IntrinsicInfo {op:Intrinsic::Modulo, name:"modulo",family:Family::Operator,signature:Signature::BinaryNumber,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("Modulo"),glsl:Some("Modulo"),hlsl:Some("Modulo"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Negate, name:"negate",family:Family::Operator,signature:Signature::UnaryNumber,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("Negate"),glsl:Some("Negate"),hlsl:Some("Negate"),intel_opencl:Some("Negate")},
    IntrinsicInfo {op:Intrinsic::Equal, name:"equal",family:Family::Operator,signature:Signature::Equality,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("Equal"),glsl:Some("Equal"),hlsl:Some("Equal"),intel_opencl:Some("Equal")},
    IntrinsicInfo {op:Intrinsic::NotEqual, name:"notEqual",family:Family::Operator,signature:Signature::Equality,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("NotEqual"),glsl:Some("NotEqual"),hlsl:Some("NotEqual"),intel_opencl:Some("NotEqual")},
    IntrinsicInfo {op:Intrinsic::Less, name:"less",family:Family::Operator,signature:Signature::Comparison,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("Less"),glsl:Some("Less"),hlsl:Some("Less"),intel_opencl:Some("Less")},
    IntrinsicInfo {op:Intrinsic::LessEqual, name:"lessEqual",family:Family::Operator,signature:Signature::Comparison,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("LessEqual"),glsl:Some("LessEqual"),hlsl:Some("LessEqual"),intel_opencl:Some("LessEqual")},
    IntrinsicInfo {op:Intrinsic::Greater, name:"greater",family:Family::Operator,signature:Signature::Comparison,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("Greater"),glsl:Some("Greater"),hlsl:Some("Greater"),intel_opencl:Some("Greater")},
    IntrinsicInfo {op:Intrinsic::GreaterEqual, name:"greaterEqual",family:Family::Operator,signature:Signature::Comparison,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("GreaterEqual"),glsl:Some("GreaterEqual"),hlsl:Some("GreaterEqual"),intel_opencl:Some("GreaterEqual")},
    IntrinsicInfo {op:Intrinsic::LogicalAnd, name:"logicalAnd",family:Family::Operator,signature:Signature::BoolBinary,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("LogicalAnd"),glsl:Some("LogicalAnd"),hlsl:Some("LogicalAnd"),intel_opencl:Some("LogicalAnd")},
    IntrinsicInfo {op:Intrinsic::LogicalOr, name:"logicalOr",family:Family::Operator,signature:Signature::BoolBinary,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("LogicalOr"),glsl:Some("LogicalOr"),hlsl:Some("LogicalOr"),intel_opencl:Some("LogicalOr")},
    IntrinsicInfo {op:Intrinsic::LogicalNot, name:"logicalNot",family:Family::Operator,signature:Signature::BoolUnary,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("LogicalNot"),glsl:Some("LogicalNot"),hlsl:Some("LogicalNot"),intel_opencl:Some("LogicalNot")},
    IntrinsicInfo {op:Intrinsic::BitAnd, name:"bitAnd",family:Family::Operator,signature:Signature::BinaryInteger,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("BitAnd"),glsl:Some("BitAnd"),hlsl:Some("BitAnd"),intel_opencl:Some("BitAnd")},
    IntrinsicInfo {op:Intrinsic::BitOr, name:"bitOr",family:Family::Operator,signature:Signature::BinaryInteger,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("BitOr"),glsl:Some("BitOr"),hlsl:Some("BitOr"),intel_opencl:Some("BitOr")},
    IntrinsicInfo {op:Intrinsic::BitXor, name:"bitXor",family:Family::Operator,signature:Signature::BinaryInteger,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("BitXor"),glsl:Some("BitXor"),hlsl:Some("BitXor"),intel_opencl:Some("BitXor")},
    IntrinsicInfo {op:Intrinsic::ShiftLeft, name:"shiftLeft",family:Family::Operator,signature:Signature::BinaryInteger,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("ShiftLeft"),glsl:Some("ShiftLeft"),hlsl:Some("ShiftLeft"),intel_opencl:Some("ShiftLeft")},
    IntrinsicInfo {op:Intrinsic::ShiftRight, name:"shiftRight",family:Family::Operator,signature:Signature::BinaryInteger,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("ShiftRight"),glsl:Some("ShiftRight"),hlsl:Some("ShiftRight"),intel_opencl:Some("ShiftRight")},
    IntrinsicInfo {op:Intrinsic::BitNot, name:"bitNot",family:Family::Operator,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("BitNot"),glsl:Some("BitNot"),hlsl:Some("BitNot"),intel_opencl:Some("BitNot")},
    IntrinsicInfo {op:Intrinsic::Select, name:"select",family:Family::Selection,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Core,stage:None,wgsl:Some("select"),glsl:Some("mix"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::All, name:"all",family:Family::Comparison,signature:Signature::ReductionBool,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("all"),glsl:Some("all"),hlsl:Some("all"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Any, name:"any",family:Family::Comparison,signature:Signature::ReductionBool,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("any"),glsl:Some("any"),hlsl:Some("any"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::CountOneBits, name:"countOneBits",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("countOneBits"),glsl:Some("bitCount"),hlsl:Some("countbits"),intel_opencl:Some("popcount")},
    IntrinsicInfo {op:Intrinsic::ReverseBits, name:"reverseBits",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("reverseBits"),glsl:Some("bitfieldReverse"),hlsl:Some("reversebits"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::FirstLeadingBit, name:"firstLeadingBit",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("firstLeadingBit"),glsl:Some("findMSB"),hlsl:Some("firstbithigh"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::FirstTrailingBit, name:"firstTrailingBit",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("firstTrailingBit"),glsl:Some("findLSB"),hlsl:Some("firstbitlow"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::CountLeadingZeros, name:"countLeadingZeros",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("countLeadingZeros"),glsl:None,hlsl:Some("firstbithigh"),intel_opencl:Some("clz")},
    IntrinsicInfo {op:Intrinsic::CountTrailingZeros, name:"countTrailingZeros",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("countTrailingZeros"),glsl:None,hlsl:Some("firstbitlow"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::FindLsb, name:"findLsb",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:Some("findLSB"),hlsl:Some("firstbitlow"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::FindMsb, name:"findMsb",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:Some("findMSB"),hlsl:Some("firstbithigh"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::ExtractBits, name:"extractBits",family:Family::Bits,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Core,stage:None,wgsl:Some("extractBits"),glsl:Some("bitfieldExtract"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::InsertBits, name:"insertBits",family:Family::Bits,signature:Signature::Special,arity:Arity{min:4,max:4},capability:Capability::Core,stage:None,wgsl:Some("insertBits"),glsl:Some("bitfieldInsert"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::RotateLeft, name:"rotateLeft",family:Family::Bits,signature:Signature::BinaryInteger,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("rotateLeft"),glsl:Some("rotateLeft"),hlsl:Some("rotateLeft"),intel_opencl:Some("RotateLeft")},
    IntrinsicInfo {op:Intrinsic::RotateRight, name:"rotateRight",family:Family::Bits,signature:Signature::BinaryInteger,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:Some("rotateRight"),glsl:Some("rotateRight"),hlsl:Some("rotateRight"),intel_opencl:Some("RotateRight")},
    IntrinsicInfo {op:Intrinsic::BitfieldMask, name:"bitfieldMask",family:Family::Bits,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Core,stage:None,wgsl:None,glsl:Some("bitfieldMask"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::BitfieldReverse, name:"bitfieldReverse",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:Some("bitfieldReverse"),hlsl:Some("reversebits"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::BitfieldExtractSigned, name:"bitfieldExtractSigned",family:Family::Bits,signature:Signature::Special,arity:Arity{min:3,max:4},capability:Capability::Core,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::BitfieldExtractUnsigned, name:"bitfieldExtractUnsigned",family:Family::Bits,signature:Signature::Special,arity:Arity{min:3,max:4},capability:Capability::Core,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::BitfieldInsertSigned, name:"bitfieldInsertSigned",family:Family::Bits,signature:Signature::Special,arity:Arity{min:3,max:4},capability:Capability::Core,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::BitfieldCount, name:"bitfieldCount",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:Some("bitCount"),hlsl:Some("countbits"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::FindFirstSet, name:"findFirstSet",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:Some("findLSB"),hlsl:Some("firstbitlow"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::FindLastSet, name:"findLastSet",family:Family::Bits,signature:Signature::UnaryInteger,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:Some("findMSB"),hlsl:Some("firstbithigh"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack4x8Unorm, name:"pack4x8Unorm",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:Some("pack4x8unorm"),glsl:Some("packUnorm4x8"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack4x8Snorm, name:"pack4x8Snorm",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:Some("pack4x8snorm"),glsl:Some("packSnorm4x8"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack2x16Unorm, name:"pack2x16Unorm",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:Some("pack2x16unorm"),glsl:Some("packUnorm2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack2x16Snorm, name:"pack2x16Snorm",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:Some("pack2x16snorm"),glsl:Some("packSnorm2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack2x16Float, name:"pack2x16Float",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:Some("pack2x16float"),glsl:Some("packHalf2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack4xI8, name:"pack4xI8",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack4xU8, name:"pack4xU8",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack4xI8Clamp, name:"pack4xI8Clamp",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack4xU8Clamp, name:"pack4xU8Clamp",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack4xI8Saturate, name:"pack4xI8Saturate",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Pack4xU8Saturate, name:"pack4xU8Saturate",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Unpack4x8Unorm, name:"unpack4x8Unorm",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:Some("unpack4x8unorm"),glsl:Some("unpackUnorm4x8"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Unpack4x8Snorm, name:"unpack4x8Snorm",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:Some("unpack4x8snorm"),glsl:Some("unpackSnorm4x8"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Unpack2x16Unorm, name:"unpack2x16Unorm",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:Some("unpack2x16unorm"),glsl:Some("unpackUnorm2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Unpack2x16Snorm, name:"unpack2x16Snorm",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:Some("unpack2x16snorm"),glsl:Some("unpackSnorm2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Unpack2x16Float, name:"unpack2x16Float",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:Some("unpack2x16float"),glsl:Some("unpackHalf2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Unpack4xI8, name:"unpack4xI8",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Unpack4xU8, name:"unpack4xU8",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::PackSnorm2x16, name:"packSnorm2x16",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:Some("packSnorm2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::PackUnorm2x16, name:"packUnorm2x16",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:Some("packUnorm2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::PackHalf2x16, name:"packHalf2x16",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:Some("packHalf2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::PackSnorm4x8, name:"packSnorm4x8",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:Some("packSnorm4x8"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::PackUnorm4x8, name:"packUnorm4x8",family:Family::Packing,signature:Signature::Pack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:Some("packUnorm4x8"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::UnpackSnorm2x16, name:"unpackSnorm2x16",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:Some("unpackSnorm2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::UnpackUnorm2x16, name:"unpackUnorm2x16",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:Some("unpackUnorm2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::UnpackHalf2x16, name:"unpackHalf2x16",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:Some("unpackHalf2x16"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::UnpackSnorm4x8, name:"unpackSnorm4x8",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:Some("unpackSnorm4x8"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::UnpackUnorm4x8, name:"unpackUnorm4x8",family:Family::Packing,signature:Signature::Unpack,arity:Arity{min:1,max:1},capability:Capability::Packing,stage:None,wgsl:None,glsl:Some("unpackUnorm4x8"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Dpdx, name:"dpdx",family:Family::Derivative,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:Some("dpdx"),glsl:Some("dFdx"),hlsl:Some("ddx"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Dpdy, name:"dpdy",family:Family::Derivative,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:Some("dpdy"),glsl:Some("dFdy"),hlsl:Some("ddy"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Fwidth, name:"fwidth",family:Family::Derivative,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:Some("fwidth"),glsl:Some("fwidth"),hlsl:Some("fwidth"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::DpdxFine, name:"dpdxFine",family:Family::Derivative,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:Some("dpdxFine"),glsl:Some("dFdxFine"),hlsl:Some("ddx_fine"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::DpdyFine, name:"dpdyFine",family:Family::Derivative,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:Some("dpdyFine"),glsl:Some("dFdyFine"),hlsl:Some("ddy_fine"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::FwidthFine, name:"fwidthFine",family:Family::Derivative,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:Some("fwidthFine"),glsl:Some("fwidthFine"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::DpdxCoarse, name:"dpdxCoarse",family:Family::Derivative,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:Some("dpdxCoarse"),glsl:Some("dFdxCoarse"),hlsl:Some("ddx_coarse"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::DpdyCoarse, name:"dpdyCoarse",family:Family::Derivative,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:Some("dpdyCoarse"),glsl:Some("dFdyCoarse"),hlsl:Some("ddy_coarse"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::FwidthCoarse, name:"fwidthCoarse",family:Family::Derivative,signature:Signature::UnaryFloat,arity:Arity{min:1,max:1},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:Some("fwidthCoarse"),glsl:Some("fwidthCoarse"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::InterpolateAtCentroid, name:"interpolateAtCentroid",family:Family::Derivative,signature:Signature::Special,arity:Arity{min:1,max:2},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:None,glsl:Some("interpolateAtCentroid"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::InterpolateAtSample, name:"interpolateAtSample",family:Family::Derivative,signature:Signature::Special,arity:Arity{min:1,max:2},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:None,glsl:Some("interpolateAtSample"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::InterpolateAtOffset, name:"interpolateAtOffset",family:Family::Derivative,signature:Signature::Special,arity:Arity{min:1,max:2},capability:Capability::Derivatives,stage:Some(Stage::Fragment),wgsl:None,glsl:Some("interpolateAtOffset"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureDimensions, name:"textureDimensions",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:2},capability:Capability::Textures,stage:None,wgsl:Some("textureDimensions"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureNumLayers, name:"textureNumLayers",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Textures,stage:None,wgsl:Some("textureNumLayers"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureNumLevels, name:"textureNumLevels",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Textures,stage:None,wgsl:Some("textureNumLevels"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureNumSamples, name:"textureNumSamples",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Textures,stage:None,wgsl:Some("textureNumSamples"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureLoad, name:"textureLoad",family:Family::Texture,signature:Signature::Special,arity:Arity{min:2,max:4},capability:Capability::Textures,stage:None,wgsl:Some("textureLoad"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureStore, name:"textureStore",family:Family::Texture,signature:Signature::Special,arity:Arity{min:3,max:4},capability:Capability::StorageTextures,stage:None,wgsl:Some("textureStore"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSample, name:"textureSample",family:Family::Texture,signature:Signature::Special,arity:Arity{min:3,max:5},capability:Capability::Textures,stage:Some(Stage::Fragment),wgsl:Some("textureSample"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSampleBias, name:"textureSampleBias",family:Family::Texture,signature:Signature::Special,arity:Arity{min:4,max:6},capability:Capability::Textures,stage:Some(Stage::Fragment),wgsl:Some("textureSampleBias"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSampleLevel, name:"textureSampleLevel",family:Family::Texture,signature:Signature::Special,arity:Arity{min:4,max:6},capability:Capability::Textures,stage:None,wgsl:Some("textureSampleLevel"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSampleGrad, name:"textureSampleGrad",family:Family::Texture,signature:Signature::Special,arity:Arity{min:5,max:7},capability:Capability::Textures,stage:None,wgsl:Some("textureSampleGrad"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSampleCompare, name:"textureSampleCompare",family:Family::Texture,signature:Signature::Special,arity:Arity{min:4,max:6},capability:Capability::Textures,stage:Some(Stage::Fragment),wgsl:Some("textureSampleCompare"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSampleCompareLevel, name:"textureSampleCompareLevel",family:Family::Texture,signature:Signature::Special,arity:Arity{min:4,max:6},capability:Capability::Textures,stage:None,wgsl:Some("textureSampleCompareLevel"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGather, name:"textureGather",family:Family::Texture,signature:Signature::Special,arity:Arity{min:3,max:5},capability:Capability::Textures,stage:Some(Stage::Fragment),wgsl:Some("textureGather"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGatherCompare, name:"textureGatherCompare",family:Family::Texture,signature:Signature::Special,arity:Arity{min:4,max:6},capability:Capability::Textures,stage:Some(Stage::Fragment),wgsl:Some("textureGatherCompare"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSampleBaseClampToEdge, name:"textureSampleBaseClampToEdge",family:Family::Texture,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Textures,stage:Some(Stage::Fragment),wgsl:Some("textureSampleBaseClampToEdge"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSampleCompareBias, name:"textureSampleCompareBias",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSampleCompareGrad, name:"textureSampleCompareGrad",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGatherRed, name:"textureGatherRed",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGatherGreen, name:"textureGatherGreen",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGatherBlue, name:"textureGatherBlue",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGatherAlpha, name:"textureGatherAlpha",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureFetch, name:"textureFetch",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("texelFetch"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureFetchOffset, name:"textureFetchOffset",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("texelFetchOffset"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureFetchLod, name:"textureFetchLod",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("texelFetch"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureFetchLodOffset, name:"textureFetchLodOffset",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("texelFetchOffset"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureQueryLod, name:"textureQueryLod",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureQueryLod"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureQueryLevels, name:"textureQueryLevels",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureQueryLevels"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureQuerySamples, name:"textureQuerySamples",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureSamples"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSize, name:"textureSize",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureSize"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureSamples, name:"textureSamples",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureSamples"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureOffset, name:"textureOffset",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureOffset"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureProj, name:"textureProj",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureProj"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureProjLod, name:"textureProjLod",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureProjLod"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureProjGrad, name:"textureProjGrad",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureProjGrad"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGrad, name:"textureGrad",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureGrad"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureLod, name:"textureLod",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureLod"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGatherOffset, name:"textureGatherOffset",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureGatherOffset"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGatherOffsets, name:"textureGatherOffsets",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureGatherOffsets"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGatherCompareOffset, name:"textureGatherCompareOffset",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureGatherOffset"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::TextureGatherCompareOffsets, name:"textureGatherCompareOffsets",family:Family::Texture,signature:Signature::Special,arity:Arity{min:1,max:6},capability:Capability::Textures,stage:None,wgsl:None,glsl:Some("textureGatherOffsets"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicLoad, name:"atomicLoad",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicLoad"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicStore, name:"atomicStore",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicStore"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicAdd, name:"atomicAdd",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicAdd"),glsl:Some("atomicAdd"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicSub, name:"atomicSub",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicSub"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicMax, name:"atomicMax",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicMax"),glsl:Some("atomicMax"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicMin, name:"atomicMin",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicMin"),glsl:Some("atomicMin"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicAnd, name:"atomicAnd",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicAnd"),glsl:Some("atomicAnd"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicOr, name:"atomicOr",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicOr"),glsl:Some("atomicOr"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicXor, name:"atomicXor",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicXor"),glsl:Some("atomicXor"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicExchange, name:"atomicExchange",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicExchange"),glsl:Some("atomicExchange"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicCompareExchangeWeak, name:"atomicCompareExchangeWeak",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:Some("atomicCompareExchangeWeak"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicCompSwap, name:"atomicCompSwap",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:1,max:4},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:None,glsl:Some("atomicCompSwap"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicInc, name:"atomicInc",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:1,max:4},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicDec, name:"atomicDec",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:1,max:4},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicCounter, name:"atomicCounter",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:1,max:4},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicCounterIncrement, name:"atomicCounterIncrement",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:1,max:4},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:None,glsl:Some("atomicCounterIncrement"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AtomicCounterDecrement, name:"atomicCounterDecrement",family:Family::Atomic,signature:Signature::Special,arity:Arity{min:1,max:4},capability:Capability::Atomics,stage:Some(Stage::Compute),wgsl:None,glsl:Some("atomicCounterDecrement"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::WorkgroupBarrier, name:"workgroupBarrier",family:Family::Barrier,signature:Signature::Barrier,arity:Arity{min:0,max:0},capability:Capability::Workgroup,stage:Some(Stage::Compute),wgsl:Some("workgroupBarrier"),glsl:None,hlsl:None,intel_opencl:Some("barrier")},
    IntrinsicInfo {op:Intrinsic::StorageBarrier, name:"storageBarrier",family:Family::Barrier,signature:Signature::Barrier,arity:Arity{min:0,max:0},capability:Capability::Workgroup,stage:Some(Stage::Compute),wgsl:Some("storageBarrier"),glsl:None,hlsl:None,intel_opencl:Some("barrier")},
    IntrinsicInfo {op:Intrinsic::TextureBarrier, name:"textureBarrier",family:Family::Barrier,signature:Signature::Barrier,arity:Arity{min:0,max:0},capability:Capability::Workgroup,stage:Some(Stage::Compute),wgsl:Some("textureBarrier"),glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Barrier, name:"barrier",family:Family::Barrier,signature:Signature::Barrier,arity:Arity{min:0,max:0},capability:Capability::Workgroup,stage:Some(Stage::Compute),wgsl:None,glsl:Some("barrier"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::MemoryBarrier, name:"memoryBarrier",family:Family::Barrier,signature:Signature::Barrier,arity:Arity{min:0,max:0},capability:Capability::Workgroup,stage:Some(Stage::Compute),wgsl:None,glsl:Some("memoryBarrier"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::MemoryBarrierBuffer, name:"memoryBarrierBuffer",family:Family::Barrier,signature:Signature::Barrier,arity:Arity{min:0,max:0},capability:Capability::Workgroup,stage:Some(Stage::Compute),wgsl:None,glsl:Some("memoryBarrierBuffer"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::MemoryBarrierImage, name:"memoryBarrierImage",family:Family::Barrier,signature:Signature::Barrier,arity:Arity{min:0,max:0},capability:Capability::Workgroup,stage:Some(Stage::Compute),wgsl:None,glsl:Some("memoryBarrierImage"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::GroupMemoryBarrier, name:"groupMemoryBarrier",family:Family::Barrier,signature:Signature::Barrier,arity:Arity{min:0,max:0},capability:Capability::Workgroup,stage:Some(Stage::Compute),wgsl:None,glsl:Some("groupMemoryBarrier"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::DeviceMemoryBarrier, name:"deviceMemoryBarrier",family:Family::Barrier,signature:Signature::Barrier,arity:Arity{min:0,max:0},capability:Capability::Workgroup,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:Some("DeviceMemoryBarrier"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AllMemoryBarrier, name:"allMemoryBarrier",family:Family::Barrier,signature:Signature::Barrier,arity:Arity{min:0,max:0},capability:Capability::Workgroup,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:Some("AllMemoryBarrier"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupSize, name:"subgroupSize",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("get_sub_group_size")},
    IntrinsicInfo {op:Intrinsic::SubgroupInvocationId, name:"subgroupInvocationId",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("get_sub_group_local_id")},
    IntrinsicInfo {op:Intrinsic::SubgroupElect, name:"subgroupElect",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupElect"),hlsl:Some("WaveIsFirstLane"),intel_opencl:Some("sub_group_elect")},
    IntrinsicInfo {op:Intrinsic::SubgroupBroadcast, name:"subgroupBroadcast",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupBroadcast"),hlsl:Some("WaveReadLaneAt"),intel_opencl:Some("sub_group_broadcast")},
    IntrinsicInfo {op:Intrinsic::SubgroupBroadcastFirst, name:"subgroupBroadcastFirst",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupBroadcastFirst"),hlsl:Some("WaveReadLaneFirst"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupShuffle, name:"subgroupShuffle",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::IntelSubgroups,stage:None,wgsl:None,glsl:Some("subgroupShuffle"),hlsl:Some("WaveReadLaneAt"),intel_opencl:Some("intel_sub_group_shuffle")},
    IntrinsicInfo {op:Intrinsic::SubgroupShuffleXor, name:"subgroupShuffleXor",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::IntelSubgroups,stage:None,wgsl:None,glsl:Some("subgroupShuffleXor"),hlsl:None,intel_opencl:Some("intel_sub_group_shuffle_xor")},
    IntrinsicInfo {op:Intrinsic::SubgroupShuffleUp, name:"subgroupShuffleUp",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::IntelSubgroups,stage:None,wgsl:None,glsl:Some("subgroupShuffleUp"),hlsl:None,intel_opencl:Some("intel_sub_group_shuffle_up")},
    IntrinsicInfo {op:Intrinsic::SubgroupShuffleDown, name:"subgroupShuffleDown",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::IntelSubgroups,stage:None,wgsl:None,glsl:Some("subgroupShuffleDown"),hlsl:None,intel_opencl:Some("intel_sub_group_shuffle_down")},
    IntrinsicInfo {op:Intrinsic::SubgroupAll, name:"subgroupAll",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupAll"),hlsl:Some("WaveActiveAllTrue"),intel_opencl:Some("sub_group_all")},
    IntrinsicInfo {op:Intrinsic::SubgroupAny, name:"subgroupAny",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupAny"),hlsl:Some("WaveActiveAnyTrue"),intel_opencl:Some("sub_group_any")},
    IntrinsicInfo {op:Intrinsic::SubgroupBallot, name:"subgroupBallot",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupBallot"),hlsl:Some("WaveActiveBallot"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupAllEqual, name:"subgroupAllEqual",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupAllEqual"),hlsl:Some("WaveActiveAllEqual"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupAdd, name:"subgroupAdd",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupAdd"),hlsl:Some("WaveActiveSum"),intel_opencl:Some("sub_group_reduce_add")},
    IntrinsicInfo {op:Intrinsic::SubgroupMul, name:"subgroupMul",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupMul"),hlsl:Some("WaveActiveProduct"),intel_opencl:Some("sub_group_reduce_mul")},
    IntrinsicInfo {op:Intrinsic::SubgroupMin, name:"subgroupMin",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupMin"),hlsl:Some("WaveActiveMin"),intel_opencl:Some("sub_group_reduce_min")},
    IntrinsicInfo {op:Intrinsic::SubgroupMax, name:"subgroupMax",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupMax"),hlsl:Some("WaveActiveMax"),intel_opencl:Some("sub_group_reduce_max")},
    IntrinsicInfo {op:Intrinsic::SubgroupAnd, name:"subgroupAnd",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupAnd"),hlsl:Some("WaveActiveBitAnd"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupOr, name:"subgroupOr",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupOr"),hlsl:Some("WaveActiveBitOr"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupXor, name:"subgroupXor",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupXor"),hlsl:Some("WaveActiveBitXor"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupInclusiveAdd, name:"subgroupInclusiveAdd",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupInclusiveAdd"),hlsl:None,intel_opencl:Some("sub_group_scan_inclusive_add")},
    IntrinsicInfo {op:Intrinsic::SubgroupInclusiveMul, name:"subgroupInclusiveMul",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupInclusiveMul"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupInclusiveMin, name:"subgroupInclusiveMin",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("sub_group_scan_inclusive_min")},
    IntrinsicInfo {op:Intrinsic::SubgroupInclusiveMax, name:"subgroupInclusiveMax",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("sub_group_scan_inclusive_max")},
    IntrinsicInfo {op:Intrinsic::SubgroupExclusiveAdd, name:"subgroupExclusiveAdd",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupExclusiveAdd"),hlsl:None,intel_opencl:Some("sub_group_scan_exclusive_add")},
    IntrinsicInfo {op:Intrinsic::SubgroupExclusiveMul, name:"subgroupExclusiveMul",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupExclusiveMul"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupExclusiveMin, name:"subgroupExclusiveMin",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("sub_group_scan_exclusive_min")},
    IntrinsicInfo {op:Intrinsic::SubgroupExclusiveMax, name:"subgroupExclusiveMax",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("sub_group_scan_exclusive_max")},
    IntrinsicInfo {op:Intrinsic::SubgroupQuadBroadcast, name:"subgroupQuadBroadcast",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupQuadBroadcast"),hlsl:Some("QuadReadLaneAt"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupQuadSwapX, name:"subgroupQuadSwapX",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:Some("QuadReadAcrossX"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupQuadSwapY, name:"subgroupQuadSwapY",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:Some("QuadReadAcrossY"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupQuadSwapDiagonal, name:"subgroupQuadSwapDiagonal",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:Some("QuadReadAcrossDiagonal"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupClusteredAdd, name:"subgroupClusteredAdd",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupClusteredAdd"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupClusteredMul, name:"subgroupClusteredMul",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupClusteredMul"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupClusteredMin, name:"subgroupClusteredMin",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupClusteredMin"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupClusteredMax, name:"subgroupClusteredMax",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:Some("subgroupClusteredMax"),hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupPartitionedAdd, name:"subgroupPartitionedAdd",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupPartitionedInclusiveAdd, name:"subgroupPartitionedInclusiveAdd",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SubgroupPartitionedExclusiveAdd, name:"subgroupPartitionedExclusiveAdd",family:Family::Subgroup,signature:Signature::Special,arity:Arity{min:0,max:2},capability:Capability::Subgroups,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Discard, name:"discard",family:Family::Utility,signature:Signature::Special,arity:Arity{min:1,max:4},capability:Capability::Core,stage:Some(Stage::Fragment),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Clip, name:"clip",family:Family::Utility,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:Some(Stage::Fragment),wgsl:None,glsl:None,hlsl:Some("clip"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::SinCos, name:"sinCos",family:Family::Utility,signature:Signature::Special,arity:Arity{min:1,max:4},capability:Capability::Core,stage:None,wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::MsAd4, name:"msAd4",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:4},capability:Capability::Vendor,stage:None,wgsl:None,glsl:None,hlsl:Some("msad4"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Dot4AddI8Packed, name:"dot4AddI8Packed",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Vendor,stage:None,wgsl:None,glsl:None,hlsl:Some("dot4add_i8packed"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::Dot4AddU8Packed, name:"dot4AddU8Packed",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Vendor,stage:None,wgsl:None,glsl:None,hlsl:Some("dot4add_u8packed"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NonUniformIndex, name:"nonUniformIndex",family:Family::Utility,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:None,hlsl:Some("NonUniformResourceIndex"),intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AsFloat, name:"asFloat",family:Family::Utility,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("bitcast<f32>"),glsl:Some("uintBitsToFloat"),hlsl:Some("asfloat"),intel_opencl:Some("as_float")},
    IntrinsicInfo {op:Intrinsic::AsUInt, name:"asUInt",family:Family::Utility,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("bitcast<u32>"),glsl:Some("floatBitsToUint"),hlsl:Some("asuint"),intel_opencl:Some("as_uint")},
    IntrinsicInfo {op:Intrinsic::AsInt, name:"asInt",family:Family::Utility,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:Some("bitcast<i32>"),glsl:Some("floatBitsToInt"),hlsl:Some("asint"),intel_opencl:Some("as_int")},
    IntrinsicInfo {op:Intrinsic::IsNan, name:"isNan",family:Family::Utility,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:Some("isnan"),hlsl:Some("isnan"),intel_opencl:Some("isnan")},
    IntrinsicInfo {op:Intrinsic::IsInf, name:"isInf",family:Family::Utility,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:Some("isinf"),hlsl:Some("isinf"),intel_opencl:Some("isinf")},
    IntrinsicInfo {op:Intrinsic::IsFinite, name:"isFinite",family:Family::Utility,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:None,wgsl:None,glsl:None,hlsl:Some("isfinite"),intel_opencl:Some("isfinite")},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockRead, name:"intelSubGroupBlockRead",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::IntelSubgroups,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("intel_sub_group_block_read")},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockRead2, name:"intelSubGroupBlockRead2",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::IntelSubgroups,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("intel_sub_group_block_read2")},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockRead4, name:"intelSubGroupBlockRead4",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::IntelSubgroups,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("intel_sub_group_block_read4")},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockRead8, name:"intelSubGroupBlockRead8",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::IntelSubgroups,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockWrite, name:"intelSubGroupBlockWrite",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::IntelSubgroups,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("intel_sub_group_block_write")},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockWrite2, name:"intelSubGroupBlockWrite2",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::IntelSubgroups,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("intel_sub_group_block_write2")},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockWrite4, name:"intelSubGroupBlockWrite4",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::IntelSubgroups,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:Some("intel_sub_group_block_write4")},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockWrite8, name:"intelSubGroupBlockWrite8",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::IntelSubgroups,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockReadUs, name:"intelSubGroupBlockReadUs",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::IntelSubgroupsShort,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockReadUs2, name:"intelSubGroupBlockReadUs2",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::IntelSubgroupsShort,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockReadUs4, name:"intelSubGroupBlockReadUs4",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::IntelSubgroupsShort,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockReadUs8, name:"intelSubGroupBlockReadUs8",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::IntelSubgroupsShort,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockWriteUs, name:"intelSubGroupBlockWriteUs",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::IntelSubgroupsShort,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockWriteUs2, name:"intelSubGroupBlockWriteUs2",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::IntelSubgroupsShort,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockWriteUs4, name:"intelSubGroupBlockWriteUs4",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::IntelSubgroupsShort,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::IntelSubGroupBlockWriteUs8, name:"intelSubGroupBlockWriteUs8",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::IntelSubgroupsShort,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaActiveMask, name:"nvidiaActiveMask",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::NvidiaWarp,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaBallotSync, name:"nvidiaBallotSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::NvidiaWarp,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaAnySync, name:"nvidiaAnySync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::NvidiaWarp,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaAllSync, name:"nvidiaAllSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::NvidiaWarp,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaShuffleSync, name:"nvidiaShuffleSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:4},capability:Capability::NvidiaWarp,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaShuffleUpSync, name:"nvidiaShuffleUpSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:4},capability:Capability::NvidiaWarp,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaShuffleDownSync, name:"nvidiaShuffleDownSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:4},capability:Capability::NvidiaWarp,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaShuffleXorSync, name:"nvidiaShuffleXorSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:4},capability:Capability::NvidiaWarp,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaSyncWarp, name:"nvidiaSyncWarp",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::NvidiaWarp,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaMatchAnySync, name:"nvidiaMatchAnySync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::NvidiaWarp,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaReduceAddSync, name:"nvidiaReduceAddSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::NvidiaSm80,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaReduceMinSync, name:"nvidiaReduceMinSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::NvidiaSm80,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaReduceMaxSync, name:"nvidiaReduceMaxSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::NvidiaSm80,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaReduceAndSync, name:"nvidiaReduceAndSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::NvidiaSm80,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaReduceOrSync, name:"nvidiaReduceOrSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::NvidiaSm80,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::NvidiaReduceXorSync, name:"nvidiaReduceXorSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::NvidiaSm80,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdWaveSize, name:"amdWaveSize",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdLaneId, name:"amdLaneId",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdShuffle, name:"amdShuffle",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdShuffleXor, name:"amdShuffleXor",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdShuffleUp, name:"amdShuffleUp",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdShuffleDown, name:"amdShuffleDown",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdBallot, name:"amdBallot",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdActiveMask, name:"amdActiveMask",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdAny, name:"amdAny",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdAll, name:"amdAll",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdWaveSync, name:"amdWaveSync",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::AmdWave,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdReadFirstLane, name:"amdReadFirstLane",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::AmdGcn,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdReadLane, name:"amdReadLane",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::AmdGcn,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdWriteLane, name:"amdWriteLane",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::AmdGcn,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdDsBpermute, name:"amdDsBpermute",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::AmdGcn,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdDsPermute, name:"amdDsPermute",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::AmdGcn,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdFastSin, name:"amdFastSin",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdFastCos, name:"amdFastCos",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdFastExp, name:"amdFastExp",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdFastLog, name:"amdFastLog",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdFastLog2, name:"amdFastLog2",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdFastPow, name:"amdFastPow",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdFastRcp, name:"amdFastRcp",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdBytePerm, name:"amdBytePerm",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdFunnelShiftLeft, name:"amdFunnelShiftLeft",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdFunnelShiftRight, name:"amdFunnelShiftRight",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdMul24, name:"amdMul24",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdUMul24, name:"amdUMul24",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:2,max:2},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdSad, name:"amdSad",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdUSad, name:"amdUSad",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:3,max:3},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdPopcount64, name:"amdPopcount64",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdReverseBits64, name:"amdReverseBits64",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:1,max:1},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdThreadFenceBlock, name:"amdThreadFenceBlock",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdThreadFenceDevice, name:"amdThreadFenceDevice",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    IntrinsicInfo {op:Intrinsic::AmdThreadFenceSystem, name:"amdThreadFenceSystem",family:Family::Vendor,signature:Signature::Special,arity:Arity{min:0,max:0},capability:Capability::Core,stage:Some(Stage::Compute),wgsl:None,glsl:None,hlsl:None,intel_opencl:None},
    ];
impl Intrinsic {
    /// Retrieve metadata without querying the GPU.
    pub fn info(self)->&'static IntrinsicInfo{&INFOS[self as usize]}
    /// Look up a catalog operation by its stable camel-case identifier.
    pub fn from_name(name:&str)->Option<Self>{
        ALL_INTRINSICS.iter().copied().find(|v| v.info().name==name)
    }
}
