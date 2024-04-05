use std::f32::EPSILON;

use num_traits::{AsPrimitive, Num};
use raylib::prelude::Vector2;

#[derive(Copy, Clone)]
pub struct Rect<T: Num> {
    pub x: T,
    pub y: T,
    pub w: T,
    pub h: T,
}

// This is identitical to raylib's implementation. However raylib::check_collision_lines() returns bogus values on MacOS
pub fn check_collision_lines(a0: &Vector2, a1: &Vector2, b0: &Vector2, b1: &Vector2) -> bool {
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

// from raylib, copied because of potential MacOS issue
pub fn collision_circle_rect(center: &Vector2, radius: f32, in_rect: &Rect<i32>) -> bool
{
    let rect: Rect<f32> = in_rect.into_f32();
    let rect_cen_x: f32 = rect.x + rect.w/2.0f32;
    let rect_cen_y: f32 = rect.y + rect.h/2.0f32;

    let dx: f32 = (center.x - rect_cen_x).abs();
    let dy: f32 = (center.y - rect_cen_y).abs();

    if dx > (rect.w/2.0f32 + radius) { return false; }
    if dy > (rect.h/2.0f32 + radius) { return false; }

    if dx <= (rect.w/2.0f32) { return true; }
    if dy <= (rect.h/2.0f32) { return true; }

    let corner_sq_dist: f32 = (dx - rect.w/2.0f32) * (dx - rect.w/2.0f32) +
                                (dy - rect.h/2.0f32) * (dy - rect.h/2.0f32);

    corner_sq_dist <= (radius * radius)
}

impl<T: Num + PartialOrd + Copy + AsPrimitive<f32>> Rect<T> {
    pub fn into_f32(self: &Rect<T>) -> Rect<f32> {
        Rect {
            x: self.x.as_(),
            y: self.y.as_(),
            w: self.w.as_(),
            h: self.h.as_(),
        }
    }
    pub fn contains(self: &Rect<T>, child: &Rect<T>) -> bool {
        child.x >= self.x && child.x + child.w <= self.x + self.w &&
            child.y >= self.y && child.y + child.h <= self.y + self.h
    }

    pub fn contains_point(self: &Rect<T>, p: &Vector2) -> bool {
        p.x >= self.x.as_() && p.x <= self.x.as_() + self.w.as_() &&
        p.y >= self.y.as_() && p.y <= self.y.as_() + self.h.as_()
    }

    // the contains_point checks are unnecessary, they can potentially provide short circuit benefit
    pub fn collide_line(self: &Rect<T>, p1: &Vector2, p2: &Vector2) -> bool {
        self.contains_point(p1) || self.contains_point(p2) ||
            self.lines().iter().any(|l| check_collision_lines(&l[0], &l[1], p1, p2))
    }

    pub fn collide(self: &Rect<T>, other: &Rect<T>) -> bool {
        let t = self.y;
        let b = t + self.h;
        let l = self.x;
        let r = l + self.w;
        let tt = other.y;
        let bb = tt + other.h;
        let ll = other.x;
        let rr = ll + other.w;
        !(b < tt || t > bb || l > rr || r < ll) 
    }

    pub fn center(self: &Rect<T>) -> Vector2 {
        Vector2::new(self.x.as_(), self.y.as_()) + Vector2::new(self.w.as_(), self.h.as_()).scale_by(0.5f32)
    }

    pub fn lines(self: &Rect<T>) -> [[Vector2; 2]; 4] {
        [[Vector2 { x: self.x.as_(), y: self.y.as_() }, Vector2 { x: (self.x + self.w).as_(), y: self.y.as_() }], // top
        [Vector2 { x: self.x.as_(), y: self.y.as_() }, Vector2 { x: self.x.as_(), y: (self.y + self.h).as_() }], // left
        [Vector2 { x: self.x.as_(), y: (self.y + self.h).as_() }, Vector2 { x: (self.x + self.w).as_(), y: (self.y + self.h).as_() }], // bottom
        [Vector2 { x: (self.x + self.w).as_(), y: self.y.as_() }, Vector2 { x: (self.x + self.w).as_(), y: (self.y + self.h).as_() }]] // right
    }

    pub fn size(self: &Rect<T>) -> Vector2 {
        let rf32 = self.into_f32();
        Vector2::new(rf32.w, rf32.h)
    }
}