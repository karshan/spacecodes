use sc_types::shapes::*;
use raylib::prelude::*;

pub fn path_collides(rects: &[Rect<i32>], offsets: [Vector2; 4], pos: Vector2, target: Vector2) -> bool {
    let mut collided = false;
    for r in rects {
        for l in r.lines() {
            for o in offsets {
                if let Some(_) = raylib::check_collision_lines(pos + o, target + o, l[0], l[1]) {
                    collided = true;
                    break;
                }
            }
            if collided { break; }
        }
        if collided { break; }
    }
    collided
}