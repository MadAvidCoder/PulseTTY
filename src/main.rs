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

    stdout.execute(terminal::Clear(terminal::ClearType::All))?;

    loop {
        // TODO: Switch dummy data to FFT values.
        let target_values: Vec<f32> = (0..20)
            .map(|_| rng.random_range(0..100) as f32)
            .collect();

        let mut lines = vec![String::new(); 16];

        for i in 0..20 {
            cur_values[i] += ((target_values[i] - cur_values[i]) as f32) * 0.2;
            let height: u32 = (((cur_values[i] * 16.0) + 99.0) / 100.0) as u32;
            for (e, l) in lines.iter_mut().enumerate() {
                if height >= 16 - e as u32 {
                    l.push_str("▒▒▒ ");
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
        thread::sleep(Duration::from_millis(50));
    }
}