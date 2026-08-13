use crate::config::{
    DEFAULT_PITCH_VARIATION, DEFAULT_VELOCITY_VARIATION, MAX_VARIATION, clamp_volume,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, StreamConfig};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

const FIXED_BUFFER_FRAMES: u32 = 256;
const DECODE_CACHE_LIMIT: usize = 512;
const MAX_STALLED_PACKETS: u32 = 8;
const NORMALIZE_TARGET_PEAK: f32 = 0.1;
const MAX_NORMALIZE_GAIN: f32 = 16.0;
const LIMITER_KNEE: f32 = 0.75;
const LIMITER_CEILING: f32 = 1.0;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("no default audio output device is available")]
    NoOutputDevice,
    #[error("output device does not support an f32 stream: {0}")]
    UnsupportedFormat(String),
    #[error("could not build the output stream: {0}")]
    BuildStream(cpal::Error),
    #[error("could not start the output stream: {0}")]
    PlayStream(cpal::Error),
    #[error("could not open audio file {}: {source}", path.display())]
    OpenFile { path: PathBuf, source: io::Error },
    #[error("could not decode audio file {}: {0}", path.display())]
    Decode { path: PathBuf, reason: String },
    #[error("audio file {} decoded to no samples", _0.display())]
    EmptyAudio(PathBuf),
}

#[derive(Debug)]
pub struct DecodedSound {
    samples: Arc<Vec<f32>>,
    channels: u16,
    sample_rate: u32,
}

pub trait AudioControl: Send + Sync {
    fn play(&self, path: &Path) -> Result<(), AudioError>;
    fn play_decoded(&self, sound: Arc<DecodedSound>) -> Result<(), AudioError>;
    fn set_master_volume(&self, volume: f32);
    fn volume(&self) -> f32;
    fn stream_failed(&self) -> bool;
    fn set_output_device(&self, name: Option<&str>) -> Result<(), AudioError>;
    fn output_device(&self) -> Option<String>;
    fn set_tone(&self, pan: f32, distance: f32);
    fn tone(&self) -> TonePad;
    fn set_variation(&self, pitch: f32, velocity: f32);
}

impl AudioControl for Audio {
    fn play(&self, path: &Path) -> Result<(), AudioError> {
        self.play(path)
    }

    fn play_decoded(&self, sound: Arc<DecodedSound>) -> Result<(), AudioError> {
        self.play_decoded(sound)
    }

    fn set_master_volume(&self, volume: f32) {
        self.set_master_volume(volume);
    }

    fn volume(&self) -> f32 {
        self.volume()
    }

    fn stream_failed(&self) -> bool {
        self.stream_failed()
    }

    fn set_output_device(&self, name: Option<&str>) -> Result<(), AudioError> {
        self.set_output_device(name)
    }

    fn output_device(&self) -> Option<String> {
        self.output_device()
    }

    fn set_tone(&self, pan: f32, distance: f32) {
        self.set_tone(pan, distance);
    }

    fn tone(&self) -> TonePad {
        self.tone()
    }

    fn set_variation(&self, pitch: f32, velocity: f32) {
        self.set_variation(pitch, velocity);
    }
}

pub struct Audio {
    _stream: Mutex<Option<cpal::Stream>>,
    voices: Arc<Mutex<VoicePool>>,
    volume: Arc<AtomicU32>,
    tone_pan: Arc<AtomicU32>,
    tone_distance: Arc<AtomicU32>,
    pitch_variation: Arc<AtomicU32>,
    velocity_variation: Arc<AtomicU32>,
    cache: Mutex<DecodeCache>,
    stream_failed: Arc<AtomicBool>,
    underrun_count: Arc<AtomicU32>,
    output_device_name: Mutex<Option<String>>,
}

