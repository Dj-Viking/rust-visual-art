use console_engine::pixel;
use console_engine::Color;
use console_engine::KeyCode;
use console_engine::screen::Screen;

const FPS: u32 = 60;

fn draw_stuff() {
    let mut engine = console_engine::ConsoleEngine::init(
        // dimensions
        136, 40,
        FPS
    ).unwrap();

    let mut thing: i32 = 0;

    loop {
        thing += 1;
        engine.wait_frame();
        engine.clear_screen();

        for i in 0..136 
        {
            for j in 2..40 
            {
                engine.set_pxl(
                    i,j,
                    pixel::pxl_bg(' ', Color::Green));
            }
        }
        // draw * around perimeter of 'screen'
        // engine.rect(
        //     2,2, 
        //     20,20,
        //     pixel::pxl('*'));
        // engine.fill_circle(
        //     7,8,
        //     2,
        //     pixel::pxl_bg(' ', Color::Green)
        // );

        engine.print(
            0,0,
            format!("delta counter: {}",
                thing.to_string()).as_str());

        engine.print(1,1,
            "press q to quit");

        if engine.is_key_pressed(
            KeyCode::Char('q')) 
        {
            break;
        }
        engine.draw();
    }
}

fn main() {

    draw_stuff();

}
