use rustfft::{Fft, FftPlanner};
use rustfft::num_complex::Complex;

use std::cmp::Ordering;
use std::sync::Arc;

pub struct AudioProcessor {
	pub buffer: Vec<f32>,
	pub buffer_size: usize,
	fft: Arc<dyn Fft<f32>>,
}

impl AudioProcessor {
	pub fn new(sample_rate: usize, frame_rate: f32) -> Self {
		let buffer_size = (sample_rate as f32 / frame_rate).ceil() as usize;
		let mut planner: FftPlanner<f32> = FftPlanner::new();
		let fft = planner.plan_fft_forward(buffer_size);

		Self {
			buffer: vec![0.0; buffer_size],
			buffer_size,
			fft,
		}
	}

	pub fn add_samples(&mut self, samples: &[f32]) {
		self.buffer.extend_from_slice(samples);

		// deal with possible race condition of the sketch
		// update happening and requesting data before buffer is full.
		// fft buffer may be too small before processing it
		match self.buffer.len().cmp(&self.buffer_size) {
			Ordering::Greater => {
				self.buffer.drain(0..(self.buffer.len() - self.buffer_size));
			},
			Ordering::Less => {
				while self.buffer.len() < self.buffer_size {
					self.buffer.push(0.0);
				}
			},
			_ => {},
		}
	}

	pub fn get_magnitudes(&self, decay: f32) -> Vec<f32> {
		let mut complex_input: Vec<Complex<f32>> =
			self.buffer.iter().map(|&x| Complex::new(x, 0.0)).collect();

		self.fft.process(&mut complex_input);

		// keep state on next call to this func
		static mut DECAY_BUF: [f32; 1024] = [0.0; 1024];

		let mut mags = complex_input
			.iter().map(|c| {
				let mag = c.norm() / complex_input.len() as f32;
				20.0 * (mag.max(1e-8)).log10()
			})
			.collect::<Vec<f32>>();

		// add decay
		mags.iter_mut().zip(unsafe { DECAY_BUF.iter_mut() }).for_each(|(curr_frame, prev_frame)| {
				// if the current sample is 'louder' than previous sample then apply the decay factor
				if *curr_frame > *prev_frame {
					*curr_frame = *prev_frame * decay;
				}

				// now set our static buffer frame to the current one for the next sample check
				*prev_frame = *curr_frame;
			});

		mags
	}
}
