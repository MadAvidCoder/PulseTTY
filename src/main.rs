mod render;
mod fft;
mod audio;
mod helpers;

use std::thread;
use std::time::Duration;
use std::io::{stdout, Write};
use crossterm::{ExecutableCommand, terminal};
use rustfft::{FftPlanner, num_complex::Complex};

// const FFT_SIZE: usize = 4096;
const FFT_SIZE: usize = 2048; //works better for lower sample rate wasAPI
const HOP_SIZE: usize = FFT_SIZE / 2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = stdout();
    stdout.execute(terminal::Clear(terminal::ClearType::All))?;

    let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);

    // let mut audio_state = audio::AudioState::from_file("test.wav");
    let mut audio_state = audio::AudioState::from_system();

    let mut cur_values: Vec<f32> = vec![0f32; 20];
    let mut peaks: Vec<f32> = vec![0f32; 20];
    let mut target_values: Vec<f32> = vec![0f32; 20];

    loop {
        audio_state.next_sample().expect("Error fetching sample.");

        match audio_state.source {
            audio::AudioSource::File { format: _, sample_buf: _, decoder: _, track_id: _ } => {
                if audio_state.buffer.len() >= FFT_SIZE {
                    let chunk: Vec<Complex<f32>> = audio_state.buffer[audio_state.buffer.len() - FFT_SIZE..]
                        .iter()
                        .map(|&f| Complex::new(f, 0f32))
                        .collect();
                    target_values = fft::transform(&fft, chunk, audio_state.sample_rate);
                }
            },

            audio::AudioSource::System {format: _, capture_client: _, mut readpos } => {
                if audio_state.buffer.len() >= FFT_SIZE {
                    let end = audio_state.buffer.len();
                    if readpos + HOP_SIZE <= end {
                        readpos = end.saturating_sub(FFT_SIZE);
                    }

                    let chunk = &audio_state.buffer[readpos..readpos+FFT_SIZE];
                    let mean: f32 = chunk.iter().sum::<f32>() / chunk.len() as f32;
                    let scaled: Vec<Complex<f32>> = chunk.iter().map(|&v| Complex::new(v - mean, 0.0)).collect();
                    target_values = fft::transform(&fft, scaled, audio_state.sample_rate);

                    readpos += HOP_SIZE;
                }
            },

            audio::AudioSource::Microphone {} => {
                unimplemented!()
            }
        }

        fft::smooth(&target_values, &mut cur_values, &mut peaks);

        render::draw(&mut stdout, &cur_values, &peaks)?;

        stdout.flush()?;

        thread::sleep(Duration::from_millis(15));
    }
}