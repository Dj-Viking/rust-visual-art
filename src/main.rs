use console_engine::{pixel, Color, KeyCode, ConsoleEngine};

// todo: somehow determine width based on current size of terminal character (preference?)
//
const WIDTH: u32 = 40;
const HEIGHT: u32 = 30;

const FPS: u32 = 10;

fn handle_keypress_quit(engine: &ConsoleEngine) -> bool {
    let mut should_quit = false;
    if engine.is_key_pressed(KeyCode::Char('q')) {
        should_quit = true;
    }
    should_quit
}

fn handle_keypress_interactive(engine: &ConsoleEngine) -> i32 {
    if engine.is_key_pressed(KeyCode::Char('9')) {
        return 1;
    }
    if engine.is_key_pressed(KeyCode::Char('0')) {
        return -1;
    }
    0
}

fn v2_pattern(coord: (i32, i32), delta: i32, adjust: i32) -> () {
    // 32 / (countRef / (row + 1)) +
    // (col + 1) / ((row + 1) / (col + 1) - Number(animVarCoeff) / countRef) +
    // countRef * ((col + 1) * 0.05);
}
// chance to overflow with multiply :( maybe have to use 64 
fn spiral_pattern(coord: (i32, i32), delta: i32, adjust: i32) -> Color {
    // THIS IS THE SPIRAL PATTERN! add delta to animate overtime!
    //const num = row * 8 * col * Number(animVarCoeff) + countRef;
    //
    let col = coord.1;
    let row = coord.0;
    let blue: u8 = (col * (delta * adjust) * row * (adjust * 2)) as u8;
    let red: u8 = (20) as u8;
    let green: u8 = (10) as u8;

    let rgb_vals = (red,green,blue);
    let rgb = Color::from(rgb_vals);
    rgb
}

fn draw_stuff() {

    let mut engine = ConsoleEngine::init(
        // dimensions
        WIDTH, HEIGHT,
        FPS
    ).unwrap();

    let mut delta: i32 = 0;
    let mut adjust: i32 = 0;

    loop {
        delta += 1;

        // reset delta after reaches half height of screen
        if delta > 255 { delta = 1 }

        engine.wait_frame();
        engine.clear_screen();

        // collection audio samples snapshot here?

        for col in 0..WIDTH 
        {
            for row in 2..HEIGHT 
            {
                engine.set_pxl(
                    col as i32, row as i32,
                    pixel::pxl_bg(' ', 
                        spiral_pattern(
                            (col as i32,
                            row as i32),
                            delta,
                            adjust
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

        engine.print(28,1,
            format!("adjust runtime value: {}", adjust).as_str());

        engine.print(28,0,
            format!("color at runtime: {:?}", 
                spiral_pattern((20, 20), delta, adjust)
            ).as_str());

        if handle_keypress_quit(&engine) { break; }

        adjust += handle_keypress_interactive(&engine);

        engine.draw();
    }
}

fn main1() {
    let data: u8 = 255;
    println!("hex data => {} => {:x?}", data, data);
}

fn main() {

    draw_stuff();

}
