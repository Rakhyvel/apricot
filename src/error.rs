use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SDL: {0}")]
    Sdl(String),

    #[error("failed to compile {stage} shader:\n{log}")]
    ShaderCompile { stage: &'static str, log: String },

    #[error("failed to link shader program:\n{0}")]
    ShaderLink(String),

    #[error("couldn't load image {path}: {source}")]
    Image {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },

    #[error("couldn't load font {font}: {msg}")]
    Font { font: String, msg: String },

    #[error("couldn't parse .obj: {0}")]
    Obj(#[from] obj::ObjError),

    #[error("couldn't load sound {sound}: {msg}")]
    Sound { sound: String, msg: String },

    #[error("framebuffer incomplete (status {0:#x})")]
    Framebuffer(u32),
}

pub type Result<T> = std::result::Result<T, Error>;
