//! Hardware-independent graphics-backend preference and adapter ranking.
//!
//! This module never probes a device at compile time. On Windows the D3D11
//! renderer enumerates physical DXGI adapters at runtime, filters them using
//! this policy, tries the highest-ranked candidate, and falls back to the next.
//! On other platforms the game retains its software renderer until a native
//! GPU rasterizer for that platform has been implemented.

/// Which rendering implementation to attempt. Vendor modes select a DXGI GPU,
/// not a CUDA/HIP/OpenCL compute runtime. The HLSL intrinsic table remains
/// the correct shader generator for the Direct3D 11 rendering backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GpuPreference {
    /// Prefer discrete GPUs with more dedicated memory; try other GPUs if needed.
    #[default]
    Auto,
    /// Do not create a GPU device; use the existing software renderer.
    Software,
    /// Any hardware DXGI adapter that can actually initialize D3D11.
    D3d11,
    /// Limit D3D11 adapter selection to Intel devices.
    Intel,
    /// Limit D3D11 adapter selection to NVIDIA devices.
    Nvidia,
    /// Limit D3D11 adapter selection to AMD devices.
    Amd,
}
impl GpuPreference {
    /// Parse a user-supplied backend choice; invalid names are reported.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" | "best" => Ok(Self::Auto),
            "cpu" | "software" | "off" | "0" => Ok(Self::Software),
            "d3d11" | "gpu" | "hardware" => Ok(Self::D3d11),
            "intel" => Ok(Self::Intel),
            "nvidia" => Ok(Self::Nvidia),
            "amd" | "radeon" => Ok(Self::Amd),
            other => Err(format!("unrecognized MATA_GPU_BACKEND '{other}'; use auto, cpu, d3d11, intel, nvidia, or amd")),
        }
    }
    /// Human-readable setting to report in startup logs.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto=>"auto", Self::Software=>"cpu", Self::D3d11=>"d3d11",
            Self::Intel=>"intel", Self::Nvidia=>"nvidia", Self::Amd=>"amd",
        }
    }
    /// Does this preference admit the supplied PCI vendor?
    pub const fn admits(self, vendor: u32) -> bool {
        match self {
            Self::Software=>false, Self::Auto|Self::D3d11=>true,
            Self::Intel=>vendor==0x8086,
            Self::Nvidia=>vendor==0x10de,
            Self::Amd=>vendor==0x1002 || vendor==0x1022,
        }
    }
}
/// Available DXGI device information, independent of Windows types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterInfo {
    /// DXGI adapter ordinal used to select the actual device.
    pub index: u32,
    /// PCI vendor code, e.g. Intel `0x8086`.
    pub vendor_id: u32,
    /// Dedicated video-memory bytes reported by DXGI (not a throughput benchmark).
    pub dedicated_video_memory: u64,
    /// Shared system memory reported by DXGI.
    pub shared_system_memory: u64,
    /// Is the device flagged as software by DXGI?
    pub software: bool,
    /// Device name from the DXGI descriptor.
    pub name: String,
}
impl AdapterInfo {
    /// Human-readable vendor name, without any runtime driver dependency.
    pub const fn vendor(&self) -> &'static str {
        match self.vendor_id {
            0x8086=>"Intel", 0x10de=>"NVIDIA", 0x1002|0x1022=>"AMD",
            0x1414=>"Microsoft", _=>"Other",
        }
    }
    fn ranking_key(&self) -> (bool, u64, u8, u64) {
        // Dedicated memory is an *imperfect proxy* for likely performance.
        // No vendor is preferred over a better-equipped hardware adapter.
        let tie = match self.vendor_id {0x10de=>3,0x1002|0x1022=>2,0x8086=>1,_=>0};
        (self.dedicated_video_memory>0, self.dedicated_video_memory, tie, self.shared_system_memory)
    }
}
/// Sort usable adapters from most promising to least. Ties retain DXGI order.
/// This is a static policy; every candidate still must create a real device.
pub fn ranked_adapters(adapters: impl IntoIterator<Item=AdapterInfo>, choice: GpuPreference)
    -> Vec<AdapterInfo> {
    let mut results: Vec<_> = adapters.into_iter()
        .filter(|a| !a.software && choice.admits(a.vendor_id)).collect();
    results.sort_by(|a,b| b.ranking_key().cmp(&a.ranking_key())
        .then_with(||a.index.cmp(&b.index)));
    results
}

