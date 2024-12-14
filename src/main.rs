use console_engine::pixel;
use console_engine::Color;
use console_engine::KeyCode;
use console_engine::screen::Screen;
fn draw_stuff() {
    let mut engine = console_engine::ConsoleEngine::init(20, 20, 3).unwrap();
    loop {
        engine.wait_frame();
        engine.clear_screen();
//('█',
        engine.print(1,0,
            "press q to quit");

        for i in 1..19 
        {
            for j in 1..19 
            {
                engine.set_pxl(
                    i,j,
                    pixel::pxl_fg(
                        '0',
                        Color::Yellow));
            }
        }
        // draw # around perimeter of 'screen'
        engine.rect(
            1,1, 
            19,19,
            pixel::pxl('*'));
        engine.fill_circle(
            5,5,
            3,
            pixel::pxl_fg('█', Color::Green)
        );

        if engine.is_key_pressed(
            KeyCode::Char('q')) 
        {
            break;
        }
        engine.draw();
    }
}
fn draw_shapes() {
    let mut screen = Screen::new(20,11);

    screen.rect(
        0,0, 
        19,10,
        pixel::pxl('#'));

    screen.draw();
}
fn main() {

    draw_stuff();
    //draw_shapes();

}
