"""Generate a *reviewable* operation registry; run with Python stdlib only."""
from pathlib import Path
from collections import Counter
ops=[]

def add(names, family, sig, arity=1, cap='Core', stage='Any', spell=None):
    for name in names.split():
        wgsl,glsl,hlsl = (name[0].lower()+name[1:],)*3 if spell is None else spell.get(name, (name[0].lower()+name[1:],)*3)
        if isinstance(arity,int): mini=maxi=arity
        else: mini,maxi=arity
        ops.append((name,family,sig,mini,maxi,cap,stage,wgsl,glsl,hlsl))

# Functions that are built into at least one major shading language.
spells={
'Atan2':('atan2','atan','atan2'), 'InverseSqrt':('inverseSqrt','inversesqrt','rsqrt'),
'Fract':('fract','fract','frac'),'Mix':('mix','mix','lerp'),
'FaceForward':('faceForward','faceforward','faceforward'),
'CountLeadingZeros':('countLeadingZeros',None,'firstbithigh'), # HLSL leading-bit position differs: do not expose incompatible spelling
'CountOneBits':('countOneBits','bitCount','countbits'),
'CountTrailingZeros':('countTrailingZeros',None,'firstbitlow'),
'ReverseBits':('reverseBits','bitfieldReverse','reversebits'),
'FirstLeadingBit':('firstLeadingBit','findMSB','firstbithigh'),
'FirstTrailingBit':('firstTrailingBit','findLSB','firstbitlow'),
'ExtractBits':('extractBits','bitfieldExtract',None),
'InsertBits':('insertBits','bitfieldInsert',None),
'QuantizeToF16':('quantizeToF16',None,None),
'Fma':('fma','fma','mad'),
'Inverse':(None,'inverse',None),
'Radians':('radians','radians','radians'),'Degrees':('degrees','degrees','degrees'),
'All':('all','all','all'), 'Any':('any','any','any'),
'FindLsb':(None,'findLSB','firstbitlow'),
'FindMsb':(None,'findMSB','firstbithigh'),
'Pack4x8Unorm':('pack4x8unorm','packUnorm4x8',None),
'Pack4x8Snorm':('pack4x8snorm','packSnorm4x8',None),
'Unpack4x8Unorm':('unpack4x8unorm','unpackUnorm4x8',None),
'Unpack4x8Snorm':('unpack4x8snorm','unpackSnorm4x8',None),
'Pack2x16Unorm':('pack2x16unorm','packUnorm2x16',None),
'Pack2x16Snorm':('pack2x16snorm','packSnorm2x16',None),
'Unpack2x16Unorm':('unpack2x16unorm','unpackUnorm2x16',None),
'Unpack2x16Snorm':('unpack2x16snorm','unpackSnorm2x16',None),
'Pack2x16Float':('pack2x16float','packHalf2x16',None),
'Unpack2x16Float':('unpack2x16float','unpackHalf2x16',None),
'Pack4xI8':(None,None,None),'Pack4xU8':(None,None,None),
'Unpack4xI8':(None,None,None),'Unpack4xU8':(None,None,None),
'Pack4xI8Clamp':(None,None,None),'Pack4xU8Clamp':(None,None,None),
'Pack4xI8Saturate':(None,None,None),'Pack4xU8Saturate':(None,None,None),
}
add('Abs Sign', 'Math','UnaryNumber',spell=spells)
add('Sin Cos Tan Asin Acos Atan Sinh Cosh Tanh Asinh Acosh Atanh Exp Exp2 Log Log2 Sqrt InverseSqrt Floor Ceil Trunc Round Fract Degrees Radians', 'Math','UnaryFloat',spell=spells)
add('Pow Atan2', 'Math','BinaryFloat',2,spell=spells)
add('Min Max', 'Math','BinaryNumber',2)
add('Clamp', 'Math','TernaryNumber',3)
add('Fma', 'Math','TernaryFloat',3,spell=spells)
add('Saturate', 'Math','UnaryFloat',1,spell={'Saturate':('saturate',None,'saturate')})
add('Mix', 'Math','TernaryFloat',3,spell=spells)
add('Step', 'Math','BinaryFloat',2)
add('Smoothstep', 'Math','TernaryFloat',3,spell={'Smoothstep':('smoothstep','smoothstep','smoothstep')})
add('Modf', 'Math','Special',1,spell={'Modf':('modf',None,None)})
add('Frexp', 'Math','Special',1,spell={'Frexp':('frexp',None,None)})
add('Ldexp', 'Math','Special',2,spell={'Ldexp':('ldexp','ldexp','ldexp')})
add('QuantizeToF16', 'Math','UnaryFloat',1,'F16',spell=spells)
# vector and matrix
add('Length', 'Geometry','Length',1)
add('Distance', 'Geometry','Distance',2)
add('Dot', 'Geometry','Dot',2)
add('Cross', 'Geometry','Cross',2)
add('Normalize', 'Geometry','UnaryFloatVector',1)
add('Reflect', 'Geometry','BinaryFloatVector',2)
add('Refract', 'Geometry','Refract',3)
add('FaceForward', 'Geometry','TernaryFloatVector',3,spell=spells)
add('Transpose', 'Matrix','Matrix',1)
add('Determinant', 'Matrix','MatrixScalar',1)
add('Inverse', 'Matrix','Matrix',1,spell=spells)
# Types can be used with these operator IDs and emitted as infix or ternary expressions.
for names,sig,num in [
    ('Add Sub Multiply Divide Modulo','BinaryNumber',2),
    ('Negate','UnaryNumber',1),
    ('Equal NotEqual','Equality',2),
    ('Less LessEqual Greater GreaterEqual','Comparison',2),
    ('LogicalAnd LogicalOr','BoolBinary',2),('LogicalNot','BoolUnary',1),
    ('BitAnd BitOr BitXor ShiftLeft ShiftRight','BinaryInteger',2),
    ('BitNot','UnaryInteger',1),
]:add(names,'Operator',sig,num,spell={n:(n,n,n) for n in names.split()})
add('Select','Selection','Special',3,spell={'Select':('select','mix',None)})
add('All Any', 'Comparison','ReductionBool',1,spell=spells)
add('CountOneBits ReverseBits FirstLeadingBit FirstTrailingBit CountLeadingZeros CountTrailingZeros FindLsb FindMsb', 'Bits','UnaryInteger',1,spell=spells)
add('ExtractBits', 'Bits','Special',3,spell=spells)
add('InsertBits', 'Bits','Special',4,spell=spells)
# Synthesized shifts, not driver-specific intrinsics.
add('RotateLeft RotateRight', 'Bits','BinaryInteger',2,spell={'RotateLeft':('rotateLeft','rotateLeft','rotateLeft'),'RotateRight':('rotateRight','rotateRight','rotateRight')})
add('BitfieldMask', 'Bits','Special',2,spell={'BitfieldMask':(None,'bitfieldMask',None)})
add('BitfieldReverse', 'Bits','UnaryInteger',1,spell={'BitfieldReverse':(None,'bitfieldReverse','reversebits')})
add('BitfieldExtractSigned BitfieldExtractUnsigned BitfieldInsertSigned', 'Bits','Special',(3,4),spell={n:(None,None,None) for n in 'BitfieldExtractSigned BitfieldExtractUnsigned BitfieldInsertSigned'.split()})
add('BitfieldCount', 'Bits','UnaryInteger',1,spell={'BitfieldCount':(None,'bitCount','countbits')})
add('FindFirstSet FindLastSet', 'Bits','UnaryInteger',1,spell={'FindFirstSet':(None,'findLSB','firstbitlow'),'FindLastSet':(None,'findMSB','firstbithigh')})

