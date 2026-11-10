use crate::pdf::{
    Dict, Document, Result, Syntax, Value, bad, is_name, limit, number, required, unsupported,
};
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};
use vello_cpu::{
    Pixmap,
    kurbo::{Affine, BezPath, ParamCurve, Rect, Shape, Stroke},
};

#[derive(Clone)]
pub(crate) struct Clip {
    pub path: Arc<BezPath>,
    pub transform: Affine,
    pub even: bool,
}
#[derive(Clone)]
pub(crate) enum Paint {
    Solid([f32; 3], f32),
    Image(Arc<Pixmap>, f32),
}
pub(crate) struct Draw {
    pub path: Arc<BezPath>,
    pub transform: Affine,
    pub clips: Vec<Clip>,
    pub paint: Paint,
    pub stroke: Option<Stroke>,
    pub even: bool,
}
#[derive(Clone, Serialize)]
pub struct TextRun {
    pub text: String,
    pub transform: [f64; 6],
    pub width: f64,
    pub font_size: f64,
}
pub(crate) struct Scene {
    pub draws: Vec<Draw>,
    pub text: Vec<TextRun>,
}
#[derive(Clone)]
struct State {
    ctm: Affine,
    fill: [f32; 3],
    stroke: [f32; 3],
    line: Stroke,
    fill_alpha: f32,
    stroke_alpha: f32,
    clips: Vec<Clip>,
    font: String,
    size: f64,
    tm: Affine,
    lm: Affine,
    leading: f64,
    char_space: f64,
    word_space: f64,
    hscale: f64,
    rise: f64,
    text_mode: usize,
}
impl Default for State {
    fn default() -> Self {
        Self {
            ctm: Affine::IDENTITY,
            fill: [0.; 3],
            stroke: [0.; 3],
            line: Stroke::new(1.),
            fill_alpha: 1.,
            stroke_alpha: 1.,
            clips: vec![],
            font: String::new(),
            size: 0.,
            tm: Affine::IDENTITY,
            lm: Affine::IDENTITY,
            leading: 0.,
            char_space: 0.,
            word_space: 0.,
            hscale: 1.,
            rise: 0.,
            text_mode: 0,
        }
    }
}
struct Font {
    data: Arc<[u8]>,
    widths: HashMap<u16, f64>,
    default_width: f64,
    gids: Option<Arc<[u8]>>,
    unicode: HashMap<u16, String>,
    paths: HashMap<u16, BezPath>,
}
impl Font {
    fn load(doc: &mut Document, value: &Value) -> Result<Self> {
        let font = doc.resolve(value)?;
        let d = font.dict()?;
        if !is_name(d, "Subtype", "Type0") || !is_name(d, "Encoding", "Identity-H") {
            return Err(unsupported("requires embedded Type0 /Identity-H font"));
        }
        let descendants = required(d, "DescendantFonts")?.array()?;
        if descendants.len() != 1 {
            return Err(bad("font descendants"));
        }
        let child = doc.resolve(&descendants[0])?;
        let cd = child.dict()?;
        if !is_name(cd, "Subtype", "CIDFontType2") {
            return Err(unsupported("requires CIDFontType2"));
        }
        let descriptor = doc.resolve(required(cd, "FontDescriptor")?)?;
        let program = doc.resolve(required(descriptor.dict()?, "FontFile2")?)?;
        let data = doc.decode(&program)?;
        ttf_parser::Face::parse(&data, 0).map_err(|_| bad("embedded TrueType font"))?;
        let mut widths = HashMap::new();
        if let Some(w) = cd.get("W") {
            let a = w.array()?;
            let mut i = 0;
            while i < a.len() {
                let first = a[i].int()?;
                i += 1;
                let next = a.get(i).ok_or_else(|| bad("font W"))?;
                i += 1;
                if let Value::Array(v) = next {
                    for (n, width) in v.iter().enumerate() {
                        let cid = u16::try_from(first + n).map_err(|_| bad("font CID"))?;
                        widths.insert(cid, width.num()?);
                    }
                } else {
                    let last = next.int()?;
                    let width = a.get(i).ok_or_else(|| bad("font W range"))?.num()?;
                    i += 1;
                    if last < first || last > 65535 {
                        return Err(bad("font W range"));
                    }
                    for cid in first..=last {
                        widths.insert(cid as u16, width);
                    }
                }
            }
        }
        let gids = match cd.get("CIDToGIDMap") {
            None => None,
            Some(Value::Name(s)) if s == "Identity" => None,
            Some(v) => {
                let v = doc.resolve(v)?;
                Some(doc.decode(&v)?)
            }
        };
        let unicode = if let Some(v) = d.get("ToUnicode") {
            let v = doc.resolve(v)?;
            cmap(&doc.decode(&v)?)?
        } else {
            return Err(unsupported("font requires ToUnicode for usable text"));
        };
        Ok(Self {
            data,
            widths,
            default_width: number(cd, "DW", 1000.)?,
            gids,
            unicode,
            paths: HashMap::new(),
        })
    }
    fn glyph(&mut self, cid: u16) -> Result<&BezPath> {
        let gid = if let Some(map) = &self.gids {
            let i = cid as usize * 2;
            if i + 2 > map.len() {
                return Err(bad("CIDToGIDMap length"));
            }
            u16::from_be_bytes([map[i], map[i + 1]])
        } else {
            cid
        };
        if !self.paths.contains_key(&gid) {
            let face = ttf_parser::Face::parse(&self.data, 0).map_err(|_| bad("font"))?;
            if gid >= face.number_of_glyphs() {
                return Err(bad("glyph index"));
            }
            let mut path = Outline(BezPath::new());
            face.outline_glyph(ttf_parser::GlyphId(gid), &mut path);
            self.paths.insert(gid, path.0);
        }
        Ok(&self.paths[&gid])
    }
}
struct Outline(BezPath);
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to((x as f64, y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to((x as f64, y as f64));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to((x1 as f64, y1 as f64), (x as f64, y as f64));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.curve_to(
            (x1 as f64, y1 as f64),
            (x2 as f64, y2 as f64),
            (x as f64, y as f64),
        );
    }
    fn close(&mut self) {
        self.0.close_path();
    }
}
fn cmap(bytes: &[u8]) -> Result<HashMap<u16, String>> {
    let mut c = Syntax::new(bytes, 0);
    let mut out = HashMap::new();
    let mut previous = 0usize;
    while c.pos < bytes.len() {
        c.skip();
        if c.pos == bytes.len() {
            break;
        }
        let start = c.pos;
        if matches!(bytes[c.pos], b'<' | b'[' | b'/' | b'(') {
            let _ = c.value(0)?;
            previous = 0;
            continue;
        }
        let t = c.token()?;
        match t {
            b"beginbfchar" => {
                if previous > 65536 {
                    return Err(limit("ToUnicode entries"));
                }
                for _ in 0..previous {
                    let from = c.value(0)?;
                    let to = c.value(0)?;
                    out.insert(cid(from.bytes()?)?, unicode(to.bytes()?)?);
                }
                c.expect(b"endbfchar")?;
            }
            b"beginbfrange" => {
                if previous > 65536 {
                    return Err(limit("ToUnicode ranges"));
                }
                for _ in 0..previous {
                    let first = cid(c.value(0)?.bytes()?)?;
                    let last = cid(c.value(0)?.bytes()?)?;
                    if last < first {
                        return Err(bad("CMap range"));
                    }
                    let to = c.value(0)?;
                    match to {
                        Value::Bytes(b) => {
                            if b.len() != 2 {
                                return Err(unsupported("CMap range UTF16 length"));
                            }
                            let base = u16::from_be_bytes([b[0], b[1]]);
                            for code in first..=last {
                                let scalar = base
                                    .checked_add(code - first)
                                    .ok_or_else(|| bad("CMap overflow"))?;
                                out.insert(code, unicode(&scalar.to_be_bytes())?);
                            }
                        }
                        Value::Array(a) => {
                            if a.len() != (last - first) as usize + 1 {
                                return Err(bad("CMap range array"));
                            }
                            for (i, v) in a.iter().enumerate() {
                                out.insert(first + i as u16, unicode(v.bytes()?)?);
                            }
                        }
                        _ => return Err(bad("CMap target")),
                    }
                }
                c.expect(b"endbfrange")?;
            }
            _ => {
                previous = std::str::from_utf8(t)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
        }
        if c.pos <= start {
            return Err(bad("CMap progress"));
        }
    }
    Ok(out)
}
fn cid(b: &[u8]) -> Result<u16> {
    if b.len() != 2 {
        return Err(unsupported("two-byte CIDs required"));
    }
    Ok(u16::from_be_bytes([b[0], b[1]]))
}
fn unicode(b: &[u8]) -> Result<String> {
    if !b.len().is_multiple_of(2) {
        return Err(bad("UTF16 mapping"));
    }
    String::from_utf16(
        &b.as_chunks::<2>()
            .0
            .iter()
            .map(|x| u16::from_be_bytes([x[0], x[1]]))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| bad("UTF16 mapping"))
}

impl Document {
    pub(crate) fn scene(&mut self, index: usize) -> Result<Arc<Scene>> {
        if let Some(scene) = self.scenes.get(&index) {
            self.scene_order.retain(|i| *i != index);
            self.scene_order.push_back(index);
            return Ok(scene.clone());
        }
        let page = self
            .pages
            .get(index)
            .cloned()
            .ok_or_else(|| bad("page index"))?;
        let resources = self.resolve(&page.resources)?;
        let mut scene = Scene {
            draws: vec![],
            text: vec![],
        };
        let mut interpreter = Interpreter {
            doc: self,
            scene: &mut scene,
            fonts: HashMap::new(),
            operations: 0,
        };
        let bytes = interpreter.contents(&page.contents)?;
        interpreter.run(&bytes, resources.dict()?, State::default(), 0)?;
        let estimate = scene
            .draws
            .iter()
            .map(|d| std::mem::size_of_val(d.path.elements()) + d.clips.len() * 64 + 256)
            .sum::<usize>()
            + scene.text.iter().map(|t| t.text.len() + 128).sum::<usize>();
        self.reserve_cache(estimate)?;
        let scene = Arc::new(scene);
        self.scenes.insert(index, scene.clone());
        self.scene_order.push_back(index);
        self.scene_bytes.insert(index, estimate);
        Ok(scene)
    }
}
struct Interpreter<'a> {
    doc: &'a mut Document,
    scene: &'a mut Scene,
    fonts: HashMap<String, Font>,
    operations: usize,
}
impl Interpreter<'_> {
    fn contents(&mut self, v: &Value) -> Result<Vec<u8>> {
        let v = self.doc.resolve(v)?;
        match &*v {
            Value::Null => Ok(vec![]),
            Value::Stream(_, _) => Ok(self.doc.decode(&v)?.to_vec()),
            Value::Array(a) => {
                let mut bytes = Vec::new();
                for v in a {
                    let v = self.doc.resolve(v)?;
                    let data = self.doc.decode(&v)?;
                    if bytes.len() + data.len() + 1 > self.doc.limits.stream_bytes {
                        return Err(limit("page contents"));
                    }
                    bytes.extend_from_slice(&data);
                    bytes.push(b'\n');
                }
                Ok(bytes)
            }
            _ => Err(bad("page content type")),
        }
    }
    fn run(&mut self, bytes: &[u8], resources: &Dict, initial: State, depth: usize) -> Result<()> {
        if depth > 32 {
            return Err(limit("Form recursion"));
        }
        let mut c = Syntax::new(bytes, 0);
        let mut args: Vec<Value> = vec![];
        let mut state = initial;
        let mut stack = vec![];
        let mut path = BezPath::new();
        let mut path_ctm = None;
        let mut clip = None;
        let mut in_text = false;
        while {
            c.skip();
            c.pos < bytes.len()
        } {
            let b = bytes[c.pos];
            if matches!(b, b'/' | b'[' | b'(' | b'<')
                || b.is_ascii_digit()
                || matches!(b, b'-' | b'+' | b'.')
            {
                args.push(c.value(0)?);
                if args.len() > 64 {
                    return Err(limit("content operands"));
                }
                continue;
            }
            let op = c.token()?;
            self.operations += 1;
            if self.operations > self.doc.limits.operations {
                return Err(limit("content operations"));
            }
            let nums = || args.iter().map(Value::num).collect::<Result<Vec<_>>>();
            let n = |count: usize| -> Result<Vec<f64>> {
                if args.len() != count {
                    return Err(bad(format!("{} arity", String::from_utf8_lossy(op))));
                }
                let values = nums()?;
                if values.iter().any(|x| x.abs() > 1e6) {
                    return Err(limit("content coordinate magnitude"));
                }
                Ok(values)
            };
            match op {
                b"q" => {
                    n(0)?;
                    if stack.len() >= 64 {
                        return Err(limit("graphics stack"));
                    }
                    stack.push(state.clone());
                }
                b"Q" => {
                    n(0)?;
                    state = stack.pop().ok_or_else(|| bad("unbalanced Q"))?;
                }
                b"cm" => {
                    let a = n(6)?;
                    let m = Affine::new(a.try_into().unwrap());
                    if m.as_coeffs().iter().any(|x| x.abs() > 1e9) {
                        return Err(limit("transform magnitude"));
                    }
                    state.ctm *= m;
                    bounded_transform(state.ctm)?;
                }
                b"rg" | b"RG" => {
                    let a = n(3)?;
                    let rgb = [unit(a[0])?, unit(a[1])?, unit(a[2])?];
                    if op == b"rg" {
                        state.fill = rgb;
                    } else {
                        state.stroke = rgb;
                    }
                }
                b"g" | b"G" => {
                    let a = n(1)?;
                    let rgb = [unit(a[0])?; 3];
                    if op == b"g" {
                        state.fill = rgb;
                    } else {
                        state.stroke = rgb;
                    }
                }
                b"w" => {
                    let a = n(1)?;
                    if a[0] <= 0. {
                        return Err(unsupported("hairline/negative stroke width"));
                    }
                    state.line.width = a[0];
                }
                b"J" => {
                    let a = n(1)?;
                    state.line.start_cap = match a[0] {
                        0. => vello_cpu::kurbo::Cap::Butt,
                        1. => vello_cpu::kurbo::Cap::Round,
                        2. => vello_cpu::kurbo::Cap::Square,
                        _ => return Err(bad("line cap")),
                    };
                    state.line.end_cap = state.line.start_cap;
                }
                b"j" => {
                    let a = n(1)?;
                    state.line.join = match a[0] {
                        0. => vello_cpu::kurbo::Join::Miter,
                        1. => vello_cpu::kurbo::Join::Round,
                        2. => vello_cpu::kurbo::Join::Bevel,
                        _ => return Err(bad("line join")),
                    };
                }
                b"M" => {
                    let miter = n(1)?[0];
                    if miter < 1. {
                        return Err(bad("miter limit"));
                    }
                    state.line.miter_limit = miter;
                }
                b"d" => {
                    if args.len() != 2 {
                        return Err(bad("dash operands"));
                    }
                    let dash = args[0]
                        .array()?
                        .iter()
                        .map(Value::num)
                        .collect::<Result<Vec<_>>>()?;
                    if dash.len() > 32
                        || dash.iter().any(|&v| v < 0.)
                        || !dash.is_empty() && dash.iter().all(|&v| v == 0.)
                    {
                        return Err(bad("dash pattern"));
                    }
                    state.line.dash_pattern = dash.into();
                    state.line.dash_offset = args[1].num()?;
                }
                b"m" | b"l" | b"c" | b"v" | b"y" | b"h" | b"re" => {
                    if path_ctm.is_some_and(|t| t != state.ctm) {
                        return Err(unsupported("CTM changes inside a path"));
                    }
                    path_ctm = Some(state.ctm);
                    match op {
                        b"m" => {
                            let a = n(2)?;
                            path.move_to((a[0], a[1]));
                        }
                        b"l" => {
                            let a = n(2)?;
                            path.line_to((a[0], a[1]));
                        }
                        b"c" => {
                            let a = n(6)?;
                            path.curve_to((a[0], a[1]), (a[2], a[3]), (a[4], a[5]));
                        }
                        b"v" => {
                            let a = n(4)?;
                            let last = path
                                .segments()
                                .last()
                                .map(|s| s.end())
                                .ok_or_else(|| bad("v without current point"))?;
                            path.curve_to(last, (a[0], a[1]).into(), (a[2], a[3]).into());
                        }
                        b"y" => {
                            let a = n(4)?;
                            path.curve_to((a[0], a[1]), (a[2], a[3]), (a[2], a[3]));
                        }
                        b"h" => {
                            n(0)?;
                            path.close_path();
                        }
                        b"re" => {
                            let a = n(4)?;
                            path.extend(
                                Rect::new(a[0], a[1], a[0] + a[2], a[1] + a[3]).path_elements(0.1),
                            );
                        }
                        _ => unreachable!(),
                    }
                }
                b"W" | b"W*" => {
                    n(0)?;
                    clip = Some(op == b"W*");
                }
                b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" | b"n" => {
                    n(0)?;
                    if matches!(op, b"s" | b"b" | b"b*") {
                        path.close_path();
                    }
                    let p = Arc::new(std::mem::take(&mut path));
                    let transform = path_ctm.take().unwrap_or(state.ctm);
                    if matches!(op, b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*") {
                        self.scene.draws.push(Draw {
                            path: p.clone(),
                            transform,
                            clips: state.clips.clone(),
                            paint: Paint::Solid(state.fill, state.fill_alpha),
                            stroke: None,
                            even: matches!(op, b"f*" | b"B*" | b"b*"),
                        });
                    }
                    if matches!(op, b"S" | b"s" | b"B" | b"B*" | b"b" | b"b*") {
                        self.scene.draws.push(Draw {
                            path: p.clone(),
                            transform,
                            clips: state.clips.clone(),
                            paint: Paint::Solid(state.stroke, state.stroke_alpha),
                            stroke: Some(state.line.clone()),
                            even: false,
                        });
                    }
                    if let Some(even) = clip.take() {
                        state.clips.push(Clip {
                            path: p,
                            transform,
                            even,
                        });
                        if state.clips.len() > 64 {
                            return Err(limit("clip stack"));
                        }
                    }
                }
                b"BT" => {
                    n(0)?;
                    if in_text {
                        return Err(bad("nested BT"));
                    }
                    in_text = true;
                    state.tm = Affine::IDENTITY;
                    state.lm = Affine::IDENTITY;
                }
                b"ET" => {
                    n(0)?;
                    if !in_text {
                        return Err(bad("ET without BT"));
                    }
                    in_text = false;
                }
                b"Tf" => {
                    if args.len() != 2 {
                        return Err(bad("Tf"));
                    }
                    state.font = args[0].name()?.into();
                    state.size = args[1].num()?;
                    if state.size <= 0. || state.size > 10000. {
                        return Err(bad("font size"));
                    }
                }
                b"Tm" => {
                    let a = n(6)?;
                    state.tm = Affine::new(a.try_into().unwrap());
                    state.lm = state.tm;
                }
                b"Td" | b"TD" => {
                    let a = n(2)?;
                    if op == b"TD" {
                        state.leading = -a[1];
                    }
                    state.lm *= Affine::translate((a[0], a[1]));
                    state.tm = state.lm;
                }
                b"T*" => {
                    n(0)?;
                    state.lm *= Affine::translate((0., -state.leading));
                    state.tm = state.lm;
                }
                b"Tc" => state.char_space = n(1)?[0],
                b"Tw" => state.word_space = n(1)?[0],
                b"Tz" => state.hscale = n(1)?[0] / 100.,
                b"TL" => state.leading = n(1)?[0],
                b"Ts" => state.rise = n(1)?[0],
                b"Tr" => {
                    let a = n(1)?;
                    state.text_mode = if a[0] == 0. {
                        0
                    } else if a[0] == 3. {
                        3
                    } else {
                        return Err(unsupported("text render mode"));
                    };
                }
                b"Tj" | b"TJ" => {
                    if !in_text || args.len() != 1 {
                        return Err(bad("text showing outside BT/arity"));
                    }
                    if op == b"Tj" {
                        self.show(args[0].bytes()?, resources, &mut state)?;
                    } else {
                        for item in args[0].array()? {
                            match item {
                                Value::Bytes(b) => self.show(b, resources, &mut state)?,
                                Value::Num(n) => {
                                    state.tm *= Affine::translate((
                                        -n / 1000. * state.size * state.hscale,
                                        0.,
                                    ))
                                }
                                _ => return Err(bad("TJ element")),
                            }
                        }
                    }
                }
                b"Do" => {
                    if args.len() != 1 {
                        return Err(bad("Do"));
                    }
                    let name = args[0].name()?;
                    let xobjects = self.doc.resolve(required(resources, "XObject")?)?;
                    let object = self.doc.resolve(required(xobjects.dict()?, name)?)?;
                    let d = object.dict()?;
                    if is_name(d, "Subtype", "Image") {
                        let image = self.doc.image(&object, 0)?;
                        self.scene.draws.push(Draw {
                            path: Arc::new(Rect::new(0., 0., 1., 1.).to_path(0.1)),
                            transform: state.ctm,
                            clips: state.clips.clone(),
                            paint: Paint::Image(image, state.fill_alpha),
                            stroke: None,
                            even: false,
                        });
                    } else if is_name(d, "Subtype", "Form") {
                        if d.contains_key("Group") || d.contains_key("Ref") {
                            return Err(unsupported("Form group/reference"));
                        }
                        let mut inner = state.clone();
                        if let Some(m) = d.get("Matrix") {
                            let a = m
                                .array()?
                                .iter()
                                .map(Value::num)
                                .collect::<Result<Vec<_>>>()?;
                            inner.ctm *= Affine::new(a.try_into().map_err(|_| bad("Form matrix"))?);
                            bounded_transform(inner.ctm)?;
                        }
                        let bbox = required(d, "BBox")?.array()?;
                        if bbox.len() != 4 {
                            return Err(bad("Form BBox"));
                        }
                        let a = bbox.iter().map(Value::num).collect::<Result<Vec<_>>>()?;
                        inner.clips.push(Clip {
                            path: Arc::new(Rect::new(a[0], a[1], a[2], a[3]).to_path(0.1)),
                            transform: inner.ctm,
                            even: false,
                        });
                        let r = if let Some(r) = d.get("Resources") {
                            self.doc.resolve(r)?
                        } else {
                            Arc::new(Value::Dict(resources.clone()))
                        };
                        let content = self.doc.decode(&object)?;
                        let saved = std::mem::take(&mut self.fonts);
                        self.run(&content, r.dict()?, inner, depth + 1)?;
                        self.fonts = saved;
                    } else {
                        return Err(unsupported("XObject subtype"));
                    }
                }
                b"gs" => {
                    if args.len() != 1 {
                        return Err(bad("gs"));
                    }
                    let name = args[0].name()?;
                    let all = self.doc.resolve(required(resources, "ExtGState")?)?;
                    let g = self.doc.resolve(required(all.dict()?, name)?)?;
                    let g = g.dict()?;
                    for key in g.keys() {
                        if !["Type", "ca", "CA", "BM"].contains(&key.as_str()) {
                            return Err(unsupported(format!("ExtGState /{key}")));
                        }
                    }
                    if let Some(blend) = g.get("BM")
                        && blend.name()? != "Normal"
                    {
                        return Err(unsupported("blend mode"));
                    }
                    state.fill_alpha = unit(number(g, "ca", state.fill_alpha as f64)?)?;
                    state.stroke_alpha = unit(number(g, "CA", state.stroke_alpha as f64)?)?;
                }
                b"BMC" => {
                    if args.len() != 1 {
                        return Err(bad("BMC"));
                    }
                }
                b"BDC" => {
                    if args.len() != 2 {
                        return Err(bad("BDC"));
                    }
                    if args[0].name()? == "OC" {
                        return Err(unsupported("optional content"));
                    }
                }
                b"EMC" => {
                    n(0)?;
                }
                _ => {
                    return Err(unsupported(format!(
                        "content operator {}",
                        String::from_utf8_lossy(op)
                    )));
                }
            }
            bounded_transform(state.ctm)?;
            bounded_transform(state.tm)?;
            bounded_transform(state.lm)?;
            if path.elements().len() > self.doc.limits.operations {
                return Err(limit("path elements"));
            }
            args.clear();
        }
        if !stack.is_empty()
            || in_text
            || !args.is_empty()
            || clip.is_some()
            || !path.elements().is_empty()
        {
            return Err(bad("unbalanced/incomplete content"));
        }
        Ok(())
    }
    fn show(&mut self, bytes: &[u8], resources: &Dict, state: &mut State) -> Result<()> {
        if !bytes.len().is_multiple_of(2) {
            return Err(bad("CID string length"));
        }
        if !self.fonts.contains_key(&state.font) {
            let fonts = self.doc.resolve(required(resources, "Font")?)?;
            let font = Font::load(self.doc, required(fonts.dict()?, &state.font)?)?;
            self.fonts.insert(state.font.clone(), font);
        }
        let font = self.fonts.get_mut(&state.font).unwrap();
        let face = ttf_parser::Face::parse(&font.data, 0).map_err(|_| bad("font"))?;
        let units = face.units_per_em() as f64;
        let mut path = BezPath::new();
        let mut advance = 0.;
        let mut text = String::new();
        if bytes.len() / 2 > self.doc.limits.operations.saturating_sub(self.operations) {
            return Err(limit("glyph budget"));
        }
        self.operations += bytes.len() / 2;
        for pair in bytes.as_chunks::<2>().0 {
            let cid = u16::from_be_bytes([pair[0], pair[1]]);
            let unicode = font
                .unicode
                .get(&cid)
                .cloned()
                .unwrap_or_else(|| "\u{fffd}".into());
            text.push_str(&unicode);
            let transform = Affine::translate((advance, 0.)) * Affine::scale(state.size / units);
            let glyph = font.glyph(cid)?;
            if path.elements().len() + glyph.elements().len() > self.doc.limits.operations {
                return Err(limit("glyph outline nodes"));
            }
            path.extend(glyph.elements().iter().copied().map(|el| transform * el));
            // Identity-H codes are two bytes; PDF word spacing only applies to a
            // single-byte 0x20 code, so it never applies here.
            advance += font.widths.get(&cid).copied().unwrap_or(font.default_width) / 1000.
                * state.size
                + state.char_space;
        }
        let transform = state.ctm
            * state.tm
            * Affine::translate((0., state.rise))
            * Affine::scale_non_uniform(state.hscale, 1.);
        if state.text_mode == 0 {
            self.scene.draws.push(Draw {
                path: Arc::new(path),
                transform,
                clips: state.clips.clone(),
                paint: Paint::Solid(state.fill, state.fill_alpha),
                stroke: None,
                even: false,
            });
        }
        self.scene.text.push(TextRun {
            text,
            transform: transform.as_coeffs(),
            width: advance,
            font_size: state.size,
        });
        state.tm *= Affine::translate((advance * state.hscale, 0.));
        Ok(())
    }
}
fn unit(v: f64) -> Result<f32> {
    if !(0.0..=1.0).contains(&v) {
        Err(bad("color/alpha range"))
    } else {
        Ok(v as f32)
    }
}

