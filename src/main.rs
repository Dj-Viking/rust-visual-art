use console_engine::pixel;
use console_engine::Color;
use console_engine::KeyCode;
use console_engine::screen::Screen;

fn draw_stuff() {
    let mut engine = console_engine::ConsoleEngine::init(
        21, 21, 60
    ).unwrap();

    let mut thing: i32 = 0;
    loop {
        thing += 1;
        engine.wait_frame();
        engine.clear_screen();

        engine.print(1,1,
            "press q to quit");

        for i in 2..20 
        {
            for j in 2..20 
            {
                engine.set_pxl(
                    i,j,
                    pixel::pxl_bg(
                        ' ',
                        Color::Black));
            }
        }
        // draw # around perimeter of 'screen'
        engine.rect(
            2,2, 
            20,20,
            pixel::pxl('*'));
        engine.fill_circle(
            7,7,
            4,
            pixel::pxl_bg(' ', Color::Green)
        );

        engine.print(0,0,
            format!("delta counter: {}", thing.to_string()).as_str());

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