add('Pack4x8Unorm Pack4x8Snorm Pack2x16Unorm Pack2x16Snorm Pack2x16Float Pack4xI8 Pack4xU8 Pack4xI8Clamp Pack4xU8Clamp Pack4xI8Saturate Pack4xU8Saturate', 'Packing','Pack',1,'Packing',spell=spells)
add('Unpack4x8Unorm Unpack4x8Snorm Unpack2x16Unorm Unpack2x16Snorm Unpack2x16Float Unpack4xI8 Unpack4xU8', 'Packing','Unpack',1,'Packing',spell=spells)
# math special quantization/packing only GLSL/HLSL if backend says it exists
add('PackSnorm2x16 PackUnorm2x16 PackHalf2x16 PackSnorm4x8 PackUnorm4x8', 'Packing','Pack',1,'Packing',spell={n:(None,dict(PackSnorm2x16='packSnorm2x16',PackUnorm2x16='packUnorm2x16',PackHalf2x16='packHalf2x16',PackSnorm4x8='packSnorm4x8',PackUnorm4x8='packUnorm4x8')[n],None) for n in 'PackSnorm2x16 PackUnorm2x16 PackHalf2x16 PackSnorm4x8 PackUnorm4x8'.split()})
add('UnpackSnorm2x16 UnpackUnorm2x16 UnpackHalf2x16 UnpackSnorm4x8 UnpackUnorm4x8', 'Packing','Unpack',1,'Packing',spell={n:(None,dict(UnpackSnorm2x16='unpackSnorm2x16',UnpackUnorm2x16='unpackUnorm2x16',UnpackHalf2x16='unpackHalf2x16',UnpackSnorm4x8='unpackSnorm4x8',UnpackUnorm4x8='unpackUnorm4x8')[n],None) for n in 'UnpackSnorm2x16 UnpackUnorm2x16 UnpackHalf2x16 UnpackSnorm4x8 UnpackUnorm4x8'.split()})
# stable derivatives WGSL, GLSL and HLSL naming
add('Dpdx Dpdy Fwidth DpdxFine DpdyFine FwidthFine DpdxCoarse DpdyCoarse FwidthCoarse', 'Derivative','UnaryFloat',1,'Derivatives','Fragment', spell={
'Dpdx':('dpdx','dFdx','ddx'),'Dpdy':('dpdy','dFdy','ddy'),'Fwidth':('fwidth','fwidth','fwidth'),
'DpdxFine':('dpdxFine','dFdxFine','ddx_fine'),'DpdyFine':('dpdyFine','dFdyFine','ddy_fine'),'FwidthFine':('fwidthFine','fwidthFine',None),
'DpdxCoarse':('dpdxCoarse','dFdxCoarse','ddx_coarse'),'DpdyCoarse':('dpdyCoarse','dFdyCoarse','ddy_coarse'),'FwidthCoarse':('fwidthCoarse','fwidthCoarse',None),})
# auxiliary derivatives are shader compiler feature-specific; tag as unavailable where not standard.
add('InterpolateAtCentroid InterpolateAtSample InterpolateAtOffset', 'Derivative','Special',(1,2),'Derivatives','Fragment', spell={
'InterpolateAtCentroid':(None,'interpolateAtCentroid',None),'InterpolateAtSample':(None,'interpolateAtSample',None),
'InterpolateAtOffset':(None,'interpolateAtOffset',None)})

