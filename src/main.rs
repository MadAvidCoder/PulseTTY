mod render;

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

const FFT_SIZE: usize = 1024;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = rand::rng();
    let mut stdout = stdout();

    let mut cur_values: Vec<f32> = vec![0f32; 20];
    let mut peaks: Vec<f32> = vec![0f32; 20];

    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut sample_count = 0;
    let mut buffer: Vec<f32> = Vec::new();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

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

    loop {
        let mut target_values: Vec<f32> = vec![0f32; 20];

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
                    buffer.extend_from_slice(samples);
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
            let mut chunk: Vec<Complex<f32>> = buffer.drain(0..FFT_SIZE).map(|f| Complex::new(f, 0f32)).collect();
            fft.process(&mut chunk);

            let processed_chunk: Vec<f32> = chunk.iter().map(|c| ((c.re*c.re)+(c.im*c.im)).sqrt()).collect::<Vec<f32>>()[0..chunk.len()/2].to_vec();

            let bins_per_bar = processed_chunk.len() / 20;
            for i in (0..20) {
                let contents = processed_chunk[i*bins_per_bar..(i+1)*bins_per_bar].to_vec();
                target_values[i] = contents.iter().sum::<f32>() / contents.len() as f32;
            }
        }

        for i in 0..20 {
            if target_values[i] > cur_values[i] {
                cur_values[i] += ((target_values[i] - cur_values[i]) as f32) * 0.23;
            } else {
                cur_values[i] += ((target_values[i] - cur_values[i]) as f32) * 0.12;
            }

            if cur_values[i] > peaks[i] {
                peaks[i] = cur_values[i];
            } else {
                peaks[i] -= 0.21
            }
            peaks[i] = peaks[i].max(cur_values[i]);
        }

        render::draw(&mut stdout, &cur_values, &peaks)?;

        stdout.flush()?;

        thread::sleep(Duration::from_millis(75));
    }
}