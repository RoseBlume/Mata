//! Native Apple AudioQueue I/O for macOS and iOS.
//!
//! CoreAudio / AudioToolbox only; no CPAL, Objective-C crate or external audio runtime.
//! Supported interleaved PCM formats: signed 16-bit and float 32-bit.
//! iOS applications must set NSMicrophoneUsageDescription to record audio.
use std::{ffi::c_void, ptr, sync::Mutex, time::{Duration,Instant}};
use crate::audio_io::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, Data, DeviceDescription, DeviceDescriptionBuilder, DeviceDirection, DeviceId,
    DeviceType, Error, ErrorKind, FrameCount, InputCallbackInfo, InputStreamTimestamp,
    InterfaceType, OutputCallbackInfo, OutputStreamTimestamp, SampleFormat, StreamConfig,
    StreamInstant, SupportedBufferSize, SupportedStreamConfig, SupportedStreamConfigRange,
    iter::{SupportedInputConfigs, SupportedOutputConfigs},
};
use crate::audio_io::platform::HostId;

type AQ = *mut c_void;
type AudioQueueBufferRef = *mut AudioQueueBuffer;
const K_AUDIO_FORMAT_LINEAR_PCM:u32 = 0x6c70636d;
const IS_FLOAT:u32=1<<0;
const IS_SIGNED_INTEGER:u32=1<<2;
const IS_PACKED:u32=1<<3;

