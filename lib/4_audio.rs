#[unsafe(no_mangle)]
pub extern "C" fn transform(x: f32, y: f32, t: f32, fft: *const std::ffi::c_void, fft_len: usize) -> f32 {
	let fft = unsafe { std::slice::from_raw_parts(fft as *const (f32, f32), fft_len) };

	// magnitudes are huge coming from fft_data
	// lets make it a usable number for our situation
	// can noise clamp be controllable?
	const NOISE_CLAMP: f32 = 10.0;
	const FREQ_AVERAGE: f32 = 500.0;
	const MAG_DIVISOR: f32 = 1000000.0;

	let mut magthing = fft.iter()
		.map(|&(f, m)| (f, m / MAG_DIVISOR))
		.find(|&(f, _)| f >= FREQ_AVERAGE)
		.and_then(|(_, m)| (m > NOISE_CLAMP).then_some(m))
		.unwrap_or(0.0);

	// can't get around the noise - not sure what to do with this yet
	if magthing < 101.0 { magthing = 0.0 }
	(y - magthing) * (x + magthing) * t / 100.0
}
