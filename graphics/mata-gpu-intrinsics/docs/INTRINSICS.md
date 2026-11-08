# GPU Intrinsics Reference — all supported operations

> **Scope:** This is a Rust **catalog and shader-source expression emitter**, not a list of Rust machine intrinsics or a GPU driver. GPU op availability depends on shader target, compiler version, feature flags, and device at **runtime**. Building this crate never compiles GPU shaders or detects an adapter.

## Reading the tables

- **Enum** is the `Intrinsic::Variant` used from Rust. **Native name** is the spelling emitted or documented for WGSL / GLSL / HLSL / Intel OpenCL C / NVIDIA CUDA C++ / AMD HIP C++. `—` indicates that the implementation refuses emission for that backend.
- Operators and synthetic bit rotates are expanded by `Emitter` even though their catalog native spelling is metadata only.
- **Arguments** is a conservative pre-validation range; exact overload and argument types remain the responsibility of the selected shader compiler.
- **Stage** `Any` means not restricted by this catalogue. Certain operations still carry target-specific shader-stage restrictions and uniformity requirements.
- **Capability** must be enabled with `CapabilitySet::with`; this models a requested capability, **not** a successful runtime device capability query.
- Some entries are **catalog-only** (no valid normal function-call emission). Examples include shader statements requiring syntax not representable as an expression or extension-only operations without a safe cross-backend mapping.

## Summary

**322 total registered operations** across 15 families.

| Family | Count |
|---|---:|
| Math | 41 |
| Geometry | 8 |
| Matrix | 3 |
| Operator | 21 |
| Selection | 1 |
| Comparison | 2 |
| Bits | 20 |
| Packing | 28 |
| Derivative | 12 |
| Texture | 40 |
| Atomic | 17 |
| Barrier | 10 |
| Subgroup | 39 |
| Utility | 10 |
| Vendor | 70 |

## Complete operation catalog

### Math (41)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `Abs` | Absolute value | 1 | `Core` | `Any` | `abs` | `abs` | `abs` | `fabs` | `fabsf` | `fabsf` |
| `Sign` | Sign of a number | 1 | `Core` | `Any` | `sign` | `sign` | `sign` | `sign` | `sign` | `sign` |
| `Sin` | Sine | 1 | `Core` | `Any` | `sin` | `sin` | `sin` | `sin` | `sinf` | `sinf` |
| `Cos` | Cosine | 1 | `Core` | `Any` | `cos` | `cos` | `cos` | `cos` | `cosf` | `cosf` |
| `Tan` | Tangent | 1 | `Core` | `Any` | `tan` | `tan` | `tan` | `tan` | `tanf` | `tanf` |
| `Asin` | Inverse sine | 1 | `Core` | `Any` | `asin` | `asin` | `asin` | `asin` | `asinf` | `asinf` |
| `Acos` | Inverse cosine | 1 | `Core` | `Any` | `acos` | `acos` | `acos` | `acos` | `acosf` | `acosf` |
| `Atan` | Inverse tangent | 1 | `Core` | `Any` | `atan` | `atan` | `atan` | `atan` | `atanf` | `atanf` |
| `Sinh` | Hyperbolic sine | 1 | `Core` | `Any` | `sinh` | `sinh` | `sinh` | `sinh` | `sinhf` | `sinhf` |
| `Cosh` | Hyperbolic cosine | 1 | `Core` | `Any` | `cosh` | `cosh` | `cosh` | `cosh` | `coshf` | `coshf` |
| `Tanh` | Hyperbolic tangent | 1 | `Core` | `Any` | `tanh` | `tanh` | `tanh` | `tanh` | `tanhf` | `tanhf` |
| `Asinh` | Inverse hyperbolic sine | 1 | `Core` | `Any` | `asinh` | `asinh` | `asinh` | `asinh` | `asinhf` | `asinhf` |
| `Acosh` | Inverse hyperbolic cosine | 1 | `Core` | `Any` | `acosh` | `acosh` | `acosh` | `acosh` | `acoshf` | `acoshf` |
| `Atanh` | Inverse hyperbolic tangent | 1 | `Core` | `Any` | `atanh` | `atanh` | `atanh` | `atanh` | `atanhf` | `atanhf` |
| `Exp` | Natural exponential | 1 | `Core` | `Any` | `exp` | `exp` | `exp` | `exp` | `expf` | `expf` |
| `Exp2` | Base-two exponential | 1 | `Core` | `Any` | `exp2` | `exp2` | `exp2` | `exp2` | `exp2f` | `exp2f` |
| `Log` | Natural logarithm | 1 | `Core` | `Any` | `log` | `log` | `log` | `log` | `logf` | `logf` |
| `Log2` | Base-two logarithm | 1 | `Core` | `Any` | `log2` | `log2` | `log2` | `log2` | `log2f` | `log2f` |
| `Sqrt` | Square root | 1 | `Core` | `Any` | `sqrt` | `sqrt` | `sqrt` | `sqrt` | `sqrtf` | `sqrtf` |
| `InverseSqrt` | Reciprocal square root | 1 | `Core` | `Any` | `inverseSqrt` | `inversesqrt` | `rsqrt` | `rsqrt` | `rsqrtf` | `rsqrtf` |
| `Floor` | Round toward negative infinity | 1 | `Core` | `Any` | `floor` | `floor` | `floor` | `floor` | `floorf` | `floorf` |
| `Ceil` | Round toward positive infinity | 1 | `Core` | `Any` | `ceil` | `ceil` | `ceil` | `ceil` | `ceilf` | `ceilf` |
| `Trunc` | Truncate toward zero | 1 | `Core` | `Any` | `trunc` | `trunc` | `trunc` | `trunc` | `truncf` | `truncf` |
| `Round` | Round to nearest integer | 1 | `Core` | `Any` | `round` | `round` | `round` | `round` | `roundf` | `roundf` |
| `Fract` | Fractional part | 1 | `Core` | `Any` | `fract` | `fract` | `frac` | — | `fract` | `fract` |
| `Degrees` | Convert radians to degrees | 1 | `Core` | `Any` | `degrees` | `degrees` | `degrees` | `degrees` | `degrees` | `degrees` |
| `Radians` | Convert degrees to radians | 1 | `Core` | `Any` | `radians` | `radians` | `radians` | `radians` | `radians` | `radians` |
| `Pow` | Raise a value to a power | 2 | `Core` | `Any` | `pow` | `pow` | `pow` | `pow` | `powf` | `powf` |
| `Atan2` | Two-argument inverse tangent | 2 | `Core` | `Any` | `atan2` | `atan` | `atan2` | `atan2` | `atan2f` | `atan2f` |
| `Min` | Minimum | 2 | `Core` | `Any` | `min` | `min` | `min` | `min` | `fminf` | `fminf` |
| `Max` | Maximum | 2 | `Core` | `Any` | `max` | `max` | `max` | `max` | `fmaxf` | `fmaxf` |
| `Clamp` | Clamp within two bounds | 3 | `Core` | `Any` | `clamp` | `clamp` | `clamp` | `clamp` | `fminf` | `fminf` |
| `Fma` | Multiply-add (contraction may depend on target) | 3 | `Core` | `Any` | `fma` | `fma` | `mad` | `fma` | `fmaf` | `fmaf` |
| `Saturate` | Clamp to [0,1] (not valid in every target) | 1 | `Core` | `Any` | — | — | `saturate` | — | `fminf` | `fminf` |
| `Mix` | Interpolate/mix values | 3 | `Core` | `Any` | `mix` | `mix` | `lerp` | `mix` | `lerp` | `lerp` |
| `Step` | Step at a threshold | 2 | `Core` | `Any` | `step` | `step` | `step` | `step` | `step` | `step` |
| `Smoothstep` | Smooth Hermite threshold | 3 | `Core` | `Any` | `smoothstep` | `smoothstep` | `smoothstep` | `smoothstep` | `smoothstep` | `smoothstep` |
| `Modf` | Fractional and integral components (multi-value result) | 1 | `Core` | `Any` | `modf` | — | — | — | — | — |
| `Frexp` | Mantissa and exponent (multi-value result) | 1 | `Core` | `Any` | `frexp` | — | — | — | — | — |
| `Ldexp` | Multiply by a power of two | 2 | `Core` | `Any` | `ldexp` | `ldexp` | `ldexp` | `ldexp` | — | — |
| `QuantizeToF16` | Quantize numeric precision to 16-bit float | 1 | `F16` | `Any` | `quantizeToF16` | — | — | — | — | — |

