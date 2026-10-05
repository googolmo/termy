//! Renderer-neutral image ownership, animation, layout and shared-memory IO.
//! These utilities are independent of both the grid and the terminal parser.

mod animation;
mod geometry;
mod image;
mod shared_memory;

pub use animation::{
    GraphicsAnimation, GraphicsAnimationControl, GraphicsComposition, GraphicsFrameUpdate,
};
pub use geometry::{
    GraphicsDisplayLayout, GraphicsRowSpan, graphics_display_layout, graphics_display_size,
};
pub use image::GraphicsImage;
pub use shared_memory::read_graphics_shared_memory;

fn encode_png(width: u32, height: u32, channels: u8, pixels: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(if channels == 4 {
            png::ColorType::Rgba
        } else {
            png::ColorType::Rgb
        });
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("validated image dimensions");
        writer
            .write_image_data(pixels)
            .expect("validated image pixels");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lazy_png_export_round_trips_pixels_and_reuses_encoded_allocation() {
        let image = GraphicsImage::from_rgba(1, 1, vec![12, 34, 56, 255]);
        let first = image.png();
        assert!(std::sync::Arc::ptr_eq(first, image.png()));
        let mut decoder = png::Decoder::new(std::io::Cursor::new(first.as_ref()))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
        decoder.next_frame(&mut pixels).unwrap();
        assert_eq!(pixels, vec![12, 34, 56, 255]);
    }
}