impl Document {
    pub(crate) fn image(&mut self, object: &Value, depth: usize) -> Result<Arc<Pixmap>> {
        if depth > 1 {
            return Err(unsupported("nested soft masks"));
        }
        let Value::Stream(d, range) = object else {
            return Err(bad("image stream"));
        };
        if let Some(image) = self.images.get(range) {
            return Ok(image.clone());
        }
        let width = required(d, "Width")?.int()?;
        let height = required(d, "Height")?.int()?;
        if width == 0
            || height == 0
            || width > u16::MAX as usize
            || height > u16::MAX as usize
            || width
                .checked_mul(height)
                .is_none_or(|n| n > self.limits.image_pixels)
        {
            return Err(limit("image dimensions/pixels"));
        }
        if number(d, "BitsPerComponent", 8.)? != 8. {
            return Err(unsupported("image bit depth"));
        }
        for key in ["Mask", "SMaskInData", "Alternates", "OPI", "Matte"] {
            if d.contains_key(key) {
                return Err(unsupported(format!("image /{key}")));
            }
        }
        if matches!(d.get("ImageMask"), Some(Value::Bool(true))) {
            return Err(unsupported("image stencil"));
        }
        let color = required(d, "ColorSpace")?.name()?;
        let gray = match color {
            "DeviceGray" => true,
            "DeviceRGB" => false,
            _ => return Err(unsupported("image color space")),
        };
        let components = if gray { 1 } else { 3 };
        if let Some(decode) = d.get("Decode") {
            let a = decode.array()?;
            if a.len() != components * 2
                || a.iter()
                    .enumerate()
                    .any(|(i, v)| v.num().ok() != Some((i % 2) as f64))
            {
                return Err(unsupported("nonidentity image Decode"));
            }
        }
        let filter = match d.get("Filter") {
            Some(Value::Name(n)) => n.as_str(),
            Some(Value::Array(a)) => a.last().ok_or_else(|| bad("empty filters"))?.name()?,
            None => "",
            _ => return Err(bad("image Filter")),
        };
        let data = self.decode(object)?;
        let mut rgba = match filter {
            "JXLDecode" => crate::codecs::decode_jxl(&data, width, height, gray, &self.limits)?,
            "DCTDecode" => {
                let options = zune_jpeg::zune_core::options::DecoderOptions::default()
                    .jpeg_set_out_colorspace(if gray {
                        zune_jpeg::zune_core::colorspace::ColorSpace::Luma
                    } else {
                        zune_jpeg::zune_core::colorspace::ColorSpace::RGB
                    });
                let mut decoder = zune_jpeg::JpegDecoder::new_with_options(
                    std::io::Cursor::new(data.as_ref()),
                    options,
                );
                decoder.decode_headers().map_err(|e| bad(e.to_string()))?;
                let source_color = decoder.input_colorspace();
                use zune_jpeg::zune_core::colorspace::ColorSpace;
                if !matches!(
                    (gray, source_color),
                    (true, Some(ColorSpace::Luma))
                        | (false, Some(ColorSpace::RGB | ColorSpace::YCbCr))
                ) {
                    return Err(unsupported("JPEG source components"));
                }
                if decoder.dimensions() != Some((width, height)) {
                    return Err(bad("JPEG dimensions"));
                }
                let pixels = decoder.decode().map_err(|e| bad(e.to_string()))?;
                let count = width * height;
                if pixels.len() != count * components {
                    return Err(unsupported("JPEG component/color conversion"));
                }
                expand(&pixels, components)
            }
            _ => {
                if data.len() != width * height * components {
                    return Err(bad("image sample length"));
                }
                expand(&data, components)
            }
        };
        if let Some(mask) = d.get("SMask")
            && !matches!(mask,Value::Name(n) if n=="None")
        {
            let mask = self.resolve(mask)?;
            let md = mask.dict()?;
            if !is_name(md, "ColorSpace", "DeviceGray") {
                return Err(unsupported("soft mask color"));
            }
            let mask = self.image(&mask, depth + 1)?;
            let mw = mask.width() as usize;
            let mh = mask.height() as usize;
            let samples = mask.data_as_u8_slice();
            for y in 0..height {
                for x in 0..width {
                    let sx = (x * mw / width).min(mw - 1);
                    let sy = (y * mh / height).min(mh - 1);
                    rgba[(y * width + x) * 4 + 3] = samples[(sy * mw + sx) * 4];
                }
            }
        }
        self.reserve_cache(rgba.len())?;
        let image = Arc::new(Pixmap::from_parts(
            rgba,
            width as u16,
            height as u16,
            vello_cpu::PixelMetadata::new(vello_cpu::peniko::ImageAlphaType::Alpha, true),
        ));
        self.images.insert(range.clone(), image.clone());
        Ok(image)
    }
}
fn expand(data: &[u8], components: usize) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(data.len() / components * 4);
    for p in data.chunks_exact(components) {
        if components == 1 {
            rgba.extend_from_slice(&[p[0], p[0], p[0], 255]);
        } else {
            rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
        }
    }
    rgba
}

fn bounded_transform(t: Affine) -> Result<()> {
    if t.as_coeffs()
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1e9)
    {
        Err(limit("composed transform magnitude"))
    } else {
        Ok(())
    }
}