### Geometry (8)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `Length` | Euclidean vector length | 1 | `Core` | `Any` | `length` | `length` | `length` | `length` | — | — |
| `Distance` | Euclidean distance | 2 | `Core` | `Any` | `distance` | `distance` | `distance` | `distance` | — | — |
| `Dot` | Vector dot product | 2 | `Core` | `Any` | `dot` | `dot` | `dot` | `dot` | — | — |
| `Cross` | 3D vector cross product | 2 | `Core` | `Any` | `cross` | `cross` | `cross` | `cross` | — | — |
| `Normalize` | Unit-length direction | 1 | `Core` | `Any` | `normalize` | `normalize` | `normalize` | `normalize` | — | — |
| `Reflect` | Reflection about surface normal | 2 | `Core` | `Any` | `reflect` | `reflect` | `reflect` | `reflect` | — | — |
| `Refract` | Refraction about surface normal | 3 | `Core` | `Any` | `refract` | `refract` | `refract` | `refract` | — | — |
| `FaceForward` | Orient normal against an incident direction | 3 | `Core` | `Any` | `faceForward` | `faceforward` | `faceforward` | `faceforward` | — | — |

### Matrix (3)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `Transpose` | Transpose a matrix | 1 | `Core` | `Any` | `transpose` | `transpose` | `transpose` | — | — | — |
| `Determinant` | Compute a square-matrix determinant | 1 | `Core` | `Any` | `determinant` | `determinant` | `determinant` | — | — | — |
| `Inverse` | Invert a square matrix | 1 | `Core` | `Any` | — | `inverse` | — | — | — | — |

### Operator (21)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `Add` | Addition | 2 | `Core` | `Any` | `Add` | `Add` | `Add` | `Add` | `+` | `+` |
| `Sub` | Subtraction | 2 | `Core` | `Any` | `Sub` | `Sub` | `Sub` | `Sub` | `-` | `-` |
| `Multiply` | Multiplication | 2 | `Core` | `Any` | `Multiply` | `Multiply` | `Multiply` | `Multiply` | `*` | `*` |
| `Divide` | Division | 2 | `Core` | `Any` | `Divide` | `Divide` | `Divide` | `Divide` | `/` | `/` |
| `Modulo` | Remainder | 2 | `Core` | `Any` | `Modulo` | `Modulo` | `Modulo` | — | `%` | `%` |
| `Negate` | Unary negation | 1 | `Core` | `Any` | `Negate` | `Negate` | `Negate` | `Negate` | `-` | `-` |
| `Equal` | Equality comparison | 2 | `Core` | `Any` | `Equal` | `Equal` | `Equal` | `Equal` | `==` | `==` |
| `NotEqual` | Inequality comparison | 2 | `Core` | `Any` | `NotEqual` | `NotEqual` | `NotEqual` | `NotEqual` | `!=` | `!=` |
| `Less` | Less-than comparison | 2 | `Core` | `Any` | `Less` | `Less` | `Less` | `Less` | `<` | `<` |
| `LessEqual` | Less-than-or-equal comparison | 2 | `Core` | `Any` | `LessEqual` | `LessEqual` | `LessEqual` | `LessEqual` | `<=` | `<=` |
| `Greater` | Greater-than comparison | 2 | `Core` | `Any` | `Greater` | `Greater` | `Greater` | `Greater` | `>` | `>` |
| `GreaterEqual` | Greater-than-or-equal comparison | 2 | `Core` | `Any` | `GreaterEqual` | `GreaterEqual` | `GreaterEqual` | `GreaterEqual` | `>=` | `>=` |
| `LogicalAnd` | Boolean AND | 2 | `Core` | `Any` | `LogicalAnd` | `LogicalAnd` | `LogicalAnd` | `LogicalAnd` | `&&` | `&&` |
| `LogicalOr` | Boolean OR | 2 | `Core` | `Any` | `LogicalOr` | `LogicalOr` | `LogicalOr` | `LogicalOr` | `||` | `||` |
| `LogicalNot` | Boolean NOT | 1 | `Core` | `Any` | `LogicalNot` | `LogicalNot` | `LogicalNot` | `LogicalNot` | `!` | `!` |
| `BitAnd` | Bitwise AND | 2 | `Core` | `Any` | `BitAnd` | `BitAnd` | `BitAnd` | `BitAnd` | `&` | `&` |
| `BitOr` | Bitwise OR | 2 | `Core` | `Any` | `BitOr` | `BitOr` | `BitOr` | `BitOr` | `|` | `|` |
| `BitXor` | Bitwise exclusive OR | 2 | `Core` | `Any` | `BitXor` | `BitXor` | `BitXor` | `BitXor` | `^` | `^` |
| `ShiftLeft` | Bit shift left | 2 | `Core` | `Any` | `ShiftLeft` | `ShiftLeft` | `ShiftLeft` | `ShiftLeft` | `<<` | `<<` |
| `ShiftRight` | Bit shift right | 2 | `Core` | `Any` | `ShiftRight` | `ShiftRight` | `ShiftRight` | `ShiftRight` | `>>` | `>>` |
| `BitNot` | Bitwise complement | 1 | `Core` | `Any` | `BitNot` | `BitNot` | `BitNot` | `BitNot` | `~` | `~` |

