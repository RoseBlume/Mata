// Native macOS AppKit + Metal backend for mata-win. No third-party libraries.
#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#import <CoreFoundation/CoreFoundation.h>
#import <stdint.h>
#import <math.h>

typedef struct { int32_t kind, a, b; } MataEvent;
enum { MATA_CLOSE=1, MATA_RESIZE, MATA_MOVE, MATA_KEY_DOWN, MATA_KEY_UP,
       MATA_MOUSE_MOVE, MATA_SCROLL, MATA_MINIMIZED, MATA_RESTORED };

@class MataState;
@interface MataView : NSView { @public __unsafe_unretained MataState *owner; }
@end
@interface MataDelegate : NSObject <NSWindowDelegate> { @public __unsafe_unretained MataState *owner; }
@end
@interface MataState : NSObject {
@public
    NSWindow *window;
    MataView *view;
    MataDelegate *delegate;
    CAMetalLayer *layer;
    id<MTLDevice> device;
    id<MTLCommandQueue> commands;
    id<MTLRenderPipelineState> pipeline;
    id<MTLTexture> uploadTexture;
    uint32_t textureWidth, textureHeight;
    BOOL cursorVisible, mouseInside;
    MataEvent queue[512];
    unsigned head, tail;
}
- (void)emit:(int32_t)kind a:(int32_t)a b:(int32_t)b;
- (void)updateDrawableSize;
@end

@implementation MataState
- (void)emit:(int32_t)kind a:(int32_t)a b:(int32_t)b {
    unsigned next = (tail + 1u) % 512u;
    if (next == head) head = (head + 1u) % 512u;
    queue[tail] = (MataEvent){kind,a,b}; tail = next;
}
- (void)updateDrawableSize {
    if (!view || !layer) return;
    layer.contentsScale = view.window.backingScaleFactor;
    NSSize pixels = [view convertSizeToBacking:view.bounds.size];
    uint32_t w = (uint32_t)MAX(1.0, round(pixels.width));
    uint32_t h = (uint32_t)MAX(1.0, round(pixels.height));
    CGSize old = layer.drawableSize;
    if (old.width != w || old.height != h) {
        layer.drawableSize = CGSizeMake(w,h);
        [self emit:MATA_RESIZE a:(int32_t)w b:(int32_t)h];
    }
}
@end

@implementation MataView
- (BOOL)isFlipped { return YES; }
- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)wantsUpdateLayer { return YES; }
- (void)updateLayer {}
- (void)viewDidChangeBackingProperties { [super viewDidChangeBackingProperties]; [owner updateDrawableSize]; }
- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    for (NSTrackingArea *a in self.trackingAreas) [self removeTrackingArea:a];
    NSTrackingArea *a = [[NSTrackingArea alloc] initWithRect:NSZeroRect
        options:NSTrackingMouseMoved|NSTrackingMouseEnteredAndExited|NSTrackingActiveAlways|NSTrackingInVisibleRect
        owner:self userInfo:nil];
    [self addTrackingArea:a];
}
- (void)mouseEntered:(NSEvent *)e { owner->mouseInside=YES; if (!owner->cursorVisible) [NSCursor hide]; [self mouseMoved:e]; }
- (void)mouseExited:(NSEvent *)e { (void)e; if (!owner->cursorVisible) [NSCursor unhide]; owner->mouseInside=NO; }
- (void)mouseMoved:(NSEvent *)e {
    NSPoint p=[self convertPoint:e.locationInWindow fromView:nil];
    NSPoint bp=[self convertPointToBacking:p];
    [owner emit:MATA_MOUSE_MOVE a:(int32_t)lround(bp.x) b:(int32_t)lround(bp.y)];
}
- (void)mouseDragged:(NSEvent *)e { [self mouseMoved:e]; }
- (void)rightMouseDragged:(NSEvent *)e { [self mouseMoved:e]; }
- (void)otherMouseDragged:(NSEvent *)e { [self mouseMoved:e]; }
// Mouse buttons use 0x100 + button index so they don't overlap macOS key codes.
- (void)mouseDown:(NSEvent *)e { [self mouseMoved:e]; [owner emit:MATA_KEY_DOWN a:0x100 b:0]; }
- (void)mouseUp:(NSEvent *)e { [self mouseMoved:e]; [owner emit:MATA_KEY_UP a:0x100 b:0]; }
- (void)rightMouseDown:(NSEvent *)e { [self mouseMoved:e]; [owner emit:MATA_KEY_DOWN a:0x101 b:0]; }
- (void)rightMouseUp:(NSEvent *)e { [self mouseMoved:e]; [owner emit:MATA_KEY_UP a:0x101 b:0]; }
- (void)otherMouseDown:(NSEvent *)e { [self mouseMoved:e]; [owner emit:MATA_KEY_DOWN a:0x102 b:0]; }
- (void)otherMouseUp:(NSEvent *)e { [self mouseMoved:e]; [owner emit:MATA_KEY_UP a:0x102 b:0]; }
- (void)scrollWheel:(NSEvent *)e {
    [owner emit:MATA_SCROLL a:(int32_t)lround(e.scrollingDeltaX) b:(int32_t)lround(e.scrollingDeltaY)];
}
- (void)keyDown:(NSEvent *)e { if (!e.isARepeat) [owner emit:MATA_KEY_DOWN a:(int32_t)e.keyCode b:0]; }
- (void)keyUp:(NSEvent *)e { [owner emit:MATA_KEY_UP a:(int32_t)e.keyCode b:0]; }
- (void)flagsChanged:(NSEvent *)e {
    NSEventModifierFlags f=e.modifierFlags;
    BOOL pressed=NO;
    switch(e.keyCode) {
        case 56: case 60: pressed=(f & NSEventModifierFlagShift)!=0; break;
        case 59: case 62: pressed=(f & NSEventModifierFlagControl)!=0; break;
        default: return;
    }
    [owner emit:pressed?MATA_KEY_DOWN:MATA_KEY_UP a:(int32_t)e.keyCode b:0];
}
@end

