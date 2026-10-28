//! Voice activity detection: hearing when a speaker finishes a turn.
//!
//! Providers without their own turn detection (OpenAI with
//! `turn_detection: null`, vLLM) need the relay to say when a segment ends.
//! An energy detector is enough for dictation, where one speaker talks close
//! to the microphone: a 20 ms frame is speech when it is clearly louder than
//! the noise floor, and a turn ends after a stretch of silence that follows
//! speech.
//!
//! The noise floor drops at once to any quieter frame, follows frames that
//! are louder but not speech, and creeps up during speech. Creeping up lets
//! it learn a room that starts loud, or gets louder, within about two
//! seconds; real speech dips to the floor between words long before that,
//! so the floor never climbs to the level of speech.

use canvist_dictation::SAMPLE_RATE;

/// Samples in one 20 ms analysis frame.
pub const FRAME_SAMPLES: usize = SAMPLE_RATE as usize / 50;
/// The length of one analysis frame, in milliseconds.
const FRAME_MILLIS: u32 = 20;
/// Silence after speech that ends a turn.
pub const END_OF_TURN_MILLIS: u32 = 700;
/// How far above the noise floor a frame must be to count as speech: 10 dB
/// in energy.
const SPEECH_OVER_FLOOR: f64 = 10.0;
/// The quietest frame that can be speech, as mean square energy: an RMS of
/// 150 out of 32,768, about -47 dBFS. Below it, everything is silence.
const MIN_SPEECH_ENERGY: f64 = 150.0 * 150.0;
/// The noise floor a stream starts with: an RMS of 30, a quiet room.
const INITIAL_FLOOR: f64 = 30.0 * 30.0;
/// How fast the floor rises per speech frame, as an energy ratio: about
/// 10 dB per second.
const SPEECH_FLOOR_RISE: f64 = 1.05;
/// How far the floor moves toward a louder frame that is not speech.
const NOISE_TRACKING: f64 = 0.1;
/// The lowest the floor goes, so digital silence keeps it positive.
const MIN_FLOOR: f64 = 1.0;
/// Consecutive speech frames before a turn starts, so clicks and taps do
/// not count.
const SPEECH_ONSET_FRAMES: u32 = 3;

/// An energy-based voice activity detector over 16 kHz PCM16.
#[derive(Clone, Debug)]
pub struct Vad {
	/// Samples of the frame being filled.
	frame: [i16; FRAME_SAMPLES],
	/// How many samples of `frame` are filled.
	filled: usize,
	/// The noise floor, as mean square energy.
	floor: f64,
	/// Consecutive speech frames.
	speech_frames: u32,
	/// Consecutive silent frames.
	silent_frames: u32,
	/// Whether speech was heard since the last turn ended.
	in_turn: bool,
}

impl Default for Vad {
	fn default() -> Self {
		Self::new()
	}
}

impl Vad {
	/// A detector that has heard nothing yet.
	pub fn new() -> Self {
		Self {
			frame: [0; FRAME_SAMPLES],
			filled: 0,
			floor: INITIAL_FLOOR,
			speech_frames: 0,
			silent_frames: 0,
			in_turn: false,
		}
	}

	/// Feed `samples` of any length. Returns whether a turn ended inside
	/// them.
	pub fn push(&mut self, mut samples: &[i16]) -> bool {
		let mut ended = false;

		while !samples.is_empty() {
			let take = (FRAME_SAMPLES - self.filled).min(samples.len());
			self.frame[self.filled..self.filled + take].copy_from_slice(&samples[..take]);
			self.filled += take;
			samples = &samples[take..];

			if self.filled == FRAME_SAMPLES {
				self.filled = 0;
				ended |= self.analyse_frame();
			}
		}

		ended
	}

	/// Whether the speaker is mid-turn: speech was heard and the silence
	/// since is shorter than a turn's end.
	pub fn in_turn(&self) -> bool {
		self.in_turn
	}

	/// Classify the full frame and advance the turn. Returns whether the
	/// turn ended on this frame.
	fn analyse_frame(&mut self) -> bool {
		let energy = mean_square(&self.frame);
		let is_speech = energy >= MIN_SPEECH_ENERGY && energy >= self.floor * SPEECH_OVER_FLOOR;

		if is_speech {
			self.floor *= SPEECH_FLOOR_RISE;
			self.speech_frames += 1;
			self.silent_frames = 0;
			self.in_turn |= self.speech_frames >= SPEECH_ONSET_FRAMES;

			return false;
		}

		self.floor = if energy < self.floor {
			energy.max(MIN_FLOOR)
		} else {
			self.floor + (energy - self.floor) * NOISE_TRACKING
		};

		self.speech_frames = 0;
		self.silent_frames += 1;

		if self.in_turn && self.silent_frames * FRAME_MILLIS >= END_OF_TURN_MILLIS {
			self.in_turn = false;

			return true;
		}

		false
	}
}

fn mean_square(frame: &[i16]) -> f64 {
	let sum: f64 = frame
		.iter()
		.map(|&sample| {
			let sample = f64::from(sample);
			sample * sample
		})
		.sum();

	sum / frame.len() as f64
}