### Selection (1)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `Select` | Choose a value using a Boolean mask | 3 | `Core` | `Any` | `select` | `mix` | — | — | `?:` | `?:` |

### Comparison (2)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `All` | Check whether every vector component is true | 1 | `Core` | `Any` | `all` | `all` | `all` | — | — | — |
| `Any` | Check whether some component is true | 1 | `Core` | `Any` | `any` | `any` | `any` | — | — | — |

### Bits (20)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `CountOneBits` | Population count | 1 | `Core` | `Any` | `countOneBits` | `bitCount` | `countbits` | `popcount` | `__popc` | `__popc` |
| `ReverseBits` | Reverse bit order | 1 | `Core` | `Any` | `reverseBits` | `bitfieldReverse` | `reversebits` | — | `__brev` | `__brev` |
| `FirstLeadingBit` | First set bit from the high end (target semantics vary) | 1 | `Core` | `Any` | `firstLeadingBit` | `findMSB` | `firstbithigh` | — | — | — |
| `FirstTrailingBit` | First set bit from the low end | 1 | `Core` | `Any` | `firstTrailingBit` | `findLSB` | `firstbitlow` | — | — | — |
| `CountLeadingZeros` | Count high-order zero bits | 1 | `Core` | `Any` | `countLeadingZeros` | — | `firstbithigh` | `clz` | — | — |
| `CountTrailingZeros` | Count low-order zero bits | 1 | `Core` | `Any` | `countTrailingZeros` | — | `firstbitlow` | — | — | — |
| `FindLsb` | Bit-manipulation operation | 1 | `Core` | `Any` | — | `findLSB` | `firstbitlow` | — | — | — |
| `FindMsb` | Bit-manipulation operation | 1 | `Core` | `Any` | — | `findMSB` | `firstbithigh` | — | — | — |
| `ExtractBits` | Bit-manipulation operation | 3 | `Core` | `Any` | `extractBits` | `bitfieldExtract` | — | — | — | — |
| `InsertBits` | Bit-manipulation operation | 4 | `Core` | `Any` | `insertBits` | `bitfieldInsert` | — | — | — | — |
| `RotateLeft` | Rotate bits to the left (synthesized expression) | 2 | `Core` | `Any` | `rotateLeft` | `rotateLeft` | `rotateLeft` | `RotateLeft` | `rotl` | `rotl` |
| `RotateRight` | Rotate bits to the right (synthesized expression) | 2 | `Core` | `Any` | `rotateRight` | `rotateRight` | `rotateRight` | `RotateRight` | `rotr` | `rotr` |
| `BitfieldMask` | Bit-manipulation operation | 2 | `Core` | `Any` | — | `bitfieldMask` | — | — | — | — |
| `BitfieldReverse` | Bit-manipulation operation | 1 | `Core` | `Any` | — | `bitfieldReverse` | `reversebits` | — | `__brev` | `__brev` |
| `BitfieldExtractSigned` | Bit-manipulation operation | 3–4 | `Core` | `Any` | — | — | — | — | — | — |
| `BitfieldExtractUnsigned` | Bit-manipulation operation | 3–4 | `Core` | `Any` | — | — | — | — | — | — |
| `BitfieldInsertSigned` | Bit-manipulation operation | 3–4 | `Core` | `Any` | — | — | — | — | — | — |
| `BitfieldCount` | Bit-manipulation operation | 1 | `Core` | `Any` | — | `bitCount` | `countbits` | — | — | — |
| `FindFirstSet` | Bit-manipulation operation | 1 | `Core` | `Any` | — | `findLSB` | `firstbitlow` | — | — | — |
| `FindLastSet` | Bit-manipulation operation | 1 | `Core` | `Any` | — | `findMSB` | `firstbithigh` | — | — | — |

### Packing (28)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `Pack4x8Unorm` | Pack packed numeric representation | 1 | `Packing` | `Any` | `pack4x8unorm` | `packUnorm4x8` | — | — | — | — |
| `Pack4x8Snorm` | Pack packed numeric representation | 1 | `Packing` | `Any` | `pack4x8snorm` | `packSnorm4x8` | — | — | — | — |
| `Pack2x16Unorm` | Pack packed numeric representation | 1 | `Packing` | `Any` | `pack2x16unorm` | `packUnorm2x16` | — | — | — | — |
| `Pack2x16Snorm` | Pack packed numeric representation | 1 | `Packing` | `Any` | `pack2x16snorm` | `packSnorm2x16` | — | — | — | — |
| `Pack2x16Float` | Pack two half-precision floats into u32 | 1 | `Packing` | `Any` | `pack2x16float` | `packHalf2x16` | — | — | — | — |
| `Pack4xI8` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | — | — | — | — | — |
| `Pack4xU8` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | — | — | — | — | — |
| `Pack4xI8Clamp` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | — | — | — | — | — |
| `Pack4xU8Clamp` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | — | — | — | — | — |
| `Pack4xI8Saturate` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | — | — | — | — | — |
| `Pack4xU8Saturate` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | — | — | — | — | — |
| `Unpack4x8Unorm` | Unpack packed numeric representation | 1 | `Packing` | `Any` | `unpack4x8unorm` | `unpackUnorm4x8` | — | — | — | — |
| `Unpack4x8Snorm` | Unpack packed numeric representation | 1 | `Packing` | `Any` | `unpack4x8snorm` | `unpackSnorm4x8` | — | — | — | — |
| `Unpack2x16Unorm` | Unpack packed numeric representation | 1 | `Packing` | `Any` | `unpack2x16unorm` | `unpackUnorm2x16` | — | — | — | — |
| `Unpack2x16Snorm` | Unpack packed numeric representation | 1 | `Packing` | `Any` | `unpack2x16snorm` | `unpackSnorm2x16` | — | — | — | — |
| `Unpack2x16Float` | Unpack two half-precision floats | 1 | `Packing` | `Any` | `unpack2x16float` | `unpackHalf2x16` | — | — | — | — |
| `Unpack4xI8` | Unpack packed numeric representation | 1 | `Packing` | `Any` | — | — | — | — | — | — |
| `Unpack4xU8` | Unpack packed numeric representation | 1 | `Packing` | `Any` | — | — | — | — | — | — |
| `PackSnorm2x16` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | `packSnorm2x16` | — | — | — | — |
| `PackUnorm2x16` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | `packUnorm2x16` | — | — | — | — |
| `PackHalf2x16` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | `packHalf2x16` | — | — | — | — |
| `PackSnorm4x8` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | `packSnorm4x8` | — | — | — | — |
| `PackUnorm4x8` | Pack packed numeric representation | 1 | `Packing` | `Any` | — | `packUnorm4x8` | — | — | — | — |
| `UnpackSnorm2x16` | Unpack packed numeric representation | 1 | `Packing` | `Any` | — | `unpackSnorm2x16` | — | — | — | — |
| `UnpackUnorm2x16` | Unpack packed numeric representation | 1 | `Packing` | `Any` | — | `unpackUnorm2x16` | — | — | — | — |
| `UnpackHalf2x16` | Unpack packed numeric representation | 1 | `Packing` | `Any` | — | `unpackHalf2x16` | — | — | — | — |
| `UnpackSnorm4x8` | Unpack packed numeric representation | 1 | `Packing` | `Any` | — | `unpackSnorm4x8` | — | — | — | — |
| `UnpackUnorm4x8` | Unpack packed numeric representation | 1 | `Packing` | `Any` | — | `unpackUnorm4x8` | — | — | — | — |

