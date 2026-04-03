mod render;
mod fft;
mod audio;
mod helpers;

use std::cmp::max;
use std::thread;
use std::time::Duration;
use std::io::{stdout, Write};
use crossterm::{cursor, execute, style, terminal::{self, EnterAlternateScreen, LeaveAlternateScreen}, event::{self, Event, KeyCode, KeyModifiers}, QueueableCommand};
use rustfft::num_complex::Complex;
use clap::{Parser, ValueHint, ArgAction};
use crossterm::event::KeyEventKind;
use crossterm::style::{Color, SetForegroundColor};
use helpers::{fit_width, get_filename};

// const FFT_SIZE: usize = 4096;
const FFT_SIZE: usize = 2048; //works better for lower sample rate wasAPI
const HOP_SIZE: usize = FFT_SIZE / 2;

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> std::io::Result<Self> {
        terminal::enable_raw_mode()?;

        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0,0), cursor::Hide)?;
        stdout.flush()?;

        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        terminal::disable_raw_mode().ok();

        let mut stdout = stdout();
        execute!(
            stdout,
            style::ResetColor,
            style::SetAttribute(style::Attribute::Reset),
            cursor::Show,
            LeaveAlternateScreen
        ).ok();
        stdout.flush().ok();
    }
}

#[derive(Parser)]
#[command(
    name = "PulseTTY",
    about = "A terminal-based music visualiser (system audio, microphone, or file)",
    version = env!("CARGO_PKG_VERSION"),
    author = "MadAvidCoder",
    disable_help_subcommand = true,
    arg_required_else_help = false,
    after_help = "Examples:\n  pulsetty\n  pulsetty song.mp3\n  pulsetty --mode line\n  pulsetty --device 0\n  pulsetty --list-devices\n  pulsetty --mic --gain 1.5\n  pulsetty --compact --ascii --no-colour\n  pulsetty --columns 28 --height 32"
)]
struct Args {
    #[arg(value_name = "FILE", value_hint = ValueHint::FilePath)]
    file: Option<std::path::PathBuf>,

    #[arg(short = 'M', value_enum, default_value_t = render::RenderMode::Bars, long, help_heading = "Visual Options", help = "Selects the visualiser mode.")]
    mode: render::RenderMode,

    #[arg(short = 'c', long, value_name = "N", help_heading = "Visual Options", help = "Overrides the number of frequency columns/bars. If omitted, auto-fits to terminal width. (Must be >= 2)")]
    columns: Option<usize>,

    #[arg(short = 'H', long, value_name = "ROWS", help_heading = "Visual Options", help = "Overrides the height (in terminal rows) of each column. If omitted, auto-fits to terminal height. (Must be >= 2)")]
    height: Option<usize>,

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

    #[arg(short='d', long, value_name = "IDX", help_heading = "Input Selection", help = "Output device to capture from (index or substring). Use --list-devices to view available sources.", conflicts_with = "mic", conflicts_with = "file")]
    device: Option<String>,

    #[arg(long, help_heading = "Input Selection", help = "List all available output devices and exit.", conflicts_with = "list_mics")]
    list_devices: bool,

    #[arg(short = 'm', long, conflicts_with = "file", num_args = 0..=1, default_missing_value = "", conflicts_with="device", value_name = "IDX", help_heading = "Input Selection", help = "Use microphone input (optional selector: index or substring). Use --list-mics to view available mics.")]
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

    if let Some(c) = args.columns {
        if c < 2 {
            return Err("--columns must be at least 2".into());
        }
    }
    if let Some(h) = args.height {
        if h < 2 {
            return Err("--height must be at least 2".into());
        }
    }
    if args.frame_ms == 0 {
        return Err("--frame-ms cannot be 0".into());
    }
    if !args.gain.is_finite() || args.gain == 0.0 {
        return Err("--gain cannot be 0".into());
    }

    let (terminal_width, terminal_height) = terminal::size().unwrap_or((80, 24));
    let terminal_width = terminal_width as usize;
    let terminal_height = terminal_height as usize;

    let mut height = args.height.unwrap_or_else(|| max(terminal_height.saturating_sub(2), 2));
    let columns = args.columns.unwrap_or_else(|| {
        match args.mode {
            render::RenderMode::Spectrogram => (height*2).clamp(16, 128),
            _ => {
                let cell_width = if args.compact { 1 } else { 4 };
                (terminal_width / cell_width).max(2).clamp(8, 128)
            }
        }
    });

    let file = args.file;
    let frame_ms = args.frame_ms;
    let mut gain = args.gain;

