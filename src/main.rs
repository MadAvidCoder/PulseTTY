mod render;

use std::thread;
use std::time::Duration;
use rand::prelude::*;
use std::io::{self, stdout, Write};
use crossterm::{ExecutableCommand, terminal};
use symphonia::core::io::MediaSourceStream;
use std::fs::File;

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

        for i in 0..20 {
            if target_values[i] > cur_values[i] {
                cur_values[i] += ((target_values[i] - cur_values[i]) as f32) * 0.23;
            } else {
                cur_values[i] += ((target_values[i] - cur_values[i]) as f32) * 0.12;
            }

            if cur_values[i] > peaks[i] {
                peaks[i] = cur_values[i];
            } else {
                peaks[i] -= 0.21
            }
            peaks[i] = peaks[i].max(cur_values[i]);
        }

        render::draw(&mut stdout, &cur_values, &peaks)?;

        stdout.flush()?;

        thread::sleep(Duration::from_millis(75));
    }
}