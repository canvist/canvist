//! Audio as the protocol carries it: 16-bit little-endian PCM, mono.
//!
//! Clients always send 16 kHz. Providers that want another rate get it
//! through a [`Resampler`], which keeps its position between frames so a
//! stream resampled frame by frame matches one resampled whole.

use canvist_dictation::BYTES_PER_SAMPLE;

/// Decode little-endian PCM16 `bytes` into `samples`, replacing their
/// contents. Returns `false`, leaving `samples` empty, when `bytes` holds a
/// partial sample.
pub fn decode_pcm16(bytes: &[u8], samples: &mut Vec<i16>) -> bool {
	samples.clear();

	if !bytes.len().is_multiple_of(BYTES_PER_SAMPLE) {
		return false;
	}

	samples.extend(
		bytes
			.chunks_exact(BYTES_PER_SAMPLE)
			.map(|pair| i16::from_le_bytes([pair[0], pair[1]])),
	);

	true
}

/// Encode `samples` as little-endian PCM16.
pub fn encode_pcm16(samples: &[i16]) -> Vec<u8> {
	samples
		.iter()
		.flat_map(|sample| sample.to_le_bytes())
		.collect()
}

/// A streaming linear-interpolation resampler between two integer rates.
///
/// Linear interpolation is enough for speech models: they band-limit their
/// input themselves, and the relay only ever upsamples (16 to 24 kHz), where
/// it cannot alias.
#[derive(Clone, Debug)]
pub struct Resampler {
	/// Input samples per step, after dividing both rates by their GCD.
	step: u64,
	/// Output samples per step.
	scale: u64,
	/// Where the next output sample falls, in input samples times `scale`,
	/// counted from `previous`.
	position: u64,
	/// The last input sample of the previous frame; interpolation needs one
	/// sample of history across frame boundaries.
	previous: Option<i16>,
}

impl Resampler {
	/// A resampler from `from` hertz to `to` hertz. Both must be non-zero.
	pub fn new(from: u32, to: u32) -> Self {
		let divisor = gcd(u64::from(from), u64::from(to)).max(1);

		Self {
			step: u64::from(from) / divisor,
			scale: u64::from(to) / divisor,
			position: 0,
			previous: None,
		}
	}

	/// Resample `input`, appending the result to `output`.
	pub fn process(&mut self, input: &[i16], output: &mut Vec<i16>) {
		if self.step == self.scale {
			output.extend_from_slice(input);

			return;
		}

		let Some(&last) = input.last() else {
			return;
		};

		// The frame as interpolation sees it: the previous frame's last
		// sample at index 0, when there is one, then this frame.
		let offset = usize::from(self.previous.is_some());
		let available = (input.len() + offset) as u64;
		let sample = |index: u64| -> i64 {
			let index = index as usize;

			match (self.previous, index.checked_sub(offset)) {
				(Some(previous), None) => i64::from(previous),
				(_, Some(index)) => i64::from(input[index]),
				(None, None) => 0,
			}
		};

		output.reserve((input.len() as u64 * self.scale / self.step) as usize + 1);

		while self.position / self.scale + 1 < available {
			let index = self.position / self.scale;
			let fraction = (self.position % self.scale) as i64;
			let from = sample(index);
			let to = sample(index + 1);
			let interpolated = from + (to - from) * fraction / self.scale as i64;
			output.push(interpolated as i16);
			self.position += self.step;
		}

		// Re-anchor on this frame's last sample, which becomes index 0.
		self.position -= (available - 1) * self.scale;
		self.previous = Some(last);
	}
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
	while right != 0 {
		(left, right) = (right, left % right);
	}

	left
}

#[cfg(test)]
mod tests {
	use super::*;

	const CLIENT_RATE: u32 = 16_000;
	const OPENAI_RATE: u32 = 24_000;

	#[test]
	fn pcm16_round_trips() {
		let samples = [0, 1, -1, i16::MAX, i16::MIN, 12_345];
		let mut decoded = Vec::new();

		assert!(decode_pcm16(&encode_pcm16(&samples), &mut decoded));
		assert_eq!(decoded, samples);
	}

	#[test]
	fn partial_samples_are_refused() {
		let mut decoded = vec![1, 2];

		assert!(!decode_pcm16(&[0, 1, 2], &mut decoded));
		assert!(decoded.is_empty());
	}

	#[test]
	fn equal_rates_pass_through() {
		let mut resampler = Resampler::new(CLIENT_RATE, CLIENT_RATE);
		let mut output = Vec::new();
		resampler.process(&[1, 2, 3], &mut output);

		assert_eq!(output, [1, 2, 3]);
	}

	#[test]
	fn upsampling_interpolates_a_ramp() {
		let mut resampler = Resampler::new(CLIENT_RATE, OPENAI_RATE);
		let mut output = Vec::new();
		resampler.process(&[0, 300, 600, 900, 1200], &mut output);

		assert_eq!(output, [0, 200, 400, 600, 800, 1000]);
	}

	#[test]
	fn one_second_becomes_one_second() {
		let mut resampler = Resampler::new(CLIENT_RATE, OPENAI_RATE);
		let mut output = Vec::new();
		let level = 7;
		resampler.process(&vec![level; CLIENT_RATE as usize], &mut output);

		// The last output waits for the next input sample to interpolate.
		assert!(
			(23_998..=24_000).contains(&output.len()),
			"{}",
			output.len()
		);
		assert!(output.iter().all(|&sample| sample == 7));
	}

	#[test]
	fn frame_by_frame_matches_whole() {
		// A sawtooth, so every interpolated sample differs.
		let input: Vec<i16> = (0..1_000_i32)
			.map(|index| (index * 37 % 2_000 - 1_000) as i16)
			.collect();
		let mut whole = Vec::new();
		Resampler::new(CLIENT_RATE, OPENAI_RATE).process(&input, &mut whole);

		let mut framed = Vec::new();
		let mut resampler = Resampler::new(CLIENT_RATE, OPENAI_RATE);

		let odd_frame = 7;

		for frame in input.chunks(odd_frame) {
			resampler.process(frame, &mut framed);
		}

		assert_eq!(framed, whole);
	}

	#[test]
	fn single_sample_frames_still_progress() {
		let mut resampler = Resampler::new(CLIENT_RATE, OPENAI_RATE);
		let mut output = Vec::new();

		for sample in [0, 300, 600] {
			resampler.process(&[sample], &mut output);
		}

		assert_eq!(output, [0, 200, 400]);
	}
}
