use crate::pdf::{Dict, Limits, Result, Value, bad, limit, number, unsupported};
use std::io::Read;

pub(crate) fn decode_stream(source: &[u8], dict: &Dict, limits: &Limits) -> Result<Vec<u8>> {
    let filters = match dict.get("Filter") {
        None => vec![],
        Some(Value::Name(n)) => vec![n.as_str()],
        Some(Value::Array(a)) => a.iter().map(Value::name).collect::<Result<Vec<_>>>()?,
        _ => return Err(bad("Filter")),
    };
    if filters.len() > 8 {
        return Err(limit("filter chain"));
    }
    let mut data = source.to_vec();
    for (index, filter) in filters.iter().enumerate() {
        let params = match dict.get("DecodeParms") {
            None | Some(Value::Null) => None,
            Some(Value::Dict(d)) if filters.len() == 1 => Some(d),
            Some(Value::Array(a)) if a.len() == filters.len() => match &a[index] {
                Value::Null => None,
                Value::Dict(d) => Some(d),
                _ => return Err(bad("DecodeParms entry")),
            },
            _ => return Err(bad("DecodeParms")),
        };
        data = match *filter {
            "FlateDecode" => {
                let mut decoder = flate2::read::ZlibDecoder::new(data.as_slice());
                let mut result = Vec::new();
                decoder
                    .by_ref()
                    .take(limits.stream_bytes as u64 + 1)
                    .read_to_end(&mut result)
                    .map_err(|e| bad(format!("Flate: {e}")))?;
                if result.len() > limits.stream_bytes {
                    return Err(limit("decoded Flate bytes"));
                }
                if decoder.total_in() != data.len() as u64 {
                    return Err(bad("truncated/trailing Flate"));
                }
                result
            }
            "BrotliDecode" => brotli(&data, limits.stream_bytes)?,
            "DCTDecode" | "JXLDecode" if index + 1 == filters.len() => {
                if params.is_some() {
                    return Err(unsupported("terminal image DecodeParms"));
                }
                data
            }
            _ => return Err(unsupported(format!("stream filter /{filter}"))),
        };
        if matches!(*filter, "FlateDecode" | "BrotliDecode")
            && let Some(params) = params
        {
            data = predictor(data, params, limits.stream_bytes)?;
        }
    }
    if data.len() > limits.stream_bytes {
        return Err(limit("stream bytes"));
    }
    Ok(data)
}
fn brotli(input: &[u8], maximum: usize) -> Result<Vec<u8>> {
    use brotli_decompressor::{BrotliDecompressStream, BrotliResult, BrotliState, StandardAlloc};
    // The large-window syntax is accepted, but window allocations above 24 bits
    // are outside this deliberately bounded viewing profile.
    if input.first() == Some(&0x11) && input.get(1).is_some_and(|b| b & 63 > 24) {
        return Err(limit("Brotli window > 24 bits"));
    }
    let mut state = BrotliState::new(
        StandardAlloc::default(),
        StandardAlloc::default(),
        StandardAlloc::default(),
    );
    let mut available = input.len();
    let mut offset = 0;
    let mut total = 0;
    let mut output = Vec::new();
    loop {
        let mut block = [0u8; 8192];
        let mut free = block.len();
        let mut written = 0;
        let result = BrotliDecompressStream(
            &mut available,
            &mut offset,
            input,
            &mut free,
            &mut written,
            &mut block,
            &mut total,
            &mut state,
        );
        if output.len().saturating_add(written) > maximum {
            return Err(limit("decoded Brotli bytes"));
        }
        output.extend_from_slice(&block[..written]);
        match result {
            BrotliResult::ResultSuccess => {
                if available != 0 {
                    return Err(bad("trailing Brotli data"));
                }
                return Ok(output);
            }
            BrotliResult::NeedsMoreOutput => {}
            _ => return Err(bad("truncated/invalid Brotli")),
        }
    }
}
fn predictor(data: Vec<u8>, p: &Dict, maximum: usize) -> Result<Vec<u8>> {
    let prediction = number(p, "Predictor", 1.)?;
    if prediction == 1. {
        return Ok(data);
    }
    let columns = number(p, "Columns", 1.)?;
    let colors = number(p, "Colors", 1.)?;
    if number(p, "BitsPerComponent", 8.)? != 8.
        || columns < 1.
        || columns.fract() != 0.
        || colors < 1.
        || colors > 4.
        || colors.fract() != 0.
    {
        return Err(unsupported("predictor parameters"));
    }
    let stride = (columns as usize)
        .checked_mul(colors as usize)
        .filter(|&n| n <= maximum)
        .ok_or_else(|| limit("predictor row"))?;
    let bpp = colors as usize;
    if prediction == 2. {
        if !data.len().is_multiple_of(stride) {
            return Err(bad("TIFF row length"));
        }
        let mut out = data;
        for row in out.chunks_exact_mut(stride) {
            for i in bpp..stride {
                row[i] = row[i].wrapping_add(row[i - bpp]);
            }
        }
        return Ok(out);
    }
    if !(10.0..=15.0).contains(&prediction) || prediction.fract() != 0. {
        return Err(unsupported("predictor"));
    }
    if !data.len().is_multiple_of(stride + 1) {
        return Err(bad("PNG row length"));
    }
    let mut out = Vec::with_capacity(data.len());
    for row in data.chunks_exact(stride + 1) {
        let filter = row[0];
        if filter > 4 {
            return Err(bad("PNG filter"));
        }
        let start = out.len();
        for x in 0..stride {
            let a = if x >= bpp { out[start + x - bpp] } else { 0 };
            let b = if start >= stride {
                out[start - stride + x]
            } else {
                0
            };
            let c = if start >= stride && x >= bpp {
                out[start - stride + x - bpp]
            } else {
                0
            };
            let predict = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((a as u16 + b as u16) / 2) as u8,
                4 => paeth(a, b, c),
                _ => unreachable!(),
            };
            out.push(row[x + 1].wrapping_add(predict));
        }
    }
    Ok(out)
}
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

