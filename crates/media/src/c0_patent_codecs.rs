// Upstream's unchanged codec adapters, enabled only with patent-codecs.
// Included after plane_pic! so the HEVC adapter uses the existing conversion implementation.

#[cfg(feature = "patent-codecs")]
struct H264 {
    avcc: Vec<u8>,
    dec: deckcraft_h264::Decoder,
}

#[cfg(feature = "patent-codecs")]
impl H264 {
    fn new(avcc: Vec<u8>) -> Result<H264> {
        let dec = deckcraft_h264::Decoder::from_avcc(&avcc).map_err(dec_err)?;
        Ok(H264 { avcc, dec })
    }
    fn convert(p: deckcraft_h264::Picture) -> Pic {
        let (w, h) = (p.width as usize, p.height as usize);
        let planes = Planes {
            y: &p.y,
            u: &p.u,
            v: &p.v,
            y_stride: p.y_stride,
            c_stride: p.uv_stride,
            width: w,
            height: h,
            sx: u32::from(p.chroma_width < p.width),
            sy: u32::from(p.chroma_height < p.height),
            bits: 8,
            mono: p.chroma_width == 0,
        };
        let rgba = yuv::to_rgba(&planes, Matrix::from_code(p.color.matrix, p.height), p.color.full_range);
        Pic { pts: p.pts, width: p.width, height: p.height, rgba }
    }
}

#[cfg(feature = "patent-codecs")]
impl Codec for H264 {
    fn decode(&mut self, sample: &[u8], pts: i64) -> Result<Vec<Pic>> {
        Ok(self.dec.decode(sample, pts).map_err(dec_err)?.into_iter().map(H264::convert).collect())
    }
    fn flush(&mut self) -> Vec<Pic> {
        self.dec.flush().into_iter().map(H264::convert).collect()
    }
    fn reset(&mut self) {
        if let Ok(d) = deckcraft_h264::Decoder::from_avcc(&self.avcc) {
            self.dec = d;
        }
    }
}

#[cfg(feature = "patent-codecs")]
struct Hevc {
    hvcc: Vec<u8>,
    dec: deckcraft_hevc::Decoder,
}

#[cfg(feature = "patent-codecs")]
impl Hevc {
    fn new(hvcc: Vec<u8>) -> Result<Hevc> {
        let dec = deckcraft_hevc::Decoder::from_hvcc(&hvcc).map_err(dec_err)?;
        Ok(Hevc { hvcc, dec })
    }
    fn convert(p: deckcraft_hevc::Picture) -> Pic {
        let sx = u32::from(p.chroma_width < p.width);
        let sy = u32::from(p.chroma_height < p.height);
        let m = Matrix::from_code(p.color.matrix, p.height);
        plane_pic!(&p, deckcraft_hevc::Plane, sx, sy, m, p.color.full_range, p.chroma_width == 0)
    }
}

#[cfg(feature = "patent-codecs")]
impl Codec for Hevc {
    fn decode(&mut self, sample: &[u8], pts: i64) -> Result<Vec<Pic>> {
        Ok(self.dec.decode(sample, pts).map_err(dec_err)?.into_iter().map(Hevc::convert).collect())
    }
    fn flush(&mut self) -> Vec<Pic> {
        self.dec.flush().into_iter().map(Hevc::convert).collect()
    }
    fn reset(&mut self) {
        if let Ok(d) = deckcraft_hevc::Decoder::from_hvcc(&self.hvcc) {
            self.dec = d;
        }
    }
}
