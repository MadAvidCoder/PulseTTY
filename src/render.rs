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
            stdout.queue(cursor::MoveTo(0, e as u16))?;
            let line: String = row.into_iter().collect();
            stdout.queue(style::Print(line))?;
        }

        Ok(())
    }
}

fn merge(existing: char, new: char, ascii: bool) -> char {
    if existing == ' ' { return new; }
    if existing == new { return existing; }
    if ascii { '+' } else { '┼' }
}