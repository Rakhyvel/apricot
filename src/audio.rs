//! Loading and playing sound effects
//! TODO: Some kinda synth stuff like DOS games stuff or midi

use std::{cell::RefCell, path::Path};

use sdl2::{
    mixer::{self, Chunk, LoaderRWops, Sdl2MixerContext},
    rwops::RWops,
};

use crate::{
    error::Error,
    registry::{Handle, Registry},
};

/// How many sounds can play at once
const CHANNELS: i32 = 16;

/// A decoded sound effect. Empty when there's no audio device
pub struct Sound(Option<Chunk>);

/// Refers to a sound loaded into `Audio`
pub type SoundId = Handle<Sound>;

/// Loads and plays sound effects. Stays silent if there's no audio device
pub struct Audio {
    mixer: Option<Sdl2MixerContext>,
    sounds: RefCell<Registry<Sound>>,
}

impl Audio {
    pub(crate) fn new() -> Self {
        let mixer = open_mixer()
            .map_err(|e| eprintln!("no audio, playing silently: {e}"))
            .ok();
        Self {
            mixer,
            sounds: RefCell::new(Registry::new()),
        }
    }

    /// Load and decode a sound file, like a .wav or .ogg
    pub fn load_sound(&self, path: impl AsRef<Path>) -> Result<SoundId, Error> {
        let path = path.as_ref();
        let chunk = match self.mixer {
            Some(_) => Some(Chunk::from_file(path).map_err(|msg| Error::Sound {
                sound: path.display().to_string(),
                msg,
            })?),
            None => None,
        };
        Ok(self.sounds.borrow_mut().add(Sound(chunk), None))
    }

    /// Decode a sound from the bytes of a sound file
    pub fn load_sound_from_bytes(&self, bytes: &[u8]) -> Result<SoundId, Error> {
        let error = |msg| Error::Sound {
            sound: "from bytes".to_string(),
            msg,
        };
        let chunk = match self.mixer {
            Some(_) => Some(
                RWops::from_bytes(bytes)
                    .and_then(|rw| rw.load_wav())
                    .map_err(error)?,
            ),
            None => None,
        };
        Ok(self.sounds.borrow_mut().add(Sound(chunk), None))
    }

    /// Play a sound once at `volume`, from 0 (silent) to 1 (full). Does nothing if every channel
    /// is already busy
    pub fn play_sound(&self, sound: SoundId, volume: f32) {
        let sounds = self.sounds.borrow();
        let Some(chunk) = &sounds.get(sound).expect("invalid SoundId").0 else {
            return;
        };
        if let Ok(channel) = mixer::Channel::all().play(chunk, 0) {
            channel.set_volume((volume.clamp(0.0, 1.0) * mixer::MAX_VOLUME as f32) as i32);
        }
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        if self.mixer.is_some() {
            *self.sounds.get_mut() = Registry::new(); // sounds go before the mixer closes
            mixer::close_audio();
        }
    }
}

fn open_mixer() -> Result<Sdl2MixerContext, String> {
    let context = mixer::init(mixer::InitFlag::OGG)?;
    mixer::open_audio(44_100, mixer::AUDIO_S16LSB, mixer::DEFAULT_CHANNELS, 1_024)?;
    mixer::allocate_channels(CHANNELS);
    Ok(context)
}
