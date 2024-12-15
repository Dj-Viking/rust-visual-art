use console_engine::pixel;
use console_engine::Color;
use console_engine::KeyCode;
use console_engine::screen::Screen;

// todo: somehow determine width based on current size of terminal character (preference?)
const WIDTH: u32 = 136;
const HEIGHT: u32 = 40;

const FPS: u32 = 60;

fn get_color_from_coord(coord: (i32, i32)) -> Color {
    let mut ret = Color::Black;
    if coord.0 % 2 == 0 && coord.1 % 3 == 0 {
        ret = Color::Green;
    }
    else if coord.0 % 2 == 0 && coord.1 % 3 == 1 {
        ret = Color::Cyan;
    }
    else if coord.0 % 4 == 0 && coord.1 % 2 == 0 {
        ret = Color::Blue;
    }
    ret
}

fn draw_stuff() {

    let mut engine = console_engine::ConsoleEngine::init(
        // dimensions
        WIDTH, HEIGHT,
        FPS
    ).unwrap();

    let mut thing: i32 = 0;

    loop {
        thing += 1;
        engine.wait_frame();
        engine.clear_screen();

        // collection audio samples snapshot here?
        for i in 0..WIDTH 
        {
            for j in 2..HEIGHT 
            {
                engine.set_pxl(
                    i as i32, j as i32,
                    pixel::pxl_bg(' ', 
                        get_color_from_coord(
                            (i as i32,
                            j as i32)
                        )
                    )
                );
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
        //     pixel::pxl_bg(' ', Color::Red)
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