textures='''TextureDimensions TextureNumLayers TextureNumLevels TextureNumSamples TextureLoad TextureStore TextureSample TextureSampleBias TextureSampleLevel TextureSampleGrad TextureSampleCompare TextureSampleCompareLevel TextureGather TextureGatherCompare TextureSampleBaseClampToEdge TextureSampleCompareBias TextureSampleCompareGrad TextureGatherRed TextureGatherGreen TextureGatherBlue TextureGatherAlpha TextureFetch TextureFetchOffset TextureFetchLod TextureFetchLodOffset TextureQueryLod TextureQueryLevels TextureQuerySamples TextureSize TextureSamples TextureOffset TextureProj TextureProjLod TextureProjGrad TextureGrad TextureLod TextureGatherOffset TextureGatherOffsets TextureGatherCompareOffset TextureGatherCompareOffsets'''
texture_map={name:(None,None,None) for name in textures.split()}
for name in 'TextureDimensions TextureNumLayers TextureNumLevels TextureNumSamples TextureLoad TextureStore TextureSample TextureSampleBias TextureSampleLevel TextureSampleGrad TextureSampleCompare TextureSampleCompareLevel TextureGather TextureGatherCompare TextureSampleBaseClampToEdge'.split():
    texture_map[name]=(name[0].lower()+name[1:],None,None)
for name,gl in {'TextureFetch':'texelFetch','TextureFetchOffset':'texelFetchOffset','TextureFetchLod':'texelFetch','TextureFetchLodOffset':'texelFetchOffset','TextureQueryLod':'textureQueryLod','TextureQueryLevels':'textureQueryLevels','TextureQuerySamples':'textureSamples','TextureSize':'textureSize','TextureSamples':'textureSamples','TextureOffset':'textureOffset','TextureProj':'textureProj','TextureProjLod':'textureProjLod','TextureProjGrad':'textureProjGrad','TextureGrad':'textureGrad','TextureLod':'textureLod','TextureGatherOffset':'textureGatherOffset','TextureGatherOffsets':'textureGatherOffsets','TextureGatherCompareOffset':'textureGatherOffset','TextureGatherCompareOffsets':'textureGatherOffsets'}.items():texture_map[name]=(None,gl,None)
# GLSL texture sampling works through combined samplers, therefore WGSL-style signatures are not translatable.
add(textures,'Texture','Special',(1,6),'Textures','Any',spell=texture_map)
# Atomic and workgroup operations have backend-specific pointer/statement semantics.
atomics='AtomicLoad AtomicStore AtomicAdd AtomicSub AtomicMax AtomicMin AtomicAnd AtomicOr AtomicXor AtomicExchange AtomicCompareExchangeWeak AtomicCompSwap AtomicInc AtomicDec AtomicCounter AtomicCounterIncrement AtomicCounterDecrement'
atomic_map={name:(None,None,None) for name in atomics.split()}
for name in 'AtomicLoad AtomicStore AtomicAdd AtomicSub AtomicMax AtomicMin AtomicAnd AtomicOr AtomicXor AtomicExchange AtomicCompareExchangeWeak'.split():atomic_map[name]=(name[0].lower()+name[1:],None,None)
for name,gl in {'AtomicAdd':'atomicAdd','AtomicMin':'atomicMin','AtomicMax':'atomicMax','AtomicAnd':'atomicAnd','AtomicOr':'atomicOr','AtomicXor':'atomicXor','AtomicExchange':'atomicExchange','AtomicCompSwap':'atomicCompSwap','AtomicCounterIncrement':'atomicCounterIncrement','AtomicCounterDecrement':'atomicCounterDecrement'}.items():
    a,_,c=atomic_map[name];atomic_map[name]=(a,gl,c)
