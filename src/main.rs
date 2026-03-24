use std::thread;
use std::time::Duration;
use rand::prelude::*;
use std::io::{self, Write, stdout};
use crossterm::{QueueableCommand, cursor, style, ExecutableCommand, terminal};
use crossterm::style::Stylize;

fn main() -> io::Result<()> {
    let mut values: Vec<u32>;
    let mut rng = rand::rng();
    let mut stdout = stdout();

    stdout.execute(terminal::Clear(terminal::ClearType::All))?;

    loop {
        // TODO: Switch dummy data to FFT values.
        values = (0..20)
            .map(|_| rng.random_range(0..100))
            .collect();

        let mut lines = vec![String::new(); 16];

        for v in &values {
            let height = (v * 16) / 100;
            for (e, l) in lines.iter_mut().enumerate() {
                if height >= 16 - e as u32 {
                    l.push_str("▒▒▒ ");
                } else {
                    l.push_str("    ");
                }
            }
        }

        for (e, line) in lines.into_iter().enumerate() {
            stdout.queue(cursor::MoveTo(0, e as u16))?;
            stdout.queue(style::PrintStyledContent(line.magenta()))?;
        }
        
        stdout.flush()?;
        thread::sleep(Duration::from_secs(1));
    }
}