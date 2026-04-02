pub fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

pub fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10f32.powf(mel / 2595.0) - 1.0)
}

pub fn fit_width(s: &str, width: usize) -> String {
    let mut modified = s.to_string();

    if modified.len() > width {
        modified.truncate(width);
    } else if modified.len() < width {
        let padding_len = width - modified.len();
        modified.insert_str(0, " ".repeat(padding_len / 2).as_str());
        modified.push_str(" ".repeat(padding_len - (padding_len / 2)).as_str());
    }
    modified
}

pub fn get_filename(path: &std::path::Path, max: usize) -> String {
    let extension = path.extension().and_then(|os_str| os_str.to_str());
    let s: String = path
        .file_name()
        .and_then(|os_str| os_str.to_str())
        .unwrap_or_else(|| path.to_str().unwrap_or("Unknown"))
        .to_string();

    if s.len() <= max {
        s
    } else if max <= 3 {
        "...".to_string()
    } else if let Some(e) = extension {
        format!(
            "{}...{}",
            (&s)
                .chars()
                .take(
                    max.saturating_sub(3 + e.len())
                ).collect::<String>(),
            e,
        )
    } else {
        format!("...{}", &s[s.len()-(max-3)..])
    }
}