add(atomics,'Atomic','Special',(1,4),'Atomics','Compute',spell=atomic_map)
barriers='WorkgroupBarrier StorageBarrier TextureBarrier Barrier MemoryBarrier MemoryBarrierBuffer MemoryBarrierImage GroupMemoryBarrier DeviceMemoryBarrier AllMemoryBarrier'
bar_map={x:(None,None,None) for x in barriers.split()}
bar_map.update(WorkgroupBarrier=('workgroupBarrier',None,None),StorageBarrier=('storageBarrier',None,None),TextureBarrier=('textureBarrier',None,None),Barrier=(None,'barrier',None),MemoryBarrier=(None,'memoryBarrier',None),MemoryBarrierBuffer=(None,'memoryBarrierBuffer',None),MemoryBarrierImage=(None,'memoryBarrierImage',None),GroupMemoryBarrier=(None,'groupMemoryBarrier',None),DeviceMemoryBarrier=(None,None,'DeviceMemoryBarrier'),AllMemoryBarrier=(None,None,'AllMemoryBarrier'))
add(barriers,'Barrier','Barrier',0,'Workgroup','Compute',spell=bar_map)

subgroups='''SubgroupSize SubgroupInvocationId SubgroupElect SubgroupBroadcast SubgroupBroadcastFirst SubgroupShuffle SubgroupShuffleXor SubgroupShuffleUp SubgroupShuffleDown SubgroupAll SubgroupAny SubgroupBallot SubgroupAllEqual SubgroupAdd SubgroupMul SubgroupMin SubgroupMax SubgroupAnd SubgroupOr SubgroupXor SubgroupInclusiveAdd SubgroupInclusiveMul SubgroupInclusiveMin SubgroupInclusiveMax SubgroupExclusiveAdd SubgroupExclusiveMul SubgroupExclusiveMin SubgroupExclusiveMax SubgroupQuadBroadcast SubgroupQuadSwapX SubgroupQuadSwapY SubgroupQuadSwapDiagonal SubgroupClusteredAdd SubgroupClusteredMul SubgroupClusteredMin SubgroupClusteredMax SubgroupPartitionedAdd SubgroupPartitionedInclusiveAdd SubgroupPartitionedExclusiveAdd'''
sub_map={x:(None,None,None) for x in subgroups.split()}
# HLSL SM6 Wave ops, GLSL subgroup extensions; no blanket WGSL subgroup guarantee.
for key,v in {
'SubgroupElect':(None,'subgroupElect','WaveIsFirstLane'),
'SubgroupBroadcast':(None,'subgroupBroadcast','WaveReadLaneAt'),
'SubgroupBroadcastFirst':(None,'subgroupBroadcastFirst','WaveReadLaneFirst'),
'SubgroupShuffle':(None,'subgroupShuffle','WaveReadLaneAt'),
'SubgroupShuffleXor':(None,'subgroupShuffleXor',None),
'SubgroupShuffleUp':(None,'subgroupShuffleUp',None),
'SubgroupShuffleDown':(None,'subgroupShuffleDown',None),
'SubgroupAll':(None,'subgroupAll','WaveActiveAllTrue'),
'SubgroupAny':(None,'subgroupAny','WaveActiveAnyTrue'),
'SubgroupBallot':(None,'subgroupBallot','WaveActiveBallot'),
'SubgroupAllEqual':(None,'subgroupAllEqual','WaveActiveAllEqual'),
'SubgroupAdd':(None,'subgroupAdd','WaveActiveSum'),
'SubgroupMul':(None,'subgroupMul','WaveActiveProduct'),
'SubgroupMin':(None,'subgroupMin','WaveActiveMin'),
'SubgroupMax':(None,'subgroupMax','WaveActiveMax'),
'SubgroupAnd':(None,'subgroupAnd','WaveActiveBitAnd'),
'SubgroupOr':(None,'subgroupOr','WaveActiveBitOr'),
'SubgroupXor':(None,'subgroupXor','WaveActiveBitXor'),
'SubgroupInclusiveAdd':(None,'subgroupInclusiveAdd',None),
'SubgroupInclusiveMul':(None,'subgroupInclusiveMul',None),
'SubgroupExclusiveAdd':(None,'subgroupExclusiveAdd',None),
'SubgroupExclusiveMul':(None,'subgroupExclusiveMul',None),
'SubgroupQuadBroadcast':(None,'subgroupQuadBroadcast','QuadReadLaneAt'),
'SubgroupQuadSwapX':(None,None,'QuadReadAcrossX'),
'SubgroupQuadSwapY':(None,None,'QuadReadAcrossY'),
'SubgroupQuadSwapDiagonal':(None,None,'QuadReadAcrossDiagonal'),
'SubgroupClusteredAdd':(None,'subgroupClusteredAdd',None),
'SubgroupClusteredMul':(None,'subgroupClusteredMul',None),
'SubgroupClusteredMin':(None,'subgroupClusteredMin',None),
'SubgroupClusteredMax':(None,'subgroupClusteredMax',None),
}.items():sub_map[key]=v
# A handful require extra shader extensions: caller must enable compiler/vendor capability.
add(subgroups,'Subgroup','Special',(0,2),'Subgroups','Any',spell=sub_map)