@implementation MataDelegate
- (BOOL)windowShouldClose:(NSWindow *)sender { (void)sender; [owner emit:MATA_CLOSE a:0 b:0]; return NO; }
- (void)windowDidResize:(NSNotification *)n { (void)n; [owner updateDrawableSize]; }
- (void)windowDidMove:(NSNotification *)n {
    (void)n; NSPoint p=owner->window.frame.origin;
    [owner emit:MATA_MOVE a:(int32_t)p.x b:(int32_t)p.y];
}
- (void)windowDidMiniaturize:(NSNotification *)n { (void)n; [owner emit:MATA_MINIMIZED a:0 b:0]; }
- (void)windowDidDeminiaturize:(NSNotification *)n {
    (void)n; [owner updateDrawableSize];
    [owner emit:MATA_RESTORED a:(int32_t)owner->layer.drawableSize.width b:(int32_t)owner->layer.drawableSize.height];
}
@end

// Shader handles scaling mismatched source and drawable dimensions, nearest filtering.
static NSString * const MataShader = @"#include <metal_stdlib>\n"
@"using namespace metal;\n"
@"struct V {float4 position [[position]]; float2 tex;};\n"
@"vertex V vs(uint i [[vertex_id]]) { float2 xy[4] = { {-1,-1},{1,-1},{-1,1},{1,1} };"
@"V v; v.position=float4(xy[i],0,1); v.tex=float2((xy[i].x+1)*0.5,(1-xy[i].y)*0.5); return v; }\n"
@"fragment float4 fs(V v [[stage_in]], texture2d<float> image [[texture(0)]]) {"
@"constexpr sampler s(coord::normalized, address::clamp_to_edge, filter::nearest);"
@"float4 c=image.sample(s,v.tex); return float4(c.rgb,1.0); }\n";

