use crate::{codecs, content::Scene};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    ops::Range,
    sync::Arc,
};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Malformed PDF: {0}")]
    Malformed(String),
    #[error("Outside the Skald View profile: {0}")]
    Unsupported(String),
    #[error("Resource limit exceeded: {0}")]
    Limit(String),
}
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) fn bad(s: impl Into<String>) -> Error {
    Error::Malformed(s.into())
}
pub(crate) fn unsupported(s: impl Into<String>) -> Error {
    Error::Unsupported(s.into())
}
pub(crate) fn limit(s: impl Into<String>) -> Error {
    Error::Limit(s.into())
}

#[derive(Clone, Debug)]
pub struct Limits {
    pub source_bytes: usize,
    pub frame_cache_bytes: usize,
    pub stream_bytes: usize,
    pub cached_bytes: usize,
    pub image_pixels: usize,
    pub render_pixels: usize,
    pub objects: usize,
    pub pages: usize,
    pub operations: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            source_bytes: 64 << 20,
            frame_cache_bytes: 8 << 20,
            stream_bytes: 16 << 20,
            cached_bytes: 128 << 20,
            image_pixels: 16_000_000,
            render_pixels: 16_000_000,
            objects: 100_000,
            pages: 2048,
            operations: 500_000,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Name(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Dict(Dict),
    Ref(u32),
    Stream(Dict, Range<usize>),
}
pub(crate) type Dict = BTreeMap<String, Value>;
impl Value {
    pub fn dict(&self) -> Result<&Dict> {
        match self {
            Self::Dict(d) | Self::Stream(d, _) => Ok(d),
            _ => Err(bad("expected dictionary")),
        }
    }
    pub fn array(&self) -> Result<&[Value]> {
        if let Self::Array(a) = self {
            Ok(a)
        } else {
            Err(bad("expected array"))
        }
    }
    pub fn num(&self) -> Result<f64> {
        if let Self::Num(n) = self {
            Ok(*n)
        } else {
            Err(bad("expected number"))
        }
    }
    pub fn int(&self) -> Result<usize> {
        let n = self.num()?;
        if n >= 0. && n.fract() == 0. && n <= usize::MAX as f64 {
            Ok(n as usize)
        } else {
            Err(bad("expected nonnegative integer"))
        }
    }
    pub fn name(&self) -> Result<&str> {
        if let Self::Name(n) = self {
            Ok(n)
        } else {
            Err(bad("expected name"))
        }
    }
    pub fn bytes(&self) -> Result<&[u8]> {
        if let Self::Bytes(b) = self {
            Ok(b)
        } else {
            Err(bad("expected string"))
        }
    }
}
pub(crate) fn required<'a>(d: &'a Dict, k: &str) -> Result<&'a Value> {
    d.get(k).ok_or_else(|| bad(format!("missing /{k}")))
}
pub(crate) fn number(d: &Dict, k: &str, default: f64) -> Result<f64> {
    d.get(k).map_or(Ok(default), Value::num)
}
pub(crate) fn is_name(d: &Dict, k: &str, n: &str) -> bool {
    matches!(d.get(k),Some(Value::Name(s)) if s==n)
}

