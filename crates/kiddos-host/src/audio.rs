//! A square-wave beeper on the default output device. The cpal stream is
//! not `Send`, so it lives on its own thread; the beeper only shares the
//! current tone with it. No audio device means a silent beeper.

use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct Tone {
    freq: f32,
    until: Instant,
}

pub struct Beeper {
    tone: Arc<Mutex<Option<Tone>>>,
}

impl Default for Beeper {
    fn default() -> Self {
        Beeper::new()
    }
}

impl Beeper {
    pub fn new() -> Beeper {
        let tone: Arc<Mutex<Option<Tone>>> = Arc::new(Mutex::new(None));
        let t = tone.clone();
        let _ = std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || match build_stream(t) {
                Some(_stream) => loop {
                    std::thread::park();
                },
                None => log::warn!("no audio output; beep will be silent"),
            });
        Beeper { tone }
    }

    pub fn beep(&self, freq: u32, ms: u32) {
        *self.tone.lock() = Some(Tone {
            freq: freq as f32,
            until: Instant::now() + Duration::from_millis(ms as u64),
        });
    }
}

fn build_stream(tone: Arc<Mutex<Option<Tone>>>) -> Option<cpal::Stream> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let config = device.default_output_config().ok()?;
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    let mut phase = 0.0f32;
    let stream = device
        .build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| {
                let t = *tone.lock();
                let now = Instant::now();
                for frame in data.chunks_mut(channels) {
                    let v = match t {
                        Some(tone) if now < tone.until => {
                            phase = (phase + tone.freq / sample_rate) % 1.0;
                            if phase < 0.5 {
                                0.18
                            } else {
                                -0.18
                            }
                        }
                        _ => 0.0,
                    };
                    for s in frame {
                        *s = v;
                    }
                }
            },
            |e| log::warn!("audio error: {e}"),
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some(stream)
}