### Derivative (12)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `Dpdx` | Partial derivative in screen X | 1 | `Derivatives` | `Fragment` | `dpdx` | `dFdx` | `ddx` | — | — | — |
| `Dpdy` | Partial derivative in screen Y | 1 | `Derivatives` | `Fragment` | `dpdy` | `dFdy` | `ddy` | — | — | — |
| `Fwidth` | Sum of absolute derivatives | 1 | `Derivatives` | `Fragment` | `fwidth` | `fwidth` | `fwidth` | — | — | — |
| `DpdxFine` | Fragment derivative or sample interpolation operation | 1 | `Derivatives` | `Fragment` | `dpdxFine` | `dFdxFine` | `ddx_fine` | — | — | — |
| `DpdyFine` | Fragment derivative or sample interpolation operation | 1 | `Derivatives` | `Fragment` | `dpdyFine` | `dFdyFine` | `ddy_fine` | — | — | — |
| `FwidthFine` | Fragment derivative or sample interpolation operation | 1 | `Derivatives` | `Fragment` | `fwidthFine` | `fwidthFine` | — | — | — | — |
| `DpdxCoarse` | Fragment derivative or sample interpolation operation | 1 | `Derivatives` | `Fragment` | `dpdxCoarse` | `dFdxCoarse` | `ddx_coarse` | — | — | — |
| `DpdyCoarse` | Fragment derivative or sample interpolation operation | 1 | `Derivatives` | `Fragment` | `dpdyCoarse` | `dFdyCoarse` | `ddy_coarse` | — | — | — |
| `FwidthCoarse` | Fragment derivative or sample interpolation operation | 1 | `Derivatives` | `Fragment` | `fwidthCoarse` | `fwidthCoarse` | — | — | — | — |
| `InterpolateAtCentroid` | Fragment derivative or sample interpolation operation | 1–2 | `Derivatives` | `Fragment` | — | `interpolateAtCentroid` | — | — | — | — |
| `InterpolateAtSample` | Fragment derivative or sample interpolation operation | 1–2 | `Derivatives` | `Fragment` | — | `interpolateAtSample` | — | — | — | — |
| `InterpolateAtOffset` | Fragment derivative or sample interpolation operation | 1–2 | `Derivatives` | `Fragment` | — | `interpolateAtOffset` | — | — | — | — |

### Texture (40)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `TextureDimensions` | Read texture dimensions | 1–2 | `Textures` | `Any` | `textureDimensions` | — | — | — | — | — |
| `TextureNumLayers` | Texture num layers operation (backend overloads vary) | 1 | `Textures` | `Any` | `textureNumLayers` | — | — | — | — | — |
| `TextureNumLevels` | Texture num levels operation (backend overloads vary) | 1 | `Textures` | `Any` | `textureNumLevels` | — | — | — | — | — |
| `TextureNumSamples` | Texture num samples operation (backend overloads vary) | 1 | `Textures` | `Any` | `textureNumSamples` | — | — | — | — | — |
| `TextureLoad` | Read texture texel using integer coordinates | 2–4 | `Textures` | `Any` | `textureLoad` | — | — | — | — | — |
| `TextureStore` | Write a storage texture texel | 3–4 | `StorageTextures` | `Any` | `textureStore` | — | — | — | — | — |
| `TextureSample` | Sample texture at normalized coordinates using a sampler | 3–5 | `Textures` | `Fragment` | `textureSample` | — | — | — | — | — |
| `TextureSampleBias` | Texture sample bias operation (backend overloads vary) | 4–6 | `Textures` | `Fragment` | `textureSampleBias` | — | — | — | — | — |
| `TextureSampleLevel` | Texture sample level operation (backend overloads vary) | 4–6 | `Textures` | `Any` | `textureSampleLevel` | — | — | — | — | — |
| `TextureSampleGrad` | Texture sample grad operation (backend overloads vary) | 5–7 | `Textures` | `Any` | `textureSampleGrad` | — | — | — | — | — |
| `TextureSampleCompare` | Texture sample compare operation (backend overloads vary) | 4–6 | `Textures` | `Fragment` | `textureSampleCompare` | — | — | — | — | — |
| `TextureSampleCompareLevel` | Texture sample compare level operation (backend overloads vary) | 4–6 | `Textures` | `Any` | `textureSampleCompareLevel` | — | — | — | — | — |
| `TextureGather` | Texture gather operation (backend overloads vary) | 3–5 | `Textures` | `Fragment` | `textureGather` | — | — | — | — | — |
| `TextureGatherCompare` | Texture gather compare operation (backend overloads vary) | 4–6 | `Textures` | `Fragment` | `textureGatherCompare` | — | — | — | — | — |
| `TextureSampleBaseClampToEdge` | Texture sample base clamp to edge operation (backend overloads vary) | 3 | `Textures` | `Fragment` | `textureSampleBaseClampToEdge` | — | — | — | — | — |
| `TextureSampleCompareBias` | Texture sample compare bias operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | — | — | — | — | — |
| `TextureSampleCompareGrad` | Texture sample compare grad operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | — | — | — | — | — |
| `TextureGatherRed` | Texture gather red operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | — | — | — | — | — |
| `TextureGatherGreen` | Texture gather green operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | — | — | — | — | — |
| `TextureGatherBlue` | Texture gather blue operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | — | — | — | — | — |
| `TextureGatherAlpha` | Texture gather alpha operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | — | — | — | — | — |
| `TextureFetch` | Texture fetch operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `texelFetch` | — | — | — | — |
| `TextureFetchOffset` | Texture fetch offset operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `texelFetchOffset` | — | — | — | — |
| `TextureFetchLod` | Texture fetch lod operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `texelFetch` | — | — | — | — |
| `TextureFetchLodOffset` | Texture fetch lod offset operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `texelFetchOffset` | — | — | — | — |
| `TextureQueryLod` | Texture query lod operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureQueryLod` | — | — | — | — |
| `TextureQueryLevels` | Texture query levels operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureQueryLevels` | — | — | — | — |
| `TextureQuerySamples` | Texture query samples operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureSamples` | — | — | — | — |
| `TextureSize` | Texture size operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureSize` | — | — | — | — |
| `TextureSamples` | Texture samples operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureSamples` | — | — | — | — |
| `TextureOffset` | Texture offset operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureOffset` | — | — | — | — |
| `TextureProj` | Texture proj operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureProj` | — | — | — | — |
| `TextureProjLod` | Texture proj lod operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureProjLod` | — | — | — | — |
| `TextureProjGrad` | Texture proj grad operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureProjGrad` | — | — | — | — |
| `TextureGrad` | Texture grad operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureGrad` | — | — | — | — |
| `TextureLod` | Texture lod operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureLod` | — | — | — | — |
| `TextureGatherOffset` | Texture gather offset operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureGatherOffset` | — | — | — | — |
| `TextureGatherOffsets` | Texture gather offsets operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureGatherOffsets` | — | — | — | — |
| `TextureGatherCompareOffset` | Texture gather compare offset operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureGatherOffset` | — | — | — | — |
| `TextureGatherCompareOffsets` | Texture gather compare offsets operation (backend overloads vary) | 1–6 | `Textures` | `Any` | — | `textureGatherOffsets` | — | — | — | — |