pub(crate) struct Syntax<'a> {
    pub data: &'a [u8],
    pub pos: usize,
    pub nodes: usize,
}
impl<'a> Syntax<'a> {
    pub fn new(data: &'a [u8], pos: usize) -> Self {
        Self {
            data,
            pos,
            nodes: 0,
        }
    }
    pub fn skip(&mut self) {
        loop {
            while self.pos < self.data.len()
                && matches!(self.data[self.pos], 0 | 9 | 10 | 12 | 13 | 32)
            {
                self.pos += 1;
            }
            if self.data.get(self.pos) == Some(&b'%') {
                while self.pos < self.data.len() && !matches!(self.data[self.pos], 10 | 13) {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }
    pub fn token(&mut self) -> Result<&'a [u8]> {
        self.skip();
        let start = self.pos;
        while self.pos < self.data.len() && !delimiter(self.data[self.pos]) {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(bad("expected token"));
        }
        Ok(&self.data[start..self.pos])
    }
    pub fn expect(&mut self, s: &[u8]) -> Result<()> {
        if self.token()? == s {
            Ok(())
        } else {
            Err(bad(format!("expected {}", String::from_utf8_lossy(s))))
        }
    }
    pub fn uint(&mut self) -> Result<usize> {
        let t = self.token()?;
        if t.iter().any(|b| !b.is_ascii_digit()) {
            return Err(bad("expected unsigned integer"));
        }
        std::str::from_utf8(t)
            .map_err(|_| bad("integer"))?
            .parse()
            .map_err(|_| bad("integer overflow"))
    }
    pub fn value(&mut self, depth: usize) -> Result<Value> {
        self.nodes += 1;
        if depth > 64 || self.nodes > 1_000_000 {
            return Err(limit("syntax depth/node budget"));
        }
        self.skip();
        let b = *self
            .data
            .get(self.pos)
            .ok_or_else(|| bad("unexpected end"))?;
        match b {
            b'/' => {
                self.pos += 1;
                let mut n = Vec::new();
                while self.pos < self.data.len() && !delimiter(self.data[self.pos]) {
                    let b = self.data[self.pos];
                    self.pos += 1;
                    if b == b'#' {
                        if self.pos + 2 > self.data.len() {
                            return Err(bad("name escape"));
                        }
                        n.push(hex(self.data[self.pos])? * 16 + hex(self.data[self.pos + 1])?);
                        self.pos += 2;
                    } else {
                        n.push(b);
                    }
                }
                Ok(Value::Name(
                    String::from_utf8(n).map_err(|_| unsupported("non-UTF8 name"))?,
                ))
            }
            b'[' => {
                self.pos += 1;
                let mut v = Vec::new();
                loop {
                    self.skip();
                    if self.data.get(self.pos) == Some(&b']') {
                        self.pos += 1;
                        break;
                    }
                    v.push(self.value(depth + 1)?);
                }
                Ok(Value::Array(v))
            }
            b'<' if self.data.get(self.pos + 1) == Some(&b'<') => {
                self.pos += 2;
                let mut d = Dict::new();
                loop {
                    self.skip();
                    if self.data.get(self.pos..self.pos + 2) == Some(b">>") {
                        self.pos += 2;
                        break;
                    }
                    let k = self.value(depth + 1)?.name()?.to_owned();
                    let v = self.value(depth + 1)?;
                    if d.insert(k, v).is_some() {
                        return Err(bad("duplicate dictionary key"));
                    }
                }
                Ok(Value::Dict(d))
            }
            b'<' => {
                self.pos += 1;
                let mut bytes = Vec::new();
                let mut high = None;
                loop {
                    self.skip();
                    let c = *self.data.get(self.pos).ok_or_else(|| bad("hex string"))?;
                    self.pos += 1;
                    if c == b'>' {
                        break;
                    }
                    let x = hex(c)?;
                    if let Some(h) = high.take() {
                        bytes.push(h * 16 + x);
                    } else {
                        high = Some(x);
                    }
                }
                if let Some(h) = high {
                    bytes.push(h * 16);
                }
                Ok(Value::Bytes(bytes))
            }
            b'(' => {
                self.pos += 1;
                let mut bytes = Vec::new();
                let mut nesting = 1;
                while nesting > 0 {
                    let c = *self
                        .data
                        .get(self.pos)
                        .ok_or_else(|| bad("literal string"))?;
                    self.pos += 1;
                    match c {
                        b'(' => {
                            nesting += 1;
                            if nesting > 64 {
                                return Err(limit("literal string nesting"));
                            }
                            bytes.push(c)
                        }
                        b')' => {
                            nesting -= 1;
                            if nesting > 0 {
                                bytes.push(c)
                            }
                        }
                        b'\\' => {
                            let c = *self
                                .data
                                .get(self.pos)
                                .ok_or_else(|| bad("string escape"))?;
                            self.pos += 1;
                            match c {
                                b'n' => bytes.push(10),
                                b'r' => bytes.push(13),
                                b't' => bytes.push(9),
                                b'b' => bytes.push(8),
                                b'f' => bytes.push(12),
                                b'\n' => {}
                                b'\r' => {
                                    if self.data.get(self.pos) == Some(&10) {
                                        self.pos += 1;
                                    }
                                }
                                b'0'..=b'7' => {
                                    let mut n = (c - b'0') as u16;
                                    for _ in 0..2 {
                                        if let Some(c @ b'0'..=b'7') = self.data.get(self.pos) {
                                            n = n * 8 + (c - b'0') as u16;
                                            self.pos += 1;
                                        } else {
                                            break;
                                        }
                                    }
                                    bytes.push(n as u8)
                                }
                                _ => bytes.push(c),
                            }
                        }
                        b'\r' => {
                            bytes.push(10);
                            if self.data.get(self.pos) == Some(&10) {
                                self.pos += 1;
                            }
                        }
                        _ => bytes.push(c),
                    }
                }
                Ok(Value::Bytes(bytes))
            }
            _ => {
                let token = self.token()?;
                match token {
                    b"null" => Ok(Value::Null),
                    b"true" => Ok(Value::Bool(true)),
                    b"false" => Ok(Value::Bool(false)),
                    _ => {
                        let s = std::str::from_utf8(token).map_err(|_| bad("number encoding"))?;
                        if !s
                            .bytes()
                            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-'))
                        {
                            return Err(bad("invalid number token"));
                        }
                        let n: f64 = s.parse().map_err(|_| bad("number syntax"))?;
                        if !n.is_finite() {
                            return Err(bad("nonfinite number"));
                        }
                        let saved = self.pos;
                        if n > 0.
                            && n.fract() == 0.
                            && n <= u32::MAX as f64
                            && token.iter().all(u8::is_ascii_digit)
                        {
                            if let Ok(g) = self.uint()
                                && let Ok(t) = self.token()
                                && t == b"R"
                            {
                                if g != 0 {
                                    return Err(unsupported("nonzero generations"));
                                }
                                return Ok(Value::Ref(n as u32));
                            }
                            self.pos = saved;
                        }
                        Ok(Value::Num(n))
                    }
                }
            }
        }
    }
}
fn delimiter(b: u8) -> bool {
    matches!(
        b,
        0 | 9
            | 10
            | 12
            | 13
            | 32
            | b'('
            | b')'
            | b'<'
            | b'>'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'/'
            | b'%'
    )
}
fn hex(b: u8) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(bad("invalid hex digit")),
    }
}

