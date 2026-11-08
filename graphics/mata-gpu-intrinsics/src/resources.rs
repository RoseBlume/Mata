//! Phase 6 resource descriptors, independent of vendor runtime libraries.
//! Exact resource support must be negotiated against an actual device. These
//! declarations never imply that a backend accepts a native image or sampler.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum ImageDimension { D1, D2, D3, D1Array, D2Array }
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum ImageFormat { R8Unorm, Rg8Unorm, Rgba8Unorm, R32Float, Rgba32Float }
impl ImageFormat {
    pub const fn bytes_per_pixel(self)->usize {match self {
        Self::R8Unorm=>1,Self::Rg8Unorm=>2,Self::Rgba8Unorm|Self::R32Float=>4,Self::Rgba32Float=>16,
    }}
    pub const fn channels(self)->usize {match self {
        Self::R8Unorm|Self::R32Float=>1,Self::Rg8Unorm=>2,Self::Rgba8Unorm|Self::Rgba32Float=>4,
    }}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ImageAccess { ReadOnly, WriteOnly, ReadWrite }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ImageDesc {
    pub dimension:ImageDimension,pub extent:[usize;3],pub array_layers:usize,
    pub format:ImageFormat,pub access:ImageAccess,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ResourceError { InvalidDimension, InvalidLayerCount, SizeOverflow, UnsupportedFormat, RegionOutOfBounds, BadPitch }
impl ImageDesc {
    pub fn new_2d(width:usize,height:usize,format:ImageFormat,access:ImageAccess)->Result<Self,ResourceError>{
        let desc=Self{dimension:ImageDimension::D2,extent:[width,height,1],array_layers:1,format,access};
        desc.validate()?;Ok(desc)
    }
    pub fn validate(self)->Result<(),ResourceError>{
        let [w,h,d]=self.extent;
        if w==0 || h==0 || d==0{return Err(ResourceError::InvalidDimension)}
        if self.array_layers==0{return Err(ResourceError::InvalidLayerCount)}
        let ok=match self.dimension {
            ImageDimension::D1=>h==1&&d==1&&self.array_layers==1,
            ImageDimension::D2=>d==1&&self.array_layers==1,
            ImageDimension::D3=>self.array_layers==1,
            ImageDimension::D1Array=>h==1&&d==1,
            ImageDimension::D2Array=>d==1,
        };
        if !ok{return Err(ResourceError::InvalidDimension)}
        self.byte_len()?;Ok(())
    }
    pub fn row_bytes(self)->Result<usize,ResourceError>{self.extent[0].checked_mul(self.format.bytes_per_pixel()).ok_or(ResourceError::SizeOverflow)}
    pub fn slice_bytes(self)->Result<usize,ResourceError>{self.row_bytes()?.checked_mul(self.extent[1]).ok_or(ResourceError::SizeOverflow)}
    pub fn byte_len(self)->Result<usize,ResourceError>{
        self.slice_bytes()?.checked_mul(self.extent[2]).and_then(|n|n.checked_mul(self.array_layers)).ok_or(ResourceError::SizeOverflow)
    }
    pub fn region(self,origin:[usize;3],extent:[usize;3])->Result<ImageRegion,ResourceError>{
        self.validate()?;
        let logical_z=self.extent[2].checked_mul(self.array_layers).ok_or(ResourceError::SizeOverflow)?;
        let full=[self.extent[0],self.extent[1],logical_z];
        for i in 0..3 {
            if extent[i]==0 || origin[i]>full[i] || extent[i]>full[i]-origin[i]{return Err(ResourceError::RegionOutOfBounds)}
        }
        Ok(ImageRegion{origin,extent})
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ImageRegion {pub origin:[usize;3],pub extent:[usize;3]}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum AddressMode { ClampToEdge, ClampToBorder, Repeat, MirroredRepeat }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum FilterMode { Nearest, Linear }
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct SamplerDesc {pub normalized_coordinates:bool,pub address:AddressMode,pub filter:FilterMode}
impl SamplerDesc {
    pub fn validate(self)->Result<(),ResourceError>{
        if !self.normalized_coordinates && matches!(self.address,AddressMode::Repeat|AddressMode::MirroredRepeat) {
            return Err(ResourceError::InvalidDimension);
        }
        Ok(())
    }
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum MemoryAdvice { ReadMostly, PreferredDevice, AccessedByDevice, UnsetReadMostly }