# GLSL/HLSL specific control/primitive helpers - zero source-generation deps.
for name,backend_map in {
'Discard':(None,None,None), # keyword/statement, not callable
'Clip':(None,None,'clip'),
'SinCos':(None,None,None), # HLSL requires out parameters
'MsAd4':(None,None,'msad4'),
'Dot4AddI8Packed':(None,None,'dot4add_i8packed'),
'Dot4AddU8Packed':(None,None,'dot4add_u8packed'),
'NonUniformIndex':(None,None,'NonUniformResourceIndex'),
'AsFloat':('bitcast<f32>', 'uintBitsToFloat','asfloat'),
'AsUInt':('bitcast<u32>', 'floatBitsToUint','asuint'),
'AsInt':('bitcast<i32>', 'floatBitsToInt','asint'),
'IsNan':(None,'isnan','isnan'),
'IsInf':(None,'isinf','isinf'),
'IsFinite':(None,None,'isfinite'),
}.items():
    add(name,'Vendor' if name in ('MsAd4','Dot4AddI8Packed','Dot4AddU8Packed') else 'Utility','Special',(1,4),
        'Vendor' if name in ('MsAd4','Dot4AddI8Packed','Dot4AddU8Packed') else 'Core',
        'Fragment' if name in ('Discard','Clip') else 'Any',spell={name:backend_map})

# Intel GPU execution dialect: OpenCL C source targeting Intel OpenCL runtime.
# Only built-ins known to work as ordinary OpenCL C functions or emitted operators.
# No Intel device, driver, SDK, or SPIR-V tools are required to *generate* source.
intel_core = {
  **{x: x.lower() for x in "Sin Cos Tan Asin Acos Atan Sinh Cosh Tanh Asinh Acosh Atanh Exp Exp2 Log Log2 Sqrt Floor Ceil Trunc Round Degrees Radians Pow Atan2 Min Max Clamp Fma Mix Step Smoothstep Length Distance Dot Cross Normalize Reflect Refract FaceForward".split()},
  'Abs':'fabs', 'Sign':'sign', 'InverseSqrt':'rsqrt', 'Ldexp':'ldexp',
  'CountOneBits':'popcount', 'CountLeadingZeros':'clz',
  'IsNan':'isnan', 'IsInf':'isinf', 'IsFinite':'isfinite',
  'AsFloat':'as_float','AsUInt':'as_uint','AsInt':'as_int',
  # Barriers are lowered with the OpenCL C memory fence flag by Emitter.
  'WorkgroupBarrier':'barrier','StorageBarrier':'barrier',
  # OpenCL builtins use subgroup names distinct from SPIR-V GLSL equivalents.
  'SubgroupSize':'get_sub_group_size','SubgroupInvocationId':'get_sub_group_local_id',
  'SubgroupElect':'sub_group_elect','SubgroupBroadcast':'sub_group_broadcast',
  'SubgroupAll':'sub_group_all','SubgroupAny':'sub_group_any',
  'SubgroupAdd':'sub_group_reduce_add','SubgroupMul':'sub_group_reduce_mul',
  'SubgroupMin':'sub_group_reduce_min','SubgroupMax':'sub_group_reduce_max',
  'SubgroupInclusiveAdd':'sub_group_scan_inclusive_add',
  'SubgroupInclusiveMin':'sub_group_scan_inclusive_min',
  'SubgroupInclusiveMax':'sub_group_scan_inclusive_max',
  'SubgroupExclusiveAdd':'sub_group_scan_exclusive_add',
  'SubgroupExclusiveMin':'sub_group_scan_exclusive_min',
  'SubgroupExclusiveMax':'sub_group_scan_exclusive_max',
  # Intel extension cl_intel_subgroups, available only on supporting devices.
  'SubgroupShuffle':'intel_sub_group_shuffle',
  'SubgroupShuffleXor':'intel_sub_group_shuffle_xor',
  'SubgroupShuffleUp':'intel_sub_group_shuffle_up',
  'SubgroupShuffleDown':'intel_sub_group_shuffle_down',
  'IntelSubGroupBlockRead':'intel_sub_group_block_read',
  'IntelSubGroupBlockRead2':'intel_sub_group_block_read2',
  'IntelSubGroupBlockRead4':'intel_sub_group_block_read4',
  'IntelSubGroupBlockRead8':'intel_sub_group_block_read8',
  'IntelSubGroupBlockWrite':'intel_sub_group_block_write',
  'IntelSubGroupBlockWrite2':'intel_sub_group_block_write2',
  'IntelSubGroupBlockWrite4':'intel_sub_group_block_write4',
  'IntelSubGroupBlockWrite8':'intel_sub_group_block_write8',
  'IntelSubGroupBlockReadUs':'intel_sub_group_block_read_us',
  'IntelSubGroupBlockReadUs2':'intel_sub_group_block_read_us2',
  'IntelSubGroupBlockReadUs4':'intel_sub_group_block_read_us4',
  'IntelSubGroupBlockReadUs8':'intel_sub_group_block_read_us8',
  'IntelSubGroupBlockWriteUs':'intel_sub_group_block_write_us',
  'IntelSubGroupBlockWriteUs2':'intel_sub_group_block_write_us2',
  'IntelSubGroupBlockWriteUs4':'intel_sub_group_block_write_us4',
  'IntelSubGroupBlockWriteUs8':'intel_sub_group_block_write_us8',
}
# OpenCL vector8, ushort/ushortN return values, and short pointers are not
# represented by the original four-lane f32/i32/u32 type model. Keep the
# variants in the catalog for discovery but do not claim emitter support.
for name in list(intel_core):
    if name.startswith('IntelSubGroupBlock') and (name.endswith('8') or 'Us' in name):
        del intel_core[name]