#[repr(C)]
struct AudioStreamBasicDescription {
    sample_rate:f64, format_id:u32, format_flags:u32,
    bytes_per_packet:u32, frames_per_packet:u32, bytes_per_frame:u32,
    channels_per_frame:u32,bits_per_channel:u32,reserved:u32,
}
#[repr(C)]
struct AudioQueueBuffer {
    capacity:u32,data:*mut c_void,size:u32,user:*mut c_void,
    description_capacity:u32, descriptions:*mut c_void, description_count:u32,
}
#[link(name="AudioToolbox",kind="framework")]
unsafe extern "C" {
    fn AudioQueueNewOutput(fmt:*const AudioStreamBasicDescription,
        cb:unsafe extern "C" fn(*mut c_void,AQ,AudioQueueBufferRef),
        user:*mut c_void,run_loop:*const c_void,mode:*const c_void,flags:u32,out:*mut AQ)->i32;
    fn AudioQueueNewInput(fmt:*const AudioStreamBasicDescription,
        cb:unsafe extern "C" fn(*mut c_void,AQ,AudioQueueBufferRef,*const c_void,u32,*const c_void),
        user:*mut c_void,run_loop:*const c_void,mode:*const c_void,flags:u32,out:*mut AQ)->i32;
    fn AudioQueueAllocateBuffer(queue:AQ,size:u32,out:*mut AudioQueueBufferRef)->i32;
    fn AudioQueueEnqueueBuffer(queue:AQ,buffer:AudioQueueBufferRef,packet_count:u32,packets:*const c_void)->i32;
    fn AudioQueueStart(queue:AQ,time:*const c_void)->i32;
    fn AudioQueuePause(queue:AQ)->i32;
    fn AudioQueueStop(queue:AQ,immediate:u8)->i32;
    fn AudioQueueDispose(queue:AQ,immediate:u8)->i32;
}
fn os_error(status:i32,what:&str)->Error {
    Error::with_message(ErrorKind::BackendError,format!("AudioToolbox {what}: OSStatus {status}"))
}
fn check(status:i32,what:&str)->Result<(),Error>{if status==0{Ok(())}else{Err(os_error(status,what))}}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub struct Device { input:bool }
impl std::fmt::Display for Device {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        f.write_str(if self.input {"Default Apple Input"}else{"Default Apple Output"})
    }
}
pub struct Host;
pub type Devices=std::vec::IntoIter<Device>;
impl Host {pub fn new()->Result<Self,Error>{Ok(Self)}}
impl HostTrait for Host {
    type Devices=Devices;
    type Device=Device;
    fn is_available()->bool{true}
    fn devices(&self)->Result<Devices,Error>{Ok(vec![Device{input:false},Device{input:true}].into_iter())}
    fn device_by_id(&self,id:&DeviceId)->Option<Device>{
        if id.host()!=HostId::Apple {return None;}
        match id.id() {"default-input"=>Some(Device{input:true}),"default-output"=>Some(Device{input:false}),_=>None}
    }
    fn default_input_device(&self)->Option<Device>{Some(Device{input:true})}
    fn default_output_device(&self)->Option<Device>{Some(Device{input:false})}
}
fn config_ranges()->Vec<SupportedStreamConfigRange>{
    [SampleFormat::F32,SampleFormat::I16].into_iter().flat_map(|sample_format| {
        [1,2].into_iter().map(move |channels|SupportedStreamConfigRange {
            channels,min_sample_rate:8000,max_sample_rate:96000,
            buffer_size:SupportedBufferSize::Range{min:64,max:8192},sample_format
        })
    }).collect()
}
impl DeviceTrait for Device {
    type Stream=Stream;
    type SupportedInputConfigs=SupportedInputConfigs;
    type SupportedOutputConfigs=SupportedOutputConfigs;
    fn description(&self)->Result<DeviceDescription,Error>{
        Ok(DeviceDescriptionBuilder::new(if self.input {"Default Apple Input"} else {"Default Apple Output"})
            .driver("AudioToolbox AudioQueue").device_type(if self.input {DeviceType::Microphone}else{DeviceType::Speaker})
            .interface_type(InterfaceType::Unknown)
            .direction(if self.input {DeviceDirection::Input}else{DeviceDirection::Output}).build())
    }
    fn id(&self)->Result<DeviceId,Error>{Ok(DeviceId::new(HostId::Apple,if self.input {"default-input"}else{"default-output"}))}
    fn supports_input(&self)->bool{self.input}
    fn supports_output(&self)->bool{!self.input}
    fn supported_input_configs(&self)->Result<SupportedInputConfigs,Error>{
        if !self.input{return Err(Error::with_message(ErrorKind::UnsupportedOperation,"not an input device"));}
        Ok(config_ranges().into_iter())
    }
    fn supported_output_configs(&self)->Result<SupportedOutputConfigs,Error>{
        if self.input{return Err(Error::with_message(ErrorKind::UnsupportedOperation,"not an output device"));}
        Ok(config_ranges().into_iter())
    }
    fn default_input_config(&self)->Result<SupportedStreamConfig,Error>{
        if !self.input{return Err(Error::with_message(ErrorKind::UnsupportedOperation,"not an input device"));}
        Ok(SupportedStreamConfig::new(1,48000,SupportedBufferSize::Range{min:64,max:8192},SampleFormat::F32))
    }
    fn default_output_config(&self)->Result<SupportedStreamConfig,Error>{
        if self.input{return Err(Error::with_message(ErrorKind::UnsupportedOperation,"not an output device"));}
        Ok(SupportedStreamConfig::new(2,48000,SupportedBufferSize::Range{min:64,max:8192},SampleFormat::F32))
    }
    fn build_input_stream_raw<D,E>(&self,config:StreamConfig,format:SampleFormat,data:D,error:E,_timeout:Option<Duration>)->Result<Stream,Error>
      where D:FnMut(&Data,&InputCallbackInfo)+Send+'static, E:FnMut(Error)+Send+'static {
        if !self.input {return Err(Error::with_message(ErrorKind::UnsupportedOperation,"input requested from output device"));}
        Stream::create(config,format,Callback::Input(Box::new(data)),Box::new(error),true)
    }
    fn build_output_stream_raw<D,E>(&self,config:StreamConfig,format:SampleFormat,data:D,error:E,_timeout:Option<Duration>)->Result<Stream,Error>
      where D:FnMut(&mut Data,&OutputCallbackInfo)+Send+'static,E:FnMut(Error)+Send+'static {
        if self.input {return Err(Error::with_message(ErrorKind::UnsupportedOperation,"output requested from input device"));}
        Stream::create(config,format,Callback::Output(Box::new(data)),Box::new(error),false)
    }
}

