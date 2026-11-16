use std::path::PathBuf;

/// Which kind of shader
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
}

impl std::fmt::Display for ShaderStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ShaderStage::Vertex => "vertex",
            ShaderStage::Fragment => "fragment",
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SDL: {0}")]
    Sdl(String),

    #[error("failed to compile {stage} shader:\n{log}")]
    ShaderCompile { stage: ShaderStage, log: String },

    #[error("the {stage} shader's source has a NUL byte in it")]
    ShaderSourceNul { stage: ShaderStage },

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

    #[error("couldn't load sound {}: {msg}", path.display())]
    Sound { path: PathBuf, msg: String },

    #[error("couldn't load sound from bytes: {0}")]
    SoundBytes(String),

    #[error("framebuffer incomplete (status {0:#x})")]
    Framebuffer(u32),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shader_error_names_its_stage() {
        let error = Error::ShaderCompile {
            stage: ShaderStage::Fragment,
            log: "bad".to_string(),
        };
        assert_eq!(error.to_string(), "failed to compile fragment shader:\nbad");
        assert_eq!(ShaderStage::Vertex.to_string(), "vertex");
    }

    #[test]
    fn a_sound_error_names_the_file_only_when_there_is_one() {
        let from_file = Error::Sound {
            path: "a.wav".into(),
            msg: "bad".to_string(),
        };
        assert_eq!(from_file.to_string(), "couldn't load sound a.wav: bad");
        assert_eq!(
            Error::SoundBytes("bad".to_string()).to_string(),
            "couldn't load sound from bytes: bad"
        );
    }
}
