mod render;
mod fft;
mod audio;

use std::thread;
use std::time::Duration;
use std::io::{stdout, Write};
use crossterm::{ExecutableCommand, terminal};
use rustfft::{FftPlanner, num_complex::Complex};
use crate::audio::AudioState;
use crate::fft::process;

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

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    stdout.execute(terminal::Clear(terminal::ClearType::All))?;

    let mut target_values: Vec<f32> = vec![0f32; 20];

    let mut audio_state = AudioState::new("test.mp3");

    loop {
        audio_state.next_sample().expect("");

        if audio_state.buffer.len() >= FFT_SIZE {
            let chunk: Vec<Complex<f32>> = audio_state.buffer[audio_state.buffer.len() - FFT_SIZE..].iter().map(|&f| Complex::new(f, 0f32)).collect();
            target_values = process(&fft, chunk, audio_state.sample_rate);
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
            let release = 0.11 + 0.09 * freq;

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
                peaks[i] -= 0.072 + 0.25 * freq;
            }
            peaks[i] = peaks[i].max(cur_values[i]);
        }

        render::draw(&mut stdout, &cur_values, &peaks)?;

        stdout.flush()?;

        thread::sleep(Duration::from_millis(75));
    }
}