### Atomic (17)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `AtomicLoad` | Read an atomic integer | 1 | `Atomics` | `Compute` | `atomicLoad` | — | — | — | — | — |
| `AtomicStore` | Write an atomic integer | 2 | `Atomics` | `Compute` | `atomicStore` | — | — | — | — | — |
| `AtomicAdd` | Atomic add operation | 2 | `Atomics` | `Compute` | `atomicAdd` | `atomicAdd` | — | — | `atomicAdd` | `atomicAdd` |
| `AtomicSub` | Atomic sub operation | 2 | `Atomics` | `Compute` | `atomicSub` | — | — | — | — | — |
| `AtomicMax` | Atomic max operation | 2 | `Atomics` | `Compute` | `atomicMax` | `atomicMax` | — | — | `atomicMax` | `atomicMax` |
| `AtomicMin` | Atomic min operation | 2 | `Atomics` | `Compute` | `atomicMin` | `atomicMin` | — | — | `atomicMin` | `atomicMin` |
| `AtomicAnd` | Atomic and operation | 2 | `Atomics` | `Compute` | `atomicAnd` | `atomicAnd` | — | — | `atomicAnd` | `atomicAnd` |
| `AtomicOr` | Atomic or operation | 2 | `Atomics` | `Compute` | `atomicOr` | `atomicOr` | — | — | `atomicOr` | `atomicOr` |
| `AtomicXor` | Atomic xor operation | 2 | `Atomics` | `Compute` | `atomicXor` | `atomicXor` | — | — | `atomicXor` | `atomicXor` |
| `AtomicExchange` | Atomic exchange operation | 2 | `Atomics` | `Compute` | `atomicExchange` | `atomicExchange` | — | — | `atomicExch` | `atomicExch` |
| `AtomicCompareExchangeWeak` | Compare and optionally replace atomic value | 3 | `Atomics` | `Compute` | `atomicCompareExchangeWeak` | — | — | — | — | — |
| `AtomicCompSwap` | Atomic compswap operation | 1–4 | `Atomics` | `Compute` | — | `atomicCompSwap` | — | — | `atomicCAS` | `atomicCAS` |
| `AtomicInc` | Atomic inc operation | 1–4 | `Atomics` | `Compute` | — | — | — | — | — | — |
| `AtomicDec` | Atomic dec operation | 1–4 | `Atomics` | `Compute` | — | — | — | — | — | — |
| `AtomicCounter` | Atomic counter operation | 1–4 | `Atomics` | `Compute` | — | — | — | — | — | — |
| `AtomicCounterIncrement` | Atomic counterincrement operation | 1–4 | `Atomics` | `Compute` | — | `atomicCounterIncrement` | — | — | — | — |
| `AtomicCounterDecrement` | Atomic counterdecrement operation | 1–4 | `Atomics` | `Compute` | — | `atomicCounterDecrement` | — | — | — | — |

### Barrier (10)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `WorkgroupBarrier` | Synchronize compute workgroup memory operations | 0 | `Workgroup` | `Compute` | `workgroupBarrier` | — | — | `barrier` | `__syncthreads` | `__syncthreads` |
| `StorageBarrier` | Synchronize storage operations in the workgroup | 0 | `Workgroup` | `Compute` | `storageBarrier` | — | — | `barrier` | — | — |
| `TextureBarrier` | GPU memory synchronization barrier | 0 | `Workgroup` | `Compute` | `textureBarrier` | — | — | — | — | — |
| `Barrier` | GPU memory synchronization barrier | 0 | `Workgroup` | `Compute` | — | `barrier` | — | — | — | — |
| `MemoryBarrier` | GPU memory synchronization barrier | 0 | `Workgroup` | `Compute` | — | `memoryBarrier` | — | — | — | — |
| `MemoryBarrierBuffer` | GPU memory synchronization barrier | 0 | `Workgroup` | `Compute` | — | `memoryBarrierBuffer` | — | — | — | — |
| `MemoryBarrierImage` | GPU memory synchronization barrier | 0 | `Workgroup` | `Compute` | — | `memoryBarrierImage` | — | — | — | — |
| `GroupMemoryBarrier` | GPU memory synchronization barrier | 0 | `Workgroup` | `Compute` | — | `groupMemoryBarrier` | — | — | — | — |
| `DeviceMemoryBarrier` | GPU memory synchronization barrier | 0 | `Workgroup` | `Compute` | — | — | `DeviceMemoryBarrier` | — | — | — |
| `AllMemoryBarrier` | GPU memory synchronization barrier | 0 | `Workgroup` | `Compute` | — | — | `AllMemoryBarrier` | — | — | — |

