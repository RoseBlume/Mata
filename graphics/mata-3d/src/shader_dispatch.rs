//! GPU-independent HLSL source generation for Mata's D3D11 renderer.
//!
//! Shader intrinsic names and validation come from `mata-gpu-intrinsics`
//! function pointer tables; **not** hard-coded backend spelling. This module
//! never opens a device and works on Linux, macOS and Windows at build/test time.
//! The returned source is compiled to shader bytecode at runtime by D3DCompiler
//! on Windows. OpenCL/CUDA/HIP are separate compute languages, not substitutes
//! for this D3D11 graphics shader pipeline.

use mata_gpu_intrinsics::{Backend, CapabilitySet, Expr, FunctionTable, Intrinsic, Stage, Type};

/// Broadcast a scalar to a three-component HLSL vector for the catalog's
/// homogeneous Mix signature. D3DCompiler's SM 4.0 numeric constructor
/// requires exactly three scalar components: `float3(x)` is invalid.
fn hlsl_broadcast_f32_to_float3(scalar: &Expr) -> Expr {
    debug_assert_eq!(scalar.ty(), Type::F32);
    let x = scalar.code();
    Expr::from_source_unchecked(format!("float3({x}, {x}, {x})"), Type::VEC3)
}

/// Source texts for the actual geometry and composition shader stages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct D3d11Shaders {
    /// Includes `GVS` and `GPS` for the native triangle rendering pass.
    pub geometry: String,
    /// GSI/PSI: true D3D11 DrawInstanced atom sphere shader.
    pub sphere_instances: String,
    /// GBI/PBI: shared GPU cylinder mesh with per-bond transform.
    pub bond_instances: String,
    /// Includes `VS` and `PS` for the HUD/scene fullscreen composition pass.
    pub composite: String,
    /// Compositor that generates the original scene sky gradient on the GPU;
    /// removes the full-screen CPU background texture upload every frame.
    pub composite_sky: String,
}

impl D3d11Shaders {
    /// Construct the default shader programs using core HLSL intrinsic pointers.
    /// No GPU, shader compiler or graphics driver is needed here.
    pub fn new() -> Result<Self, String> {
        let fragment = FunctionTable::hlsl(Stage::Fragment, CapabilitySet::CORE);
        Self::from_table(&fragment)
    }

