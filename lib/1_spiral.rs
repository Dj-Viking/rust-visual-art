#[unsafe(no_mangle)]
pub extern "C" fn transform(x: f32, y: f32, t: f32, _: *const std::ffi::c_void, _: usize) -> f32 {
	y * x * t
}
