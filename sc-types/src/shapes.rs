use num_traits::{AsPrimitive, Num};
use raylib::prelude::Vector2;

#[derive(Copy, Clone)]
pub struct Rect<T: Num> {
    pub x: T,
    pub y: T,
    pub w: T,
    pub h: T,
}

pub fn contains_point(r: &Rect<i32>, p: &Vector2) -> bool {
    let px = p.x.round() as i32;
    let py = p.y.round() as i32;
    px >= r.x && px <= r.x + r.w &&
        py >= r.y && py <= r.y + r.h
}

impl<T: Num + PartialOrd + Copy + AsPrimitive<f32>> Rect<T> {
    pub fn contains(self: &Rect<T>, child: &Rect<T>) -> bool {
        child.x >= self.x && child.x + child.w <= self.x + self.w &&
            child.y >= self.y && child.y + child.h <= self.y + self.h
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

    pub fn lines(self: &Rect<T>) -> [[Vector2; 2]; 4] {
        [[Vector2 { x: self.x.as_(), y: self.y.as_() }, Vector2 { x: (self.x + self.w).as_(), y: self.y.as_() }], // top
        [Vector2 { x: self.x.as_(), y: self.y.as_() }, Vector2 { x: self.x.as_(), y: (self.y + self.h).as_() }], // left
        [Vector2 { x: self.x.as_(), y: (self.y + self.h).as_() }, Vector2 { x: (self.x + self.w).as_(), y: (self.y + self.h).as_() }], // bottom
        [Vector2 { x: (self.x + self.w).as_(), y: self.y.as_() }, Vector2 { x: (self.x + self.w).as_(), y: (self.y + self.h).as_() }]] // right
    }
}