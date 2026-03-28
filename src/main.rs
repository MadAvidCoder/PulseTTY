mod render;

use std::cmp::{max, min};
use std::f32::consts::PI;
use symphonia::core::errors::Error;use std::thread;
use std::time::Duration;
use symphonia::core::io::MediaSourceStreamOptions;
use std::io::{self, stdout, Write};
use std::fs::File;
use rand::prelude::*;
use crossterm::{ExecutableCommand, terminal};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::audio::SampleBuffer;
use rustfft::{FftPlanner, num_complex::Complex};
use rustfft::num_traits::Zero;

const FFT_SIZE: usize = 4096;

fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10f32.powf(mel / 2595.0) - 1.0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = stdout();

    let mut cur_values: Vec<f32> = vec![0f32; 20];
    let mut peaks: Vec<f32> = vec![0f32; 20];

    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut buffer: Vec<f32> = Vec::new();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    let mut smooth_max: f32 = 1.0;

    stdout.execute(terminal::Clear(terminal::ClearType::All))?;

    let file = Box::new(File::open("test.mp3").expect("Failed to open file."));
    let mss = MediaSourceStream::new(file, MediaSourceStreamOptions::default());

    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();
    let decoder_opts = DecoderOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .expect("unsupported format");

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .expect("no supported audio tracks");

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .expect("unsupported codec");

    let track_id = track.id;

    let mut sample_rate: f32 = 44100f32;
    let mut target_values: Vec<f32> = vec![0f32; 20];

    loop {

        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::ResetRequired) => {
                // Track list changed (e.g., chained OGG streams)
                // Recreate decoders and restart
                unimplemented!();
            }
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                // End of stream reached
                return Ok(());
            }
            Err(err) => {
                eprintln!("Error reading packet: {}", err);
                return Err(err.into());
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        while !format.metadata().is_latest() {
            format.metadata().pop();
            // Process metadata if needed
        }


        // Decode the packet
        match decoder.decode(&packet) {
            Ok(decoded) => {
                if sample_buf.is_none() {
                    sample_rate = decoded.spec().rate as f32;
                    let spec = *decoded.spec();
                    let duration = decoded.capacity() as u64;
                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                if let Some(buf) = &mut sample_buf {
                    buf.copy_interleaved_ref(decoded);

                    // Access the samples
                    let samples = buf.samples();
                    // sample_count += samples.len();
                    // println!("Total samples decoded: {}", sample_count);
                    for frame in samples.chunks(2) {
                        let mono: f32 = if frame.len() == 2 {
                            (frame[0] + frame[1]) * 0.5
                        } else {
                            frame[0]
                        };
                        buffer.push(mono);
                    }
                    if buffer.len() > FFT_SIZE * 4 {
                        buffer.drain(0..buffer.len() - FFT_SIZE * 2);
                    }
                }
            }
            Err(Error::IoError(_)) => {
                continue;
            }
            Err(Error::DecodeError(_)) => {
                continue;
            }
            Err(err) => {
                eprintln!("Unrecoverable decode error: {}", err);
                return Err(err.into());
            }
        }

        if buffer.len() >= FFT_SIZE {
            let mut chunk: Vec<Complex<f32>> = buffer[buffer.len() - FFT_SIZE..].iter().map(|&f| Complex::new(f, 0f32)).collect();

            for (i, sample) in chunk.iter_mut().enumerate() {
                let a0 = 0.35875;
                let a1 = 0.48829;
                let a2 = 0.14128;
                let a3 = 0.01168;

                let t = i as f32 / FFT_SIZE as f32;

                let w = a0
                    - a1 * (2.0 * PI * t).cos()
                    + a2 * (4.0 * PI * t).cos()
                    - a3 * (6.0 * PI * t).cos();

                sample.re *= w;
            }

            fft.process(&mut chunk);

            let magnitudes: Vec<f32> = chunk.iter()
                .take(FFT_SIZE / 2)
                .map(|c| (c.re * c.re + c.im * c.im).sqrt())
                .collect();

            let fft_bins = FFT_SIZE / 2;
            let mel_min = hz_to_mel(20.0);
            let mel_max = hz_to_mel(sample_rate / 2.0);

            for i in 0..20 {
                let mel_start = mel_min + (mel_max - mel_min) * (i as f32 / 20.0);
                let mel_end   = mel_min + (mel_max - mel_min) * ((i as f32 + 1.0) / 20.0);

                let freq_start = mel_to_hz(mel_start);
                let freq_end   = mel_to_hz(mel_end);

                let start_bin = (freq_start * FFT_SIZE as f32 / sample_rate) as usize;
                let end_bin   = (freq_end   * FFT_SIZE as f32 / sample_rate) as usize;

                let start_bin = min(start_bin, fft_bins - 1);
                let end_bin = min(max(end_bin, start_bin + 1), fft_bins);

                let slice = &magnitudes[start_bin..end_bin];

                let mut sum = 0.0;
                for m in slice {
                    sum += m * m;
                }

                let rms = (sum / slice.len() as f32).sqrt();

                let db = 20.0 * rms.max(1e-6).log10();
                let mut value = ((db + 80.0) / 80.0).clamp(0.0, 1.0);

                let noise_floor = 0.08;

                value = (value - noise_floor).max(0.0) / (1.0 - noise_floor);

                let freq = i as f32 / 19.0;
                let gate = 0.04 + 0.10 * freq;

                if value < gate {
                    value = 0.0;
                }

                target_values[i] = value * 100.0;

            }
        }

        let mut smoothed_targets = target_values.clone();

        for i in 0..20 {
            let mut sum = target_values[i];
            let mut count = 1.0;

            if i > 0 {
                sum += target_values[i - 1] * 0.5;
                count += 0.5;
            }
            if i < 19 {
                sum += target_values[i + 1] * 0.5;
                count += 0.5;
            }

            smoothed_targets[i] = sum / count;
        }

        target_values = smoothed_targets;

        for i in 0..20 {
            let freq = i as f32 / 19.0;
            let attack = 0.25 + 0.35 * freq;
            let release = 0.08 + 0.10 * freq;

            let coeff = if target_values[i] > cur_values[i] {
                attack
            } else {
                release
            };
            cur_values[i] += (target_values[i] - cur_values[i]) * coeff;

            let delta = (target_values[i] - cur_values[i]).max(0.0);
            cur_values[i] += delta * 0.15;

            if cur_values[i] > peaks[i] {
                peaks[i] = cur_values[i];
            } else {
                peaks[i] -= 0.05 + 0.25 * freq;
            }
            peaks[i] = peaks[i].max(cur_values[i]);
        }

        render::draw(&mut stdout, &cur_values, &peaks)?;

        stdout.flush()?;

        thread::sleep(Duration::from_millis(50));
    }
}