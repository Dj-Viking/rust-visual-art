use console_engine::pixel;
use console_engine::Color;
use console_engine::KeyCode;
fn main() {

    let mut engine = console_engine::ConsoleEngine::init(20, 20, 3).unwrap();

    loop {
        engine.wait_frame();
        engine.clear_screen();

        engine.print(0,0,
            "press q to quit");

        for i in 1..19 {
            for j in 1..19 {
                engine.set_pxl(
                    i,j,
                    pixel::pxl_fg(
                        '0',
                        Color::Yellow));
            }
        }

        if engine.is_key_pressed(
            KeyCode::Char('q')) 
        {
            break;
        }
        engine.draw();
    }
}