static MataState *state(void *p) { return (__bridge MataState *)p; }
void *mata_apple_window_new(const char *title, uint32_t w, uint32_t h) {
    @autoreleasepool {
        if (![NSThread isMainThread]) return NULL;
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        MataState *s=[MataState new];
        s->cursorVisible=YES;
        NSWindowStyleMask mask=NSWindowStyleMaskTitled|NSWindowStyleMaskClosable|NSWindowStyleMaskMiniaturizable|NSWindowStyleMaskResizable;
        s->window=[[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,MAX(1,w),MAX(1,h))
                        styleMask:mask backing:NSBackingStoreBuffered defer:NO];
        if (!s->window) return NULL;
        s->view=[[MataView alloc] initWithFrame:s->window.contentView.bounds];
        s->view->owner=s;
        s->view.autoresizingMask=NSViewWidthSizable|NSViewHeightSizable;
        s->view.wantsLayer=YES;
        s->layer=[CAMetalLayer layer];
        s->device=MTLCreateSystemDefaultDevice();
        if (!s->device) return NULL;
        s->commands=[s->device newCommandQueue];
        s->layer.device=s->device;
        s->layer.pixelFormat=MTLPixelFormatBGRA8Unorm;
        s->layer.framebufferOnly=YES;
        s->layer.contentsScale=s->window.backingScaleFactor;
        s->view.layer=s->layer;
        s->window.contentView=s->view;
        s->delegate=[MataDelegate new]; s->delegate->owner=s;
        s->window.delegate=s->delegate;
        s->window.title=title?[NSString stringWithUTF8String:title]:@"MataWin";
        s->window.acceptsMouseMovedEvents=YES;
        [s->window center];
        NSError *error=nil;
        id<MTLLibrary> lib=[s->device newLibraryWithSource:MataShader options:nil error:&error];
        if (!lib) { NSLog(@"MataWin Metal shader error: %@",error); return NULL; }
        MTLRenderPipelineDescriptor *pd=[MTLRenderPipelineDescriptor new];
        pd.vertexFunction=[lib newFunctionWithName:@"vs"];
        pd.fragmentFunction=[lib newFunctionWithName:@"fs"];
        pd.colorAttachments[0].pixelFormat=MTLPixelFormatBGRA8Unorm;
        s->pipeline=[s->device newRenderPipelineStateWithDescriptor:pd error:&error];
        if (!s->pipeline) { NSLog(@"MataWin Metal pipeline error: %@",error); return NULL; }
        [s updateDrawableSize];
        [s->window makeKeyAndOrderFront:nil]; [s->window makeFirstResponder:s->view];
        [NSApp activateIgnoringOtherApps:YES];
        return (__bridge_retained void *)s;
    }
}
void mata_apple_window_free(void *p) {
    if (!p) return;
    @autoreleasepool {
        MataState *s=state(p);
        if (!s->cursorVisible && s->mouseInside) [NSCursor unhide];
        s->window.delegate=nil;
        s->view->owner=nil; s->delegate->owner=nil;
        [s->window orderOut:nil]; [s->window close];
        CFBridgingRelease(p);
    }
}
void mata_apple_window_pump(void *p) {
    if (!p) return;
    @autoreleasepool {
        for (unsigned i=0;i<256;i++) {
            NSEvent *e=[NSApp nextEventMatchingMask:NSEventMaskAny untilDate:[NSDate distantPast]
                  inMode:NSDefaultRunLoopMode dequeue:YES];
            if (!e) break;
            [NSApp sendEvent:e];
        }
        [NSApp updateWindows];
        [state(p) updateDrawableSize];
    }
}
uint32_t mata_apple_window_events(void *p, MataEvent *out, uint32_t capacity) {
    MataState *s=state(p); if (!s || !out) return 0;
    uint32_t count=0;
    while(s->head!=s->tail && count<capacity) {
        out[count++]=s->queue[s->head]; s->head=(s->head+1u)%512u;
    }
    return count;
}
void mata_apple_window_size(void *p, uint32_t *w, uint32_t *h) {
    MataState *s=state(p); [s updateDrawableSize];
    if(w) *w=(uint32_t)s->layer.drawableSize.width;
    if(h) *h=(uint32_t)s->layer.drawableSize.height;
}
void mata_apple_window_present(void *p, const uint32_t *pixels, uint32_t w, uint32_t h,
                                const uint32_t *regions, uint32_t regionCount) {
    if(!p || !pixels || !w || !h) return;
    @autoreleasepool {
        MataState *s=state(p);
        [s updateDrawableSize];
        if (!s->uploadTexture || s->textureWidth!=w || s->textureHeight!=h) {
            MTLTextureDescriptor *td=[MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                width:w height:h mipmapped:NO];
            td.usage=MTLTextureUsageShaderRead;
            s->uploadTexture=[s->device newTextureWithDescriptor:td];
            s->textureWidth=w; s->textureHeight=h;
            regions=NULL; regionCount=0; // full upload after texture reconstruction
        }
        if (!s->uploadTexture) return;
        if (!regions || regionCount==0) {
            MTLRegion rect=MTLRegionMake2D(0,0,w,h);
            [s->uploadTexture replaceRegion:rect mipmapLevel:0 withBytes:pixels bytesPerRow:(NSUInteger)w*4u];
        } else {
            for(uint32_t i=0;i<regionCount;i++) {
                const uint32_t *r=regions+4u*i;
                uint32_t x=MIN(r[0],w), y=MIN(r[1],h);
                uint32_t rw=MIN(r[2],w-x), rh=MIN(r[3],h-y);
                if (!rw || !rh) continue;
                const uint8_t *src=(const uint8_t *)(pixels+(size_t)y*w+x);
                [s->uploadTexture replaceRegion:MTLRegionMake2D(x,y,rw,rh) mipmapLevel:0
                      withBytes:src bytesPerRow:(NSUInteger)w*4u];
            }
        }
        id<CAMetalDrawable> drawable=[s->layer nextDrawable];
        if(!drawable) return;
        id<MTLCommandBuffer> command=[s->commands commandBuffer];
        MTLRenderPassDescriptor *pass=[MTLRenderPassDescriptor renderPassDescriptor];
        pass.colorAttachments[0].texture=drawable.texture;
        pass.colorAttachments[0].loadAction=MTLLoadActionDontCare;
        pass.colorAttachments[0].storeAction=MTLStoreActionStore;
        id<MTLRenderCommandEncoder> encoder=[command renderCommandEncoderWithDescriptor:pass];
        [encoder setRenderPipelineState:s->pipeline];
        [encoder setFragmentTexture:s->uploadTexture atIndex:0];
        [encoder drawPrimitives:MTLPrimitiveTypeTriangleStrip vertexStart:0 vertexCount:4];
        [encoder endEncoding];
        [command presentDrawable:drawable];
        [command commit];
    }
}
void mata_apple_window_title(void *p,const char *title) {
    state(p)->window.title=title?[NSString stringWithUTF8String:title]:@"";
}
void mata_apple_window_action(void *p, int action, int32_t a, int32_t b) {
    MataState *s=state(p); NSWindow *w=s->window;
    switch(action) {
        case 1: [w orderOut:nil]; break;  // hide
        case 2: [w makeKeyAndOrderFront:nil]; break;
        case 3: if (![w isZoomed]) [w zoom:nil]; break;
        case 4: [w miniaturize:nil]; break;
        case 5: if ([w isMiniaturized]) [w deminiaturize:nil]; else if ([w isZoomed]) [w zoom:nil]; break;
        case 6: [w setFrameOrigin:NSMakePoint(a,b)]; break;
        case 7: {
            CGFloat scale=w.backingScaleFactor;
            [w setContentSize:NSMakeSize(MAX(1,a)/scale,MAX(1,b)/scale)]; break;
        }
        case 8: { // decorations
            NSWindowStyleMask newStyle=a ? NSWindowStyleMaskTitled|NSWindowStyleMaskClosable|NSWindowStyleMaskMiniaturizable|NSWindowStyleMaskResizable : NSWindowStyleMaskBorderless;
            [w setStyleMask:newStyle]; break;
        }
        case 9: [[w standardWindowButton:NSWindowZoomButton] setEnabled:a!=0]; break;
        case 10: [[w standardWindowButton:NSWindowMiniaturizeButton] setEnabled:a!=0]; break;
        case 11: [w setStyleMask:a?(w.styleMask|NSWindowStyleMaskResizable):(w.styleMask&~NSWindowStyleMaskResizable)]; break;
    }
}
int mata_apple_window_cursor(void *p,int visible) {
    MataState *s=state(p);
    if (s->cursorVisible != (BOOL)visible) {
        if (s->mouseInside) { if (visible) [NSCursor unhide]; else [NSCursor hide]; }
        s->cursorVisible=visible;
    }
    return 0;
}
int mata_apple_window_cursor_visible(void *p) { return state(p)->cursorVisible; }
int mata_apple_window_cursor_focused(void *p) { return state(p)->mouseInside; }
void *mata_apple_window_handle(void *p) { return (__bridge void *)state(p)->window; }
