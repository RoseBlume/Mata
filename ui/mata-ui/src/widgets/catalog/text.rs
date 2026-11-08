//! Text-oriented catalog widgets.

use std::sync::{ Arc, Mutex };

#[cfg(feature = "fonts")]
use crate::Framebuffer;
#[cfg(feature = "fonts")]
use crate::widgets::{ FontRenderer, WidgetError };
use super::base::{ Widget, WidgetExt };

#[derive(Debug, Clone)]
/// Stores the state and data for text state.
///
/// Visibility: **private**.
struct TextState {
    /// Stores widget.
    ///
    /// Visibility: **private**.
    widget: Widget,
    /// Stores text.
    ///
    /// Visibility: **private**.
    text: Arc<Mutex<String>>,
    selection: Arc<Mutex<TextSelection>>,
}

/// Byte offsets always point to UTF-8 character boundaries.
#[derive(Debug, Clone, Default)]
struct TextSelection {
    selectable: bool,
    anchor: usize,
    caret: usize,
}

impl TextState {
    /// Creates a new instance.
    ///
    /// Visibility: **private**.
    #[doc = "Creates a new value. Visibility: private/internal."]
    fn new(text: impl Into<String>) -> Self {
        Self { widget: Widget::new(), text: Arc::new(Mutex::new(text.into())), selection: Arc::new(Mutex::new(TextSelection::default())) }
    }
    /// Performs the text operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the text operation. Visibility: private/internal."]
    fn text(&self) -> String {
        self.text.lock().unwrap().clone()
    }
    /// Sets text.
    ///
    /// Visibility: **private**.
    #[doc = "Sets text. Visibility: private/internal."]
    fn set_text(&self, text: impl Into<String>) {
        *self.text.lock().unwrap() = text.into();
        let mut selection = self.selection.lock().unwrap();
        selection.anchor = 0;
        selection.caret = 0;
        drop(selection);
        self.widget.invalidate();
    }
    fn selection_anchor(&self) -> usize {self.selection.lock().unwrap().anchor}
    fn selection_caret(&self) -> usize {self.selection.lock().unwrap().caret}
    fn selectable(&self) -> bool { self.selection.lock().unwrap().selectable }
    fn set_selectable(&self, enabled: bool) {
        let mut selection = self.selection.lock().unwrap();
        selection.selectable = enabled;
        if !enabled { selection.anchor = 0; selection.caret = 0; }
        drop(selection);
        self.widget.invalidate();
    }
    fn selection_range(&self) -> Option<(usize, usize)> {
        let (a,b) = {
            let selection = self.selection.lock().unwrap();
            if !selection.selectable || selection.anchor == selection.caret { return None; }
            (selection.anchor.min(selection.caret), selection.anchor.max(selection.caret))
        };
        let value = self.text.lock().unwrap();
        (value.is_char_boundary(a) && value.is_char_boundary(b) && b <= value.len()).then_some((a,b))
    }
    fn selected_text(&self) -> Option<String> {
        let (start,end) = self.selection_range()?;
        self.text.lock().unwrap().get(start..end).map(str::to_owned)
    }
    fn set_selection(&self, anchor: usize, caret: usize) {
        let value = self.text.lock().unwrap();
        let clamp = |mut n: usize| { n = n.min(value.len()); while !value.is_char_boundary(n) { n -= 1; } n };
        let mut selection = self.selection.lock().unwrap();
        if !selection.selectable { return; }
        selection.anchor = clamp(anchor);
        selection.caret = clamp(caret);
        drop(selection);
        self.widget.invalidate();
    }
    fn clear_selection(&self) {
        let mut s = self.selection.lock().unwrap();
        s.anchor = 0; s.caret = 0;
        drop(s);
        self.widget.invalidate();
    }
}

