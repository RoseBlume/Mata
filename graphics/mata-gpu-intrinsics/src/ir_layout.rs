// Checked host-visible layout calculations. This module only describes byte
// layout; the target compiler/ABI remains authoritative for native pointers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout { pub size: usize, pub align: usize, pub stride: usize }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldLayout { pub name: String, pub offset: usize, pub layout: Layout }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordLayout { pub name: String, pub fields: Vec<FieldLayout>, pub layout: Layout }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode { Storage430, Uniform140, WgslUniform, HlslConstant, HlslStructured, OpenClNatural, CudaNative }

fn roundup(n:usize,a:usize)->Result<usize,IrError>{
    if a==0 {return Err(IrError::InvalidLayout)}
    n.checked_add(a-1).map(|sum|(sum/a)*a).ok_or(IrError::InvalidLayout)
}
fn scalar_bytes(n:Number,mode:LayoutMode)->Result<usize,IrError>{
    use Number::*;
    let size=match n {
        Bool=>if mode==LayoutMode::HlslConstant {4}else{return Err(IrError::InvalidLayout)},
        I8|U8=>1, I16|U16|F16|Bf16=>2,
        I32|U32|F32=>4, I64|U64|F64=>8,
    };
    // HLSL constant buffer native 8/16-bit packing needs target-specific
    // native layout and binding options, so Phase 1 rejects it explicitly.
    if mode==LayoutMode::HlslConstant && size<4{return Err(IrError::InvalidLayout)}
    Ok(size)
}
impl Program {
    pub fn storage_layout(&self,ty:&DataType)->Result<Layout,IrError>{self.layout_with(ty,LayoutMode::Storage430)}
    pub fn uniform_layout(&self,ty:&DataType)->Result<Layout,IrError>{self.layout_with(ty,LayoutMode::Uniform140)}
    pub fn wgsl_uniform_layout(&self,ty:&DataType)->Result<Layout,IrError>{self.layout_with(ty,LayoutMode::WgslUniform)}
    pub fn hlsl_cbuffer_layout(&self,ty:&DataType)->Result<Layout,IrError>{self.layout_with(ty,LayoutMode::HlslConstant)}
    pub fn layout_for_backend(&self,ty:&DataType,backend:Backend,space:AddressSpace)->Result<Layout,IrError>{
        let mode=match backend{
            Backend::Wgsl=>if space==AddressSpace::Constant{LayoutMode::WgslUniform}else{LayoutMode::Storage430},
            Backend::Glsl=>if space==AddressSpace::Constant{LayoutMode::Uniform140}else{LayoutMode::Storage430},
            Backend::Hlsl=>if space==AddressSpace::Constant{LayoutMode::HlslConstant}else{LayoutMode::HlslStructured},
            Backend::IntelOpenCl=>LayoutMode::OpenClNatural,
            Backend::NvidiaCuda|Backend::AmdHip=>LayoutMode::CudaNative,
        };self.layout_with(ty,mode)
    }
    /// Host ABI layout of scalar/record uniforms packed into the generated
    /// WGSL uniform struct or HLSL constant buffer for one kernel.
    pub fn kernel_uniform_layout(&self,name:&str,backend:Backend)->Result<RecordLayout,IrError>{
        let k=self.kernels.iter().find(|kernel|kernel.name==name).ok_or(IrError::UnknownSymbol)?;
        let mode=match backend{
            Backend::Wgsl=>LayoutMode::WgslUniform,
            Backend::Hlsl=>LayoutMode::HlslConstant,
            _=>return Err(IrError::Unsupported{backend,detail:"uniform parameter packing currently defined for WGSL/HLSL"}),
        };
        let fields=k.args.iter().filter(|a|!matches!(a.ty,DataType::Pointer(_,_,_)))
            .map(|arg|Field{name:arg.name.clone(),ty:arg.ty.clone()}).collect::<Vec<_>>();
        if fields.is_empty(){return Err(IrError::InvalidLayout)}
        self.fields_layout(&format!("{}_params",name),&fields,mode,&mut Vec::new())
    }
    pub fn layout_with(&self,ty:&DataType,mode:LayoutMode)->Result<Layout,IrError>{
        ty.validate()?; self.layout_of(ty,mode,&mut Vec::new())
    }
    pub fn record_layout(&self,name:&str)->Result<RecordLayout,IrError>{
        self.record_layout_with(name,LayoutMode::Storage430)
    }
    pub fn record_layout_with(&self,name:&str,mode:LayoutMode)->Result<RecordLayout,IrError>{
        let rec=self.records.iter().find(|r|r.name==name).ok_or(IrError::UnknownRecord)?;
        let mut seen=alloc::vec![name.to_string()];
        self.fields_layout(name,&rec.fields,mode,&mut seen)
    }
    fn fields_layout(&self,name:&str,fields:&[Field],mode:LayoutMode,seen:&mut Vec<String>)->Result<RecordLayout,IrError>{
        let mut out=Vec::new(); let mut cursor=0usize;
        let mut struct_align=if mode==LayoutMode::HlslConstant {16}else{1};
        for field in fields {
            let l=self.layout_of(&field.ty,mode,seen)?;
            let mut align=l.align;
            if matches!(mode,LayoutMode::WgslUniform|LayoutMode::Uniform140) && matches!(field.ty,DataType::Array(_,_)|DataType::Record(_)|DataType::Tuple(_)) {
                align=align.max(16);
            }
            cursor=roundup(cursor,align)?;
            if mode==LayoutMode::HlslConstant {
                let needs_register=matches!(field.ty,DataType::Array(_,_)|DataType::Record(_)|DataType::Tuple(_));
                if needs_register || (cursor%16).checked_add(l.size).ok_or(IrError::InvalidLayout)?>16 {
                    cursor=roundup(cursor,16)?;
                }
            }
            out.push(FieldLayout{name:field.name.clone(),offset:cursor,layout:l});
            cursor=cursor.checked_add(l.size).ok_or(IrError::InvalidLayout)?;
            struct_align=struct_align.max(align);
        }
        if matches!(mode,LayoutMode::Uniform140|LayoutMode::WgslUniform|LayoutMode::HlslConstant) {struct_align=struct_align.max(16);}
        let size=roundup(cursor,struct_align)?;
        Ok(RecordLayout{name:name.into(),fields:out,layout:Layout{size,align:struct_align,stride:size}})
    }
    fn layout_of(&self,ty:&DataType,mode:LayoutMode,seen:&mut Vec<String>)->Result<Layout,IrError>{
        let layout=match ty {
            DataType::Scalar(n)=>{let sz=scalar_bytes(*n,mode)?;Layout{size:sz,align:sz,stride:sz}},
            DataType::Vector(n,w)=>{
                if ![2,3,4].contains(w){return Err(IrError::InvalidLayout)}
                let unit=scalar_bytes(*n,mode)?;
                let size=unit.checked_mul(*w as usize).ok_or(IrError::InvalidLayout)?;
                let lanes=match mode {
                    LayoutMode::OpenClNatural=>if *w==3{4}else{*w as usize},
                    LayoutMode::CudaNative=>if *w==3{1}else{*w as usize},
                    LayoutMode::HlslStructured|LayoutMode::HlslConstant=>1,
                    _=>if *w==3{4}else{*w as usize},
                };
                let align=unit.checked_mul(lanes).ok_or(IrError::InvalidLayout)?;
                let size=if mode==LayoutMode::OpenClNatural && *w==3{unit.checked_mul(4).ok_or(IrError::InvalidLayout)?}else{size};
                Layout{size,align,stride:roundup(size,align)?}
            },
            DataType::Array(inner,count)=>{
                if *count==0{return Err(IrError::InvalidLayout)}
                let item=self.layout_of(inner,mode,seen)?;
                let align=if matches!(mode,LayoutMode::Uniform140|LayoutMode::WgslUniform|LayoutMode::HlslConstant){item.align.max(16)}else{item.align};
                let stride=roundup(item.size,align)?;
                Layout{size:stride.checked_mul(*count).ok_or(IrError::InvalidLayout)?,align,stride}
            },
            DataType::Record(name)=>{
                if seen.iter().any(|it|it==name) {return Err(IrError::InvalidLayout)}
                let r=self.records.iter().find(|r|&r.name==name).ok_or(IrError::UnknownRecord)?;
                seen.push(name.clone());let layout=self.fields_layout(name,&r.fields,mode,seen)?.layout;seen.pop();layout
            },
            DataType::Tuple(items)=>{
                let fields=items.iter().enumerate().map(|(i,t)|Field{name:format!("value{}",i),ty:t.clone()}).collect::<Vec<_>>();
                self.fields_layout(&tuple_name(ty),&fields,mode,seen)?.layout
            },
            DataType::Void|DataType::Pointer(_,_,_)=>return Err(IrError::InvalidLayout),
        };
        Ok(layout)
    }
}
