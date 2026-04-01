use std::cmp::max;
use std::io::{self, Write};
use crossterm::{QueueableCommand, cursor, style};
use crossterm::style::{SetForegroundColor, Color};
use rustfft::num_traits::Saturating;

pub enum RenderMode {
    Bars,
    Line,
    Spectrogram,
    Vu,
}

pub struct RenderConfig {
    pub height: usize,
    pub ascii: bool,
    pub compact: bool,
    pub no_colour: bool,
    pub columns: usize,
}

pub struct Renderer {
    mode: RenderMode,
    config: RenderConfig,
    history: Vec<Vec<f32>>,
}

impl Renderer {
    pub fn new(mode: RenderMode, config: RenderConfig) -> Self {
        Self {
            mode,
            config,
            history: Vec::new(),
        }
    }

    pub fn draw(&mut self, stdout: &mut impl Write,  cur_values: &[f32], peaks: &[f32]) -> io::Result<()> {
        match self.mode {
            RenderMode::Bars => self.draw_bars(stdout, cur_values, peaks),
            RenderMode::Line => self.draw_line(stdout, cur_values, peaks),
            RenderMode::Spectrogram => unimplemented!(),
            RenderMode::Vu => unimplemented!(),
        }
    }

    fn draw_bars(&mut self, stdout: &mut impl Write, cur_values: &[f32], peaks: &[f32]) -> io::Result<()> {
        let mut lines = vec![String::new(); self.config.height as usize];

        for i in 0..cur_values.len() {
            let height: u32 = (cur_values[i] / 100.0 * self.config.height as f32).round().clamp(0.0, self.config.height as f32) as u32;
            let peak_height: u32 = max((peaks[i] / 100.0 * self.config.height as f32).round().clamp(0.0, self.config.height as f32) as u32, height.saturating_add(1));

            for (e, l) in lines.iter_mut().enumerate() {
                if self.config.height - e == peak_height as usize {
                    if self.config.compact {
                        if self.config.ascii {
                            l.push_str("-")
                        } else {
                            l.push_str("▄")
                        }
                    } else {
                        if self.config.ascii {
                            l.push_str("--- ")
                        } else {
                            l.push_str("▄▄▄ ")
                        }
                    }
                } else if self.config.height - e <= height as usize {
                    if self.config.compact {
                        if self.config.ascii {
                            l.push_str("#")
                        } else {
                            l.push_str("▒");
                        }
                    } else {
                        if self.config.ascii {
                            l.push_str("### ")
                        } else {
                            l.push_str("▒▒▒ ");
                            // l.push_str("░░░ ")
                        }
                    }
                } else {
                    if self.config.compact {
                        l.push_str(" ")
                    } else {
                        l.push_str("    ");
                    }
                }
            }
        }

        let red = (self.config.height as f32 * 0.2) as usize;
        let yellow = (self.config.height as f32 * 0.45) as usize;

        for (e, line) in lines.into_iter().enumerate() {
            if !self.config.no_colour {
                stdout.queue(SetForegroundColor(match e {
                    _ if e <= red => Color::Red,
                    _ if e <= yellow => Color::Yellow,
                    _ => Color::Green,
                }))?;
            }
            stdout.queue(cursor::MoveTo(0, e as u16))?;
            stdout.queue(style::Print(line))?;
        }

        Ok(())
    }

    fn draw_line(&mut self, stdout: &mut impl Write, cur_values: &[f32], peaks: &[f32]) -> io::Result<()> {
        let column_width = if self.config.compact { 1 } else { 4 };
        let mut lines = vec![String::new(); self.config.height as usize];

        let mut y_values: Vec<usize> = Vec::new();
        for &v in cur_values {
            let height = (v / 100.0 * self.config.height.saturating_sub(1) as f32)
                .round()
                .clamp(0.0, (self.config.height.saturating_sub(1)) as f32 ) as usize;

            y_values.push(height);
        }

        for row in 0..self.config.height {
            let row_height = self.config.height - 1 - row;

            for (col, &y) in y_values.iter().enumerate() {
                let is_a_point = y == row_height;

                if self.config.compact {
                    lines[row].push(
                        if is_a_point {
                            if self.config.ascii { '*' } else { '•' }
                        } else { ' ' }
                    );
                } else {
                    lines[row].push_str(
                        if is_a_point {
                            if self.config.ascii { "*  " } else { "•  " }
                        } else { "   " }
                    );
                }
            }
        }

        let red = (self.config.height as f32 * 0.2) as usize;
        let yellow = (self.config.height as f32 * 0.45) as usize;

        for (e, line) in lines.into_iter().enumerate() {
            if !self.config.no_colour {
                stdout.queue(SetForegroundColor(match e {
                    _ if e <= red => Color::Red,
                    _ if e <= yellow => Color::Yellow,
                    _ => Color::Green,
                }))?;
            }
            stdout.queue(cursor::MoveTo(0, e as u16))?;
            stdout.queue(style::Print(line))?;
        }

        Ok(())
    }
}