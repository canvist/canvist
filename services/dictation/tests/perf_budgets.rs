//! Performance budgets for the relay's per-session audio work.
//!
//! Every second of every session's audio runs through voice activity
//! detection, and through a resampler for OpenAI. These tests fail when
//! either gets slower than its budget per second of audio, which bounds how
//! many sessions one core carries. They only mean something in release
//! mode:
//!
//! ```sh
//! cargo test --release -p canvist_dictation_relay --test perf_budgets -- --ignored --nocapture
//! ```
//!
//! `CANVIST_PERF_SLACK` multiplies every budget.

#![cfg(not(debug_assertions))]

use std::hint::black_box;
use std::time::Duration;
use std::time::Instant;

use canvist_dictation_relay::audio::Resampler;
use canvist_dictation_relay::vad::Vad;

/// Seconds of audio each sample processes.
const SECONDS: usize = 60;
const SAMPLES: usize = 31;
/// A typical client frame: 40 ms at 16 kHz.
const FRAME: usize = 640;

fn slack() -> f64 {
	std::env::var("CANVIST_PERF_SLACK")
		.ok()
		.and_then(|value| value.parse().ok())
		.unwrap_or(1.0)
}

fn assert_time(name: &str, measured: Duration, budget: Duration) {
	let allowed = budget.mul_f64(slack());
	eprintln!("{name}: {measured:?} (budget {budget:?}, allowed {allowed:?})");
	assert!(
		measured <= allowed,
		"{name} took {measured:?}, over its {allowed:?} budget"
	);
}

/// Speech-like audio: bursts of a tone over noise, with pauses.
fn speech(seconds: usize) -> Vec<i16> {
	let mut state: u32 = 0x9e37_79b9;

	(0..seconds * 16_000)
		.map(|index| {
			state ^= state << 13;
			state ^= state >> 17;
			state ^= state << 5;
			let noise = f64::from(state % 401) - 200.0;
			let speaking = (index / 8_000) % 3 != 2;
			let phase = 2.0 * std::f64::consts::PI * 180.0 * index as f64 / 16_000.0;
			let voice = if speaking { 6_000.0 * phase.sin() } else { 0.0 };
			(voice + noise) as i16
		})
		.collect()
}

/// The median time of `run`, divided by the seconds of audio it processed.
fn per_second_of_audio(mut run: impl FnMut()) -> Duration {
	let mut times: Vec<Duration> = (0..SAMPLES)
		.map(|_| {
			let started = Instant::now();
			run();
			started.elapsed()
		})
		.collect();
	times.sort();

	times[SAMPLES / 2] / SECONDS as u32
}

#[test]
#[ignore = "performance budget: run in release with --ignored"]
fn voice_activity_detection_per_second_of_audio() {
	let audio = speech(SECONDS);

	let measured = per_second_of_audio(|| {
		let mut vad = Vad::new();
		let mut turns = 0;

		for frame in audio.chunks(FRAME) {
			turns += usize::from(vad.push(black_box(frame)));
		}

		black_box(turns);
	});

	assert_time(
		"VAD, per second of audio",
		measured,
		Duration::from_micros(50),
	);
}

#[test]
#[ignore = "performance budget: run in release with --ignored"]
fn resampling_to_24_khz_per_second_of_audio() {
	let audio = speech(SECONDS);
	let mut output = Vec::with_capacity(FRAME * 2);

	let measured = per_second_of_audio(|| {
		let mut resampler = Resampler::new(16_000, 24_000);

		for frame in audio.chunks(FRAME) {
			output.clear();
			resampler.process(black_box(frame), &mut output);
			black_box(&output);
		}
	});

	assert_time(
		"Resampling 16 to 24 kHz, per second of audio",
		measured,
		Duration::from_micros(150),
	);
}
