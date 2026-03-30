use std::cmp::max;
use std::io::{self, Write};
use crossterm::{QueueableCommand, cursor, style};
use crossterm::style::{SetForegroundColor, Color};
use rustfft::num_traits::Saturating;

pub fn draw(stdout: &mut impl Write, cur_values: &[f32], peaks: &[f32]) -> io::Result<()> {
    let mut lines = vec![String::new(); 16];

    for i in 0..cur_values.len() {
        let height: u32 = (cur_values[i] / 100.0 * 16f32).round().clamp(0.0, 16.0) as u32;
        let peak_height: u32 = max((peaks[i] / 100.0 * 16f32).round().clamp(0.0, 16.0) as u32, height.saturating_add(1));

        for (e, l) in lines.iter_mut().enumerate() {
            if 16 - (e as u32) == peak_height {
                l.push_str("▄▄▄ ")
            } else if 16 - (e as u32) <= height {
                l.push_str("▒▒▒ ");
                // l.push_str("░░░ ")
            } else {
                l.push_str("    ");
            }
        }
    }

    for (e, line) in lines.into_iter().enumerate() {
        stdout.queue(SetForegroundColor(match e {
            0..=2 => Color::Red,
            3..=6 => Color::Yellow,
            7..=15 => Color::Green,
            _ => Color::White,
        }))?;
        stdout.queue(cursor::MoveTo(0, e as u16))?;
        stdout.queue(style::Print(line))?;
    }

    Ok(())
}