    let mut mode = args.mode;
    let mut no_colour = args.no_colour;
    let mut ascii = args.ascii;

    let cell_width: u16 = if args.compact { 1 } else { 2 };
    let spectrogram_columns = ((terminal_width as usize) / (cell_width as usize)).max(2);

    let _guard = TerminalGuard::new()?;

    let mut stdout = stdout();

    let mut left_status: String;
    let mut right_status: String;
    let mut status_changed: bool = true;
    let mut size_changed: bool = true;
    let mut help: String;

    let mut renderer = render::Renderer::new(args.mode, render::RenderConfig {
        height,
        ascii: args.ascii,
        compact: args.compact,
        no_colour: args.no_colour,
        columns,
        spectrogram_columns,
    });

    let mut fft_state = fft::FFTState::new(columns);
    let mut source_label: String;

    let mut audio_state = if let Some(path) = file {
        source_label = "FILE ".to_string();
        source_label.push_str(get_filename(&path, 28).as_str());
        audio::AudioState::from_file(path.to_string_lossy().as_ref())
    } else if let Some(sel) = args.mic.as_deref() {
        let sel = if sel.is_empty() { None } else { Some(sel) };
        source_label = "MIC ".to_string();
        source_label.push_str(if let Some(s) = sel { s } else { "default" });
        audio::AudioState::from_microphone(sel)
    } else {
        source_label = "SYS ".to_string();
        source_label.push_str(if let Some(s) = &args.device { s } else { "default" });
        audio::AudioState::from_system(args.device.as_deref())
    };

