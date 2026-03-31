use std::cmp::max;
use std::io::{self, Write};
use crossterm::{QueueableCommand, cursor, style};
use crossterm::style::{SetForegroundColor, Color};

pub fn draw(stdout: &mut impl Write, cur_values: &[f32], peaks: &[f32], max_height: usize, ascii: bool, compact: bool, no_color: bool) -> io::Result<()> {
    let mut lines = vec![String::new(); max_height as usize];

    for i in 0..cur_values.len() {
        let height: u32 = (cur_values[i] / 100.0 * max_height as f32).round().clamp(0.0, max_height as f32) as u32;
        let peak_height: u32 = max((peaks[i] / 100.0 * max_height as f32).round().clamp(0.0, max_height as f32) as u32, height.saturating_add(1));

        for (e, l) in lines.iter_mut().enumerate() {
            if max_height - e == peak_height as usize {
                if compact {
                    if ascii {
                        l.push_str("-")
                    } else {
                        l.push_str("▄")
                    }
                } else {
                    if ascii {
                        l.push_str("--- ")
                    } else {
                        l.push_str("▄▄▄ ")
                    }
                }
            } else if max_height - e <= height as usize {
                if compact {
                    if ascii {
                        l.push_str("#")
                    } else {
                        l.push_str("▒");
                    }
                } else {
                    if ascii {
                        l.push_str("### ")
                    } else {
                        l.push_str("▒▒▒ ");
                        // l.push_str("░░░ ")
                    }
                }
            } else {
                if compact {
                    l.push_str(" ")
                } else {
                    l.push_str("    ");
                }
            }
        }
    }

    let red = (max_height as f32 * 0.2) as usize;
    let yellow = (max_height as f32 * 0.45) as usize;

    for (e, line) in lines.into_iter().enumerate() {
        if !no_color {
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