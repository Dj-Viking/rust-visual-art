#[unsafe(no_mangle)]
pub static INTENSITY_RANGE: f32 = 10.0;
#[unsafe(no_mangle)]
pub static LUM_MOD: f32 = 100.0;

#[unsafe(no_mangle)]
pub extern "C" fn transform(x: f32, y: f32, t: f32) -> f32 {
	32.0 / (t / x) + y / (x / y - 1.0 / t) + t * (y * 0.05)
}
