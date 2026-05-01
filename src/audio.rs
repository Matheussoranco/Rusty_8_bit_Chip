use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, SampleFormat};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

pub struct Audio {
    _stream: Option<Stream>,
    pub beeping: Arc<AtomicBool>,
}

impl Audio {
    pub fn new() -> Self {
        let beeping = Arc::new(AtomicBool::new(false));
        let stream = Self::build_stream(Arc::clone(&beeping));

        Audio {
            _stream: stream,
            beeping,
        }
    }

    fn build_stream(beeping: Arc<AtomicBool>) -> Option<Stream> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let config = device.default_output_config().ok()?;

        let sample_rate = config.sample_rate().0 as f32;
        let channels = config.channels() as usize;

        let mut phase: f32 = 0.0;
        let freq = 440.0_f32;

        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                let b = Arc::clone(&beeping);
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let active = b.load(Ordering::Relaxed);
                        for frame in data.chunks_mut(channels) {
                            let sample = if active {
                                (phase * 2.0 * std::f32::consts::PI).sin() * 0.15
                            } else {
                                0.0
                            };
                            phase = (phase + freq / sample_rate) % 1.0;
                            for s in frame.iter_mut() {
                                *s = sample;
                            }
                        }
                    },
                    |err| eprintln!("Audio stream error: {}", err),
                    None,
                ).ok()
            }
            SampleFormat::I16 => {
                let b = Arc::clone(&beeping);
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        let active = b.load(Ordering::Relaxed);
                        for frame in data.chunks_mut(channels) {
                            let sample = if active {
                                ((phase * 2.0 * std::f32::consts::PI).sin() * 0.15 * i16::MAX as f32) as i16
                            } else {
                                0
                            };
                            phase = (phase + freq / sample_rate) % 1.0;
                            for s in frame.iter_mut() {
                                *s = sample;
                            }
                        }
                    },
                    |err| eprintln!("Audio stream error: {}", err),
                    None,
                ).ok()
            }
            SampleFormat::U16 => {
                let b = Arc::clone(&beeping);
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        let active = b.load(Ordering::Relaxed);
                        for frame in data.chunks_mut(channels) {
                            let sample = if active {
                                (((phase * 2.0 * std::f32::consts::PI).sin() * 0.15 + 1.0) * 0.5 * u16::MAX as f32) as u16
                            } else {
                                u16::MAX / 2
                            };
                            phase = (phase + freq / sample_rate) % 1.0;
                            for s in frame.iter_mut() {
                                *s = sample;
                            }
                        }
                    },
                    |err| eprintln!("Audio stream error: {}", err),
                    None,
                ).ok()
            }
            _ => None,
        };

        if let Some(ref s) = stream {
            let _ = s.play();
        }

        stream
    }

    pub fn set_beeping(&self, active: bool) {
        self.beeping.store(active, Ordering::Relaxed);
    }
}
