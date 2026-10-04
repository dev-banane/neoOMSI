pub fn fit_texture(
    data: &omsi_texture::TextureData,
    max: u32,
) -> Option<omsi_texture::TextureData> {
    use omsi_texture::PixelFormat;
    let (w, h) = (data.width.max(1), data.height.max(1));
    if w <= max && h <= max {
        return None;
    }
    let mut k = 0u32;
    while (w >> k).max(1) > max || (h >> k).max(1) > max {
        k += 1;
    }
    if (k as usize) < data.levels.len() {
        return Some(omsi_texture::TextureData {
            width: (w >> k).max(1),
            height: (h >> k).max(1),
            levels: data.levels[k as usize..].to_vec(),
            ..data.clone()
        });
    }
    let mut rgba = match (data.format, data.levels.first()) {
        (PixelFormat::Rgba8, Some(l)) => l.clone(),
        (f, Some(l)) => omsi_texture::bc::decode(
            l,
            w,
            h,
            match f {
                PixelFormat::Bc1 => omsi_texture::bc::Bc::Bc1 { punch: true },
                PixelFormat::Bc2 => omsi_texture::bc::Bc::Bc2,
                _ => omsi_texture::bc::Bc::Bc3,
            },
        ),
        _ => return None,
    };
    let (mut cw, mut ch) = (w, h);
    for _ in 0..k {
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let at = |xx: u32, yy: u32| {
                        rgba[((yy.min(ch - 1) * cw + xx.min(cw - 1)) * 4 + c) as usize] as u32
                    };
                    let v = at(2 * x, 2 * y)
                        + at(2 * x + 1, 2 * y)
                        + at(2 * x, 2 * y + 1)
                        + at(2 * x + 1, 2 * y + 1);
                    next[((y * nw + x) * 4 + c) as usize] = (v / 4) as u8;
                }
            }
        }
        rgba = next;
        (cw, ch) = (nw, nh);
    }
    Some(omsi_texture::TextureData {
        width: cw,
        height: ch,
        format: PixelFormat::Rgba8,
        levels: vec![rgba],
        has_alpha: data.has_alpha,
        gpu_mips: true,
    })
}
