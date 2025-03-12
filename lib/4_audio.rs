// may have to really control
// the frames somehow updating
// providing audio frames in time
// with nannou drawing
#[unsafe(no_mangle)]
pub extern "C" fn transform(
	x: f32, 
	y: f32, 
	t: f32, 
	fft: *const std::ffi::c_void, 
	fft_len: usize, 
	fft_buf: *const std::ffi::c_void,
	fft_buf_len: usize
) -> f32 {
	let fft = unsafe { std::slice::from_raw_parts(fft as *const (f32, f32), fft_len) };
	let fft_buf = unsafe { std::slice::from_raw_parts(fft_buf as *const (f32, f32), fft_buf_len) };

	// magnitudes are huge coming from fft_data
	// lets make it a usable number for our situation
	// can noise clamp be controllable?
	const LOWER_LIMIT: f32 = 200.0;
	const MIDLOW_LIMIT: f32 = 1000.0;
	const MIDHI_LIMIT: f32 = 5000.0;
	const HI_LIMIT: f32 = 8000.0;
	const MAG_DIVISOR: f32 = 3000.0;

	let mut low_mag: f32 = 0.0;
	let mut mid_mag: f32 = 0.0;
	let mut hi_mag: f32 = 0.0;

	// let mut low_mag = fft.iter()
	// 	.map(|&(f, m)| (f, m / MAG_DIVISOR))
	// 	.find(|&(f, _)| f >= FREQ_AVERAGE)
	// 	.and_then(|(_, m)| (m > NOISE_CLAMP).then_some(m))
	// 	.unwrap_or(0.0);

	// still not sure what I'm doing here yet....
	for (f, (_, m)) in fft.iter().map(|(f, _)| f).zip(fft_buf.iter()) {
		 //println!("mag of {f} {}", m / 100000.0);
		// println!("{f:.2}Hz => {}", "|".repeat(((m * f).sqrt() / 10000.0) as usize));
	
		// if *f <= 7000.0 && *f >= 6000.0 {

		// println!("{}", m / 10000000.0);
		// low
		if *f >= 340.0 && *f <= 500.0
		{ 
			// println!("{f}");
			// let calc = m / 10000000.0;
			// if calc >= NOISE_CLAMP  {
			// }
				let mut val = (m * f).sqrt() / MAG_DIVISOR; 
				val -= 30.0;	
				

				low_mag = val.floor();
				// println!("{low_mag}");
				break;
		}
	}

	// can't get around the noise - not sure what to do with this yet
	// if low_mag < 101.0 { low_mag = 0.0 }
	(y - low_mag) * (x + low_mag) * t / 1.0
	// (y) * (x) * (t)
}
