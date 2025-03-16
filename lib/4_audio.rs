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
	const LOW_LOWER_LIMIT: f32 = 340.0;
	const HI_LOWER_LIMIT: f32 = 500.0;
	const MIDLOW_LIMIT: f32 = 1000.0;
	const MIDHI_LIMIT: f32 = 5000.0;
	const HI_LIMIT: f32 = 8000.0;
	const MAG_DIVISOR: f32 = 10000.0;

	let mut low_mag: f32 = 0.0;
	let mut mid_mag: f32 = 0.0;
	let mut hi_mag: f32 = 0.0;

	// still not sure what I'm doing here yet....
	for (f, (_, m)) in fft.iter().map(|(f, _)| f).zip(fft_buf.iter()) {

		// low
		if low_mag == 0.0 {
			if *f >= 340.0 && *f <= 500.0
			{ 
				let mut val = (m * f).sqrt() / MAG_DIVISOR; 
				// val -= 300.0;	

				// if val < 0.0 { low_mag = 0.0 } else { 
				// 	low_mag = val.floor();
				// }

				low_mag = val;
				println!("{low_mag}");

				continue;
			}
		}

		// mid
		if mid_mag == 0.0 {
			if *f >= MIDLOW_LIMIT && *f <= MIDHI_LIMIT 
			{
				let mut val = (m * f).sqrt() / MAG_DIVISOR; 
				//val -= 300.0;
				

				// if val < 0.0 { mid_mag = 0.0 } else { 
				// 	mid_mag = val.floor();
				// }

				mid_mag = val.floor();
				//println!("{mid_mag}");
				continue;

			}
		}

		if low_mag != 0.0 && mid_mag != 0.0 { break; }

	}

	// can't get around the noise - not sure what to do with this yet
	// if low_mag < 101.0 { low_mag = 0.0 }
	x * (y + low_mag) * (t * 1.0)
}
