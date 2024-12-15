use console_engine::{pixel, Color, KeyCode, ConsoleEngine};

// todo: somehow determine width based on current size of terminal character (preference?)
//
const WIDTH: u32 = 80;
const HEIGHT: u32 = 28;

const FPS: u32 = 10;

fn handle_keypress_quit(engine: &ConsoleEngine) -> bool {
    let mut should_quit = false;
    if engine.is_key_pressed(KeyCode::Char('q')) {
        should_quit = true;
    }
    should_quit
}
fn get_color_from_coord(coord: (i32, i32), delta: i32) -> Color {
    let mut ret = Color::Black;
    if coord.0 % 2 == 0 && coord.1 % delta == 0 {
        ret = Color::Green;
    }
    else if coord.0 % delta == 0 && coord.1 % 3 == 1 {
        ret = Color::Cyan;
    }
    else if coord.0 % delta / 6 == 1 && coord.1 / delta * 2 == 0 {
        ret = Color::Blue;
    }
    ret
}

fn draw_stuff() {

    let mut engine = ConsoleEngine::init(
        // dimensions
        WIDTH, HEIGHT,
        FPS
    ).unwrap();

    let mut delta: i32 = 0;

    loop {
        delta += 1;
        if delta > (HEIGHT as i32) / 2 { delta = 1 }
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
                            j as i32),
                            delta
                        )
                    )
                );
            }
        }

        engine.print(
            0,0,
            format!("delta counter: {}",
                delta.to_string()).as_str());

        engine.print(1,1,
            "press q to quit");

        if handle_keypress_quit(&engine) { break; }

        engine.draw();
    }
}

fn main() {

    draw_stuff();

}