#[derive(Clone, Debug)]
enum Location {
    Direct(usize),
    Packed(u32, usize),
}
#[derive(Clone)]
pub(crate) struct Page {
    pub resources: Value,
    pub contents: Value,
    pub info: PageInfo,
}
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct LinkAnnotation {
    pub rect: [f64; 4],
    pub uri: String,
}
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct PageInfo {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
    pub links: Vec<LinkAnnotation>,
    pub signature_fields_present: bool,
}

pub struct Document {
    pub(crate) source: Arc<[u8]>,
    pub(crate) limits: Limits,
    locations: HashMap<u32, Location>,
    objects: HashMap<u32, Arc<Value>>,
    resolving: HashSet<u32>,
    pub(crate) pages: Vec<Page>,
    pub(crate) decoded: HashMap<Range<usize>, Arc<[u8]>>,
    pub(crate) glyphs: HashMap<(usize, u16), Arc<vello_cpu::kurbo::BezPath>>,
    pub(crate) frame: Option<(usize, u64, crate::RenderedPage)>,
    pub(crate) images: HashMap<Range<usize>, Arc<vello_cpu::Pixmap>>,
    pub(crate) scenes: HashMap<usize, Arc<Scene>>,
    pub(crate) cached_bytes: usize,
    pub(crate) scene_order: VecDeque<usize>,
    pub(crate) scene_bytes: HashMap<usize, usize>,
}
impl Document {
    pub fn open(source: Vec<u8>, limits: Limits) -> Result<Self> {
        if source.len() > limits.source_bytes {
            return Err(limit("source bytes"));
        }
        if !source.trim_ascii_end().ends_with(b"%%EOF") {
            return Err(bad("missing EOF marker"));
        }
        if !source.starts_with(b"%PDF-2.0") || !source.get(8).is_some_and(u8::is_ascii_whitespace) {
            return Err(unsupported("requires PDF 2.0 header"));
        }
        let mut d = Self {
            source: source.into(),
            limits,
            locations: HashMap::new(),
            objects: HashMap::new(),
            resolving: HashSet::new(),
            pages: Vec::new(),
            decoded: HashMap::new(),
            images: HashMap::new(),
            glyphs: HashMap::new(),
            frame: None,
            scenes: HashMap::new(),
            cached_bytes: 0,
            scene_order: VecDeque::new(),
            scene_bytes: HashMap::new(),
        };
        let pos = d
            .source
            .windows(9)
            .rposition(|x| x == b"startxref")
            .ok_or_else(|| bad("missing startxref"))?;
        let mut tail = Syntax::new(&d.source, pos + 9);
        let offset = tail.uint()?;
        let source = d.source.clone();
        if source.get(offset..offset.saturating_add(4)) == Some(b"xref") {
            return Err(unsupported("requires cross-reference stream"));
        }
        let (xref_id, xref) = Self::indirect(&source, offset)?;
        let dict = xref.dict()?;
        if !is_name(dict, "Type", "XRef") {
            return Err(unsupported("requires cross-reference stream"));
        }
        for key in ["Encrypt", "Prev", "XRefStm"] {
            if dict.contains_key(key) {
                return Err(unsupported(format!("/{key}")));
            }
        }
        let root = required(dict, "Root")?.clone();
        let widths = required(dict, "W")?.array()?;
        if widths.len() != 3 {
            return Err(bad("xref W"));
        }
        let w = widths.iter().map(Value::int).collect::<Result<Vec<_>>>()?;
        if w.iter().any(|&x| x > 8) || w.iter().sum::<usize>() == 0 {
            return Err(bad("xref widths"));
        }
        let size = required(dict, "Size")?.int()?;
        if size > d.limits.objects {
            return Err(limit("objects"));
        }
        let indexes = if let Some(i) = dict.get("Index") {
            i.array()?
                .iter()
                .map(Value::int)
                .collect::<Result<Vec<_>>>()?
        } else {
            vec![0, size]
        };
        if indexes.len() % 2 != 0 {
            return Err(bad("xref Index"));
        }
        let bytes = d.decode(&xref)?;
        let mut p = 0usize;
        for pair in indexes.as_chunks::<2>().0 {
            if pair[0].checked_add(pair[1]).is_none_or(|n| n > size) {
                return Err(bad("xref range"));
            }
            for id in pair[0]..pair[0] + pair[1] {
                let mut fields = [0u64; 3];
                for i in 0..3 {
                    for _ in 0..w[i] {
                        fields[i] = fields[i]
                            .checked_mul(256)
                            .and_then(|x| bytes.get(p).map(|&b| x + b as u64))
                            .ok_or_else(|| bad("xref entry"))?;
                        p += 1;
                    }
                }
                let kind = if w[0] == 0 { 1 } else { fields[0] };
                let location = match kind {
                    0 => None,
                    1 => {
                        if fields[2] != 0 && id != 0 {
                            return Err(unsupported("nonzero generation"));
                        }
                        Some(Location::Direct(
                            usize::try_from(fields[1]).map_err(|_| bad("offset"))?,
                        ))
                    }
                    2 => Some(Location::Packed(
                        u32::try_from(fields[1]).map_err(|_| bad("object stream number"))?,
                        usize::try_from(fields[2]).map_err(|_| bad("object stream index"))?,
                    )),
                    _ => return Err(bad("xref type")),
                };
                if let Some(l) = location
                    && id > 0
                    && d.locations.insert(id as u32, l).is_some()
                {
                    return Err(bad("overlapping xref ranges"));
                }
            }
        }
        if p != bytes.len() {
            return Err(bad("trailing xref bytes"));
        }
        d.objects.insert(xref_id, Arc::new(xref));
        let catalog = d.resolve(&root)?;
        let catalog = catalog.dict()?;
        if !is_name(catalog, "Type", "Catalog") {
            return Err(bad("catalog type"));
        }
        for key in ["OpenAction", "AA", "OCProperties", "Perms"] {
            if catalog.contains_key(key) {
                return Err(unsupported(format!("catalog /{key}")));
            }
        }
        if let Some(form) = catalog.get("AcroForm") {
            let form = d.resolve(form)?;
            let form = form.dict()?;
            for key in ["XFA", "NeedAppearances", "CO", "AA"] {
                if form.contains_key(key) {
                    return Err(unsupported(format!("AcroForm /{key}")));
                }
            }
            let fields = d.resolve(required(form, "Fields")?)?;
            for field in fields.array()? {
                let field = d.resolve(field)?;
                d.invisible_signature(field.dict()?)?;
            }
        }
        let pages = required(catalog, "Pages")?.clone();
        d.walk_pages(&pages, None, None, 0, &mut HashSet::new())?;
        if d.pages.is_empty() {
            return Err(bad("no pages"));
        }
        Ok(d)
    }
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
    pub fn page_info(&self, index: usize) -> Result<&PageInfo> {
        self.pages
            .get(index)
            .map(|p| &p.info)
            .ok_or_else(|| bad("page index"))
    }
    pub fn cached_bytes(&self) -> usize {
        self.cached_bytes
    }
    pub(crate) fn reserve_cache(&mut self, n: usize) -> Result<()> {
        if n > self.limits.cached_bytes {
            return Err(limit("single cached asset/scene bytes"));
        }
        while self.cached_bytes.saturating_add(n) > self.limits.cached_bytes {
            let Some(index) = self.scene_order.pop_front() else {
                break;
            };
            self.scenes.remove(&index);
            self.cached_bytes -= self.scene_bytes.remove(&index).unwrap_or(0);
        }
        if self.cached_bytes.saturating_add(n) > self.limits.cached_bytes {
            let mut released = 0;
            self.glyphs.retain(|_, path| {
                if Arc::strong_count(path) == 1 {
                    released += std::mem::size_of_val(path.elements()) + 64;
                    false
                } else {
                    true
                }
            });
            self.images.retain(|_, image| {
                if Arc::strong_count(image) == 1 {
                    released += image.width() as usize * image.height() as usize * 4;
                    false
                } else {
                    true
                }
            });
            self.decoded.retain(|_, bytes| {
                if Arc::strong_count(bytes) == 1 {
                    released += bytes.len();
                    false
                } else {
                    true
                }
            });
            self.cached_bytes -= released;
        }
        if self.cached_bytes.saturating_add(n) > self.limits.cached_bytes {
            return Err(limit("active decoded asset bytes"));
        }
        self.cached_bytes += n;
        Ok(())
    }
    fn invisible_signature(&mut self, d: &Dict) -> Result<()> {
        if !is_name(d, "FT", "Sig")
            || d.contains_key("Kids")
            || d.contains_key("AA")
            || d.contains_key("AP")
        {
            return Err(unsupported("only invisible signature fields are supported"));
        }
        let rect = required(d, "Rect")?.array()?;
        if rect.len() != 4 || rect[0].num()? != rect[2].num()? || rect[1].num()? != rect[3].num()? {
            return Err(unsupported("visible signature widgets"));
        }
        Ok(())
    }
    pub fn clear_frame_cache(&mut self) {
        if let Some((_, _, frame)) = self.frame.take() {
            self.cached_bytes -= frame.rgba.len();
        }
    }
    pub fn frame_cache_bytes(&self) -> usize {
        self.frame
            .as_ref()
            .map_or(0, |(_, _, frame)| frame.rgba.len())
    }
    fn indirect(source: &[u8], offset: usize) -> Result<(u32, Value)> {
        if offset >= source.len() {
            return Err(bad("object offset"));
        }
        let mut c = Syntax::new(source, offset);
        let id = u32::try_from(c.uint()?).map_err(|_| bad("object ID"))?;
        if c.uint()? != 0 {
            return Err(unsupported("object generation"));
        }
        c.expect(b"obj")?;
        let value = c.value(0)?;
        c.skip();
        if matches!(value, Value::Dict(_)) && source.get(c.pos..c.pos + 6) == Some(b"stream") {
            c.expect(b"stream")?;
            if source.get(c.pos) == Some(&13) {
                c.pos += 1;
            }
            if source.get(c.pos) == Some(&10) {
                c.pos += 1;
            } else {
                return Err(bad("stream line end"));
            }
            let Value::Dict(dict) = value else {
                unreachable!()
            };
            let n = required(&dict, "Length")?.int()?;
            let end = c
                .pos
                .checked_add(n)
                .ok_or_else(|| bad("stream length overflow"))?;
            if end > source.len() {
                return Err(bad("stream length"));
            }
            let range = c.pos..end;
            c.pos = end;
            c.expect(b"endstream")?;
            c.expect(b"endobj")?;
            Ok((id, Value::Stream(dict, range)))
        } else {
            c.expect(b"endobj")?;
            Ok((id, value))
        }
    }
    pub(crate) fn resolve(&mut self, v: &Value) -> Result<Arc<Value>> {
        let Value::Ref(id) = v else {
            return Ok(Arc::new(v.clone()));
        };
        if let Some(v) = self.objects.get(id) {
            return Ok(v.clone());
        }
        if self.resolving.len() >= 64 || !self.resolving.insert(*id) {
            return Err(bad("object resolution cycle/depth"));
        }
        let result = self.load(*id);
        self.resolving.remove(id);
        result
    }
    fn load(&mut self, id: u32) -> Result<Arc<Value>> {
        let location = self
            .locations
            .get(&id)
            .cloned()
            .ok_or_else(|| bad("undefined object"))?;
        match location {
            Location::Direct(offset) => {
                let (actual, value) = Self::indirect(&self.source, offset)?;
                if actual != id {
                    return Err(bad("xref object mismatch"));
                }
                let v = Arc::new(value);
                self.objects.insert(id, v.clone());
                Ok(v)
            }
            Location::Packed(stream, index) => {
                let object = self.resolve(&Value::Ref(stream))?;
                let dict = object.dict()?;
                if !is_name(dict, "Type", "ObjStm") {
                    return Err(bad("object stream type"));
                }
                let n = required(dict, "N")?.int()?;
                if n > self.limits.objects {
                    return Err(limit("object stream N"));
                }
                if index >= n {
                    return Err(bad("object stream index"));
                }
                let first = required(dict, "First")?.int()?;
                let data = self.decode(&object)?;
                if first > data.len() {
                    return Err(bad("object stream First"));
                }
                let mut c = Syntax::new(&data[..first], 0);
                let mut entries = Vec::new();
                for _ in 0..n {
                    entries.push((
                        u32::try_from(c.uint()?).map_err(|_| bad("object stream ID"))?,
                        c.uint()?,
                    ));
                }
                for (i, (entry, offset)) in entries.iter().enumerate() {
                    if let Some(Location::Packed(expected, idx)) = self.locations.get(entry) {
                        if *expected != stream || *idx != i {
                            continue;
                        }
                        let start = first
                            .checked_add(*offset)
                            .ok_or_else(|| bad("object stream offset"))?;
                        let end = if i + 1 < n {
                            first
                                .checked_add(entries[i + 1].1)
                                .ok_or_else(|| bad("object stream end"))?
                        } else {
                            data.len()
                        };
                        if start >= end || end > data.len() {
                            return Err(bad("object stream bounds"));
                        }
                        let mut c = Syntax::new(&data[start..end], 0);
                        let v = c.value(0)?;
                        c.skip();
                        if c.pos != c.data.len() {
                            return Err(bad("object stream trailing tokens"));
                        }
                        self.objects.entry(*entry).or_insert(Arc::new(v));
                    }
                }
                self.objects
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| bad("compressed object mismatch"))
            }
        }
    }
    pub(crate) fn decode(&mut self, value: &Value) -> Result<Arc<[u8]>> {
        let Value::Stream(dict, range) = value else {
            return Err(bad("expected stream"));
        };
        if let Some(data) = self.decoded.get(range) {
            return Ok(data.clone());
        }
        let bytes = codecs::decode_stream(&self.source[range.clone()], dict, &self.limits)?;
        self.reserve_cache(bytes.len())?;
        let bytes: Arc<[u8]> = bytes.into();
        self.decoded.insert(range.clone(), bytes.clone());
        Ok(bytes)
    }
    fn walk_pages(
        &mut self,
        node: &Value,
        resources: Option<Value>,
        media: Option<Value>,
        depth: usize,
        visited: &mut HashSet<u32>,
    ) -> Result<()> {
        if depth > 64 {
            return Err(limit("page tree depth"));
        }
        if let Value::Ref(id) = node
            && !visited.insert(*id)
        {
            return Err(bad("page tree cycle"));
        }
        let node = self.resolve(node)?;
        let d = node.dict()?;
        if number(d, "Rotate", 0.)? != 0. {
            return Err(unsupported("rotated page tree"));
        }
        if is_name(d, "Type", "Pages") && d.contains_key("CropBox") {
            return Err(unsupported("inherited CropBox"));
        }
        let resources = d.get("Resources").cloned().or(resources);
        let media = d.get("MediaBox").cloned().or(media);
        if is_name(d, "Type", "Pages") {
            for child in required(d, "Kids")?.array()? {
                self.walk_pages(child, resources.clone(), media.clone(), depth + 1, visited)?;
            }
        } else if is_name(d, "Type", "Page") {
            if self.pages.len() >= self.limits.pages {
                return Err(limit("pages"));
            }
            for k in ["AA", "Group", "UserUnit"] {
                if d.contains_key(k) {
                    return Err(unsupported(format!("page /{k}")));
                }
            }
            if number(d, "Rotate", 0.)? != 0. {
                return Err(unsupported("rotated pages"));
            }
            let bounds = d
                .get("CropBox")
                .cloned()
                .or(media)
                .ok_or_else(|| bad("page bounds"))?;
            let a = bounds.array()?;
            if a.len() != 4 {
                return Err(bad("page rectangle"));
            }
            let b = a.iter().map(Value::num).collect::<Result<Vec<_>>>()?;
            if b[2] <= b[0] || b[3] <= b[1] || b.iter().any(|v| v.abs() > 1_000_000.) {
                return Err(bad("page bounds"));
            }
            let mut links = Vec::new();
            let mut signature_fields_present = false;
            if let Some(annots) = d.get("Annots") {
                let annots = self.resolve(annots)?;
                for annotation in annots.array()? {
                    let annotation = self.resolve(annotation)?;
                    let a = annotation.dict()?;
                    if is_name(a, "Subtype", "Widget") {
                        self.invisible_signature(a)?;
                        signature_fields_present = true;
                        continue;
                    }
                    if !is_name(a, "Subtype", "Link")
                        || a.contains_key("AA")
                        || a.contains_key("AP")
                        || a.contains_key("BS")
                        || a.contains_key("QuadPoints")
                        || !matches!(number(a, "F", 0.)?, 0. | 4.)
                    {
                        return Err(unsupported("annotation type/appearance/actions"));
                    }
                    let border = required(a, "Border")?.array()?;
                    if border.len() != 3 || border.iter().any(|v| v.num().ok() != Some(0.)) {
                        return Err(unsupported("link border"));
                    }
                    let action = self.resolve(required(a, "A")?)?;
                    let action = action.dict()?;
                    if !is_name(action, "S", "URI") {
                        return Err(unsupported("link action"));
                    }
                    let uri = String::from_utf8(required(action, "URI")?.bytes()?.to_vec())
                        .map_err(|_| bad("link URI"))?;
                    if !(uri.starts_with("https://") || uri.starts_with("http://"))
                        || uri.chars().any(|c| c.is_control())
                    {
                        return Err(unsupported("link URI scheme"));
                    }
                    let rect: [f64; 4] = required(a, "Rect")?
                        .array()?
                        .iter()
                        .map(Value::num)
                        .collect::<Result<Vec<_>>>()?
                        .try_into()
                        .map_err(|_| bad("link rectangle"))?;
                    if rect[2] <= rect[0] || rect[3] <= rect[1] {
                        return Err(unsupported("empty/reversed link rectangle"));
                    }
                    links.push(LinkAnnotation { rect, uri });
                }
            }
            self.pages.push(Page {
                resources: resources.unwrap_or(Value::Dict(Dict::new())),
                contents: d.get("Contents").cloned().unwrap_or(Value::Null),
                info: PageInfo {
                    links,
                    signature_fields_present,
                    x: b[0],
                    y: b[1],
                    width: b[2] - b[0],
                    height: b[3] - b[1],
                },
            });
        } else {
            return Err(bad("page tree type"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    #[test]
    fn parses_names_strings_and_references() {
        let mut c = Syntax::new(
            b"<< /A#20B [12 0 R (hello\\nworld) <aF1> -2.5 true null] >>",
            0,
        );
        let v = c.value(0).unwrap();
        let a = v.dict().unwrap()["A B"].array().unwrap();
        assert!(matches!(a[0], Value::Ref(12)));
        assert_eq!(a[1].bytes().unwrap(), b"hello\nworld");
        assert_eq!(a[2].bytes().unwrap(), [0xaf, 0x10]);
        assert_eq!(a[3].num().unwrap(), -2.5);
    }
    #[test]
    fn rejects_duplicate_keys_and_invalid_numbers() {
        for b in [
            b"<< /A 1 /A 2 >>".as_slice(),
            b"1e3",
            b"1.2.3",
            b"<xx>",
            b"[1 2",
            b"(unclosed",
            b"1 1 R",
        ] {
            assert!(Syntax::new(b, 0).value(0).is_err(), "{b:?}");
        }
    }
    #[test]
    fn rejects_excessive_nesting() {
        let mut b = vec![b'['; 100];
        b.extend(vec![b']'; 100]);
        assert!(Syntax::new(&b, 0).value(0).is_err());
    }
    #[test]
    fn rejects_truncated_gallery_inputs() {
        let bytes = include_bytes!("../examples/pdfs/invoice-flate.pdf");
        for cut in (0..bytes.len() - 64).step_by(97) {
            assert!(Document::open(bytes[..cut].to_vec(), Limits::default()).is_err());
        }
    }
    proptest! {#![proptest_config(ProptestConfig::with_cases(2000))]
        #[test]fn arbitrary_syntax_never_panics(bytes in proptest::collection::vec(any::<u8>(),0..4096)){let _=Syntax::new(&bytes,0).value(0);}
        #[test]fn arbitrary_pdf_headers_never_panic(mut bytes in proptest::collection::vec(any::<u8>(),0..4096)){let mut input=b"%PDF-2.0\n".to_vec();input.append(&mut bytes);let _=Document::open(input,Limits::default());}
    }
}