macro_rules! text_widget {
    ($($name:ident),* $(,)?) => {
        $(
        #[derive(Debug, Clone)]
        #[doc = "Generated text-oriented catalog widget. Visibility: public."]
        pub struct $name {
            #[doc = "Stores the shared text-widget state. Visibility: private."]
            inner: TextState,
        }
        impl Default for $name { #[doc = "Creates the default value. Visibility: private/internal."] fn default() -> Self { Self::new("") } }
        impl $name {
            /// Creates a new instance.
            ///
            /// Visibility: **public**.
            #[doc = "Creates a new value. Visibility: public."] pub fn new(text: impl Into<String>) -> Self { Self { inner: TextState::new(text) } }
            /// Performs the text operation.
            ///
            /// Visibility: **public**.
            #[doc = "Implements the text operation. Visibility: public."] pub fn text(&self) -> String { self.inner.text() }
            /// Sets text.
            ///
            /// Visibility: **public**.
            #[doc = "Sets text. Visibility: public."] pub fn set_text(&self, text: impl Into<String>) { self.inner.set_text(text); }
            /// Allow selecting this text with the pointer. Disabled by default.
            pub fn set_selectable(&self, enabled: bool) { self.inner.set_selectable(enabled); }
            pub fn selectable(&self) -> bool { self.inner.selectable() }
            pub(crate) fn selection_anchor(&self)->usize{self.inner.selection_anchor()}
            pub(crate) fn selection_caret(&self)->usize{self.inner.selection_caret()}
            /// Selected UTF-8 byte interval (start inclusive, end exclusive).
            pub fn selection_range(&self) -> Option<(usize,usize)> { self.inner.selection_range() }
            pub fn selected_text(&self) -> Option<String> { self.inner.selected_text() }
            pub fn clear_selection(&self) { self.inner.clear_selection(); }
            pub fn select_all(&self) { self.inner.set_selection(0,self.text().len()); }
            /// Updates an anchor/caret pair. Indices are clamped to Unicode boundaries.
            pub fn set_selection(&self, anchor: usize, caret: usize) { self.inner.set_selection(anchor,caret); }
            /// Access the underlying text widget as a renderable Text node.
            pub(crate) fn as_text_widget(&self) -> Text { Text { inner: self.inner.clone() } }
            /// Selects an explicit font size in device-independent pixels.
            pub fn set_font_size(&self, px: f32) {
                self.inner.widget.style_handle().update_properties(|style| {
                    style.font.size = Some(crate::widgets::style::Length::Px(px.max(1.0)));
                });
            }
            /// Selects the preferred font family (with system fallback).
            pub fn set_font_family(&self, family: impl Into<String>) {
                self.inner.widget.style_handle().update_properties(|style| {
                    style.font.family = Some(vec![family.into()]);
                });
            }
            /// Selects a fallback list of font families in preference order.
            pub fn set_font_families(&self, families: Vec<String>) {
                self.inner.widget.style_handle().update_properties(|style| {
                    style.font.family = Some(families);
                });
            }

            /// Draws this text with `mata_fonts` into a MataWin framebuffer.
            #[cfg(feature = "fonts")]
            #[doc = "Implements the blit operation. Visibility: public."] pub fn blit(&self, target: &mut Framebuffer, font: &mut FontRenderer, pt: u16, color: u32) -> Result<(), WidgetError> {
                let viewport=(target.width,target.height);
                let computed=self.inner.widget.computed_style(viewport,None);
                let style=self.inner.widget.style_handle().properties();
                let pt=if style.font.size.is_some(){computed.font_size_px.round().clamp(1.0,u16::MAX as f32) as u16}else{pt};
                let color=computed.foreground_color.unwrap_or(color);
                let alpha=((color>>24)&0xff) as f32*computed.opacity;
                let color=((alpha.round().clamp(0.0,255.0) as u32)<<24)|(color&0x00ff_ffff);
                let text=self.text();
                // A zero-sized text widget is content-sized, using the
                // framebuffer as its available viewport.
                let requested=self.inner.widget.bounds();
                let max_width=target.width.saturating_sub(requested.x.max(0) as u32).max(1);
                let max_height=target.height.saturating_sub(requested.y.max(0) as u32).max(1);
                let mut inherited=crate::widgets::style::ComputedStyle::default();
                inherited.font_size_px=pt as f32;
                inherited.line_height_px=if style.font.size.is_some() {
                    computed.line_height_px
                } else {pt as f32*1.2};
                let measured=crate::widgets::text_selection::measure_wrapped(&text,
                    &self.inner.widget,if requested.width==0{max_width}else{requested.width},
                    &inherited,viewport,Some(&mut *font));
                let bounds=crate::widgets::Rect::new(requested.x,requested.y,
                    if requested.width==0{measured.0.min(max_width)}else{requested.width},
                    if requested.height==0{measured.1.min(max_height)}else{requested.height});
                inherited.bounds=bounds;
                inherited.content_bounds=bounds;
                let geometry=crate::widgets::text_selection::layout(&text,&self.inner.widget,bounds,
                    &inherited,viewport,Some(&mut *font));
                crate::widgets::text_selection::paint_selection(target,&text,&geometry,
                    self.selection_range(),&mut Some(&mut *font));
                for segment in &geometry.lines {
                    if let Some(line_text)=text.get(segment.start..segment.end){
                        font.draw_text_styled(target,line_text,segment.x,segment.y,pt,color,
                            Some(geometry.inner),geometry.style.style,geometry.style.variant,
                            geometry.style.weight,geometry.style.stretch,Some(geometry.step))?;
                    }
                }
                Ok(())
            }
        }
        impl WidgetExt for $name {
            #[doc = "Implements the widget operation. Visibility: private/internal."]
            fn widget(&self) -> &Widget { &self.inner.widget }
            fn renderable_content(&self) -> Option<crate::widgets::WidgetContent> {
                Some(crate::widgets::WidgetContent::Text(Text { inner: self.inner.clone() }))
            }
        }
        )*
    };
}

text_widget!(
    Label,
    Inscription,
    EditableLabel,
    Entry,
    PasswordEntry,
    SearchEntry,
    Text,
    TextView,
    ShortcutLabel,
    ShortcutsShortcut,
    Statusbar
);

#[cfg(test)]
mod selection_tests {
    use super::*;
    #[test]
    fn selection_is_explicit_opt_in_and_shared_by_clones() {
        let text=Text::new("hello 🌷 world");
        let copy=text.clone();
        assert!(!text.selectable());
        text.set_selection(0,5);
        assert!(text.selected_text().is_none());
        text.set_selectable(true);
        text.set_selection(0,5);
        assert_eq!(copy.selected_text().as_deref(),Some("hello"));
        text.set_selectable(false);
        assert!(copy.selected_text().is_none());
    }
    #[test]
    fn selection_clamps_utf8_and_resets_on_edit() {
        let t=Label::new("a🌷z");
        t.set_selectable(true);
        t.set_selection(2,10);
        assert_eq!(t.selection_range(),Some((1,6)));
        assert_eq!(t.selected_text().as_deref(),Some("🌷z"));
        t.set_text("replacement");
        assert!(t.selection_range().is_none());
        t.select_all();
        assert_eq!(t.selected_text().as_deref(),Some("replacement"));
    }
}
