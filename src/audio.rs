//! WARNING: Audio not fully tested yet, I don't think it works

use sdl2::mixer::{self, Chunk};

use crate::error::Error;

enum SoundCommand {
    Play(String, i32),
    Quit,
}

pub struct AudioManager {
    sender: std::sync::mpsc::Sender<SoundCommand>,
}

const NUM_CHUNKS: usize = 4;

impl AudioManager {
    pub fn new() -> Result<Self, Error> {
        // Create a new channel for sending & receiving SoundCommand's
        let (sender, receiver) = std::sync::mpsc::channel();

        // Initialize SDL2_mixer library with support for OGG files
        sdl2::mixer::init(sdl2::mixer::InitFlag::OGG).map_err(Error::Sdl)?;
        sdl2::mixer::open_audio(
            44_100,
            sdl2::mixer::AUDIO_S16LSB,
            sdl2::mixer::DEFAULT_CHANNELS,
            1_024,
        )
        .map_err(Error::Sdl)?;
        sdl2::mixer::allocate_channels(NUM_CHUNKS as i32);

        // Spawn a new thread to handle audio playback
        std::thread::spawn(|| {
            // One slot per mixer channel, `None` means the channel is free
            // Only this thread touches it so no lock
            let mut chunks: [Option<Chunk>; NUM_CHUNKS] = Default::default();

            // Pend on commands from the receiver
            for command in receiver {
                clear_unused_channels(&mut chunks);
                match command {
                    SoundCommand::Play(file_path, volume) => {
                        let sound = match mixer::Chunk::from_file(&file_path) {
                            Ok(sound) => sound,
                            Err(e) => {
                                eprintln!("couldn't load sound {file_path}: {e}");
                                continue;
                            }
                        };
                        let Some(i) = chunks.iter().position(Option::is_none) else {
                            eprintln!("no free channel to play {file_path}");
                            continue;
                        };
                        let channel = mixer::Channel(i as i32);
                        channel.set_volume(volume);
                        if let Err(e) = channel.play(&sound, 0) {
                            eprintln!("couldn't play {file_path}: {e}");
                            continue;
                        }
                        // Keep the chunk alive until the channel finishes playing it
                        chunks[i] = Some(sound);
                    }

                    SoundCommand::Quit => break,
                }
            }

            // Clean up SDL2_mixer
            sdl2::mixer::close_audio();
        });

        Ok(Self { sender })
    }

    /// Plays a sound.
    /// - file_path: relative to the crate directory
    /// - volume: [0, 128], anything above 128 is clipped to 128.
    pub fn play_sound(&self, file_path: String, volume: i32) {
        // Only fails if the audio thread is gone, and a missing sound shouldn't crash the game
        let _ = self.sender.send(SoundCommand::Play(file_path, volume));
    }
}

fn clear_unused_channels(chunks: &mut [Option<Chunk>; NUM_CHUNKS]) {
    for (i, slot) in chunks.iter_mut().enumerate() {
        if !mixer::Channel(i as i32).is_playing() {
            *slot = None;
        }
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        let _ = self.sender.send(SoundCommand::Quit);
    }
}

pub struct AudioResource {
    pub audio_mgr: AudioManager,
}
#[allow(unreachable_code)]
impl Default for AudioResource {
    fn default() -> Self {
        println!("default called, whuh oh!");
        Self { audio_mgr: todo!() }
    }
}