pub(crate) fn decode_jxl(
    data: &[u8],
    width: usize,
    height: usize,
    gray: bool,
    limits: &Limits,
) -> Result<Vec<u8>> {
    use jxl::api::{
        Event, JxlBitDepth, JxlColorEncoding, JxlColorProfile, JxlDecoder, JxlDecoderOptions,
        JxlOutputBuffer, JxlPixelFormat,
    };
    let mut options = JxlDecoderOptions::default();
    options.sample_limit = Some(limits.image_pixels * 4);
    options.adjust_orientation = false;
    let mut decoder = JxlDecoder::new(options);
    let mut input = data;
    if decoder
        .process(&mut input, None, None)
        .map_err(|e| bad(e.to_string()))?
        != Event::BasicInfo
    {
        return Err(bad("JXL header"));
    }
    let info = decoder.basic_info().ok_or_else(|| bad("JXL info"))?;
    if info.size != (width, height)
        || info.animation.is_some()
        || !info.extra_channels.is_empty()
        || !matches!(info.bit_depth, JxlBitDepth::Int { bits_per_sample: 8 })
        || !matches!(info.orientation, jxl::api::Orientation::Identity)
    {
        return Err(unsupported(
            "JXL dimensions/animation/extra channels/depth/orientation",
        ));
    }
    let expected = JxlColorProfile::Simple(JxlColorEncoding::srgb(gray));
    if !decoder
        .embedded_color_profile()
        .is_some_and(|p| p.same_color_encoding(&expected))
    {
        return Err(unsupported("JXL must be sRGB/D65 or gray sRGB/D65"));
    }
    decoder
        .set_pixel_format(JxlPixelFormat::rgba8(0))
        .map_err(|e| bad(e.to_string()))?;
    let mut rgba = vec![0u8; width * height * 4];
    loop {
        let mut buffers = [JxlOutputBuffer::new(&mut rgba, height, width * 4)];
        match decoder
            .process(&mut input, Some(&mut buffers), None)
            .map_err(|e| bad(e.to_string()))?
        {
            Event::FrameHeader => {}
            Event::FrameComplete {
                has_more_frames: false,
            } => return Ok(rgba),
            _ => return Err(bad("JXL incomplete/multiple frames")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_filters() {
        for (filter, encoded) in [
            (0, vec![10, 20, 30]),
            (1, vec![10, 10, 10]),
            (2, vec![10, 20, 30]),
            (3, vec![10, 15, 20]),
            (4, vec![10, 10, 10]),
        ] {
            let p = Dict::from([
                ("Predictor".into(), Value::Num(15.)),
                ("Columns".into(), Value::Num(3.)),
            ]);
            let mut data = vec![filter];
            data.extend(encoded);
            assert_eq!(predictor(data, &p, 100).unwrap(), [10, 20, 30]);
        }
    }
    #[test]
    fn tiff() {
        let p = Dict::from([
            ("Predictor".into(), Value::Num(2.)),
            ("Columns".into(), Value::Num(3.)),
        ]);
        assert_eq!(predictor(vec![10, 10, 10], &p, 100).unwrap(), [10, 20, 30]);
    }
    #[test]
    fn malformed_predictors() {
        let p = Dict::from([
            ("Predictor".into(), Value::Num(15.)),
            ("Columns".into(), Value::Num(3.)),
        ]);
        assert!(predictor(vec![9, 1, 2, 3], &p, 100).is_err());
        assert!(predictor(vec![0, 1], &p, 100).is_err());
    }
    #[test]
    fn brotli_rejects_truncated_and_trailing() {
        assert!(brotli(&[], 100).is_err());
        assert_eq!(brotli(&[6], 100).unwrap(), []);
        assert!(brotli(&[6, 0], 100).is_err());
        assert!(brotli(&[0x11, 30], 100).is_err());
    }
}

#[cfg(test)]
mod boundaries {
    use super::*;
    #[test]
    fn accepts_reference_large_window_and_checks_all_truncations() {
        let data = include_bytes!("../tests/assets/brotli-large-22.br");
        let expected = "Skald View: bounded large-window Brotli streams.\n"
            .repeat(20)
            .into_bytes();
        assert_eq!(brotli(data, 2048).unwrap(), expected);
        assert!(brotli(data, 10).is_err());
        for end in 0..data.len() {
            assert!(brotli(&data[..end], 2048).is_err());
        }
    }
    #[test]
    fn flate_checks_truncations_trailing_bytes_and_output_limit() {
        use std::io::Write;
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&[42; 1024]).unwrap();
        let data = encoder.finish().unwrap();
        let dict = Dict::from([("Filter".into(), Value::Name("FlateDecode".into()))]);
        let limits = Limits::default();
        assert_eq!(decode_stream(&data, &dict, &limits).unwrap(), [42; 1024]);
        for end in 0..data.len() {
            assert!(
                decode_stream(&data[..end], &dict, &limits).is_err(),
                "accepted truncation at {end}/{}",
                data.len()
            );
        }
        let mut trailing = data.clone();
        trailing.push(0);
        assert!(decode_stream(&trailing, &dict, &limits).is_err());
        assert!(
            decode_stream(
                &data,
                &dict,
                &Limits {
                    stream_bytes: 100,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