# Multi-value builtins, matrices, texture sampling, and atomics need target-specific
# lowering not provided by this string emitter, so they are intentionally unsupported.
# Intel subgroup block I/O operates on __global uint*/ushort* only in this API.
add('IntelSubGroupBlockRead IntelSubGroupBlockRead2 IntelSubGroupBlockRead4 IntelSubGroupBlockRead8 IntelSubGroupBlockWrite IntelSubGroupBlockWrite2 IntelSubGroupBlockWrite4 IntelSubGroupBlockWrite8',
    'Vendor','Special',(1,2),'IntelSubgroups', 'Compute',spell={n:(None,None,None) for n in 'IntelSubGroupBlockRead IntelSubGroupBlockRead2 IntelSubGroupBlockRead4 IntelSubGroupBlockRead8 IntelSubGroupBlockWrite IntelSubGroupBlockWrite2 IntelSubGroupBlockWrite4 IntelSubGroupBlockWrite8 IntelSubGroupBlockReadUs IntelSubGroupBlockReadUs2 IntelSubGroupBlockReadUs4 IntelSubGroupBlockReadUs8 IntelSubGroupBlockWriteUs IntelSubGroupBlockWriteUs2 IntelSubGroupBlockWriteUs4 IntelSubGroupBlockWriteUs8'.split()})
add('IntelSubGroupBlockReadUs IntelSubGroupBlockReadUs2 IntelSubGroupBlockReadUs4 IntelSubGroupBlockReadUs8 IntelSubGroupBlockWriteUs IntelSubGroupBlockWriteUs2 IntelSubGroupBlockWriteUs4 IntelSubGroupBlockWriteUs8',
    'Vendor','Special',(1,2),'IntelSubgroupsShort','Compute',spell={n:(None,None,None) for n in 'IntelSubGroupBlockRead IntelSubGroupBlockRead2 IntelSubGroupBlockRead4 IntelSubGroupBlockRead8 IntelSubGroupBlockWrite IntelSubGroupBlockWrite2 IntelSubGroupBlockWrite4 IntelSubGroupBlockWrite8 IntelSubGroupBlockReadUs IntelSubGroupBlockReadUs2 IntelSubGroupBlockReadUs4 IntelSubGroupBlockReadUs8 IntelSubGroupBlockWriteUs IntelSubGroupBlockWriteUs2 IntelSubGroupBlockWriteUs4 IntelSubGroupBlockWriteUs8'.split()})
# Restrict block I/O to its documented fixed arity, each read accepts one pointer,
# writes accept pointer and data. IntelSubgroupsShort requires both extensions.
intel_stage_only={n for n in intel_core if n.startswith('IntelSubGroupBlock')}
intel_extension_ops=set('SubgroupShuffle SubgroupShuffleXor SubgroupShuffleUp SubgroupShuffleDown'.split())

# Operator spellings are placeholders; the expression emitter lowers them to operators.
# Convert entries that are not safely expressible as normal function calls to catalog-only.
# NVIDIA CUDA device intrinsics use explicit warp masks. Generic subgroup spellings
# do not match CUDA's argument order, so those are deliberately not mapped.
add('NvidiaActiveMask', 'Vendor', 'Special', 0, 'NvidiaWarp', 'Compute', spell={'NvidiaActiveMask':(None,None,None)})
add('NvidiaBallotSync NvidiaAnySync NvidiaAllSync', 'Vendor', 'Special', 2, 'NvidiaWarp', 'Compute', spell={n:(None,None,None) for n in 'NvidiaBallotSync NvidiaAnySync NvidiaAllSync'.split()})
add('NvidiaShuffleSync NvidiaShuffleUpSync NvidiaShuffleDownSync NvidiaShuffleXorSync', 'Vendor', 'Special', (3,4), 'NvidiaWarp', 'Compute', spell={n:(None,None,None) for n in 'NvidiaShuffleSync NvidiaShuffleUpSync NvidiaShuffleDownSync NvidiaShuffleXorSync'.split()})
add('NvidiaSyncWarp', 'Vendor', 'Special', 1, 'NvidiaWarp', 'Compute', spell={'NvidiaSyncWarp':(None,None,None)})
add('NvidiaMatchAnySync', 'Vendor', 'Special', 2, 'NvidiaWarp', 'Compute', spell={'NvidiaMatchAnySync':(None,None,None)})
add('NvidiaReduceAddSync NvidiaReduceMinSync NvidiaReduceMaxSync NvidiaReduceAndSync NvidiaReduceOrSync NvidiaReduceXorSync', 'Vendor', 'Special', 2, 'NvidiaSm80', 'Compute', spell={n:(None,None,None) for n in 'NvidiaReduceAddSync NvidiaReduceMinSync NvidiaReduceMaxSync NvidiaReduceAndSync NvidiaReduceOrSync NvidiaReduceXorSync'.split()})