type InputCallback=Box<dyn FnMut(&Data,&InputCallbackInfo)+Send>;
type OutputCallback=Box<dyn FnMut(&mut Data,&OutputCallbackInfo)+Send>;
type ErrorCallback=Box<dyn FnMut(Error)+Send>;
enum Callback{Input(InputCallback),Output(OutputCallback)}
struct CallbackState {
    callback:Mutex<Callback>, error:Mutex<ErrorCallback>,
    format:SampleFormat,bytes_per_frame:u32,frames:u32,
    started:Instant,
}
impl CallbackState {
    fn instant(&self)->StreamInstant {
        StreamInstant::from_nanos(self.started.elapsed().as_nanos().min(u64::MAX as u128) as u64)
    }
    fn error(&self,status:i32,operation:&str) {
        if status!=0 {if let Ok(mut cb)=self.error.lock(){(cb)(os_error(status,operation));}}
    }
}
unsafe extern "C" fn on_output(user:*mut c_void, queue:AQ, buffer:AudioQueueBufferRef) {
    if user.is_null() || buffer.is_null(){return;}
    // SAFETY: user points to CallbackState retained until AudioQueueDispose completes.
    let state=unsafe{&*(user as *const CallbackState)};
    let size=state.frames.saturating_mul(state.bytes_per_frame);
    let b=unsafe{&mut *buffer};
    let capacity=size.min(b.capacity);
    b.size=capacity;
    if b.data.is_null(){return;}
    unsafe{ptr::write_bytes(b.data,0,capacity as usize);}
    let samples=capacity as usize/state.format.sample_size();
    let mut data=unsafe{Data::from_parts(b.data,samples,state.format)};
    let instant=state.instant();
    // AudioQueue callback clock; do not invent a hardware playback timestamp.
    let info=OutputCallbackInfo::new(OutputStreamTimestamp{callback:instant,playback:instant});
    if let Ok(mut cb)=state.callback.lock(){if let Callback::Output(ref mut c)=*cb {(c)(&mut data,&info);}}
    let code=unsafe{AudioQueueEnqueueBuffer(queue,buffer,0,ptr::null())}; state.error(code,"enqueue output");
}
unsafe extern "C" fn on_input(user:*mut c_void,queue:AQ,buffer:AudioQueueBufferRef,
    _time:*const c_void,_packets:u32,_desc:*const c_void) {
    if user.is_null() || buffer.is_null(){return;}
    let state=unsafe{&*(user as *const CallbackState)};
    let b=unsafe{&mut *buffer};
    let len=b.size as usize/state.format.sample_size();
    let data=unsafe{Data::from_parts(b.data,len,state.format)};
    let instant=state.instant();
    let info=InputCallbackInfo::new(InputStreamTimestamp{callback:instant,capture:instant});
    if let Ok(mut cb)=state.callback.lock(){if let Callback::Input(ref mut c)=*cb {(c)(&data,&info);}}
    b.size=0;
    let code=unsafe{AudioQueueEnqueueBuffer(queue,buffer,0,ptr::null())}; state.error(code,"enqueue input");
}
struct QueueControl {queue:AQ, state:*mut CallbackState, buffers:[AudioQueueBufferRef;3], primed:bool, input:bool}
// Queue native handles are only touched while holding control mutex.
unsafe impl Send for QueueControl {}

