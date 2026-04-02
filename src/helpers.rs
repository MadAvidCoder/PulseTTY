pub fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

pub fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10f32.powf(mel / 2595.0) - 1.0)
}

pub fn truncate(mut s: String, width: usize) -> String {
    if s.len() > width {
        s.truncate(width);
        return s;
    }
    s
}