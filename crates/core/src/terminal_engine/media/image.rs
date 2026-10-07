use std::sync::{Arc, Mutex, Weak};

/// Immutable image pixels shared by protocol storage, snapshots and renderers.
/// PNG encoding is deferred until an export or clipboard consumer requests it.
#[derive(Debug)]
pub struct GraphicsImage {
    pub width: u32,
    pub height: u32,
    rgba: Option<Arc<[u8]>>,
    png: Option<Arc<Vec<u8>>>,
    png_export: Mutex<Weak<Vec<u8>>>,
    encoded_source_bytes: usize,
    #[cfg(test)]
    png_encodings: std::sync::atomic::AtomicUsize,
}

impl PartialEq for GraphicsImage {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width
            && self.height == other.height
            && self.rgba == other.rgba
            && (self.rgba.is_some() || self.png == other.png)
    }
}
impl Eq for GraphicsImage {}

impl GraphicsImage {
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        assert_eq!(u64::from(width) * u64::from(height) * 4, rgba.len() as u64);
        Self {
            width,
            height,
            rgba: Some(rgba.into()),
            png: None,
            png_export: Mutex::new(Weak::new()),
            encoded_source_bytes: 0,
            #[cfg(test)]
            png_encodings: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn from_png(width: u32, height: u32, png: Vec<u8>) -> Self {
        let encoded_source_bytes = png.capacity();
        Self {
            width,
            height,
            rgba: None,
            encoded_source_bytes,
            png: Some(Arc::new(png)),
            png_export: Mutex::new(Weak::new()),
            #[cfg(test)]
            png_encodings: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn rgba(&self) -> Option<&[u8]> {
        self.rgba.as_deref()
    }

    /// Decoded size is charged even for compressed PNGs, bounding texture memory.
    /// PNG exports of RGBA pixels belong to their callers and are not retained
    /// by this image after the last export owner releases them.
    pub fn byte_len(&self) -> usize {
        (self.width as usize)
            .saturating_mul(self.height as usize)
            .saturating_mul(4)
            .saturating_add(self.encoded_source_bytes)
    }

    /// Return an owned PNG export, sharing encoding work with overlapping exports.
    /// Retain this handle while exporting several placements of the same image.
    pub fn png(&self) -> Arc<Vec<u8>> {
        if let Some(png) = &self.png {
            return png.clone();
        }
        let mut cached = self
            .png_export
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(png) = cached.upgrade() {
            return png;
        }
        let png = Arc::new(super::encode_png(
            self.width,
            self.height,
            4,
            self.rgba().unwrap_or_default(),
        ));
        // Weak<Vec<u8>> retains only the Vec header after the last strong
        // owner drops; Weak<[u8]> would retain the inline pixel allocation.
        *cached = Arc::downgrade(&png);
        #[cfg(test)]
        self.png_encodings
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        png
    }

    #[cfg(test)]
    pub(crate) fn png_encoding_count(&self) -> usize {
        self.png_encodings
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
