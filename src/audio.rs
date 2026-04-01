use std::fs::File;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use wasapi::{self, AudioCaptureClient, WaveFormat};
use crate::FFT_SIZE;

pub enum AudioSource {
    System {
        format: WaveFormat,
        capture_client: AudioCaptureClient,
        readpos: usize,
    },
    Microphone {
        format: WaveFormat,
        capture_client: AudioCaptureClient,
        readpos: usize,
    },
    File {
        format: Box<dyn symphonia::core::formats::FormatReader>,
        sample_buf: Option<SampleBuffer<f32>>,
        decoder: Box<dyn symphonia::core::codecs::Decoder>,
        track_id: u32,
    },
}

pub struct AudioState {
    pub source: AudioSource,
    pub sample_rate: f32,
    pub buffer: Vec<f32>,
}

impl AudioState {
    pub fn from_file(path: &str) -> Self {
        let file = Box::new(File::open(path).expect("Failed to open file."));
        let mss = MediaSourceStream::new(file, MediaSourceStreamOptions::default());

        let mut hint = Hint::new();
        hint.with_extension("mp3");
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .expect("no support for format");

        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .expect("no supported audio tracks");

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .expect("unsupported codec");

        let track_id = track.id;

        Self {
            source: AudioSource::File { format, sample_buf: None, decoder, track_id },
            sample_rate: 44100f32,
            buffer: Vec::new(),
        }
    }

    pub fn from_system(device_selector: Option<&str>) -> Self {
        wasapi::initialize_mta().unwrap();
        let enumerator = wasapi::DeviceEnumerator::new().unwrap();

        let device = match device_selector {
            Some(sel) => pick_render_device(Some(sel)),
            None => enumerator.get_default_device(&wasapi::Direction::Render).unwrap(),
        };

        let mut audio_client = device.get_iaudioclient().unwrap();
        let format = audio_client.get_mixformat().unwrap();
        audio_client.initialize_client(
            &format,
            &wasapi::Direction::Capture,
            &wasapi::StreamMode::PollingShared {autoconvert: true, buffer_duration_hns: 100000},
        ).unwrap();
        let capture_client = audio_client.get_audiocaptureclient().unwrap();
        audio_client.start_stream().unwrap();


        Self {
            sample_rate: format.get_samplespersec() as f32,
            buffer: Vec::new(),
            source: AudioSource::System { capture_client, format, readpos: 0usize },
        }
    }

    pub fn from_microphone(device_selector: Option<&str>) -> Self {
        wasapi::initialize_mta().unwrap();
        let enumerator = wasapi::DeviceEnumerator::new().unwrap();

        let device = match device_selector {
            Some(sel) => pick_capture_device(Some(sel)),
            None => enumerator.get_default_device(&wasapi::Direction::Capture).unwrap(),
        };

        let mut audio_client = device.get_iaudioclient().unwrap();
        let format = audio_client.get_mixformat().unwrap();
        audio_client.initialize_client(
            &format,
            &wasapi::Direction::Capture,
            &wasapi::StreamMode::PollingShared {autoconvert: true, buffer_duration_hns: 100000},
        ).unwrap();
        let capture_client = audio_client.get_audiocaptureclient().unwrap();
        audio_client.start_stream().unwrap();


        Self {
            sample_rate: format.get_samplespersec() as f32,
            buffer: Vec::new(),
            source: AudioSource::Microphone { capture_client, format, readpos: 0usize },
        }
    }