    /// Generate both shaders using the caller's backend-specific function table.
    /// Only HLSL fragment tables are suitable for this Direct3D 11 program.
    /// Fails early for an incompatible language, shader stage, or missing op.
    pub fn from_table(fragment: &FunctionTable) -> Result<Self, String> {
        if fragment.backend() != Backend::Hlsl || fragment.stage() != Stage::Fragment {
            return Err("D3D11 requires a fragment-stage HLSL function table".into());
        }
        let intrinsic = |op, args: &[Expr]| fragment.call(op, args)
            .map_err(|error| format!("HLSL {:?} intrinsic failed: {error}", op));
        let saturated_alpha = intrinsic(Intrinsic::Saturate, &[
            Expr::from_source_unchecked("h.a", Type::F32),
        ])?;
        // `float3(scalar)` is rejected by the legacy D3DCompiler (X3014).
        // Supply all three constructor components explicitly. The catalog's
        // Mix intrinsic requires the interpolation weight to be a float3.
        let alpha3 = hlsl_broadcast_f32_to_float3(&saturated_alpha);
        let rgb = intrinsic(Intrinsic::Mix, &[
            Expr::from_source_unchecked("s.rgb", Type::VEC3),
            Expr::from_source_unchecked("h.rgb", Type::VEC3),
            alpha3,
        ])?;
        let composed_rgb = intrinsic(Intrinsic::Saturate, &[rgb])?;
        let geometry_color = intrinsic(Intrinsic::Saturate, &[
            Expr::from_source_unchecked("i.color", Type::VEC4),
        ])?;
        let sky_alpha = intrinsic(Intrinsic::Saturate, &[
            Expr::from_source_unchecked("s.a", Type::F32),
        ])?;
        let scene_with_sky = intrinsic(Intrinsic::Mix, &[
            Expr::from_source_unchecked("sky", Type::VEC3),
            Expr::from_source_unchecked("s.rgb", Type::VEC3),
            hlsl_broadcast_f32_to_float3(&sky_alpha),
        ])?;
        let composite_sky_rgb = intrinsic(Intrinsic::Mix, &[
            scene_with_sky,
            Expr::from_source_unchecked("h.rgb", Type::VEC3),
            hlsl_broadcast_f32_to_float3(&saturated_alpha),
        ])?;
        let composite_sky_saturated = intrinsic(Intrinsic::Saturate, &[composite_sky_rgb])?;
        // This is the same two-part sky used by mata-neon-run/src/render.rs,
        // including the floor/horizon transition. Generated background pixels
        // are composited only where the scene target has alpha == 0.
        let composite_sky = format!(r#"
Texture2D scene : register(t0);
Texture2D overlay : register(t1);
SamplerState samplePoint : register(s0);
struct Output {{ float4 pos : SV_POSITION; float2 uv : TEXCOORD0; }};
Output VS(uint id : SV_VertexID) {{
    Output o;
    float2 xy = id == 0 ? float2(-1.0,-1.0)
              : id == 1 ? float2(-1.0, 3.0) : float2(3.0,-1.0);
    o.pos = float4(xy,0.0,1.0);
    o.uv = float2((xy.x+1.0)*0.5,(1.0-xy.y)*0.5);
    return o;
}}
float4 PS(Output i) : SV_TARGET {{
    float4 s = scene.SampleLevel(samplePoint,i.uv,0.0);
    float4 h = overlay.SampleLevel(samplePoint,i.uv,0.0);
    float t = saturate(i.uv.y);
    float3 sky = (t < 0.53)
        ? float3(5.0+25.0*t, 13.0+43.0*t, 30.0+62.0*t) / 255.0
        : float3(6.0+5.0*t, 20.0+8.0*t, 31.0+6.0*t) / 255.0;
    return float4({},1.0);
}}
"#, composite_sky_saturated.code());
        let composite = format!(r#"
Texture2D scene : register(t0);
Texture2D overlay : register(t1);
SamplerState samplePoint : register(s0);
struct Output {{ float4 pos : SV_POSITION; float2 uv : TEXCOORD0; }};
Output VS(uint id : SV_VertexID) {{
    Output o;
    float2 xy = id == 0 ? float2(-1.0,-1.0)
              : id == 1 ? float2(-1.0, 3.0) : float2(3.0,-1.0);
    o.pos = float4(xy,0.0,1.0);
    o.uv = float2((xy.x+1.0)*0.5,(1.0-xy.y)*0.5);
    return o;
}}
float4 PS(Output i) : SV_TARGET {{
    float4 s = scene.SampleLevel(samplePoint,i.uv,0.0);
    float4 h = overlay.SampleLevel(samplePoint,i.uv,0.0);
    return float4({},1.0);
}}
"#, composed_rgb.code());
        let geometry = format!(r#"
struct In {{float3 position : POSITION; float4 color : COLOR;}};
struct Out {{float4 position : SV_POSITION; float4 color : COLOR;}};
Out GVS(In i) {{ Out o; o.position=float4(i.position,1.0); o.color=i.color; return o; }}
float4 GPS(Out i):SV_TARGET {{return {};}}
"#, geometry_color.code());
        // The shared mesh holds unit-sphere vertices only. One 32-byte input
        // record supplies the screen-space center, radius/depth and color.
        // Perspective-correct expansion happens once per vertex on the GPU.
        let sphere_instances=r#"
struct SphereIn {
    float3 unit : POSITION;
    float4 center : TEXCOORD0; // NDC x/y, view-space depth, world radius
    float2 scale : TEXCOORD1;  // projected NDC radius x/y
    float nearPlane : TEXCOORD2;
    float4 color : COLOR;
};
struct SphereOut {
    float4 position : SV_POSITION;
    float4 color : COLOR;
    float3 normal : TEXCOORD3;
};
SphereOut GSI(SphereIn i) {
    SphereOut o;
    // The camera faces positive Z. Positive sphere-unit Z is the near side.
    float z=max(i.center.z-i.unit.z*i.center.w,i.nearPlane);
    float factor=i.center.z/z;
    // Keep the projected center fixed inside an offset UI viewport.
    o.position=float4(i.center.xy+i.unit.xy*i.scale*factor,
                      saturate(i.nearPlane/z),1.0);
    o.normal=i.unit;
    o.color=i.color;
    return o;
}
float4 PSI(SphereOut i) : SV_TARGET {
    float diffuse=saturate(dot(normalize(i.normal),normalize(float3(0.5,0.8,0.5))));
    return float4(saturate(i.color.rgb*(0.62+0.38*diffuse)),i.color.a);
}
"#.to_string();
        let bond_instances=r#"
struct BondIn {
    float3 unit : POSITION; // circle x/y, segment t=0..1
    float4 start : TEXCOORD0; // view x/y/z, bond radius
    float4 end : TEXCOORD1;   // view x/y/z, near plane
    float4 projection : TEXCOORD2; // NDC x/y scales, origin x/y
    float4 color : COLOR;
};
struct BondOut {
    float4 position : SV_POSITION;
    float4 color : COLOR;
    float3 normal : TEXCOORD3;
};
BondOut GBI(BondIn i) {
    BondOut o;
    float3 axis=normalize(i.end.xyz-i.start.xyz);
    float3 reference=(abs(axis.y)<0.9)?float3(0,1,0):float3(1,0,0);
    float3 u=normalize(cross(axis,reference));
    float3 v=normalize(cross(axis,u));
    float3 normal=u*i.unit.x+v*i.unit.y;
    float3 pos=lerp(i.start.xyz,i.end.xyz,i.unit.z)+normal*i.start.w;
    float z=max(i.end.w,pos.z);
    o.position=float4(i.projection.z+i.projection.x*pos.x/z,
                      i.projection.w+i.projection.y*pos.y/z,
                      saturate(i.end.w/z),1.0);
    o.color=i.color;
    o.normal=normal;
    return o;
}
float4 PBI(BondOut i):SV_TARGET {
    float shade=0.64+0.36*saturate(dot(normalize(i.normal),normalize(float3(0.5,0.8,0.5))));
    return float4(saturate(i.color.rgb*shade),i.color.a);
}
"#.to_string();
        Ok(Self {geometry, sphere_instances, bond_instances, composite, composite_sky})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hlsl_program_uses_dispatch_table_in_real_render_passes() {
        let program = D3d11Shaders::new().unwrap();
        assert!(program.geometry.contains("saturate(i.color)"));
        assert!(program.composite.contains(
            "lerp(s.rgb, h.rgb, float3(saturate(h.a), saturate(h.a), saturate(h.a)))"
        ));
        assert!(program.composite.contains("saturate(lerp("));
        assert!(program.composite.contains("SV_TARGET"));
        assert!(program.geometry.contains("SV_POSITION"));
        assert!(program.sphere_instances.contains("GSI("));
        assert!(program.sphere_instances.contains("PSI("));
        assert!(program.bond_instances.contains("GBI("));
        assert!(program.composite_sky.contains("float3 sky"));
        assert!(program.composite_sky.contains("lerp(sky, s.rgb"));
        assert!(program.composite_sky.contains(
            "float3(saturate(s.a), saturate(s.a), saturate(s.a))"
        ));
        assert!(program.composite_sky.contains("i.uv.y"));
    }

    #[test]
    fn hlsl_scalar_to_float3_has_three_numeric_constructor_arguments() {
        let source = Expr::from_source_unchecked("opacity", Type::F32);
        let vector = hlsl_broadcast_f32_to_float3(&source);
        assert_eq!(vector.ty(), Type::VEC3);
        assert_eq!(vector.code(), "float3(opacity, opacity, opacity)");
    }

    #[test]
    fn all_compositors_avoid_invalid_single_argument_float3_constructors() {
        let shader = D3d11Shaders::new().unwrap();
        for src in [&shader.composite, &shader.composite_sky] {
            // A direct scalar-to-vector constructor is invalid in the legacy
            // Direct3D 11 compiler (X3014). Keep this regression test.
            assert!(!src.contains("float3(saturate(h.a))"));
            assert!(!src.contains("float3(saturate(s.a))"));
            assert!(src.contains(
                "float3(saturate(h.a), saturate(h.a), saturate(h.a))"
            ));
        }
    }

    #[test]
    fn mismatched_backend_or_stage_cannot_generate_d3d_program() {
        assert!(D3d11Shaders::from_table(&FunctionTable::intel(CapabilitySet::CORE)).is_err());
        assert!(D3d11Shaders::from_table(&FunctionTable::nvidia(CapabilitySet::CORE)).is_err());
        assert!(D3d11Shaders::from_table(&FunctionTable::amd(CapabilitySet::CORE)).is_err());
        assert!(D3d11Shaders::from_table(&FunctionTable::hlsl(
            Stage::Compute, CapabilitySet::CORE)).is_err());
    }

    #[test]
    fn hardware_instance_shaders_have_matching_entrypoints_and_semantics() {
        let shaders=D3d11Shaders::new().unwrap();
        for (source,vertex,pixel) in [(&shaders.sphere_instances,"GSI(","PSI("),
                                      (&shaders.bond_instances,"GBI(","PBI(")] {
            assert!(source.contains(vertex));
            assert!(source.contains(pixel));
            assert!(source.contains("SV_POSITION"));
            assert!(source.contains("TEXCOORD0"));
            assert!(source.contains("COLOR"));
        }
    }
    #[test]
    fn generation_is_hardware_independent_and_deterministic() {
        let a = D3d11Shaders::new().unwrap();
        assert_eq!(a, D3d11Shaders::new().unwrap());
    }
}
