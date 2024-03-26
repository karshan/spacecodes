use std::f32::EPSILON;

use sc_types::shapes::*;
use raylib::prelude::*;

// This is identitical to raylib's implementation. However raylib::check_collision_lines() returns bogus values on MacOS
fn check_collision_lines(a0: Vector2, a1: Vector2, b0: Vector2, b1: Vector2) -> bool {
    let mut collision = false;
    let div = (b1.y - b0.y)*(a1.x - a0.x) - (b1.x - b0.x)*(a1.y - a0.y);

    if div.abs() >= EPSILON {
        collision = true;

        let xi = ((b0.x - b1.x)*(a0.x*a1.y - a0.y*a1.x) - (a0.x - a1.x)*(b0.x*b1.y - b0.y*b1.x))/div;
        let yi = ((b0.y - b1.y)*(a0.x*a1.y - a0.y*a1.x) - (a0.y - a1.y)*(b0.x*b1.y - b0.y*b1.x))/div;

        if  (((a0.x - a1.x).abs() > EPSILON) && (xi < (a0.x.min(a1.x)) || (xi > (a0.x.max(a1.x))))) ||
            (((b0.x - b1.x).abs() > EPSILON) && (xi < (b0.x.min(b1.x)) || (xi > (b0.x.max(b1.x))))) ||
            (((a0.y - a1.y).abs() > EPSILON) && (yi < (a0.y.min(a1.y)) || (yi > (a0.y.max(a1.y))))) ||
            (((b0.y - b1.y).abs() > EPSILON) && (yi < (b0.y.min(b1.y)) || (yi > (b0.y.max(b1.y))))) {
                collision = false;
        }
    }

    return collision;
}

pub fn path_collides(rects: &[Rect<i32>], offsets: [Vector2; 4], pos: Vector2, target: Vector2) -> bool {
    let mut collided = false;
    for r in rects {
        for l in r.lines() {
            for o in offsets {
                if check_collision_lines(pos + o, target + o, l[0], l[1]) {
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