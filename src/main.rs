mod render;
mod fft;
mod audio;
mod helpers;

use std::thread;
use std::time::Duration;
use std::io::{stdout, Write};
use crossterm::{ExecutableCommand, terminal};
use rustfft::{FftPlanner, num_complex::Complex};

const FFT_SIZE: usize = 4096;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = stdout();
    stdout.execute(terminal::Clear(terminal::ClearType::All))?;

    let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);

    let mut audio_state = audio::AudioState::new("test.mp3");

    let mut cur_values: Vec<f32> = vec![0f32; 20];
    let mut peaks: Vec<f32> = vec![0f32; 20];
    let mut target_values: Vec<f32> = vec![0f32; 20];

    loop {
        audio_state.next_sample().expect("Error fetching sample.");

        if audio_state.buffer.len() >= FFT_SIZE {
            let chunk: Vec<Complex<f32>> = audio_state.buffer[audio_state.buffer.len() - FFT_SIZE..]
                .iter()
                .map(|&f| Complex::new(f, 0f32))
                .collect();

            target_values = fft::transform(&fft, chunk, audio_state.sample_rate);
        }

        fft::smooth(&target_values, &mut cur_values, &mut peaks);

        render::draw(&mut stdout, &cur_values, &peaks)?;

        stdout.flush()?;

        thread::sleep(Duration::from_millis(75));
    }
}