impl Audio {
    pub fn new(default_volume: f32) -> Result<Self, AudioError> {
        let (device, config) = select_output_config()?;
        let volume = Arc::new(AtomicU32::new(clamp_volume(default_volume).to_bits()));
        let voices = Arc::new(Mutex::new(VoicePool::new(
            usize::from(config.channels),
            config.sample_rate,
        )));
        let stream_failed = Arc::new(AtomicBool::new(false));
        let underrun_count = Arc::new(AtomicU32::new(0));
        let default_tone = TonePad::default();
        let tone_pan = Arc::new(AtomicU32::new(default_tone.pan.to_bits()));
        let tone_distance = Arc::new(AtomicU32::new(default_tone.distance.to_bits()));
        let pitch_variation = Arc::new(AtomicU32::new(DEFAULT_PITCH_VARIATION.to_bits()));
        let velocity_variation = Arc::new(AtomicU32::new(DEFAULT_VELOCITY_VARIATION.to_bits()));

        let preferred = StreamConfig {
            buffer_size: BufferSize::Fixed(FIXED_BUFFER_FRAMES),
            ..config
        };
        let handles = StreamHandles {
            voices: &voices,
            volume: &volume,
            tone_pan: &tone_pan,
            tone_distance: &tone_distance,
            stream_failed: &stream_failed,
            underrun_count: &underrun_count,
        };
        let stream = match build_output_stream(&device, preferred, handles) {
            Ok(stream) => stream,
            Err(error) if error.kind() == cpal::ErrorKind::UnsupportedConfig => {
                build_output_stream(&device, config, handles).map_err(AudioError::BuildStream)?
            }
            Err(error) => return Err(AudioError::BuildStream(error)),
        };

        stream.play().map_err(AudioError::PlayStream)?;

        Ok(Self {
            _stream: Mutex::new(Some(stream)),
            voices,
            volume,
            tone_pan,
            tone_distance,
            pitch_variation,
            velocity_variation,
            cache: Mutex::new(DecodeCache::new()),
            stream_failed,
            underrun_count,
            output_device_name: Mutex::new(None),
        })
    }

    pub fn play_decoded(&self, sound: Arc<DecodedSound>) -> Result<(), AudioError> {
        let (pitch, velocity) = varied_playback(
            self.pitch_variation(),
            self.velocity_variation(),
            fastrand::f32,
        );
        self.voices
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .spawn(sound, pitch, velocity);

        Ok(())
    }

    pub fn play(&self, path: &Path) -> Result<(), AudioError> {
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let sound = cache.get_or_decode(path)?;
        let mut pool = self
            .voices
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (pitch, velocity) = varied_playback(
            self.pitch_variation(),
            self.velocity_variation(),
            fastrand::f32,
        );
        pool.spawn(sound, pitch, velocity);

        Ok(())
    }

