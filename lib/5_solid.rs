#[unsafe(no_mangle)]
pub static TIME_DIVISOR: f32 = 100.0;

#[unsafe(no_mangle)]
pub static TIME_DIALATION_RANGE: f32 = 5.0;

#[unsafe(no_mangle)]
pub static INTENSITY_RANGE: f32 = 10.0;

#[unsafe(no_mangle)]
pub static LUM_MOD: f32 = 100.0;

#[unsafe(no_mangle)]
pub extern "C" fn transform(x: f32, y: f32, t: f32) -> f32 {
	(x % 2.0 + 1000.0) / (y % 2.0 + 1000.0) * t
}