pub struct Stream {control:Mutex<QueueControl>,frames:u32,start:Instant}
unsafe impl Send for Stream {}
unsafe impl Sync for Stream {}
impl Stream {
    fn create(config:StreamConfig,format:SampleFormat,cb:Callback,err:ErrorCallback,input:bool)->Result<Self,Error>{
        if config.channels==0 || config.channels>2 || !(8000..=96000).contains(&config.sample_rate) {
            return Err(Error::with_message(ErrorKind::UnsupportedConfig,"only 1-2 channels, 8-96 kHz supported"));
        }
        if format!=SampleFormat::F32 && format!=SampleFormat::I16 {
            return Err(Error::with_message(ErrorKind::UnsupportedConfig,"Apple AudioQueue: expected F32 or I16 PCM"));
        }
        let frames=match config.buffer_size {BufferSize::Fixed(n)=>n,BufferSize::Default=>512};
        if !(64..=8192).contains(&frames){return Err(Error::with_message(ErrorKind::UnsupportedConfig,"buffer frames must be 64..8192"));}
        let bytes_per_frame=config.channels as u32*format.sample_size() as u32;
        let fmt=AudioStreamBasicDescription {
            sample_rate:config.sample_rate as f64,format_id:K_AUDIO_FORMAT_LINEAR_PCM,
            format_flags:(if format==SampleFormat::F32 {IS_FLOAT}else{IS_SIGNED_INTEGER})|IS_PACKED,
            bytes_per_packet:bytes_per_frame,frames_per_packet:1,bytes_per_frame,
            channels_per_frame:config.channels as u32,bits_per_channel:(format.sample_size()*8) as u32,reserved:0,
        };
        let clock=Instant::now();
        let state=Box::into_raw(Box::new(CallbackState{
            callback:Mutex::new(cb),error:Mutex::new(err),format,
            bytes_per_frame,frames,started:clock,
        }));
        let mut queue:AQ=ptr::null_mut();
        let status=unsafe{if input {AudioQueueNewInput(&fmt,on_input,state.cast(),ptr::null(),ptr::null(),0,&mut queue)}
            else {AudioQueueNewOutput(&fmt,on_output,state.cast(),ptr::null(),ptr::null(),0,&mut queue)}};
        if status!=0 {
            unsafe{drop(Box::from_raw(state));}
            return Err(os_error(status,"create queue"));
        }
        let mut buffers=[ptr::null_mut();3];
        for buf in &mut buffers {
            let status=unsafe{AudioQueueAllocateBuffer(queue,frames*bytes_per_frame,buf)};
            if status!=0 {
                unsafe{AudioQueueDispose(queue,1);drop(Box::from_raw(state));}
                return Err(os_error(status,"allocate audio buffer"));
            }
        }
        // Queues are initially paused; output buffers get filled at first play().
        if input {
            for &buffer in &buffers {
                let status=unsafe{AudioQueueEnqueueBuffer(queue,buffer,0,ptr::null())};
                if status!=0 {
                    unsafe{AudioQueueDispose(queue,1);drop(Box::from_raw(state));}
                    return Err(os_error(status,"queue input buffer"));
                }
            }
        }
        Ok(Self{control:Mutex::new(QueueControl{queue,state,buffers,primed:input,input}),frames,start:clock})
    }
}
impl StreamTrait for Stream {
    fn play(&self)->Result<(),Error>{
        let mut c=self.control.lock().map_err(|_|Error::with_message(ErrorKind::BackendError,"queue mutex poisoned"))?;
        if !c.primed && !c.input {
            // Prime the output queue before starting it.
            for &buffer in &c.buffers {unsafe{on_output(c.state.cast(),c.queue,buffer);}}
            c.primed=true;
        }
        check(unsafe{AudioQueueStart(c.queue,ptr::null())},"start")?;
        Ok(())
    }
    fn pause(&self)->Result<(),Error>{
        let c=self.control.lock().map_err(|_|Error::with_message(ErrorKind::BackendError,"queue mutex poisoned"))?;
        check(unsafe{AudioQueuePause(c.queue)},"pause")
    }
    fn buffer_size(&self)->Result<FrameCount,Error>{Ok(self.frames)}
    fn now(&self)->StreamInstant{StreamInstant::from_nanos(self.start.elapsed().as_nanos().min(u64::MAX as u128) as u64)}
}
impl Drop for Stream {
    fn drop(&mut self) {
        if let Ok(c)=self.control.get_mut() {
            unsafe {
                AudioQueueStop(c.queue,1);
                AudioQueueDispose(c.queue,1);
                drop(Box::from_raw(c.state));
            }
        }
    }
}
