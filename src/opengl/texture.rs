//! Images on the GPU

use super::*;

/// An OpenGL Texture
#[derive(Default)]
pub struct Texture {
    pub(crate) id: GLuint,
    width: Cell<i32>,
    height: Cell<i32>,
}

impl Texture {
    /// Create a new blank OpenGL texture
    pub fn new() -> Self {
        let mut id: GLuint = 0;
        unsafe { gl::GenTextures(1, &mut id) }
        check_gl();
        Self {
            id,
            width: Cell::new(0),
            height: Cell::new(0),
        }
    }

    /// Create an OpenGL texture from a PNG
    pub fn from_png(path: impl AsRef<Path>) -> Result<Self, Error> {
        let texture = Texture::new();
        let path = path.as_ref();
        texture.load(path).map_err(|source| Error::Image {
            path: path.into(),
            source,
        })?;
        Ok(texture)
    }

    /// Create an OpenGL texture from an SDL surface
    pub fn from_surface(surface: sdl2::surface::Surface) -> Self {
        // Whatever format SDL gave us, upload it as RGBA
        let surface = surface
            .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
            .expect("out of memory converting surface to RGBA");
        let texture = Texture::new();
        unsafe {
            texture.bind();

            let width = surface.width() as i32;
            let height = surface.height() as i32;
            texture.set_size(width, height);

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width,
                height,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                surface
                    .without_lock()
                    .expect("a converted surface never needs locking")
                    .as_ptr() as *const std::ffi::c_void,
            );
            check_gl();

            set_sampler(gl::CLAMP_TO_EDGE, gl::NEAREST);
        }
        texture
    }

    /// Bind this texture
    pub fn bind(&self) {
        unsafe { gl::BindTexture(gl::TEXTURE_2D, self.id) }
        check_gl();
    }

    /// Fill the texture with tightly packed RGBA bytes, `width * height * 4` of them. Updating a
    /// texture without changing its size is cheap
    pub fn set_pixels(&self, width: i32, height: i32, data: &[u8]) {
        assert_eq!(
            data.len(),
            (width * height * 4) as usize,
            "expected {width}x{height} RGBA pixels"
        );
        self.bind();

        if self.get_dimensions() == Some((width, height)) {
            // Same size, so just overwrite the pixels instead of reallocating
            unsafe {
                gl::TexSubImage2D(
                    gl::TEXTURE_2D,
                    0,
                    0,
                    0,
                    width,
                    height,
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    data.as_ptr() as *const _,
                );
            }
            check_gl();
            return;
        }

        self.set_size(width, height);
        unsafe {
            set_sampler(gl::CLAMP_TO_EDGE, gl::NEAREST);

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width,
                height,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                data.as_ptr() as *const _,
            );
            check_gl();
        }
    }

    /// Load this texture into it's OpenGL slot
    pub fn load(&self, path: &Path) -> Result<(), ImageError> {
        self.bind();

        let img = image::open(path)?.into_rgba8();
        self.set_pixels(img.width() as i32, img.height() as i32, img.as_bytes());
        unsafe {
            gl::GenerateMipmap(gl::TEXTURE_2D);
            check_gl();
        }
        Ok(())
    }

    /// Load this texture as a depth buffer
    pub fn load_depth_buffer(&self, width: i32, height: i32) {
        self.bind();
        self.set_size(width, height);

        unsafe {
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::DEPTH_COMPONENT32F as GLint,
                width,
                height,
                0,
                gl::DEPTH_COMPONENT,
                gl::FLOAT,
                std::ptr::null(),
            );
            check_gl();
            set_sampler(gl::CLAMP_TO_BORDER, gl::LINEAR);
            let border_color: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
            gl::TexParameterfv(
                gl::TEXTURE_2D,
                gl::TEXTURE_BORDER_COLOR,
                border_color.as_ptr(),
            );
            check_gl();
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_COMPARE_MODE,
                gl::COMPARE_REF_TO_TEXTURE as GLint,
            );
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_COMPARE_FUNC,
                gl::LEQUAL as GLint,
            );
            check_gl();
        }
    }

    /// Method to call after binding
    /// Activate this texture
    pub fn activate(&self, unit: u32) {
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0 + unit);
            self.bind();
        }
        check_gl();
    }

    /// The width and height of this texture, or `None` if it has no pixels yet
    pub fn get_dimensions(&self) -> Option<(i32, i32)> {
        let (width, height) = (self.width.get(), self.height.get());
        (width > 0 && height > 0).then_some((width, height))
    }

    fn set_size(&self, width: i32, height: i32) {
        self.width.set(width);
        self.height.set(height);
    }
}

/// Set how the bound texture repeats past its edges and blends when scaled
fn set_sampler(wrap: GLenum, filter: GLenum) {
    unsafe {
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, wrap as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, wrap as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, filter as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, filter as GLint);
    }
    check_gl();
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe { gl::DeleteTextures(1, &self.id) };
    }
}