    pub fn set_master_volume(&self, volume: f32) {
        self.volume
            .store(clamp_volume(volume).to_bits(), Ordering::Relaxed);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    pub fn set_tone(&self, pan: f32, distance: f32) {
        self.tone_pan.store(pan.to_bits(), Ordering::Relaxed);
        self.tone_distance
            .store(distance.to_bits(), Ordering::Relaxed);
    }

    pub fn tone(&self) -> TonePad {
        TonePad {
            pan: f32::from_bits(self.tone_pan.load(Ordering::Relaxed)),
            distance: f32::from_bits(self.tone_distance.load(Ordering::Relaxed)),
        }
    }

    pub fn set_variation(&self, pitch: f32, velocity: f32) {
        self.pitch_variation
            .store(pitch.clamp(0.0, MAX_VARIATION).to_bits(), Ordering::Relaxed);
        self.velocity_variation.store(
            velocity.clamp(0.0, MAX_VARIATION).to_bits(),
            Ordering::Relaxed,
        );
    }

    fn pitch_variation(&self) -> f32 {
        f32::from_bits(self.pitch_variation.load(Ordering::Relaxed))
    }

    fn velocity_variation(&self) -> f32 {
        f32::from_bits(self.velocity_variation.load(Ordering::Relaxed))
    }

    pub fn stream_failed(&self) -> bool {
        self.stream_failed.load(Ordering::Relaxed)
    }

    pub fn underrun_count(&self) -> u32 {
        self.underrun_count.load(Ordering::Relaxed)
    }

    pub fn output_devices(&self) -> Vec<String> {
        match cpal::default_host().output_devices() {
            Ok(devices) => devices.map(|device| format!("{device}")).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn output_device(&self) -> Option<String> {
        self.output_device_name
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn set_output_device(&self, name: Option<&str>) -> Result<(), AudioError> {
        let config = select_output_config_for(name)?;
        let stream = build_output_stream(
            &config.0,
            config.1,
            StreamHandles {
                voices: &self.voices,
                volume: &self.volume,
                tone_pan: &self.tone_pan,
                tone_distance: &self.tone_distance,
                stream_failed: &self.stream_failed,
                underrun_count: &self.underrun_count,
            },
        )
        .map_err(AudioError::BuildStream)?;
        stream.play().map_err(AudioError::PlayStream)?;
        let mut current = self
            ._stream
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current = Some(stream);
        drop(current);

        *self
            .output_device_name
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = name.map(str::to_string);

        Ok(())
    }
}

#[derive(Clone, Copy)]
struct StreamHandles<'a> {
    voices: &'a Arc<Mutex<VoicePool>>,
    volume: &'a Arc<AtomicU32>,
    tone_pan: &'a Arc<AtomicU32>,
    tone_distance: &'a Arc<AtomicU32>,
    stream_failed: &'a Arc<AtomicBool>,
    underrun_count: &'a Arc<AtomicU32>,
}

fn build_output_stream(
    device: &cpal::Device,
    config: StreamConfig,
    handles: StreamHandles,
) -> Result<cpal::Stream, cpal::Error> {
    let voices_callback = Arc::clone(handles.voices);
    let volume_callback = Arc::clone(handles.volume);
    let tone_pan_callback = Arc::clone(handles.tone_pan);
    let tone_distance_callback = Arc::clone(handles.tone_distance);
    let stream_failed_callback = Arc::clone(handles.stream_failed);
    let underrun_count_callback = Arc::clone(handles.underrun_count);

    device.build_output_stream(
        config,
        move |data: &mut [f32], _| {
            data.fill(0.0);
            let gain = f32::from_bits(volume_callback.load(Ordering::Relaxed)) / 10.0;
            let tone = TonePad {
                pan: f32::from_bits(tone_pan_callback.load(Ordering::Relaxed)),
                distance: f32::from_bits(tone_distance_callback.load(Ordering::Relaxed)),
            };
            let mut pool = voices_callback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            pool.mix_into(data, gain, tone);
            apply_ceiling(data);
        },
        move |_error| {
            stream_failed_callback.store(true, Ordering::Relaxed);
            underrun_count_callback.fetch_add(1, Ordering::Relaxed);
        },
        None,
    )
}

fn select_output_config() -> Result<(cpal::Device, StreamConfig), AudioError> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or(AudioError::NoOutputDevice)?;

    select_output_config_for_device(&device)
}

fn select_output_config_for(
    name: Option<&str>,
) -> Result<(cpal::Device, StreamConfig), AudioError> {
    let Some(name) = name else {
        return select_output_config();
    };

    let host = cpal::default_host();
    let devices = host.output_devices().map_err(|error| {
        AudioError::UnsupportedFormat(format!("could not enumerate output devices: {error}"))
    })?;
    let mut devices = devices;
    let device = devices
        .find(|device| format!("{device}") == name)
        .ok_or_else(|| AudioError::UnsupportedFormat(format!("no output device named '{name}'")))?;

    select_output_config_for_device(&device)
}

fn select_output_config_for_device(
    device: &cpal::Device,
) -> Result<(cpal::Device, StreamConfig), AudioError> {
    let supported = device
        .supported_output_configs()
        .map_err(|error| AudioError::UnsupportedFormat(error.to_string()))?
        .filter(|config| config.sample_format() == SampleFormat::F32)
        .find_map(|config| config.try_with_standard_sample_rate())
        .ok_or_else(|| {
            AudioError::UnsupportedFormat(String::from("no f32 standard-rate config"))
        })?;

    let stream_config = StreamConfig {
        channels: supported.channels(),
        sample_rate: supported.sample_rate(),
        buffer_size: BufferSize::Default,
    };

    Ok((device.clone(), stream_config))
}

fn decode_file(path: &Path) -> Result<DecodedSound, AudioError> {
    let file = File::open(path).map_err(|source| AudioError::OpenFile {
        path: path.to_path_buf(),
        source,
    })?;
    let source_stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        hint.with_extension(extension);
    }

    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            source_stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| AudioError::Decode {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| AudioError::Decode {
            path: path.to_path_buf(),
            reason: String::from("no audio track"),
        })?;

    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio().cloned())
        .ok_or_else(|| AudioError::Decode {
            path: path.to_path_buf(),
            reason: String::from("track has no audio codec parameters"),
        })?;

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&codec_params, &AudioDecoderOptions::default())
        .map_err(|error| AudioError::Decode {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;

    let track_id = track.id;
    let mut samples: Vec<f32> = Vec::new();
    let mut stalled = 0u32;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(_) => {
                stalled += 1;
                if stalled >= MAX_STALLED_PACKETS {
                    break;
                }
                continue;
            }
        };

        if packet.track_id != track_id {
            continue;
        }

        let before = samples.len();
        if let Ok(decoded) = decoder.decode(&packet) {
            let mut packet_samples: Vec<f32> = Vec::new();
            decoded.copy_to_vec_interleaved(&mut packet_samples);
            samples.extend_from_slice(&packet_samples);
        }

        let produced = samples.len() > before;
        if produced {
            stalled = 0;
        } else {
            stalled += 1;
            if stalled >= MAX_STALLED_PACKETS {
                break;
            }
        }
    }

    if samples.is_empty() {
        return Err(AudioError::EmptyAudio(path.to_path_buf()));
    }

    let params = decoder.codec_params();
    let channels = params
        .channels
        .as_ref()
        .map_or(1, |channels| channels.count())
        .clamp(1, 2) as u16;
    let sample_rate = params.sample_rate.unwrap_or(44_100);

    normalize_peak(&mut samples);

    Ok(DecodedSound {
        samples: Arc::new(samples),
        channels,
        sample_rate,
    })
}