#[cfg(test)]
mod tests {
	use super::*;

	const RATE: f64 = SAMPLE_RATE as f64;
	/// A voice's pitch.
	const PITCH_HZ: f64 = 220.0;
	/// Speech close to the microphone: a quarter of full scale.
	const VOICE: f64 = 8_000.0;
	/// A noisy room's peak amplitude, about -33 dBFS RMS.
	const ROOM: i32 = 1_200;
	const SPEECH_MILLIS: u32 = 1_000;
	const PAUSE_MILLIS: u32 = 1_000;

	fn samples(millis: u32) -> usize {
		(RATE * f64::from(millis) / 1_000.0) as usize
	}

	fn tone(millis: u32, amplitude: f64) -> Vec<i16> {
		(0..samples(millis))
			.map(|index| {
				let phase = std::f64::consts::TAU * PITCH_HZ * index as f64 / RATE;
				(amplitude * phase.sin()) as i16
			})
			.collect()
	}

	fn silence(millis: u32) -> Vec<i16> {
		vec![0; samples(millis)]
	}

	/// Deterministic white noise with peak `amplitude`, from a xorshift
	/// generator.
	fn noise(millis: u32, amplitude: i32) -> Vec<i16> {
		let mut state: u32 = 0x2545_f491;
		let span = 2 * amplitude.unsigned_abs() + 1;

		(0..samples(millis))
			.map(|_| {
				state ^= state << 13;
				state ^= state >> 17;
				state ^= state << 5;
				((state % span) as i32 - amplitude) as i16
			})
			.collect()
	}

	/// The milliseconds into `audio`, fed in 20 ms frames, at which turns
	/// end.
	fn turn_ends(vad: &mut Vad, audio: &[i16]) -> Vec<u32> {
		audio
			.chunks(FRAME_SAMPLES)
			.enumerate()
			.filter(|(_, frame)| vad.push(frame))
			.map(|(index, _)| (index as u32 + 1) * FRAME_MILLIS)
			.collect()
	}

	#[test]
	fn silence_never_ends_a_turn() {
		let mut vad = Vad::new();

		assert!(turn_ends(&mut vad, &silence(5 * PAUSE_MILLIS)).is_empty());
		assert!(!vad.in_turn());
	}

	#[test]
	fn a_pause_after_speech_ends_the_turn() {
		let mut vad = Vad::new();
		let audio = [tone(SPEECH_MILLIS, VOICE), silence(PAUSE_MILLIS)].concat();

		assert_eq!(
			turn_ends(&mut vad, &audio),
			[SPEECH_MILLIS + END_OF_TURN_MILLIS]
		);
	}

	#[test]
	fn short_gaps_between_words_do_not() {
		let mut vad = Vad::new();
		let word = tone(300, VOICE);
		let audio = [&word[..], &silence(250), &word, &silence(400), &word].concat();

		assert!(turn_ends(&mut vad, &audio).is_empty());
		assert!(vad.in_turn());
	}

	#[test]
	fn clicks_are_not_speech() {
		let mut vad = Vad::new();
		let click = tone(2 * FRAME_MILLIS, 2.5 * VOICE);
		let audio = [click, silence(PAUSE_MILLIS)].concat();

		assert!(turn_ends(&mut vad, &audio).is_empty());
	}

	#[test]
	fn quiet_speech_is_ignored() {
		let mut vad = Vad::new();
		let whisper = VOICE / 80.0;
		let audio = [tone(SPEECH_MILLIS, whisper), silence(PAUSE_MILLIS)].concat();

		assert!(turn_ends(&mut vad, &audio).is_empty());
	}

	#[test]
	fn the_floor_adapts_to_room_noise() {
		let mut vad = Vad::new();

		// Steady noise above the initial floor settles into the floor rather
		// than reading as one endless turn.
		turn_ends(&mut vad, &noise(3 * PAUSE_MILLIS, ROOM));
		assert!(!vad.in_turn(), "steady noise is not speech");

		let speech: Vec<i16> = tone(SPEECH_MILLIS, 1.5 * VOICE)
			.iter()
			.zip(noise(SPEECH_MILLIS, ROOM))
			.map(|(tone, noise)| tone.saturating_add(noise))
			.collect();
		let audio = [speech, noise(PAUSE_MILLIS, ROOM)].concat();

		assert_eq!(
			turn_ends(&mut vad, &audio),
			[SPEECH_MILLIS + END_OF_TURN_MILLIS]
		);
	}

	#[test]
	fn frames_of_any_length_are_accepted() {
		let audio = [tone(SPEECH_MILLIS, VOICE), silence(PAUSE_MILLIS)].concat();
		let uneven = FRAME_SAMPLES * 4 - 43;
		let mut framed = Vad::new();
		let mut ended = 0;

		for frame in audio.chunks(uneven) {
			ended += usize::from(framed.push(frame));
		}

		assert_eq!(ended, 1);
		assert!(Vad::new().push(&audio));
	}
}
