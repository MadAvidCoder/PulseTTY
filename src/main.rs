mod render;
mod fft;
mod audio;
mod helpers;

use std::thread;
use std::time::Duration;
use std::io::{stdout, Write};
use crossterm::{ExecutableCommand, terminal};
use rustfft::{FftPlanner, num_complex::Complex};
use wasapi;

// const FFT_SIZE: usize = 4096;
const FFT_SIZE: usize = 2048; //works better for lower sample rate wasAPI

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = stdout();
    stdout.execute(terminal::Clear(terminal::ClearType::All))?;

    let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);

    let mut audio_state = audio::AudioState::new("test.mp3");

    // wasAPI setup
    wasapi::initialize_mta().unwrap();
    let enumerator = wasapi::DeviceEnumerator::new()?;
    let device = enumerator.get_default_device(&wasapi::Direction::Render)?;
    let mut audio_client = device.get_iaudioclient()?;
    let format = audio_client.get_mixformat()?;
    audio_client.initialize_client(
        &format,
        &wasapi::Direction::Capture,
        &wasapi::StreamMode::PollingShared {autoconvert: true, buffer_duration_hns: 100000},
    )?;
    let capture_client = audio_client.get_audiocaptureclient()?;
    audio_client.start_stream()?;


    let mut cur_values: Vec<f32> = vec![0f32; 20];
    let mut peaks: Vec<f32> = vec![0f32; 20];
    let mut target_values: Vec<f32> = vec![0f32; 20];

    let mut rolling_buffer: Vec<f32> = Vec::with_capacity(FFT_SIZE);

    let mut readpos = 0usize;
    let hop_size = FFT_SIZE / 2;

    loop {
        let mut got_wasapi_samples: bool = false;
        // audio_state.next_sample().expect("Error fetching sample.");

        while let Some(packet_size) = capture_client.get_next_packet_size()? {
            if packet_size == 0 {
                break;
            }

            let bytes_per_frame = format.get_nchannels() as usize * (format.get_validbitspersample() as usize / 8);
            let mut buf = vec![0u8; (packet_size as usize) * bytes_per_frame];

            let (frames_read, _) = capture_client.read_from_device(&mut buf)?;
            let bytes_read = frames_read as usize * bytes_per_frame;
            let raw_bytes = &buf[..bytes_read];

            let samples: Vec<f32> = match format.get_subformat().unwrap() {
                wasapi::SampleType::Float => unsafe {
                    std::slice::from_raw_parts(raw_bytes.as_ptr() as *const f32, bytes_read / 4).to_vec()
                },
                wasapi::SampleType::Int => unsafe {
                    std::slice::from_raw_parts(raw_bytes.as_ptr() as *const i16, bytes_read / 2)
                        .iter()
                        .map(|&v| v as f32 / i16::MAX as f32)
                        .collect()
                },
            };

            let mono: Vec<f32> = if format.get_nchannels() == 2 {
                (&samples).chunks(2).map(|c| (c[0] + c[1]) * 0.5).collect()
            } else {
                samples
            };

            rolling_buffer.extend(mono);
            got_wasapi_samples = true;
        }

        if !got_wasapi_samples {
            let silence_length = (format.get_samplespersec() as f32 * 0.075) as usize;
            rolling_buffer.extend(std::iter::repeat(0f32).take(silence_length));
        }

        if rolling_buffer.len() >= FFT_SIZE {
            let end = rolling_buffer.len();
            if readpos + hop_size <= end {
                readpos = end.saturating_sub(FFT_SIZE);
            }

            let chunk = &rolling_buffer[readpos..readpos+FFT_SIZE];
            let mean: f32 = chunk.iter().sum::<f32>() / chunk.len() as f32;
            let scaled: Vec<Complex<f32>> = chunk.iter().map(|&v| Complex::new(v - mean, 0.0)).collect();
            target_values = fft::transform(&fft, scaled, format.get_samplespersec() as f32, false);

            readpos += hop_size;
        }

        if rolling_buffer.len() > FFT_SIZE * 2 {
            rolling_buffer.drain(0..readpos);
            readpos = 0;
        }
        // println!("{:?}", target_values);

        // if audio_state.buffer.len() >= FFT_SIZE {
        //     let chunk: Vec<Complex<f32>> = audio_state.buffer[audio_state.buffer.len() - FFT_SIZE..]
        //         .iter()
        //         .map(|&f| Complex::new(f, 0f32))
        //         .collect();
        //     target_values = fft::transform(&fft, chunk, audio_state.sample_rate, true);
        // }

        fft::smooth(&target_values, &mut cur_values, &mut peaks);

        render::draw(&mut stdout, &cur_values, &peaks)?;

        stdout.flush()?;

        thread::sleep(Duration::from_millis(75));
    }
}