fn normalize_peak(samples: &mut [f32]) {
    let peak = samples
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    if peak <= 0.0 {
        return;
    }

    let gain = (NORMALIZE_TARGET_PEAK / peak).min(MAX_NORMALIZE_GAIN);
    if (gain - 1.0).abs() <= f32::EPSILON {
        return;
    }

    for sample in samples {
        *sample *= gain;
    }
}

fn apply_ceiling(data: &mut [f32]) {
    for sample in data {
        let magnitude = sample.abs();
        if magnitude <= LIMITER_KNEE {
            continue;
        }

        let overshoot = magnitude - LIMITER_KNEE;
        let headroom = LIMITER_CEILING - LIMITER_KNEE;
        let compressed = LIMITER_KNEE + headroom * (1.0 - (-overshoot / headroom).exp());
        *sample = if *sample < 0.0 {
            -compressed
        } else {
            compressed
        };
    }
}

pub(crate) fn synthesized_ding() -> DecodedSound {
    const SAMPLE_RATE: u32 = 44_100;
    const DURATION_FRAMES: usize = 15_000;

    let mut samples = Vec::with_capacity(DURATION_FRAMES);
    for frame in 0..DURATION_FRAMES {
        let time = frame as f32 / SAMPLE_RATE as f32;
        let decay = (-time * 18.0).exp();
        let fundamental = (2.0 * std::f32::consts::PI * 880.0 * time).sin();
        let overtone = 0.4 * (2.0 * std::f32::consts::PI * 1760.0 * time).sin();
        samples.push((fundamental + overtone) * 0.22 * decay);
    }

    DecodedSound {
        samples: Arc::new(samples),
        channels: 1,
        sample_rate: SAMPLE_RATE,
    }
}

struct DecodeCache {
    by_path: HashMap<PathBuf, Arc<DecodedSound>>,
}

impl DecodeCache {
    fn new() -> Self {
        Self {
            by_path: HashMap::new(),
        }
    }

