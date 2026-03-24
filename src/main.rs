use std::cmp::max;
use std::thread;
use std::time::Duration;
use rand::prelude::*;
use std::io::{self, Write, stdout};
use crossterm::{QueueableCommand, cursor, style, ExecutableCommand, terminal};
use crossterm::style::{SetForegroundColor, Color};

fn main() -> io::Result<()> {
    let mut rng = rand::rng();
    let mut stdout = stdout();

    let mut cur_values: Vec<f32> = vec![0f32; 20];
    let mut peaks: Vec<f32> = vec![0f32; 20];

    stdout.execute(terminal::Clear(terminal::ClearType::All))?;

    loop {
        // TODO: Switch dummy data to FFT values.
        let target_values: Vec<f32> = (0..20)
            .map(|_| rng.random_range(0..100) as f32)
            .collect();

        let mut lines = vec![String::new(); 16];

        for i in 0..20 {
            if target_values[i] > cur_values[i] {
                cur_values[i] += ((target_values[i] - cur_values[i]) as f32) * 0.23;
            } else {
                cur_values[i] += ((target_values[i] - cur_values[i]) as f32) * 0.12;
            }

            if cur_values[i] > peaks[i] {
                peaks[i] = cur_values[i];
            } else {
                peaks[i] -= 0.12
            }
            peaks[i] = peaks[i].max(cur_values[i]);

            let height: u32 = (cur_values[i] / 100.0 * 16f32).round() as u32;
            let peak_height: u32 = max((peaks[i] / 100.0 * 16f32).round() as u32, height+1);

            for (e, l) in lines.iter_mut().enumerate() {
                if 16 - e as u32 == peak_height {
                    l.push_str("▄▄▄ ")
                } else if 16 - e as u32 <= height {
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
        
        stdout.flush()?;
        thread::sleep(Duration::from_millis(75));
    }
}