### Subgroup (39)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `SubgroupSize` | Subgroup size operation (wave/extension support required) | 0 | `Subgroups` | `Any` | — | — | — | `get_sub_group_size` | — | — |
| `SubgroupInvocationId` | Subgroup invocationid operation (wave/extension support required) | 0 | `Subgroups` | `Any` | — | — | — | `get_sub_group_local_id` | — | — |
| `SubgroupElect` | Subgroup elect operation (wave/extension support required) | 0 | `Subgroups` | `Any` | — | `subgroupElect` | `WaveIsFirstLane` | `sub_group_elect` | — | — |
| `SubgroupBroadcast` | Subgroup broadcast operation (wave/extension support required) | 2 | `Subgroups` | `Any` | — | `subgroupBroadcast` | `WaveReadLaneAt` | `sub_group_broadcast` | — | — |
| `SubgroupBroadcastFirst` | Subgroup broadcastfirst operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupBroadcastFirst` | `WaveReadLaneFirst` | — | — | — |
| `SubgroupShuffle` | Subgroup shuffle operation (wave/extension support required) | 0–2 | `IntelSubgroups` | `Any` | — | `subgroupShuffle` | `WaveReadLaneAt` | `intel_sub_group_shuffle` | — | — |
| `SubgroupShuffleXor` | Subgroup shufflexor operation (wave/extension support required) | 0–2 | `IntelSubgroups` | `Any` | — | `subgroupShuffleXor` | — | `intel_sub_group_shuffle_xor` | — | — |
| `SubgroupShuffleUp` | Subgroup shuffleup operation (wave/extension support required) | 3 | `IntelSubgroups` | `Any` | — | `subgroupShuffleUp` | — | `intel_sub_group_shuffle_up` | — | — |
| `SubgroupShuffleDown` | Subgroup shuffledown operation (wave/extension support required) | 3 | `IntelSubgroups` | `Any` | — | `subgroupShuffleDown` | — | `intel_sub_group_shuffle_down` | — | — |
| `SubgroupAll` | Subgroup all operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupAll` | `WaveActiveAllTrue` | `sub_group_all` | — | — |
| `SubgroupAny` | Subgroup any operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupAny` | `WaveActiveAnyTrue` | `sub_group_any` | — | — |
| `SubgroupBallot` | Subgroup ballot operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupBallot` | `WaveActiveBallot` | — | — | — |
| `SubgroupAllEqual` | Subgroup allequal operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupAllEqual` | `WaveActiveAllEqual` | — | — | — |
| `SubgroupAdd` | Subgroup add operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupAdd` | `WaveActiveSum` | `sub_group_reduce_add` | — | — |
| `SubgroupMul` | Subgroup mul operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupMul` | `WaveActiveProduct` | `sub_group_reduce_mul` | — | — |
| `SubgroupMin` | Subgroup min operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupMin` | `WaveActiveMin` | `sub_group_reduce_min` | — | — |
| `SubgroupMax` | Subgroup max operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupMax` | `WaveActiveMax` | `sub_group_reduce_max` | — | — |
| `SubgroupAnd` | Subgroup and operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupAnd` | `WaveActiveBitAnd` | — | — | — |
| `SubgroupOr` | Subgroup or operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupOr` | `WaveActiveBitOr` | — | — | — |
| `SubgroupXor` | Subgroup xor operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupXor` | `WaveActiveBitXor` | — | — | — |
| `SubgroupInclusiveAdd` | Subgroup inclusiveadd operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupInclusiveAdd` | — | `sub_group_scan_inclusive_add` | — | — |
| `SubgroupInclusiveMul` | Subgroup inclusivemul operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupInclusiveMul` | — | — | — | — |
| `SubgroupInclusiveMin` | Subgroup inclusivemin operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | — | — | `sub_group_scan_inclusive_min` | — | — |
| `SubgroupInclusiveMax` | Subgroup inclusivemax operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | — | — | `sub_group_scan_inclusive_max` | — | — |
| `SubgroupExclusiveAdd` | Subgroup exclusiveadd operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupExclusiveAdd` | — | `sub_group_scan_exclusive_add` | — | — |
| `SubgroupExclusiveMul` | Subgroup exclusivemul operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupExclusiveMul` | — | — | — | — |
| `SubgroupExclusiveMin` | Subgroup exclusivemin operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | — | — | `sub_group_scan_exclusive_min` | — | — |
| `SubgroupExclusiveMax` | Subgroup exclusivemax operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | — | — | `sub_group_scan_exclusive_max` | — | — |
| `SubgroupQuadBroadcast` | Subgroup quadbroadcast operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupQuadBroadcast` | `QuadReadLaneAt` | — | — | — |
| `SubgroupQuadSwapX` | Subgroup quadswapx operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | — | `QuadReadAcrossX` | — | — | — |
| `SubgroupQuadSwapY` | Subgroup quadswapy operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | — | `QuadReadAcrossY` | — | — | — |
| `SubgroupQuadSwapDiagonal` | Subgroup quadswapdiagonal operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | — | `QuadReadAcrossDiagonal` | — | — | — |
| `SubgroupClusteredAdd` | Subgroup clusteredadd operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupClusteredAdd` | — | — | — | — |
| `SubgroupClusteredMul` | Subgroup clusteredmul operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupClusteredMul` | — | — | — | — |
| `SubgroupClusteredMin` | Subgroup clusteredmin operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupClusteredMin` | — | — | — | — |
| `SubgroupClusteredMax` | Subgroup clusteredmax operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | `subgroupClusteredMax` | — | — | — | — |
| `SubgroupPartitionedAdd` | Subgroup partitionedadd operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | — | — | — | — | — |
| `SubgroupPartitionedInclusiveAdd` | Subgroup partitionedinclusiveadd operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | — | — | — | — | — |
| `SubgroupPartitionedExclusiveAdd` | Subgroup partitionedexclusiveadd operation (wave/extension support required) | 0–2 | `Subgroups` | `Any` | — | — | — | — | — | — |

### Utility (10)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `Discard` | Terminate fragment (statement; not callable) | 1–4 | `Core` | `Fragment` | — | — | — | — | — | — |
| `Clip` | Utility operation | 1 | `Core` | `Fragment` | — | — | `clip` | — | — | — |
| `SinCos` | Compute sine and cosine via output parameters (catalog-only) | 1–4 | `Core` | `Any` | — | — | — | — | — | — |
| `NonUniformIndex` | Utility operation | 1 | `Core` | `Any` | — | — | `NonUniformResourceIndex` | — | — | — |
| `AsFloat` | Utility operation | 1 | `Core` | `Any` | `bitcast<f32>` | `uintBitsToFloat` | `asfloat` | `as_float` | — | — |
| `AsUInt` | Utility operation | 1 | `Core` | `Any` | `bitcast<u32>` | `floatBitsToUint` | `asuint` | `as_uint` | — | — |
| `AsInt` | Utility operation | 1 | `Core` | `Any` | `bitcast<i32>` | `floatBitsToInt` | `asint` | `as_int` | — | — |
| `IsNan` | Utility operation | 1 | `Core` | `Any` | — | `isnan` | `isnan` | `isnan` | — | — |
| `IsInf` | Utility operation | 1 | `Core` | `Any` | — | `isinf` | `isinf` | `isinf` | — | — |
| `IsFinite` | Utility operation | 1 | `Core` | `Any` | — | — | `isfinite` | `isfinite` | — | — |