    fn get_or_decode(&mut self, path: &Path) -> Result<Arc<DecodedSound>, AudioError> {
        if let Some(sound) = self.by_path.get(path) {
            return Ok(Arc::clone(sound));
        }

        let sound = Arc::new(decode_file(path)?);
        if self.by_path.len() >= DECODE_CACHE_LIMIT
            && let Some(stale) = self.by_path.keys().next().cloned()
        {
            self.by_path.remove(&stale);
        }
        self.by_path.insert(path.to_path_buf(), Arc::clone(&sound));

        Ok(sound)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TonePad {
    pub pan: f32,
    pub distance: f32,
}

impl Default for TonePad {
    fn default() -> Self {
        Self {
            pan: 0.0,
            distance: 0.0,
        }
    }
}

struct VoicePool {
    voices: Vec<Voice>,
    out_channels: usize,
    out_rate: u32,
}

impl VoicePool {
    fn new(out_channels: usize, out_rate: u32) -> Self {
        Self {
            voices: Vec::new(),
            out_channels,
            out_rate,
        }
    }

    fn spawn(&mut self, sound: Arc<DecodedSound>, pitch: f32, velocity: f32) {
        if let Some(slot) = self.voices.iter_mut().find(|voice| voice.finished) {
            *slot = Voice::new(&sound, pitch, velocity);
        } else {
            self.voices.push(Voice::new(&sound, pitch, velocity));
        }
    }

    fn mix_into(&mut self, output: &mut [f32], gain: f32, tone: TonePad) {
        let distance_attenuation = 1.0 - tone.distance.clamp(0.0, 1.0) * 0.75;
        let pan = tone.pan.clamp(-1.0, 1.0);
        let pan_left = 1.0 - pan.max(0.0);
        let pan_right = 1.0 + pan.min(0.0);
        let voice_gain = gain * distance_attenuation;

        if self.out_channels == 2 {
            for voice in &mut self.voices {
                voice.mix_panned(
                    output,
                    self.out_channels,
                    self.out_rate,
                    voice_gain * voice.velocity,
                    pan_left,
                    pan_right,
                );
            }
        } else {
            for voice in &mut self.voices {
                voice.mix_into(
                    output,
                    self.out_channels,
                    self.out_rate,
                    voice_gain * voice.velocity,
                );
            }
        }
    }
}

struct Voice {
    samples: Arc<Vec<f32>>,
    channels: usize,
    sample_rate: u32,
    pitch: f32,
    velocity: f32,
    pos: f64,
    finished: bool,
}

impl Voice {
    fn new(sound: &DecodedSound, pitch: f32, velocity: f32) -> Self {
        Self {
            samples: Arc::clone(&sound.samples),
            channels: usize::from(sound.channels),
            sample_rate: sound.sample_rate,
            pitch,
            velocity,
            pos: 0.0,
            finished: false,
        }
    }
    fn mix_panned(
        &mut self,
        output: &mut [f32],
        out_channels: usize,
        out_rate: u32,
        gain: f32,
        pan_left: f32,
        pan_right: f32,
    ) {
        if self.finished || self.channels == 0 || self.samples.is_empty() {
            return;
        }

        let ratio = f64::from(self.sample_rate) / f64::from(out_rate) * f64::from(self.pitch);
        let source_frames = self.samples.len() / self.channels;
        let samples = &self.samples;

        for frame in output.chunks_exact_mut(out_channels) {
            let index = self.pos.floor() as usize;
            if index >= source_frames {
                self.finished = true;
                return;
            }

            let next = (index + 1).min(source_frames - 1);
            let fraction = (self.pos - index as f64) as f32;

            if self.channels == 1 {
                let sample = interpolate(samples[index], samples[next], fraction);
                frame[0] += sample * gain * pan_left;
                frame[1] += sample * gain * pan_right;
            } else {
                let base = index * self.channels;
                let left = samples[base];
                let left_next = samples[next * self.channels];
                let right = samples[base + 1];
                let right_next = samples[next * self.channels + 1];
                frame[0] += interpolate(left, left_next, fraction) * gain * pan_left;
                frame[1] += interpolate(right, right_next, fraction) * gain * pan_right;
            }

            self.pos += ratio;
        }

        if self.pos + ratio >= source_frames as f64 {
            self.finished = true;
        }
    }

    fn mix_into(&mut self, output: &mut [f32], out_channels: usize, out_rate: u32, gain: f32) {
        if self.finished || self.channels == 0 || self.samples.is_empty() {
            return;
        }

        let ratio = f64::from(self.sample_rate) / f64::from(out_rate) * f64::from(self.pitch);
        let source_frames = self.samples.len() / self.channels;
        let samples = &self.samples;

        for frame in output.chunks_exact_mut(out_channels) {
            let index = self.pos.floor() as usize;
            if index >= source_frames {
                self.finished = true;
                return;
            }

            let next = (index + 1).min(source_frames - 1);
            let fraction = (self.pos - index as f64) as f32;

            if self.channels == 1 {
                let sample = interpolate(samples[index], samples[next], fraction);
                for channel in frame {
                    *channel += sample * gain;
                }
            } else {
                for (channel, out) in frame
                    .iter_mut()
                    .enumerate()
                    .take(self.channels.min(out_channels))
                {
                    let base = index * self.channels;
                    let current = samples[base + channel];
                    let neighbour = samples[next * self.channels + channel];
                    *out += interpolate(current, neighbour, fraction) * gain;
                }
            }

            self.pos += ratio;
        }

        if self.pos + ratio >= source_frames as f64 {
            self.finished = true;
        }
    }
}

fn interpolate(current: f32, neighbour: f32, fraction: f32) -> f32 {
    current + (neighbour - current) * fraction
}

fn varied_playback(
    pitch_variation: f32,
    velocity_variation: f32,
    mut roll: impl FnMut() -> f32,
) -> (f32, f32) {
    let pitch = 1.0 - pitch_variation + roll() * pitch_variation * 2.0;
    let velocity = 1.0 - roll() * velocity_variation;

    (pitch, velocity)
}

#[cfg(test)]
mod tests {
    use super::{
        DecodeCache, DecodedSound, MAX_VARIATION, TonePad, VoicePool, decode_file, varied_playback,
    };
    use std::path::PathBuf;
    use std::sync::Arc;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn varied_playback_keeps_pitch_and_velocity_within_bounds() {
        let variations = [0.0, MAX_VARIATION / 2.0, MAX_VARIATION];
        let rolls = [0.0, 0.25, 0.5, 0.75, 0.999_999];

        for &variation in &variations {
            for &roll in &rolls {
                let (pitch, velocity) = varied_playback(variation, variation, || roll);

                assert!(
                    (0.5..=1.5).contains(&pitch),
                    "pitch {pitch} out of bounds for variation {variation}, roll {roll}"
                );
                assert!(
                    (0.5..=1.0).contains(&velocity),
                    "velocity {velocity} out of bounds for variation {variation}, roll {roll}"
                );
            }
        }
    }

    #[test]
    fn tone_pad_pans_mono_evenly_to_the_left() {
        let mut pool = VoicePool::new(2, 44_100);
        pool.spawn(
            Arc::new(DecodedSound {
                samples: Arc::new(vec![1.0; 4_410]),
                channels: 1,
                sample_rate: 44_100,
            }),
            1.0,
            1.0,
        );

        let mut output = vec![0.0; 100 * 2];
        pool.mix_into(
            &mut output,
            1.0,
            TonePad {
                pan: -1.0,
                distance: 0.0,
            },
        );

        assert!(output.iter().step_by(2).all(|s| (s - 1.0).abs() < 1e-4));
        assert!(output.iter().skip(1).step_by(2).all(|s| s.abs() < 1e-4));
    }

    #[test]
    fn tone_pad_distance_attenuates() {
        let mut pool = VoicePool::new(2, 44_100);
        pool.spawn(
            Arc::new(DecodedSound {
                samples: Arc::new(vec![1.0; 4_410]),
                channels: 1,
                sample_rate: 44_100,
            }),
            1.0,
            1.0,
        );

        let mut near = vec![0.0; 100 * 2];
        pool.mix_into(
            &mut near,
            1.0,
            TonePad {
                pan: 0.0,
                distance: 0.0,
            },
        );
        let near_peak = near.iter().fold(0.0f32, |a, s| a.max(s.abs()));

        let far = vec![0.0; 100 * 2];
        let mut far = far;
        // re-spawn on a fresh pool to compare same source at distance 1.0
        let mut pool_far = VoicePool::new(2, 44_100);
        pool_far.spawn(
            Arc::new(DecodedSound {
                samples: Arc::new(vec![1.0; 4_410]),
                channels: 1,
                sample_rate: 44_100,
            }),
            1.0,
            1.0,
        );
        pool_far.mix_into(
            &mut far,
            1.0,
            TonePad {
                pan: 0.0,
                distance: 1.0,
            },
        );
        let far_peak = far.iter().fold(0.0f32, |a, s| a.max(s.abs()));

        assert!(near_peak > far_peak, "distance must attenuate");
        assert!((far_peak - near_peak * 0.25).abs() < 1e-3);
    }

    #[test]
    fn synthesized_ding_is_audible_and_decays() {
        let ding = super::synthesized_ding();

        assert_eq!(ding.channels, 1);
        assert_eq!(ding.sample_rate, 44_100);
        assert!(ding.samples.len() > 10_000);
        let peak = ding.samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        assert!(peak > 0.1, "ding must be audible");
    }

    #[test]
    fn decodes_wav_and_mp3_fixtures_at_full_duration() {
        let wav = decode_file(&fixture("tone.wav")).expect("decode wav fixture");

        assert!(!wav.samples.is_empty());
        assert_eq!(wav.channels, 1);
        assert_eq!(wav.sample_rate, 44_100);
        assert_eq!(
            wav.samples.len(),
            6_615,
            "0.15 s mono 44.1 kHz wav keeps every frame"
        );

        let mp3 = decode_file(&fixture("tone.mp3")).expect("decode mp3 fixture");

        assert_eq!(mp3.channels, 1);
        assert_eq!(mp3.sample_rate, 44_100);
        assert!(
            (5_500..=7_000).contains(&mp3.samples.len()),
            "mp3 must decode near the full duration, got {} samples",
            mp3.samples.len()
        );
    }

    #[test]
    fn decoded_sounds_are_peak_normalized_to_the_reference_level() {
        let sound = decode_file(&fixture("tone.wav")).expect("decode wav fixture");

        let peak = sound
            .samples
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));

        assert!(
            (peak - super::NORMALIZE_TARGET_PEAK).abs() < 1e-4,
            "decoded peak must equal the normalization reference, got {peak}"
        );
    }

