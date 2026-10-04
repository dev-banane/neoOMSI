use crate::*;

#[test]
fn a_big_picture_is_halved_until_it_fits() {
    let data = omsi_texture::TextureData {
        width: 8,
        height: 4,
        format: omsi_texture::PixelFormat::Rgba8,
        levels: vec![vec![200; 8 * 4 * 4]],
        has_alpha: false,
        gpu_mips: true,
    };
    let small = fit_texture(&data, 2).unwrap();
    assert_eq!((small.width, small.height), (2, 1));
    assert_eq!(small.levels[0].len(), 2 * 4);
    assert!(fit_texture(&data, 8).is_none());
}