    let mut cur_values: Vec<f32> = vec![0f32; columns];
    let mut peaks: Vec<f32> = vec![0f32; columns];
    let mut target_values: Vec<f32> = vec![0f32; columns];
    let mut fft_input: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); FFT_SIZE];

    let mut eof = false;
    let mut eof_drain: usize = 0;
    let mut eof_drain_total: usize = 1;

    let (mut terminal_width, mut terminal_height) = terminal::size().unwrap_or((80, 24));

    loop {
        match audio_state.next_sample() {
            Ok(true) => {},
            Ok(false) => {
                if !eof {
                    eof = true;
                    eof_drain = (1800usize / frame_ms.max(1) as usize).max(1);
                    eof_drain_total = eof_drain;
                }
            }
            Err(e) => return Err(e.into()),
        }

        if eof {
            eof_drain = eof_drain.saturating_sub(1);
            if eof_drain == 0 {
                break;
            }
        }

        if !eof {
            match &mut audio_state.source {
                audio::AudioSource::File { format: _, sample_buf: _, decoder: _, track_id: _ } => {
                    if audio_state.buffer.len() >= FFT_SIZE {
                        let chunk = &audio_state.buffer[audio_state.buffer.len() - FFT_SIZE..];
                        for (space, &v) in fft_input.iter_mut().zip(chunk.iter()) {
                            *space = Complex::new(v, 0f32);
                        }

                        fft_state.transform(&mut fft_input[..], audio_state.sample_rate, &mut target_values[..]);
                    }
                },

                audio::AudioSource::System { format: _, capture_client: _, readpos } => {
                    if audio_state.buffer.len() >= FFT_SIZE {
                        let end = audio_state.buffer.len();
                        if *readpos + HOP_SIZE <= end {
                            *readpos = end.saturating_sub(FFT_SIZE);
                        }

                        let chunk = &audio_state.buffer[*readpos..*readpos + FFT_SIZE];
                        let mean: f32 = chunk.iter().sum::<f32>() / chunk.len() as f32;
                        for (space, &v) in fft_input.iter_mut().zip(chunk.iter()) {
                            *space = Complex::new(v - mean, 0.0)
                        }
                        fft_state.transform(&mut fft_input[..], audio_state.sample_rate, &mut target_values[..]);

                        *readpos += HOP_SIZE;
                    }
                },

                audio::AudioSource::Microphone { format: _, capture_client: _, readpos } => {
                    if audio_state.buffer.len() >= FFT_SIZE {
                        let end = audio_state.buffer.len();
                        if *readpos + HOP_SIZE <= end {
                            *readpos = end.saturating_sub(FFT_SIZE);
                        }

                        let chunk = &audio_state.buffer[*readpos..*readpos + FFT_SIZE];
                        let mean: f32 = chunk.iter().sum::<f32>() / chunk.len() as f32;
                        for (space, &v) in fft_input.iter_mut().zip(chunk.iter()) {
                            *space = Complex::new(v - mean, 0.0)
                        }
                        fft_state.transform(&mut fft_input[..], audio_state.sample_rate, &mut target_values[..]);

                        *readpos += HOP_SIZE;
                    }
                },
            }
        } else {
            audio_state.buffer.extend(std::iter::repeat(0f32).take(FFT_SIZE/16));
            let chunk = &audio_state.buffer[audio_state.buffer.len() - FFT_SIZE..];
            for (space, &v) in fft_input.iter_mut().zip(chunk.iter()) {
                *space = Complex::new(v, 0f32);
            }

            fft_state.transform(&mut fft_input[..], audio_state.sample_rate, &mut target_values[..]);
        }

        let fade = if eof {
            (eof_drain as f32 / eof_drain_total as f32).clamp(0.0, 1.0).powi(2)
        } else {
            1.0
        };

        for v in &mut target_values {
            if eof {
                *v = (*v * gain * fade).clamp(0.0, 100.0);
            } else {
                *v = (*v * gain).clamp(0.0, 100.0);
            }
        }

        fft_state.smooth(&target_values[..], &mut cur_values[..], &mut peaks[..]);

        if status_changed | size_changed {
            left_status = format!(" PulseTTY  [{source_label}]  mode: {mode:?}  gain: {gain:.2}  frame: {frame_ms}ms ");
            right_status = format!(
                " cols: {columns}  height: {height}  {}{}{} ",
                if ascii { "ASCII " } else { "" },
                if args.compact { "CMP " } else { "" },
                if no_colour { "NOCOL " } else { "" },
            );
            let mut status_line = left_status;
            if status_line.len() + right_status.len() <= terminal_width as usize {
                status_line.push_str(&" ".repeat(terminal_width as usize - status_line.len() - right_status.len()));
                status_line.push_str(&right_status);
            }
            status_line = fit_width(&status_line, terminal_width as usize);

            stdout.queue(cursor::MoveTo(0, 0))?;
            stdout.queue(terminal::Clear(terminal::ClearType::CurrentLine))?;
            stdout.queue(style::SetAttribute(style::Attribute::Reverse))?;
            stdout.queue(SetForegroundColor(Color::Green))?;
            stdout.queue(style::Print(format!("{status_line}")))?;
            stdout.queue(style::SetAttribute(style::Attribute::Reset))?;
        }
        if size_changed {
            help = fit_width(" q/Esc quit | m mode | +/- gain | c colour | a ascii ", terminal_width as usize);

            stdout.queue(cursor::MoveTo(0, terminal_height.saturating_sub(1)))?;
            stdout.queue(terminal::Clear(terminal::ClearType::CurrentLine))?;
            stdout.queue(style::SetAttribute(style::Attribute::Dim))?;
            stdout.queue(style::Print(format!("{help}")))?;
            stdout.queue(style::SetAttribute(style::Attribute::Reset))?;
        }

        renderer.draw(&mut stdout, &cur_values, &peaks)?;
        stdout.flush()?;

        size_changed = false;
        status_changed = false;

        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(k) => {
                    if k.kind != KeyEventKind::Press { continue; }

                    match k.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('c') => {
                            if k.modifiers.contains(KeyModifiers::CONTROL) {
                                return Ok(());
                            } else {
                                no_colour = renderer.toggle_colour();
                                status_changed = true;
                            }
                        },
                        KeyCode::Char('m') => {
                            mode = renderer.next_mode();
                            stdout.queue(terminal::Clear(terminal::ClearType::All))?;
                            status_changed = true;
                            size_changed = true;
                        },
                        KeyCode::Char('a') => {
                            ascii = renderer.toggle_ascii();
                            status_changed = true;
                        },
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            gain = (gain * 1.1).clamp(0.05, 20.0);
                            status_changed = true;
                        },
                        KeyCode::Char('-') | KeyCode::Char('_') => {
                            gain = (gain * (1.0 / 1.1)).clamp(0.05, 20.0);
                            status_changed = true;
                        },
                        KeyCode::Char('0') => {
                            gain = 1.0;
                            status_changed = true;
                        },
                        _ => {},
                    }
                },
                Event::Resize(w, h) => {
                    terminal_width = w;
                    terminal_height = h;

                    if args.height == None {
                        height = max(terminal_height.saturating_sub(2), 2) as usize;
                    }
                    let spectrogram_columns = ((terminal_width as usize) / (cell_width as usize)).max(2);
                    renderer.resize(height, spectrogram_columns);
                    size_changed = true;
                },
                _ => {},
            }
        }

        thread::sleep(Duration::from_millis(frame_ms));
    }

    Ok(())
}