    #[test]
    fn ceiling_passes_quiet_through_and_compresses_loud_peaks() {
        let input = [0.2, -0.5, 0.76, 0.9, 2.0, -1.2];
        let mut data = input;

        super::apply_ceiling(&mut data);

        for index in 0..data.len() {
            assert!(
                data[index].abs() <= 1.0,
                "no sample may exceed the unit ceiling"
            );
            assert_eq!(
                data[index].signum(),
                input[index].signum(),
                "sign must be preserved"
            );
        }
        assert_eq!(data[0], 0.2, "samples below the knee pass through");
        assert_eq!(data[1], -0.5);
        assert!(data[2] < 0.76);
        assert!(data[3] < 0.9);
        assert!(data[4] < 2.0);
        assert!(
            data[4] > data[3],
            "larger inputs stay louder after limiting"
        );
        assert!(data[5] > -1.2 && data[5] < -0.9);
    }

    #[test]
    fn cache_returns_the_same_arc_without_redecoding() {
        let mut cache = DecodeCache::new();
        let path = fixture("tone.wav");

        let first = cache.get_or_decode(&path).expect("decode");
        let second = cache.get_or_decode(&path).expect("cache hit");

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn cache_is_bounded_by_single_entry_eviction() {
        let mut cache = DecodeCache::new();
        let source = fixture("tone.wav");
        let directory =
            std::env::temp_dir().join(format!("wayvibes-audio-cache-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("create temp directory");

        for index in 0..super::DECODE_CACHE_LIMIT {
            let path = directory.join(format!("tone-{index}.wav"));
            std::fs::copy(&source, &path).expect("copy fixture");
            cache.get_or_decode(&path).expect("decode");
        }

        let extra = directory.join("tone-extra.wav");
        std::fs::copy(&source, &extra).expect("copy extra");
        cache.get_or_decode(&extra).expect("decode extra");

        assert_eq!(cache.by_path.len(), super::DECODE_CACHE_LIMIT);
        assert!(cache.by_path.contains_key(&extra));
        std::fs::remove_dir_all(directory).expect("remove temp directory");
    }

    #[test]
    fn missing_files_and_garbage_do_not_panic() {
        let mut cache = DecodeCache::new();

        assert!(cache.get_or_decode(&fixture("missing.wav")).is_err());

        let garbage =
            std::env::temp_dir().join(format!("wayvibes-audio-garbage-{}.wav", std::process::id()));
        std::fs::write(&garbage, b"not audio").expect("write garbage");
        assert!(cache.get_or_decode(&garbage).is_err());
        let _ = std::fs::remove_file(garbage);
    }

    #[test]
    fn truncated_wav_files_terminate_without_hanging() {
        let mut bytes = std::fs::read(fixture("tone.wav")).expect("read fixture");
        bytes.truncate(bytes.len() / 2);
        let truncated = std::env::temp_dir().join(format!(
            "wayvibes-audio-truncated-{}.wav",
            std::process::id()
        ));
        std::fs::write(&truncated, &bytes).expect("write truncated wav");

        match decode_file(&truncated) {
            Ok(sound) => assert!(!sound.samples.is_empty()),
            Err(super::AudioError::EmptyAudio(_)) => {}
            Err(error) => panic!("unexpected decode error: {error}"),
        }

        let _ = std::fs::remove_file(truncated);
    }

    #[test]
    fn mono_voice_broadcasts_to_stereo_with_master_gain() {
        let mut pool = VoicePool::new(2, 44_100);
        pool.spawn(
            Arc::new(DecodedSound {
                samples: Arc::new(vec![0.25; 4_410]),
                channels: 1,
                sample_rate: 44_100,
            }),
            1.0,
            1.0,
        );

        let mut output = vec![0.0; 4_410 * 2];
        pool.mix_into(&mut output, 0.5, TonePad::default());

        assert!(output.iter().all(|sample| (sample - 0.125).abs() < 1e-6));
        assert!(output[0] > 0.0);
    }

    #[test]
    fn overlapping_voices_sum_and_finished_slots_are_recycled() {
        let mut pool = VoicePool::new(2, 44_100);
        pool.spawn(
            Arc::new(DecodedSound {
                samples: Arc::new(vec![0.1; 4_410]),
                channels: 1,
                sample_rate: 44_100,
            }),
            1.0,
            1.0,
        );
        pool.spawn(
            Arc::new(DecodedSound {
                samples: Arc::new(vec![0.2; 4_410]),
                channels: 1,
                sample_rate: 44_100,
            }),
            1.0,
            1.0,
        );

        let mut output = vec![0.0; 100 * 2];
        pool.mix_into(&mut output, 1.0, TonePad::default());

        assert!((output[0] - 0.3).abs() < 1e-6);
        assert_eq!(pool.voices.len(), 2);

        let mut drain = vec![0.0; 4_411 * 2];
        pool.mix_into(&mut drain, 1.0, TonePad::default());
        assert!(pool.voices.iter().all(|voice| voice.finished));

        pool.spawn(
            Arc::new(DecodedSound {
                samples: Arc::new(vec![0.5; 100]),
                channels: 1,
                sample_rate: 44_100,
            }),
            1.0,
            1.0,
        );
        assert_eq!(pool.voices.len(), 2);
    }

    #[test]
    fn velocity_scales_a_voice_gain() {
        let mut pool = VoicePool::new(2, 44_100);
        pool.spawn(
            Arc::new(DecodedSound {
                samples: Arc::new(vec![1.0; 4_410]),
                channels: 1,
                sample_rate: 44_100,
            }),
            1.0,
            0.5,
        );

        let mut output = vec![0.0; 100 * 2];
        pool.mix_into(&mut output, 1.0, TonePad::default());

        assert!((output[0] - 0.5).abs() < 1e-6);
        assert!((output[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn pitch_above_one_finishes_the_voice_in_half_the_frames() {
        let mut pool = VoicePool::new(2, 44_100);
        pool.spawn(
            Arc::new(DecodedSound {
                samples: Arc::new(vec![0.5; 4_410]),
                channels: 1,
                sample_rate: 44_100,
            }),
            2.0,
            1.0,
        );

        let mut output = vec![0.0; 2_206 * 2];
        pool.mix_into(&mut output, 1.0, TonePad::default());

        assert!(pool.voices[0].finished);
    }

    #[test]
    fn sample_rate_mismatch_resamples_without_overflow() {
        let mut pool = VoicePool::new(2, 48_000);
        pool.spawn(
            Arc::new(DecodedSound {
                samples: Arc::new(vec![0.5; 4_410]),
                channels: 1,
                sample_rate: 44_100,
            }),
            1.0,
            1.0,
        );

        let mut output = vec![0.0; 4_800 * 2];
        pool.mix_into(&mut output, 1.0, TonePad::default());

        assert!(pool.voices[0].finished);
        assert!(output.iter().any(|sample| *sample > 0.0));
    }
}