/// Try suitable devices in priority order, retaining the last error for diagnostics.
/// Hardware backends can supply a fallible initialization callback; this is
/// also testable without a GPU, driver, or even a Windows host.
pub fn try_ranked<T, E: core::fmt::Display>(
    adapters: impl IntoIterator<Item=AdapterInfo>,
    choice: GpuPreference,
    mut initialize: impl FnMut(&AdapterInfo) -> Result<T,E>,
) -> Result<(AdapterInfo,T),String> {
    let candidates=ranked_adapters(adapters,choice);
    if candidates.is_empty(){return Err(format!("no usable hardware adapter for '{}'",choice.label()));}
    let mut failures=Vec::new();
    for candidate in candidates {
        match initialize(&candidate) {
            Ok(value)=>return Ok((candidate,value)),
            Err(error)=>failures.push(format!("{}: {error}",candidate.name)),
        }
    }
    Err(format!("all hardware adapters failed: {}",failures.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn adapter(index:u32,vendor_id:u32,gb:u64,software:bool)->AdapterInfo {
        AdapterInfo {index,vendor_id,dedicated_video_memory:gb*1024*1024*1024,
            shared_system_memory:8*1024*1024*1024,software,name:format!("GPU {index}")}
    }
    #[test]fn prefers_a_high_memory_discrete_gpu_over_integrated(){
        let selected=ranked_adapters([
            adapter(0,0x8086,0,false),adapter(1,0x10de,8,false),adapter(2,0x1002,12,false),
        ],GpuPreference::Auto);
        assert_eq!(selected.iter().map(|a|a.index).collect::<Vec<_>>(),vec![2,1,0]);
    }
    #[test]fn explicit_vendor_never_falls_back_to_a_different_vendor(){
        let available=[adapter(0,0x8086,0,false),adapter(1,0x10de,12,false)];
        assert_eq!(ranked_adapters(available.clone(),GpuPreference::Intel).len(),1);
        assert_eq!(ranked_adapters(available.clone(),GpuPreference::Nvidia)[0].index,1);
        assert!(ranked_adapters(available,GpuPreference::Amd).is_empty());
    }
    #[test]fn rejects_software_and_keeps_stable_ties(){
        let selected=ranked_adapters([
            adapter(3,0x10de,8,false),adapter(2,0x10de,8,false),
            adapter(1,0x1414,100,true)],GpuPreference::Auto);
        assert_eq!(selected.iter().map(|a|a.index).collect::<Vec<_>>(),vec![2,3]);
    }
    #[test]fn failed_discrete_gpu_retries_integrated_gpu(){
        let mut attempted=Vec::new();
        let (selected,result)=try_ranked([
            adapter(0,0x8086,0,false),adapter(1,0x10de,12,false),
        ],GpuPreference::Auto,|candidate| {
            attempted.push(candidate.index);
            if candidate.index==1 {Err("driver refused device")}else{Ok(42)}
        }).unwrap();
        assert_eq!(attempted,vec![1,0]);
        assert_eq!(selected.index,0);
        assert_eq!(result,42);
    }
    #[test]fn disable_and_validation(){
        assert!(ranked_adapters([adapter(0,0x8086,1,false)],GpuPreference::Software).is_empty());
        assert_eq!(GpuPreference::parse("BEST").unwrap(),GpuPreference::Auto);
        assert_eq!(GpuPreference::parse("Radeon").unwrap(),GpuPreference::Amd);
        assert!(GpuPreference::parse("vulkan").is_err());
    }
}
