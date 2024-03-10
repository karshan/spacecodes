use num_traits::Num;

pub struct Rect<T: Num> {
    pub x: T,
    pub y: T,
    pub w: T,
    pub h: T,
}

impl<T: Num + PartialOrd + Copy> Rect<T> {
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
}