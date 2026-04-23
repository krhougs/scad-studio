use image::{DynamicImage, ImageFormat};
use std::io::Cursor;

#[test]
fn decodes_minimal_png_to_rgba() {
    let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        1,
        1,
        image::Rgba([1, 2, 3, 255]),
    ));
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png)
        .expect("encode png");
    let (w, h, rgba) =
        scad_ui::image_decode::rgba_from_image_bytes(buf.get_ref()).expect("decode png");
    assert_eq!((w, h), (1, 1));
    assert_eq!(rgba, vec![1, 2, 3, 255]);
}

#[test]
fn rejects_non_image_bytes() {
    let err =
        scad_ui::image_decode::rgba_from_image_bytes(b"not an image").expect_err("should fail");
    assert!(err.contains("解码") || err.contains("图片"), "{err}");
}
