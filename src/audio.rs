use std::fs::File;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use crate::FFT_SIZE;

pub struct AudioState {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    pub sample_rate: f32,
    sample_buf: Option<SampleBuffer<f32>>,
    pub buffer: Vec<f32>,
}

impl AudioState {
    pub fn new(path: &str) -> Self {
        let file = Box::new(File::open(path).expect("Failed to open file."));
        let mss = MediaSourceStream::new(file, MediaSourceStreamOptions::default());

        let mut hint = Hint::new();
        hint.with_extension("mp3");
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .expect("no support for format");

        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .expect("no supported audio tracks");

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .expect("unsupported codec");

        let track_id = track.id;

        Self {
            format,
            decoder,
            track_id,
            sample_rate: 44100f32,
            sample_buf: None,
            buffer: Vec::new(),
        }
    }

    pub fn next_sample(&mut self) -> Result<(), Error> {
        let packet = match self.format.next_packet() {
            Ok(packet) => packet,
            Err(Error::ResetRequired) => {
                unimplemented!();
            }
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(());
            }
            Err(err) => {
                eprintln!("Error reading packet: {}", err);
                return Err(err.into());
            }
        };

        if packet.track_id() != self.track_id {
            return Ok(());
        }

        while !self.format.metadata().is_latest() {
            self.format.metadata().pop();
        }

        match self.decoder.decode(&packet) {
            Ok(decoded) => {
                if self.sample_buf.is_none() {
                    self.sample_rate = decoded.spec().rate as f32;
                    let spec = *decoded.spec();
                    let duration = decoded.capacity() as u64;
                    self.sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                if let Some(buf) = &mut self.sample_buf {
                    buf.copy_interleaved_ref(decoded);

                    let samples = buf.samples();
                    for frame in samples.chunks(2) {
                        // let mono = frame[0];

                        // proper mono averaging
                        let mono: f32 = if frame.len() == 2 {
                            (frame[0] + frame[1]) * 0.5
                        } else {
                            frame[0]
                        };
                        self.buffer.push(mono);
                    }
                    if self.buffer.len() > FFT_SIZE * 4 {
                        self.buffer.drain(0..self.buffer.len() - FFT_SIZE * 2);
                    }
                }
            }
            Err(Error::IoError(_)) => {
                {};
            }
            Err(Error::DecodeError(_)) => {
                {};
            }
            Err(err) => {
                eprintln!("Unrecoverable decode error: {}", err);
                return Err(err.into());
            }
        }
        Ok(())
    }
}