### Vendor (70)

| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |
|---|---|---:|---|---|---|---|---|---|---|---|
| `MsAd4` | Vendor/compiler-version-specific operation | 1–4 | `Vendor` | `Any` | — | — | `msad4` | — | — | — |
| `Dot4AddI8Packed` | Vendor/compiler-version-specific operation | 3 | `Vendor` | `Any` | — | — | `dot4add_i8packed` | — | — | — |
| `Dot4AddU8Packed` | Vendor/compiler-version-specific operation | 3 | `Vendor` | `Any` | — | — | `dot4add_u8packed` | — | — | — |
| `IntelSubGroupBlockRead` | Vendor/compiler-version-specific operation | 1 | `IntelSubgroups` | `Compute` | — | — | — | `intel_sub_group_block_read` | — | — |
| `IntelSubGroupBlockRead2` | Vendor/compiler-version-specific operation | 1 | `IntelSubgroups` | `Compute` | — | — | — | `intel_sub_group_block_read2` | — | — |
| `IntelSubGroupBlockRead4` | Vendor/compiler-version-specific operation | 1 | `IntelSubgroups` | `Compute` | — | — | — | `intel_sub_group_block_read4` | — | — |
| `IntelSubGroupBlockRead8` | Vendor/compiler-version-specific operation | 1 | `IntelSubgroups` | `Compute` | — | — | — | — | — | — |
| `IntelSubGroupBlockWrite` | Vendor/compiler-version-specific operation | 2 | `IntelSubgroups` | `Compute` | — | — | — | `intel_sub_group_block_write` | — | — |
| `IntelSubGroupBlockWrite2` | Vendor/compiler-version-specific operation | 2 | `IntelSubgroups` | `Compute` | — | — | — | `intel_sub_group_block_write2` | — | — |
| `IntelSubGroupBlockWrite4` | Vendor/compiler-version-specific operation | 2 | `IntelSubgroups` | `Compute` | — | — | — | `intel_sub_group_block_write4` | — | — |
| `IntelSubGroupBlockWrite8` | Vendor/compiler-version-specific operation | 2 | `IntelSubgroups` | `Compute` | — | — | — | — | — | — |
| `IntelSubGroupBlockReadUs` | Vendor/compiler-version-specific operation | 1 | `IntelSubgroupsShort` | `Compute` | — | — | — | — | — | — |
| `IntelSubGroupBlockReadUs2` | Vendor/compiler-version-specific operation | 1 | `IntelSubgroupsShort` | `Compute` | — | — | — | — | — | — |
| `IntelSubGroupBlockReadUs4` | Vendor/compiler-version-specific operation | 1 | `IntelSubgroupsShort` | `Compute` | — | — | — | — | — | — |
| `IntelSubGroupBlockReadUs8` | Vendor/compiler-version-specific operation | 1 | `IntelSubgroupsShort` | `Compute` | — | — | — | — | — | — |
| `IntelSubGroupBlockWriteUs` | Vendor/compiler-version-specific operation | 2 | `IntelSubgroupsShort` | `Compute` | — | — | — | — | — | — |
| `IntelSubGroupBlockWriteUs2` | Vendor/compiler-version-specific operation | 2 | `IntelSubgroupsShort` | `Compute` | — | — | — | — | — | — |
| `IntelSubGroupBlockWriteUs4` | Vendor/compiler-version-specific operation | 2 | `IntelSubgroupsShort` | `Compute` | — | — | — | — | — | — |
| `IntelSubGroupBlockWriteUs8` | Vendor/compiler-version-specific operation | 2 | `IntelSubgroupsShort` | `Compute` | — | — | — | — | — | — |
| `NvidiaActiveMask` | Vendor/compiler-version-specific operation | 0 | `NvidiaWarp` | `Compute` | — | — | — | — | `__activemask` | — |
| `NvidiaBallotSync` | Vendor/compiler-version-specific operation | 2 | `NvidiaWarp` | `Compute` | — | — | — | — | `__ballot_sync` | — |
| `NvidiaAnySync` | Vendor/compiler-version-specific operation | 2 | `NvidiaWarp` | `Compute` | — | — | — | — | `__any_sync` | — |
| `NvidiaAllSync` | Vendor/compiler-version-specific operation | 2 | `NvidiaWarp` | `Compute` | — | — | — | — | `__all_sync` | — |
| `NvidiaShuffleSync` | Vendor/compiler-version-specific operation | 3–4 | `NvidiaWarp` | `Compute` | — | — | — | — | `__shfl_sync` | — |
| `NvidiaShuffleUpSync` | Vendor/compiler-version-specific operation | 3–4 | `NvidiaWarp` | `Compute` | — | — | — | — | `__shfl_up_sync` | — |
| `NvidiaShuffleDownSync` | Vendor/compiler-version-specific operation | 3–4 | `NvidiaWarp` | `Compute` | — | — | — | — | `__shfl_down_sync` | — |
| `NvidiaShuffleXorSync` | Vendor/compiler-version-specific operation | 3–4 | `NvidiaWarp` | `Compute` | — | — | — | — | `__shfl_xor_sync` | — |
| `NvidiaSyncWarp` | Vendor/compiler-version-specific operation | 1 | `NvidiaWarp` | `Compute` | — | — | — | — | `__syncwarp` | — |
| `NvidiaMatchAnySync` | Vendor/compiler-version-specific operation | 2 | `NvidiaWarp` | `Compute` | — | — | — | — | `__match_any_sync` | — |
| `NvidiaReduceAddSync` | Vendor/compiler-version-specific operation | 2 | `NvidiaSm80` | `Compute` | — | — | — | — | `__reduce_add_sync` | — |
| `NvidiaReduceMinSync` | Vendor/compiler-version-specific operation | 2 | `NvidiaSm80` | `Compute` | — | — | — | — | `__reduce_min_sync` | — |
| `NvidiaReduceMaxSync` | Vendor/compiler-version-specific operation | 2 | `NvidiaSm80` | `Compute` | — | — | — | — | `__reduce_max_sync` | — |
| `NvidiaReduceAndSync` | Vendor/compiler-version-specific operation | 2 | `NvidiaSm80` | `Compute` | — | — | — | — | `__reduce_and_sync` | — |
| `NvidiaReduceOrSync` | Vendor/compiler-version-specific operation | 2 | `NvidiaSm80` | `Compute` | — | — | — | — | `__reduce_or_sync` | — |
| `NvidiaReduceXorSync` | Vendor/compiler-version-specific operation | 2 | `NvidiaSm80` | `Compute` | — | — | — | — | `__reduce_xor_sync` | — |
| `AmdWaveSize` | Vendor/compiler-version-specific operation | 0 | `AmdWave` | `Compute` | — | — | — | — | — | `warpSize` |
| `AmdLaneId` | Vendor/compiler-version-specific operation | 0 | `AmdWave` | `Compute` | — | — | — | — | — | `__lane_id` |
| `AmdShuffle` | Vendor/compiler-version-specific operation | 2 | `AmdWave` | `Compute` | — | — | — | — | — | `__shfl` |
| `AmdShuffleXor` | Vendor/compiler-version-specific operation | 2 | `AmdWave` | `Compute` | — | — | — | — | — | `__shfl_xor` |
| `AmdShuffleUp` | Vendor/compiler-version-specific operation | 2 | `AmdWave` | `Compute` | — | — | — | — | — | `__shfl_up` |
| `AmdShuffleDown` | Vendor/compiler-version-specific operation | 2 | `AmdWave` | `Compute` | — | — | — | — | — | `__shfl_down` |
| `AmdBallot` | Vendor/compiler-version-specific operation | 1 | `AmdWave` | `Compute` | — | — | — | — | — | `__ballot` |
| `AmdActiveMask` | Vendor/compiler-version-specific operation | 0 | `AmdWave` | `Compute` | — | — | — | — | — | `__activemask` |
| `AmdAny` | Vendor/compiler-version-specific operation | 1 | `AmdWave` | `Compute` | — | — | — | — | — | `__any` |
| `AmdAll` | Vendor/compiler-version-specific operation | 1 | `AmdWave` | `Compute` | — | — | — | — | — | `__all` |
| `AmdWaveSync` | Vendor/compiler-version-specific operation | 0 | `AmdWave` | `Compute` | — | — | — | — | — | `__syncwarp` |
| `AmdReadFirstLane` | Vendor/compiler-version-specific operation | 1 | `AmdGcn` | `Compute` | — | — | — | — | — | `__builtin_amdgcn_readfirstlane` |
| `AmdReadLane` | Vendor/compiler-version-specific operation | 2 | `AmdGcn` | `Compute` | — | — | — | — | — | `__builtin_amdgcn_readlane` |
| `AmdWriteLane` | Vendor/compiler-version-specific operation | 3 | `AmdGcn` | `Compute` | — | — | — | — | — | `__builtin_amdgcn_writelane` |
| `AmdDsBpermute` | Vendor/compiler-version-specific operation | 2 | `AmdGcn` | `Compute` | — | — | — | — | — | `__builtin_amdgcn_ds_bpermute` |
| `AmdDsPermute` | Vendor/compiler-version-specific operation | 2 | `AmdGcn` | `Compute` | — | — | — | — | — | `__builtin_amdgcn_ds_permute` |
| `AmdFastSin` | Vendor/compiler-version-specific operation | 1 | `Core` | `Compute` | — | — | — | — | — | `__sinf` |
| `AmdFastCos` | Vendor/compiler-version-specific operation | 1 | `Core` | `Compute` | — | — | — | — | — | `__cosf` |
| `AmdFastExp` | Vendor/compiler-version-specific operation | 1 | `Core` | `Compute` | — | — | — | — | — | `__expf` |
| `AmdFastLog` | Vendor/compiler-version-specific operation | 1 | `Core` | `Compute` | — | — | — | — | — | `__logf` |
| `AmdFastLog2` | Vendor/compiler-version-specific operation | 1 | `Core` | `Compute` | — | — | — | — | — | `__log2f` |
| `AmdFastPow` | Vendor/compiler-version-specific operation | 2 | `Core` | `Compute` | — | — | — | — | — | `__powf` |
| `AmdFastRcp` | Vendor/compiler-version-specific operation | 1 | `Core` | `Compute` | — | — | — | — | — | `__frcp_rn` |
| `AmdBytePerm` | Vendor/compiler-version-specific operation | 3 | `Core` | `Compute` | — | — | — | — | — | `__byte_perm` |
| `AmdFunnelShiftLeft` | Vendor/compiler-version-specific operation | 3 | `Core` | `Compute` | — | — | — | — | — | `__funnelshift_l` |
| `AmdFunnelShiftRight` | Vendor/compiler-version-specific operation | 3 | `Core` | `Compute` | — | — | — | — | — | `__funnelshift_r` |
| `AmdMul24` | Vendor/compiler-version-specific operation | 2 | `Core` | `Compute` | — | — | — | — | — | `__mul24` |
| `AmdUMul24` | Vendor/compiler-version-specific operation | 2 | `Core` | `Compute` | — | — | — | — | — | `__umul24` |
| `AmdSad` | Vendor/compiler-version-specific operation | 3 | `Core` | `Compute` | — | — | — | — | — | `__sad` |
| `AmdUSad` | Vendor/compiler-version-specific operation | 3 | `Core` | `Compute` | — | — | — | — | — | `__usad` |
| `AmdPopcount64` | Vendor/compiler-version-specific operation | 1 | `Core` | `Compute` | — | — | — | — | — | `__popcll` |
| `AmdReverseBits64` | Vendor/compiler-version-specific operation | 1 | `Core` | `Compute` | — | — | — | — | — | `__brevll` |
| `AmdThreadFenceBlock` | Vendor/compiler-version-specific operation | 0 | `Core` | `Compute` | — | — | — | — | — | `__threadfence_block` |
| `AmdThreadFenceDevice` | Vendor/compiler-version-specific operation | 0 | `Core` | `Compute` | — | — | — | — | — | `__threadfence` |
| `AmdThreadFenceSystem` | Vendor/compiler-version-specific operation | 0 | `Core` | `Compute` | — | — | — | — | — | `__threadfence_system` |

## Compatibility and correctness

1. **Compilation without a GPU:** Rust compiles catalog metadata and string-generation code with zero external dependencies; shader code runs only after your renderer compiles it on the desired device.
2. **Optional capabilities:** Derivatives, packing, textures, storage access, atomics, subgroup/wave functions, barriers and vendor features may require GLSL extensions, shader model versions, WebGPU extensions, driver support or specific pipeline stages.
3. **No automatic cross-compilation:** WGSL, GLSL and HLSL have differences in sampler/resource types, packing semantics, atomics, wave operations and return structures. An emitted expression is **not** proof a complete shader will validate.
4. **Source-only:** The emitter returns expressions and standalone helper-function definitions. It does not compile SPIR-V, DXIL or Metal AIR; it does not construct GPU pipelines or submit commands.
5. **Security:** `Expr::from_source_unchecked` accepts caller-provided shader code verbatim. Do not pass untrusted strings.

See [Rust API](API.md) and the crate [README](../README.md).
