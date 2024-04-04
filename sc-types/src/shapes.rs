use num_traits::{AsPrimitive, Num};
use raylib::prelude::Vector2;

#[derive(Copy, Clone)]
pub struct Rect<T: Num> {
    pub x: T,
    pub y: T,
    pub w: T,
    pub h: T,
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
}