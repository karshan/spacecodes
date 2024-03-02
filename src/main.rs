use raylib::prelude::*;

fn move_(pos: Vector2, target: Vector2, speed: f32) -> Vector2 {
    let delta = target - pos;
    if delta.length_sqr() < speed * speed { 
        target
    } else { 
        pos + delta.normalized().scale_by(speed)
    }
}

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(640, 480)
        .title("Space Codes")
        .build();
    rl.set_target_fps(60);

    let mut msg = Vector2 { x: 10.0, y: 10.0 };
    let mut target = None;

    while !rl.window_should_close() {
        if rl.is_mouse_button_released(MouseButton::MOUSE_LEFT_BUTTON) {
            target = Some(rl.get_mouse_position());
        }

        msg = match target {
            Some(t) => move_(msg, t, 1.0),
            None => msg,
        };

        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);
        d.draw_rectangle_v(msg, Vector2 { x: 10.0, y: 10.0 }, Color::BLACK);
    }
}