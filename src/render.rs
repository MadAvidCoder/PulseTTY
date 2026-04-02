use std::cmp::max;
use std::io::{self, Write};
use clap::ValueEnum;
use crossterm::{QueueableCommand, cursor, style};
use crossterm::style::{SetForegroundColor, Color};
use std::collections::VecDeque;

#[derive(Clone, ValueEnum, Copy, Debug)]
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
    pub spectrogram_columns: usize,
}

pub struct Renderer {
    mode: RenderMode,
    config: RenderConfig,
    history: VecDeque<Vec<f32>>,
}

impl Renderer {
    pub fn new(mode: RenderMode, config: RenderConfig) -> Self {
        let q: VecDeque<Vec<f32>> = (0..config.spectrogram_columns).map(|_| vec![0.0; config.columns]).collect();

        Self {
            mode,
            config,
            history: q,
        }
    }

    pub fn draw(&mut self, stdout: &mut impl Write,  cur_values: &[f32], peaks: &[f32]) -> io::Result<()> {
        match self.mode {
            RenderMode::Bars => self.draw_bars(stdout, cur_values, peaks),
            RenderMode::Line => self.draw_line(stdout, cur_values, peaks),
            RenderMode::Spectrogram => self.draw_spectrogram(stdout, cur_values, peaks),
            RenderMode::Vu => self.draw_vu(stdout, cur_values, peaks),
        }
    }

    pub fn next_mode(&mut self) -> RenderMode {
        self.mode = match self.mode {
            RenderMode::Vu => RenderMode::Bars,
            RenderMode::Bars => RenderMode::Line,
            RenderMode::Line => RenderMode::Spectrogram,
            RenderMode::Spectrogram => RenderMode::Vu,
        };
        self.mode
    }

    pub fn toggle_colour(&mut self) -> bool {
        self.config.no_colour = !self.config.no_colour;
        self.config.no_colour
    }

    pub fn toggle_ascii(&mut self) -> bool {
        self.config.ascii = !self.config.ascii;
        self.config.ascii
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
            stdout.queue(cursor::MoveTo(0, e as u16 + 1))?;
            stdout.queue(style::Print(line))?;
        }

