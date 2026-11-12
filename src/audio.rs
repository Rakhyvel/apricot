//! WARNING: Audio not fully tested yet, I don't think it works

use std::sync::{Arc, Mutex};

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
            // Create a thread-safe shared vector of NUM_CHUNKS Chunks. `None` means they are not playing, `Some` means they are
            let chunks: Arc<Mutex<Vec<Option<Chunk>>>> =
                Arc::new(Mutex::new((0..NUM_CHUNKS).map(|_| None).collect()));

            // Pend on commands from the receiver
            for command in receiver {
                AudioManager::clear_unused_channels(&chunks);
                match command {
                    SoundCommand::Play(file_path, volume) => {
                        let Ok(sound_file) = mixer::Chunk::from_file(&file_path) else {
                            // If the file path isn't good, just ignore it I guess?
                            println!("Couldn't open file path: {}", file_path);
                            continue;
                        };
                        // Lock the `channels` mutex to get exclusive access to the channels vector
                        let mut chunks = chunks.lock().unwrap();
                        // Find the first available (non-None) channel
                        if let Some((i, _)) = chunks
                            .iter_mut()
                            .enumerate()
                            .find(|(_, slot)| slot.is_none())
                        {
                            chunks[i] = Some(sound_file);
                            let channel = mixer::Channel(i as i32);
                            channel.set_volume(volume);
                            channel.play(chunks[i].as_ref().unwrap(), 0).unwrap();
                        } else {
                            println!("No available channel to play sound: {}", file_path);
                        }
                    }

                    SoundCommand::Quit => break,
                }
            }

            // Clean up SDL2_mixer
            sdl2::mixer::close_audio();
        });

        Ok(Self { sender })
    }

    fn clear_unused_channels(chunks: &Arc<Mutex<Vec<Option<Chunk>>>>) {
        let mut chunks = chunks.lock().unwrap();
        for i in 0..NUM_CHUNKS {
            if !mixer::Channel(i as i32).is_playing() {
                chunks[i] = None
            }
        }
    }

    /// Plays a sound.
    /// - file_path: relative to the crate directory
    /// - volume: [0, 128], anything above 128 is clipped to 128.
    pub fn play_sound(&self, file_path: String, volume: i32) {
        self.sender
            .send(SoundCommand::Play(file_path, volume))
            .unwrap();
    }
}

impl Drop for AudioManager {
    fn drop(&mut self) {
        println!("Audio manager dropped, btw!");
        self.sender.send(SoundCommand::Quit).unwrap();
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
