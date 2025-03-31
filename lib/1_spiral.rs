#[unsafe(no_mangle)]
pub static SAT_MOD: f32 = 100.0;

#[unsafe(no_mangle)]
pub extern "C" fn transform(x: f32, y: f32, t: f32, ) -> f32 {
    (y * 1.0) * (x * 1.0) * t
}