        Ok(())
    }

    fn draw_line(&mut self, stdout: &mut impl Write, cur_values: &[f32], peaks: &[f32]) -> io::Result<()> {
        let column_width = if self.config.compact { 1 } else { 4 };
        let width = cur_values.len() * column_width;
        let mut grid: Vec<Vec<char>> = vec![vec![' '; width]; self.config.height];

        let mut y_values: Vec<i32> = Vec::new();
        for &v in cur_values {
            let height = ((1.0 - (v / 100.0)) * (self.config.height - 1) as f32)
                .round()
                .clamp(0.0, (self.config.height - 1) as f32 ) as i32;

            y_values.push(height);
        }

        let convert_x = |i: usize| -> i32 {
            if self.config.compact {
                i as i32
            } else {
                (i * column_width + 1) as i32
            }
        };

        let insert = |grid: &mut [Vec<char>], x: i32, row: i32, char: char, ascii: bool, force: bool| {
            if x < 0 || row < 0 {
                return;
            }
            let (x, row) = (x as usize, row as usize);
            if row >= grid.len() || x >= grid[0].len() {
                return;
            }

            let cur = grid[row][x];
            grid[row][x] = if !force { merge(cur, char, ascii) } else { char };
        };

        for i in 0..y_values.len().saturating_sub(1) {
            let x0 = convert_x(i);
            let x1 = convert_x(i+1);
            let r0 = y_values[i];
            let r1 = y_values[i+1];

            let dx = x1 - x0;

            if dx <= 0 {
                panic!(); // safeguard. this shouldnt happen
            }

            let char = if r1 < r0 {
                if self.config.ascii { '/' } else { '╱' }
            } else if r1 > r0 {
                if self.config.ascii { '\\' } else { '╲' }
            } else {
                if self.config.ascii { '_' } else { '─' }
            };

            for step in 0..=dx {
                let t = step as f32 / dx as f32;
                let row = (r0 as f32 + (r1 - r0) as f32 * t).round() as i32;

                insert(&mut grid, x0 + step, row, char, self.config.ascii, false);
            }

            let dot = if self.config.ascii { '*' } else { '•' };
            insert(&mut grid, x0, r0, dot, self.config.ascii, true);

        }

        let red = (self.config.height as f32 * 0.2) as usize;
        let yellow = (self.config.height as f32 * 0.45) as usize;

        for (e, row) in grid.into_iter().enumerate() {
            if !self.config.no_colour {
                stdout.queue(SetForegroundColor(match e {
                    _ if e <= red => Color::Red,
                    _ if e <= yellow => Color::Yellow,
                    _ => Color::Green,
                }))?;
            }
            stdout.queue(cursor::MoveTo(0, e as u16 + 1))?;
            let line: String = row.into_iter().collect();
            stdout.queue(style::Print(line))?;
        }

        Ok(())
    }

    fn draw_vu(&mut self, stdout: &mut impl Write, cur_values: &[f32], peaks: &[f32]) -> io::Result<()> {
        let level = if cur_values.is_empty() {
            0.0
        } else {
            (cur_values.iter().sum::<f32>() / cur_values.len() as f32) * 1.1
        }.clamp(0.0, 100.0);
        let level_rows = (level / 100.0 * self.config.height as f32)
            .round()
            .clamp(0.0, (self.config.height - 1) as f32) as usize;

        let peak = (peaks.iter().sum::<f32>() / peaks.len() as f32).clamp(0.0, 100.0) * 1.15;
        let peak_row = (peak / 100.0 * self.config.height as f32)
            .round()
            .clamp(0.0, (self.config.height - 1) as f32) as usize;
        let peak_row = max(peak_row, level_rows+1);

        let mut lines = vec![String::new(); self.config.height];

        for (row, line) in lines.iter_mut().enumerate() {
            let height = self.config.height - row;

            let filled = height <= level_rows;
            let is_peak = height == peak_row;

            if self.config.compact {
                line.push_str(
                    if is_peak {
                        if self.config.ascii { "  --" } else { "  ▄▄" }
                    } else if filled {
                        if self.config.ascii { "  ##" } else { "  ▒▒" }
                    } else {
                        "    "
                    }
                )
            } else {
                line.push_str(
                    if is_peak {
                        if self.config.ascii { "  ------" } else { "  ▄▄▄▄▄▄" }
                    } else if filled {
                        if self.config.ascii { "  ######" } else { "  ▒▒▒▒▒▒" }
                    } else {
                        "        "
                    }
                )
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
            stdout.queue(cursor::MoveTo(0, e as u16 + 1))?;
            stdout.queue(style::Print(line))?;
        }

        Ok(())
    }

    fn draw_spectrogram(&mut self, stdout: &mut impl Write, cur_values: &[f32], peaks: &[f32]) -> io::Result<()> {
        if self.history.len() == self.config.spectrogram_columns {
            self.history.pop_front();
        }
        let mut newest = vec![0.0f32; self.config.columns];
        for i in 0..self.config.columns {
            newest[i] = cur_values.get(i).copied().unwrap_or(0.0).clamp(0.0, 100.0);
        }
        self.history.push_back(newest);

        const ASCII_CHARS: &[char] = &[' ', '.', '-', '=', '+', '*', '#', '%', '@'];
        const UNICODE_CHARS: &[char] = &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

        let gamma: f32 = 0.65;

        let get_glyph = |v: f32, ascii: bool| -> char {
            if v < 1.5 {
                return ' ';
            }
            let t = (v / 100.0).clamp(0.0, 1.0).powf(gamma);

            if ascii {
                let index = (t * (ASCII_CHARS.len() as f32 - 1.0)).round() as usize;
                ASCII_CHARS[index]
            } else {
                let index = (t * (UNICODE_CHARS.len() as f32 - 1.0)).round() as usize;
                UNICODE_CHARS[index]
            }
        };

        let get_colour = |v: f32| -> Color {
            match v {
                _ if v >= 85.0 => Color::Red,
                _ if v >= 65.0 => Color::Yellow,
                _ if v >= 45.0 => Color::Green,
                _ if v >= 25.0 => Color::Cyan,
                _ => Color::Blue,
            }
        };

        for row in 0..self.config.height {
            stdout.queue(cursor::MoveTo(0, row as u16 + 1))?;

            let t0 = row as f32 / self.config.height as f32;
            let t1 = (row as f32 + 1.0) / self.config.height as f32;

            let hi0 = 1.0 - t1;
            let hi1 = 1.0 - t0;

            let mut b0 = (hi0 * self.config.columns as f32).floor() as isize;
            let mut b1 = (hi1 * self.config.columns as f32).ceil() as isize - 1;

            if b0 < 0 { b0 = 0; }
            if b1 < 0 { b1 = 0; }
            if b0 as usize >= self.config.columns { b0 = self.config.columns as isize - 1; }
            if b1 as usize >= self.config.columns { b1 = self.config.columns as isize - 1; }
            if b1 < b0 { b1 = b0; }

            for frame in self.history.iter() {
                let mut sum = 0.0f32;
                let mut maxv = 0.0f32;
                let mut n = 0.0f32;

                for bi in b0..=b1 {
                    let v = frame[bi as usize];
                    sum += v;
                    maxv = maxv.max(v);
                    n += 1.0;
                }

                let avg = if n > 0.0 { sum / n } else { 0.0 };
                let v = (avg * 0.65 + maxv * 0.35).clamp(0.0, 100.0);

                if !self.config.no_colour {
                    stdout.queue(SetForegroundColor(get_colour(v)))?;
                }

                ;
                stdout.queue(style::Print(get_glyph(v, self.config.ascii)))?;

                if !self.config.compact {
                    stdout.queue(style::Print(get_glyph(v, self.config.ascii)))?;
                }
            }
        }

        Ok(())
    }
}

fn merge(existing: char, new: char, ascii: bool) -> char {
    if existing == ' ' { return new; }
    if existing == new { return existing; }
    if ascii { '+' } else { '┼' }
}