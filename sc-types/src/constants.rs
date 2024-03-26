use crate::shapes::*;
use raylib::prelude::Vector2;

#[derive(Eq, PartialEq, Hash)]
pub enum AreaEnum {
    P0Spawn,
    P1Spawn,
    P0Station,
    P1Station,
    Blocked
}

pub static START_FUEL: i32 = 3600;
pub static FUEL_LOSS: i32 = 1; // per frame
pub static MSG_FUEL: i32 = 600;
pub static MESSAGE_SIZE: &Vector2 = &Vector2 { x: 20f32, y: 20f32 };
pub static BLINK_COOLDOWN: i32 = 900;
pub static INTERCEPT_RADIUS: f32 = 40f32;
pub static BLINK_RANGE: f32 = 120f32;
pub static KILLS_TO_WIN: u8 = 10;
pub static PLAY_AREA: Rect<i32> = Rect {
    x: 0, y: 0,
    w: 1024, h: 768,
};
pub static GAME_MAP: [(AreaEnum, Rect<i32>); 5] = [
    (AreaEnum::Blocked, Rect {
        x: 328, y: 200,
        w: 368, h: 368
    }),
    (AreaEnum::P0Spawn, Rect {
        x: 477, y: 200,
        w: 70, h: 70
    }),
    (AreaEnum::P0Station, Rect {
        x: 477, y: 498,
        w: 70, h: 70
    }),
    (AreaEnum::P1Spawn, Rect {
        x: 626, y: 349,
        w: 70, h: 70
    }),
    (AreaEnum::P1Station, Rect {
        x: 328, y: 349,
        w: 70, h: 70
    }),
];

pub static P0_BLOCKED: [Rect<i32>; 3] = [
    Rect { x: 328, y: 200, w: 149, h: 368 },
    Rect { x: 477, y: 270, w: 70, h: 228 },
    Rect { x: 547, y: 200, w: 149, h: 368 }
];

pub static P1_BLOCKED: [Rect<i32>; 3] = [
    Rect { x: 328, y: 200, w: 368, h: 149 },
    Rect { x: 398, y: 349, w: 228, h: 70 },
    Rect { x: 328, y: 419, w: 368, h: 149 }
];

pub fn station(p_id: usize) -> &'static Rect<i32> {
    &GAME_MAP[2 + p_id * 2].1
}

pub fn ship(p_id: usize) -> &'static Rect<i32> {
    &GAME_MAP[1 + p_id * 2].1
}