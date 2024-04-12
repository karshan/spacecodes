use crate::shapes::*;
use raylib::{color::Color, prelude::Vector2};

#[derive(Eq, PartialEq, Hash)]
pub enum AreaEnum {
    P0Spawn,
    P1Spawn,
    P0Station,
    P1Station,
    Blocked
}

pub static START_FUEL: i32 = 1000 * 60;
pub static FUEL_LOSS: i32 = 5; // per frame
pub static PASSIVE_GOLD_GAIN: f32 = 0f32/60f32;
pub static STARTING_GOLD: f32 = 300f32;
pub static STARTING_LUMBER: i32 = 20;
pub static MSG_COOLDOWN: i32 = 10 * 60;
pub static MSG_FREE_LUMBER: i32 = 3;
pub static MSG_FUEL: i32 = FUEL_LOSS * 60 * 15;
pub static MSG_BUBBLE_LEN: f32 = 80f32;
pub static MSG_BUBBLE_WIDTH: f32 = MESSAGE_SIZE.x;
pub static MSG_DELIVERY_GOLD_BOUNTY: f32 = 0f32;
pub static MESSAGE_SIZE: &Vector2 = &Vector2 { x: 20f32, y: 20f32 };
pub static BOUNTY_SIZE: &Vector2 = &Vector2 { x: 20f32, y: 20f32 };
pub static BLINK_COOLDOWN: i32 = 900;
pub static INTERCEPT_LENGTH: f32 = 30f32;
pub static INTERCEPT_COST: f32 = 100f32;
pub static INTERCEPT_EXPIRY: f32 = 2f32 * 60f32;
pub static BLINK_RANGE: f32 = 120f32;
pub static KILLS_TO_WIN: u8 = 5;
pub static PLAY_AREA: Rect<i32> = Rect {
    x: -12, y: -12,
    w: 25, h: 25,
};
pub static SHIPS: [Vector2; 2] = [Vector2 { x: -12.0, y: 11.0 }, Vector2 { x: -11.0, y: 12.0 }];
pub static STATIONS: [Vector2; 2] = [Vector2 { x: 11.0, y: -12.0 }, Vector2 { x: 12.0, y: -11.0 }];

pub fn station(p_id: usize) -> &'static Vector2 {
    &STATIONS[p_id]
}

pub fn ship(p_id: usize) -> &'static Vector2 {
    &SHIPS[p_id]
}

pub fn ship_color(p_id: usize) -> Color {
    if p_id == 0 {
        Color::BLUE
    } else {
        Color::RED
    }
}

pub fn message_color(p_id: usize) -> Color {
    if p_id == 0 {
        // 3a86ff
        Color::from_hex("2510fd").unwrap()
    } else {
        Color::from_hex("780000").unwrap()
    }
}