    pub fn next_sample(&mut self) -> Result<bool, Error> {
        match &mut self.source {
            AudioSource::File {
                format,
                decoder,
                track_id,
                sample_buf,
            } => {
                let packet = match format.next_packet() {
                    Ok(packet) => packet,
                    Err(Error::ResetRequired) => {
                        unimplemented!();
                    }
                    Err(Error::IoError(err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                        return Ok(false);
                    }
                    Err(err) => {
                        eprintln!("Error reading packet: {}", err);
                        return Err(err.into());
                    }
                };

                if packet.track_id() != *track_id {
                    return Ok(true);
                }

                while !format.metadata().is_latest() {
                    format.metadata().pop();
                }

                match decoder.decode(&packet) {
                    Ok(decoded) => {
                        if sample_buf.is_none() {
                            self.sample_rate = decoded.spec().rate as f32;
                            let spec = *decoded.spec();
                            let duration = decoded.capacity() as u64;
                            *sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                        }

                        if let Some(buf) = sample_buf {
                            buf.copy_interleaved_ref(decoded);

                            let samples = buf.samples();
                            for frame in samples.chunks(2) {
                                // let mono = frame[0];

                                // proper mono averaging
                                let mono: f32 = if frame.len() == 2 {
                                    (frame[0] + frame[1]) * 0.5
                                } else {
                                    frame[0]
                                };
                                self.buffer.push(mono);
                            }
                            if self.buffer.len() > FFT_SIZE * 4 {
                                self.buffer.drain(0..self.buffer.len() - FFT_SIZE * 2);
                            }
                        }
                    }
                    Err(Error::IoError(_)) => {
                        {};
                    }
                    Err(Error::DecodeError(_)) => {
                        {};
                    }
                    Err(err) => {
                        eprintln!("Unrecoverable decode error: {}", err);
                        return Err(err.into());
                    }
                }
                Ok(true)
            },

            AudioSource::System {
                capture_client,
                format,
                readpos
            } => {
                let mut got_wasapi_samples= false;
                self.sample_rate = format.get_samplespersec() as f32;

                while let Some(packet_size) = capture_client.get_next_packet_size().unwrap() {
                    if packet_size == 0 {
                        break;
                    }

                    let bytes_per_frame = format.get_nchannels() as usize * (format.get_validbitspersample() as usize / 8);
                    let mut buf = vec![0u8; (packet_size as usize) * bytes_per_frame];

                    let (frames_read, _) = capture_client.read_from_device(&mut buf).unwrap();
                    let bytes_read = frames_read as usize * bytes_per_frame;
                    let raw_bytes = &buf[..bytes_read];

                    let samples: Vec<f32> = match format.get_subformat().unwrap() {
                        wasapi::SampleType::Float => unsafe {
                            std::slice::from_raw_parts(raw_bytes.as_ptr() as *const f32, bytes_read / 4).to_vec()
                        },
                        wasapi::SampleType::Int => unsafe {
                            std::slice::from_raw_parts(raw_bytes.as_ptr() as *const i16, bytes_read / 2)
                                .iter()
                                .map(|&v| v as f32 / i16::MAX as f32)
                                .collect()
                        },
                    };

                    let mono: Vec<f32> = if format.get_nchannels() == 2 {
                        (&samples).chunks(2).map(|c| (c[0] + c[1]) * 0.5).collect()
                    } else {
                        samples
                    };

                    self.buffer.extend(mono);
                    got_wasapi_samples = true;
                }

                if !got_wasapi_samples {
                    let silence_length = (format.get_samplespersec() as f32 * 0.075) as usize;
                    self.buffer.extend(std::iter::repeat(0f32).take(silence_length));
                }

                if self.buffer.len() > FFT_SIZE * 2 {
                    self.buffer.drain(0..*readpos);
                    *readpos = 0;
                }

                Ok(true)
            },

            AudioSource::Microphone {
                capture_client,
                format,
                readpos
            } => {
                let mut got_wasapi_samples= false;
                self.sample_rate = format.get_samplespersec() as f32;

                while let Some(packet_size) = capture_client.get_next_packet_size().unwrap() {
                    if packet_size == 0 {
                        break;
                    }

                    let bytes_per_frame = format.get_nchannels() as usize * (format.get_validbitspersample() as usize / 8);
                    let mut buf = vec![0u8; (packet_size as usize) * bytes_per_frame];

                    let (frames_read, _) = capture_client.read_from_device(&mut buf).unwrap();
                    let bytes_read = frames_read as usize * bytes_per_frame;
                    let raw_bytes = &buf[..bytes_read];

                    let samples: Vec<f32> = match format.get_subformat().unwrap() {
                        wasapi::SampleType::Float => unsafe {
                            std::slice::from_raw_parts(raw_bytes.as_ptr() as *const f32, bytes_read / 4).to_vec()
                        },
                        wasapi::SampleType::Int => unsafe {
                            std::slice::from_raw_parts(raw_bytes.as_ptr() as *const i16, bytes_read / 2)
                                .iter()
                                .map(|&v| v as f32 / i16::MAX as f32)
                                .collect()
                        },
                    };

                    let mono: Vec<f32> = if format.get_nchannels() == 2 {
                        (&samples).chunks(2).map(|c| (c[0] + c[1]) * 0.5).collect()
                    } else {
                        samples
                    };

                    self.buffer.extend(mono);
                    got_wasapi_samples = true;
                }

                if !got_wasapi_samples {
                    let silence_length = (format.get_samplespersec() as f32 * 0.075) as usize;
                    self.buffer.extend(std::iter::repeat(0f32).take(silence_length));
                }

                if self.buffer.len() > FFT_SIZE * 2 {
                    self.buffer.drain(0..*readpos);
                    *readpos = 0;
                }

                Ok(true)
            },
        }
    }
}

pub fn list_render_devices() {
    wasapi::initialize_mta().unwrap();
    let enumerator = wasapi::DeviceEnumerator::new().unwrap();

    let collection = enumerator.get_device_collection(&wasapi::Direction::Render).unwrap();
    let devices_count = collection.get_nbr_devices().unwrap_or(0);

    if devices_count == 0 {
        println!("No Devices Found!");
        return;
    }

    for i in 0..devices_count {
        if let Ok(device) = collection.get_device_at_index(i) {
            let name = device.get_friendlyname().unwrap_or_else(|_| "<unknown>".to_string());
            println!("{i}: {name}");
        }
    }
}

fn pick_render_device(selector: Option<&str>) -> wasapi::Device {
    wasapi::initialize_mta().unwrap();
    let enumerator = wasapi::DeviceEnumerator::new().unwrap();

    let Some(sel) = selector else {
        return enumerator.get_default_device(&wasapi::Direction::Render).unwrap();
    };

    let device_collection = enumerator.get_device_collection(&wasapi::Direction::Render).unwrap();

    if let Ok(index) = sel.parse::<usize>() {
        if let Ok(device) = device_collection.get_device_at_index(index as u32) {
            return device;
        }
        eprintln!("Invalid device index: {idx}. Run `pulsetty --list-devices` to get a list of available devices.", idx=index);
        panic!("Invalid --device index");
    }

    let search_str = sel.to_lowercase();
    let devices_count = device_collection.get_nbr_devices().unwrap_or(0);

    for i in 0..devices_count {
        if let Ok(device) = device_collection.get_device_at_index(i) {
            let name = device.get_friendlyname().unwrap_or_else(|_| "<unknown>".to_string());
            if name.as_str().to_lowercase().as_str().contains(&search_str) {
                return device;
            }
        }
    }

    eprintln!("No device matched '{selstr}'. Run `pulsetty --list-devices` to get a list of available devices.", selstr=sel);
    panic!("No matching --device")
}

pub fn list_capture_devices() {
    wasapi::initialize_mta().unwrap();
    let enumerator = wasapi::DeviceEnumerator::new().unwrap();

    let collection = enumerator.get_device_collection(&wasapi::Direction::Capture).unwrap();
    let devices_count = collection.get_nbr_devices().unwrap_or(0);

    if devices_count == 0 {
        println!("No Capture Devices Found!");
        return;
    }

    for i in 0..devices_count {
        if let Ok(device) = collection.get_device_at_index(i) {
            let name = device.get_friendlyname().unwrap_or_else(|_| "<unknown>".to_string());
            println!("{i}: {name}");
        }
    }
}

fn pick_capture_device(selector: Option<&str>) -> wasapi::Device {
    wasapi::initialize_mta().unwrap();
    let enumerator = wasapi::DeviceEnumerator::new().unwrap();

    let Some(sel) = selector else {
        return enumerator.get_default_device(&wasapi::Direction::Capture).unwrap();
    };

    let device_collection = enumerator.get_device_collection(&wasapi::Direction::Capture).unwrap();

    if let Ok(index) = sel.parse::<usize>() {
        if let Ok(device) = device_collection.get_device_at_index(index as u32) {
            return device;
        }
        eprintln!("Invalid device index: {idx}. Run `pulsetty --list-mics` to get a list of available microphones.", idx=index);
        panic!("Invalid --mic index");
    }

    let search_str = sel.to_lowercase();
    let devices_count = device_collection.get_nbr_devices().unwrap_or(0);

    for i in 0..devices_count {
        if let Ok(device) = device_collection.get_device_at_index(i) {
            let name = device.get_friendlyname().unwrap_or_else(|_| "<unknown>".to_string());
            if name.as_str().to_lowercase().as_str().contains(&search_str) {
                return device;
            }
        }
    }

    eprintln!("No device matched '{selstr}'. Run `pulsetty --list-mics` to get a list of available microphones.", selstr=sel);
    panic!("No matching --mic")
}