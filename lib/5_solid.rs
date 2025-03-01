#[unsafe(no_mangle)]
pub static TIME_DIVISOR: f32 = 1000.0;

#[unsafe(no_mangle)]
pub extern "C" fn transform(x: f32, y: f32, t: f32, _: *const std::ffi::c_void, _: usize) -> f32 {
	(x % 2.0 + 1000.0) / (y % 2.0 + 1000.0) * t
}