for i, entry in enumerate(ops):
    name,family,sig,low,high,cap,stage,wgsl,glsl,hlsl = entry
    if name == 'Saturate': wgsl = None # WGSL has no saturate builtin; clamp is portable.
    if name == 'Transpose': pass
    # Native qualifiers/out parameters require a more elaborate shader IR.
    if name in ('Discard','SinCos'): wgsl=glsl=hlsl=None
    # Conservative arities for useful runtime validation; backend shader validators remain authoritative.
    fixed = {
      'TextureDimensions':(1,2),'TextureNumLayers':(1,1),
      'TextureNumLevels':(1,1),'TextureNumSamples':(1,1),
      'TextureLoad':(2,4),'TextureStore':(3,4),
      'TextureSample':(3,5),'TextureSampleBias':(4,6),
      'TextureSampleLevel':(4,6),'TextureSampleGrad':(5,7),
      'TextureSampleCompare':(4,6),'TextureSampleCompareLevel':(4,6),
      'TextureGather':(3,5),'TextureGatherCompare':(4,6),
      'TextureSampleBaseClampToEdge':(3,3),
      'AtomicLoad':(1,1),'AtomicStore':(2,2),
      'AtomicAdd':(2,2),'AtomicSub':(2,2),'AtomicMax':(2,2),
      'AtomicMin':(2,2),'AtomicAnd':(2,2),'AtomicOr':(2,2),
      'AtomicXor':(2,2),'AtomicExchange':(2,2),
      'AtomicCompareExchangeWeak':(3,3),
      'Clip':(1,1), 'Dot4AddI8Packed':(3,3),'Dot4AddU8Packed':(3,3),
      'NonUniformIndex':(1,1), 'AsFloat':(1,1),'AsUInt':(1,1),
      'AsInt':(1,1), 'IsNan':(1,1),'IsInf':(1,1),'IsFinite':(1,1),
    }
    if name in fixed:low,high=fixed[name]
    if name == 'TextureStore': cap='StorageTextures'
    if name in ('TextureSample','TextureSampleBias','TextureSampleCompare',
                'TextureGather','TextureGatherCompare','TextureSampleBaseClampToEdge'):
        stage='Fragment' # implicit derivatives / uniform control-flow constraints
    if name=='Select': hlsl=None
    # A capability flag alone is insufficient for vendor functions, but prevents accidental use.
    intel=intel_core.get(name)
    # For core operator syntax, Emitter handles the rendering.
    if family=='Operator' and name not in ('Modulo',):
        intel=name
    if name=='Modulo':intel=None # OpenCL C float remainder uses fmod, not %.
    if name in ('RotateLeft','RotateRight'):intel=name
    if name in intel_extension_ops:cap='IntelSubgroups'
    if name.startswith('IntelSubGroupBlock'):
        low=high=1 if 'Read' in name else 2
    if name in ('SubgroupSize','SubgroupInvocationId'):low=high=0
    if name=='SubgroupElect':low=high=0
    if name=='SubgroupBroadcast':low=high=2
    if name in ('SubgroupShuffleDown','SubgroupShuffleUp'):low=high=3
    if name in intel_stage_only:stage='Compute'
    ops[i]=(name,family,sig,low,high,cap,stage,wgsl,glsl,hlsl,intel)

# ROCm HIP + AMDGCN intrinsics. Operation spellings are defined and typed in
# src/amd.rs; do not advertise these as WGSL/GLSL/HLSL/OpenCL/CUDA.
amd_wave={
 'AmdWaveSize':0,'AmdLaneId':0,'AmdShuffle':2,'AmdShuffleXor':2,
 'AmdShuffleUp':2,'AmdShuffleDown':2,'AmdBallot':1,'AmdActiveMask':0,
 'AmdAny':1,'AmdAll':1,'AmdWaveSync':0,
}
amd_gcn={
 'AmdReadFirstLane':1,'AmdReadLane':2,'AmdWriteLane':3,
 'AmdDsBpermute':2,'AmdDsPermute':2,
}
amd_core={
 'AmdFastSin':1,'AmdFastCos':1,'AmdFastExp':1,'AmdFastLog':1,
 'AmdFastLog2':1,'AmdFastPow':2,'AmdFastRcp':1,
 'AmdBytePerm':3,'AmdFunnelShiftLeft':3,'AmdFunnelShiftRight':3,
 'AmdMul24':2,'AmdUMul24':2,'AmdSad':3,'AmdUSad':3,
 'AmdPopcount64':1,'AmdReverseBits64':1,
 'AmdThreadFenceBlock':0,'AmdThreadFenceDevice':0,
 'AmdThreadFenceSystem':0,
}
for group, cap in [(amd_wave,'AmdWave'),(amd_gcn,'AmdGcn'),(amd_core,'Core')]:
    for name,arity in group.items():
        add(name,'Vendor','Special',arity,cap,'Compute',spell={name:(None,None,None)})
        ops[-1]=ops[-1]+(None,)  # normalized Intel column, no Intel mapping

