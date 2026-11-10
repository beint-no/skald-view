use crate::{
    content::{Paint, TextRun},
    pdf::{Document, Result, bad, limit},
};
use serde::Serialize;
use vello_cpu::{
    Image, ImageSource, Pixmap, RenderContext, Resources,
    color::{AlphaColor, Srgb},
    kurbo::{Affine, Rect},
    peniko::Fill,
};

#[derive(Clone, Serialize)]
#[non_exhaustive]
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    #[serde(skip)]
    pub rgba: std::sync::Arc<[u8]>,
    pub text: Vec<TextRun>,
}
impl Document {
    /// Render a zero-based page at pixels per PDF point. Reuses immutable scenes and decoded assets.
    pub fn render(&mut self, index: usize, scale: f64) -> Result<RenderedPage> {
        if !scale.is_finite() || scale <= 0. || scale > 8. {
            return Err(bad("scale must be finite, positive and <= 8"));
        }
        if let Some((page, bits, frame)) = &self.frame
            && *page == index
            && *bits == scale.to_bits()
        {
            return Ok(frame.clone());
        }
        self.clear_frame_cache();
        let info = self.page_info(index)?.clone();
        let width = (info.width * scale).ceil();
        let height = (info.height * scale).ceil();
        if width > u16::MAX as f64
            || height > u16::MAX as f64
            || width * height > self.limits.render_pixels as f64
        {
            return Err(limit("render dimensions/pixels"));
        }
        let scene = self.scene(index)?;
        let mut context = RenderContext::new(width as u16, height as u16);
        context.set_paint(AlphaColor::<Srgb>::new([1., 1., 1., 1.]));
        context.fill_rect(&Rect::new(0., 0., width, height));
        let view = Affine::new([
            scale,
            0.,
            0.,
            -scale,
            -info.x * scale,
            (info.y + info.height) * scale,
        ]);
        for draw in &scene.draws {
            for clip in &draw.clips {
                context.set_transform(view * clip.transform);
                context.set_fill_rule(if clip.even {
                    Fill::EvenOdd
                } else {
                    Fill::NonZero
                });
                context.push_clip_layer(&clip.path);
            }
            context.set_transform(view * draw.transform);
            context.set_fill_rule(if draw.even {
                Fill::EvenOdd
            } else {
                Fill::NonZero
            });
            context.set_paint_transform(Affine::IDENTITY);
            match &draw.paint {
                Paint::Solid(c, a) => {
                    context.set_paint(AlphaColor::<Srgb>::new([c[0], c[1], c[2], *a]))
                }
                Paint::Gradient(gradient, transform) => {
                    context.set_paint(gradient.clone());
                    context.set_paint_transform(*transform);
                }
                Paint::Image(image, alpha) => {
                    if *alpha != 1. {
                        context.push_layer(None, None, Some(*alpha), None, None);
                    }
                    context.set_paint(Image {
                        image: ImageSource::Pixmap(image.clone()),
                        sampler: Default::default(),
                    });
                    context.set_paint_transform(Affine::new([
                        1. / image.width() as f64,
                        0.,
                        0.,
                        -1. / image.height() as f64,
                        0.,
                        1.,
                    ]));
                }
            }
            let composed;
            let path = if let Some(glyphs) = &draw.glyphs {
                let mut p = vello_cpu::kurbo::BezPath::new();
                for glyph in glyphs {
                    p.extend(
                        glyph
                            .outline
                            .elements()
                            .iter()
                            .copied()
                            .map(|el| glyph.transform * el),
                    );
                }
                composed = p;
                &composed
            } else {
                &draw.path
            };
            if let Some(stroke) = &draw.stroke {
                context.set_stroke(stroke.clone());
                context.stroke_path(path);
            } else {
                context.fill_path(path);
            }
            if matches!(&draw.paint,Paint::Image(_,alpha) if *alpha!=1.) {
                context.pop_layer();
            }
            for _ in &draw.clips {
                context.pop_layer();
            }
        }
        context.flush();
        let mut pixmap = Pixmap::new(width as u16, height as u16);
        context.render(&mut pixmap, &mut Resources::new());
        let text = scene
            .text
            .iter()
            .cloned()
            .map(|mut t| {
                t.transform = (view * Affine::new(t.transform)).as_coeffs();
                t
            })
            .collect();
        let frame = RenderedPage {
            width: width as u32,
            height: height as u32,
            rgba: pixmap
                .take_rgba8(vello_cpu::peniko::ImageAlphaType::AlphaPremultiplied)
                .into(),
            text,
        };
        if frame.rgba.len() <= self.limits.frame_cache_bytes
            && self.reserve_cache(frame.rgba.len()).is_ok()
        {
            self.frame = Some((index, scale.to_bits(), frame.clone()));
        }
        Ok(frame)
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    pub struct Viewer {
        document: Document,
        last: Option<RenderedPage>,
    }
    #[wasm_bindgen]
    impl Viewer {
        #[wasm_bindgen(constructor)]
        pub fn new(source: Vec<u8>) -> std::result::Result<Viewer, JsError> {
            Ok(Self {
                document: Document::open(source, Default::default())?,
                last: None,
            })
        }
        pub fn metadata(&self) -> std::result::Result<String, JsError> {
            let pages = (0..self.document.page_count())
                .map(|i| self.document.page_info(i))
                .collect::<Result<Vec<_>>>()?;
            Ok(serde_json::to_string(&pages)?)
        }
        pub fn render(&mut self, page: usize, scale: f64) -> std::result::Result<Vec<u8>, JsError> {
            let mut result = self.document.render(page, scale)?;
            let rgba = std::mem::take(&mut result.rgba).to_vec();
            self.last = Some(result);
            Ok(rgba)
        }
        pub fn frame_info(&self) -> std::result::Result<String, JsError> {
            Ok(serde_json::to_string(
                &self.last.as_ref().ok_or_else(|| bad("no rendered frame"))?,
            )?)
        }
        pub fn frame_cache_bytes(&self) -> usize {
            self.document.frame_cache_bytes()
        }
        pub fn cached_bytes(&self) -> usize {
            self.document.cached_bytes()
        }
    }
}
