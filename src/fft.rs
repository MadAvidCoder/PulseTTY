use std::cmp::{max, min};
use std::f32::consts::PI;
use std::sync::{Arc, OnceLock};
use rustfft::Fft;
use rustfft::num_complex::Complex;
use crate::FFT_SIZE;
use crate::helpers::{hz_to_mel, mel_to_hz};

static WINDOW: OnceLock<Vec<f32>> = OnceLock::new();
static mut PREV: [f32; 20] = [0.0; 20];

pub fn transform(fft: &Arc<dyn Fft<f32>>, mut chunk: Vec<Complex<f32>>, sample_rate: f32, normalise_db: bool) -> Vec<f32> {
    let mut target_values: Vec<f32> = vec![0f32; 20];
    for (sample, w) in chunk.iter_mut()
        .zip(
            WINDOW.get_or_init(|| {
                (0..FFT_SIZE)
                    .map(|i| {
                        // Blackman harris. less leakage than hann
                        let a0 = 0.35875;
                        let a1 = 0.48829;
                        let a2 = 0.14128;
                        let a3 = 0.01168;

                        let t = i as f32 / FFT_SIZE as f32;

                        a0
                            - a1 * (2.0 * PI * t).cos()
                            + a2 * (4.0 * PI * t).cos()
                            - a3 * (6.0 * PI * t).cos()
                    })
                    .collect::<Vec<f32>>()
            })
            .iter()
        )
    {
        sample.re *= w;
    }

    fft.process(&mut chunk);

    let magnitudes: Vec<f32> = chunk.iter()
        .take(FFT_SIZE / 2)
        .map(|c| c.re * c.re + c.im * c.im)
        .collect();

    let mut smoothed_magnitudes = magnitudes.clone();

    for i in 1..(magnitudes.len() - 1) {
        smoothed_magnitudes[i] =
            magnitudes[i] * 0.6 +
                magnitudes[i - 1] * 0.2 +
                magnitudes[i + 1] * 0.2;
    }

    let magnitudes = smoothed_magnitudes;

    let fft_bins = FFT_SIZE / 2;
    let mel_min = hz_to_mel(20.0);
    let mel_max = hz_to_mel(sample_rate / 2.0);

    for i in 0..20 {
        // TODO: tweak bass dominance
        let mel_start = mel_min + (mel_max - mel_min) * (i as f32 / 20.0).powf(1.5);
        let mel_end   = mel_min + (mel_max - mel_min) * ((i as f32 + 1.0) / 20.0).powf(1.5);

        let freq_start = mel_to_hz(mel_start);
        let freq_end   = mel_to_hz(mel_end);

        let mut start_bin = (freq_start * FFT_SIZE as f32 / sample_rate) as usize;
        let mut end_bin= (freq_end * FFT_SIZE as f32 / sample_rate) as usize;

        start_bin = min(start_bin, fft_bins - 1);
        end_bin = min(max(end_bin, start_bin + 1), fft_bins);

        let pad = (end_bin - start_bin) / 2;

        if i < 4 {
            start_bin = start_bin.saturating_sub(2);
            end_bin = (end_bin + 2).min(fft_bins);
        }

        let start = start_bin.saturating_sub(pad);
        let end = (end_bin + pad).min(fft_bins);

        let slice = &magnitudes[start..end];

        let mut sum = 0.0;
        let mut weight_sum = 0.0;

        if slice.len() <= 1 {
            target_values[i] = slice.get(0).copied().unwrap_or(0.0);
            continue;
        }

        for (j, &mag) in slice.iter().enumerate() {
            let t: f32 = j as f32 / (slice.len() - 1) as f32;
            let weight = 1.0 - (t-0.5).abs() * 2.0;

            sum += mag * weight;
            weight_sum += weight;
        }

        let avg = if weight_sum > 0.0 {
            sum / weight_sum
        } else { 0.0 };
        let peak = slice.iter().cloned().fold(0.0, f32::max);
        let mut value = avg * 0.3 + peak * 0.7;

        value = (value - 0.02).max(0.0);
        // let noise_floor = 0.08;
        //
        // value = (value - noise_floor).max(0.0) / (1.0 - noise_floor);
        //
        // let freq = i as f32 / 19.0;
        // let gate = 0.04 + 0.10 * freq;
        //
        // if value < gate {
        //     value = 0.0;
        // }

        let rms = value.max(1e-6);
        let db = 20.0 * rms.log10();

        let mut value = ((db + 60.0) / 60.0).clamp(0.0, 1.0);

        value = value.powf(1.5);

        let freq = i as f32 / 19.0;

        let weight = 0.75 + 0.2 * freq;
        value *= weight;

        unsafe {
            let delta = value - PREV[i];

            let transient = delta.max(0.0);
            value = (value * 0.6 + transient * 1.4) / 1.3;

            PREV[i] = value;

            let stable = PREV[i] * 0.8 + value * 0.2;

            if stable < value {
                value = stable;
            }
        }

        target_values[i] = value * 100.0;
    }
    target_values
}

pub fn smooth(target_values: &Vec<f32>, cur_values: &mut Vec<f32>, peaks: &mut Vec<f32>) {
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

    for i in 0..20 {
        let freq = i as f32 / 19.0;
        let attack = if i < 5 { 0.6 } else { 0.3 };
        let release = 0.03;

        let coeff = if smoothed_targets[i] > cur_values[i] {
            attack
        } else {
            release
        };
        cur_values[i] += (smoothed_targets[i] - cur_values[i]) * coeff;

        let delta = (smoothed_targets[i] - cur_values[i]).max(0.0);
        cur_values[i] += delta * 0.33;

        if cur_values[i] > peaks[i] {
            peaks[i] = cur_values[i];
        } else {
            peaks[i] -= 0.07 + 0.08 * freq;
        }
        peaks[i] = peaks[i].max(cur_values[i]);
    }
}