mod render;
mod fft;
mod audio;
mod helpers;

use std::thread;
use std::time::Duration;
use std::io::{stdout, Write};
use crossterm::{ExecutableCommand, terminal};
use rustfft::num_complex::Complex;
use clap::{Parser, ValueHint, ArgAction};

// const FFT_SIZE: usize = 4096;
const FFT_SIZE: usize = 2048; //works better for lower sample rate wasAPI
const HOP_SIZE: usize = FFT_SIZE / 2;

#[derive(Parser)]
#[command(
    name = "PulseTTY",
    about = "A terminal-based music visualiser (system audio, microphone, or file)",
    version = "1.1.0",
    author = "MadAvidCoder",
    disable_help_subcommand = true,
    arg_required_else_help = false,
    after_help = "Examples:\n  pulsetty\n  pulsetty song.mp3\n  pulsetty --device 0\n  pulsetty --list-devices\n  pulsetty --mic --gain 1.5\n  pulsetty --list-mics\n  pulsetty --compact --ascii --no-colour\n  pulsetty --columns 28 --height 32"
)]
struct Args {
    #[arg(value_name = "FILE", value_hint = ValueHint::FilePath)]
    file: Option<std::path::PathBuf>,

    #[arg(short = 'c', long, default_value_t = 20, value_name = "N", help_heading = "Visual Options", help = "The number of frequency columns/bars. (Must be >= 2).")]
    columns: usize,

    #[arg(short = 'H', long, default_value_t = 16, value_name = "ROWS", help_heading = "Visual Options", help = "The height (in terminal rows) of each column. (Must be >= 2).")]
    height: usize,

    #[arg(short = 'g', long, value_name = "FLOAT", help = "Output gain multiplier.", help_heading = "FFT Options", default_value_t = 1.0)]
    gain: f32,

    #[arg(long, alias="no_color", action = ArgAction::SetTrue, help_heading = "Visual Options", help = "Disables ANSI colours.")]
    no_colour: bool,

    #[arg(long, action = ArgAction::SetTrue, help_heading = "Visual Options", help = "Uses ASCII characters, instead of Unicode blocks.")]
    ascii: bool,

    #[arg(long, action = ArgAction::SetTrue, help_heading = "Visual Options", help = "Enables compact mode (1 character per bar).")]
    compact: bool,

    #[arg(long, default_value_t = 15, value_name = "MS", help_heading = "FFT Options", help = "Frame delay (in milliseconds). Lower = Smoother, but higher CPU")]
    frame_ms: u64,

    #[arg(short='d', long, value_name = "IDX", help_heading = "Input Selection", help = "Output device to capture from (index or substring). Use --list-devices to view all options.", conflicts_with = "mic", conflicts_with = "file")]
    device: Option<String>,

    #[arg(long, help_heading = "Input Selection", help = "List all available output devices and exit.", conflicts_with = "list_mics")]
    list_devices: bool,

    #[arg(short = 'm', long, conflicts_with = "file", num_args = 0..=1, default_missing_value = "", conflicts_with="device", value_name = "IDX", help_heading = "Input Selection", help = "Use microphone input (optional selector: index or substring). Use --list-mics to view availableo")]
    mic: Option<String>,

    #[arg(long, help_heading = "Input Selection", help = "List all available input devices and exit.", conflicts_with = "list_devices")]
    list_mics: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.list_devices {
        audio::list_render_devices();
        return Ok(())
    };
    if args.list_mics {
        audio::list_capture_devices();
        return Ok(());
    }

    let columns = args.columns;
    let height = args.height;
    let file = args.file;
    let frame_ms = args.frame_ms;
    let gain = args.gain;

    if columns < 2 {
        return Err("--columns must be at least 2".into());
    }
    if height < 2 {
        return Err("--height must be at least 2".into());
    }
    if frame_ms == 0 {
        return Err("--frame-ms cannot be 0".into());
    }
    if !gain.is_finite() || gain == 0.0 {
        return Err("--gain cannot be 0".into());
    }

    let mut stdout = stdout();
    stdout.execute(terminal::Clear(terminal::ClearType::All))?;

    let mut fft_state = fft::FFTState::new(columns);

   let mut audio_state = if let Some(path) = file {
       audio::AudioState::from_file(path.to_string_lossy().as_ref())
   } else if let Some(sel) = args.mic.as_deref() {
       let sel = if sel.is_empty() { None } else { Some(sel) };
       audio::AudioState::from_microphone(sel)
   } else {
       audio::AudioState::from_system(args.device.as_deref())
   };

    let mut cur_values: Vec<f32> = vec![0f32; columns];
    let mut peaks: Vec<f32> = vec![0f32; columns];
    let mut target_values: Vec<f32> = vec![0f32; columns];

    loop {
        audio_state.next_sample().expect("Error fetching sample.");

        match &mut audio_state.source {
            audio::AudioSource::File { format: _, sample_buf: _, decoder: _, track_id: _ } => {
                if audio_state.buffer.len() >= FFT_SIZE {
                    let chunk: Vec<Complex<f32>> = audio_state.buffer[audio_state.buffer.len() - FFT_SIZE..]
                        .iter()
                        .map(|&f| Complex::new(f, 0f32))
                        .collect();
                    target_values = fft_state.transform(chunk, audio_state.sample_rate);
                }
            },

            audio::AudioSource::System {format: _, capture_client: _, readpos } => {
                if audio_state.buffer.len() >= FFT_SIZE {
                    let end = audio_state.buffer.len();
                    if *readpos + HOP_SIZE <= end {
                        *readpos = end.saturating_sub(FFT_SIZE);
                    }

                    let chunk = &audio_state.buffer[*readpos..*readpos+FFT_SIZE];
                    let mean: f32 = chunk.iter().sum::<f32>() / chunk.len() as f32;
                    let scaled: Vec<Complex<f32>> = chunk.iter().map(|&v| Complex::new(v - mean, 0.0)).collect();
                    target_values = fft_state.transform(scaled, audio_state.sample_rate);

                    *readpos += HOP_SIZE;
                }
            },

            audio::AudioSource::Microphone {format: _, capture_client: _, readpos } => {
                if audio_state.buffer.len() >= FFT_SIZE {
                    let end = audio_state.buffer.len();
                    if *readpos + HOP_SIZE <= end {
                        *readpos = end.saturating_sub(FFT_SIZE);
                    }

                    let chunk = &audio_state.buffer[*readpos..*readpos+FFT_SIZE];
                    let mean: f32 = chunk.iter().sum::<f32>() / chunk.len() as f32;
                    let scaled: Vec<Complex<f32>> = chunk.iter().map(|&v| Complex::new(v - mean, 0.0)).collect();
                    target_values = fft_state.transform(scaled, audio_state.sample_rate);

                    *readpos += HOP_SIZE;
                }
            },
        }

        for v in &mut target_values {
            *v = (*v * gain).clamp(0.0, 100.0);
        }

        fft_state.smooth(&target_values, &mut cur_values, &mut peaks);

        render::draw(&mut stdout, &cur_values, &peaks, height, args.ascii, args.compact, args.no_colour)?;

        stdout.flush()?;

        thread::sleep(Duration::from_millis(frame_ms));
    }
}