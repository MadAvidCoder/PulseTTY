use std::thread;
use std::time::Duration;
use rand::prelude::*;


fn main() {
    let mut values: Vec<u32>;
    let mut rng = rand::rng();
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
        for line in lines {
            println!("{}", line);
        }
        thread::sleep(Duration::from_secs(5));
    }
}