names=[x[0] for x in ops]
assert len(names)==len(set(names)), [n for n,c in Counter(names).items() if c>1]
assert len(ops)>=200,len(ops)

def option(x):return 'Some("'+x+'")' if x is not None else 'None'
lines=['//! Generated intrinsic registry; update `tools/generate_catalog.py` and regenerate.','use crate::types::{Capability, Stage};', '',
'#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]',
'/// Logical operation group (not necessarily a hardware instruction).',
'pub enum Family { Math, Geometry, Matrix, Operator, Selection, Comparison, Bits, Packing, Derivative, Texture, Atomic, Barrier, Subgroup, Vendor, Utility }',
'#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]',
'/// Type-validation strategy used when emitting an operation.',
'pub enum Signature { UnaryFloat, UnaryNumber, UnaryInteger, BinaryFloat, BinaryNumber, BinaryInteger, TernaryFloat, TernaryNumber, UnaryFloatVector, BinaryFloatVector, TernaryFloatVector, Length, Distance, Dot, Cross, Refract, Matrix, MatrixScalar, Equality, Comparison, BoolBinary, BoolUnary, ReductionBool, Pack, Unpack, Barrier, Special }',
'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
'/// Inclusive number-of-arguments constraints.',
'pub struct Arity { pub min:u8, pub max:u8 }',
'#[derive(Clone, Copy, Debug)]',
'/// Metadata: spellings of native calls (None means unavailable), signature, stage and capability.',
'pub struct IntrinsicInfo { pub op:Intrinsic, pub name:&\'static str, pub family:Family, pub signature:Signature, pub arity:Arity, pub capability:Capability, pub stage:Option<Stage>, pub wgsl:Option<&\'static str>, pub glsl:Option<&\'static str>, pub hlsl:Option<&\'static str>, pub intel_opencl:Option<&\'static str> }',
'impl IntrinsicInfo {\n    /// Return the target builtin spelling or `None` when unsupported.\n    pub fn spelling(&self, backend:crate::Backend)->Option<&\'static str> {match backend {crate::Backend::Wgsl=>self.wgsl,crate::Backend::Glsl=>self.glsl,crate::Backend::Hlsl=>self.hlsl,crate::Backend::IntelOpenCl=>self.intel_opencl,crate::Backend::NvidiaCuda=>crate::nvidia::spelling(self.op),crate::Backend::AmdHip=>crate::amd::spelling(self.op)}}\n}',
'/// Operation identifier. Some variants are catalog-only and deliberately cannot be emitted.',
'#[repr(u16)]','#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]','pub enum Intrinsic {']
for name,family,sig,low,high,cap,stage,wgsl,glsl,hlsl,intel in ops:
    lines.append(f'    /// {name}: {family.lower()} operation; {low}–{high} arguments; capability {cap}; stage {stage}.')
    lines.append('    '+name+',')
lines+=['}', 'pub static ALL_INTRINSICS:&[Intrinsic]=&[']
for name,*_ in ops:lines.append('    Intrinsic::'+name+',')
lines+=[''];lines+=['    ];','/// Number of catalogued shader operations.',
'pub const INTRINSIC_COUNT:usize=ALL_INTRINSICS.len();','static INFOS:&[IntrinsicInfo]=&[']
for name,family,sig,mini,maxi,cap,stage,wgsl,glsl,hlsl,intel in ops:
    st='None' if stage=='Any' else 'Some(Stage::'+stage+')'
    lines.append(f'    IntrinsicInfo {{op:Intrinsic::{name}, name:"{name[0].lower()+name[1:]}",family:Family::{family},signature:Signature::{sig},arity:Arity{{min:{mini},max:{maxi}}},capability:Capability::{cap},stage:{st},wgsl:{option(wgsl)},glsl:{option(glsl)},hlsl:{option(hlsl)},intel_opencl:{option(intel)}}},')
lines+=['    ];', '''impl Intrinsic {
    /// Retrieve metadata without querying the GPU.
    pub fn info(self)->&'static IntrinsicInfo{&INFOS[self as usize]}
    /// Look up a catalog operation by its stable camel-case identifier.
    pub fn from_name(name:&str)->Option<Self>{
        ALL_INTRINSICS.iter().copied().find(|v| v.info().name==name)
    }
}
''']
p=Path(__file__).parents[1]/'src/catalog.rs';p.write_text('\n'.join(lines))
print('Generated',len(ops),'operations;',Counter(x[1] for x in ops))
print('Mappings:',Counter(b for v in ops for b,s in zip(['WGSL','GLSL','HLSL','IntelOpenCL'],v[-4:]) if s is not None))
