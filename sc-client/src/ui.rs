use raylib::prelude::*;
use sc_types::constants::PLAY_AREA;
use sc_types::shapes::*;

pub struct Icon {
    tex: Texture2D,
    pos: Vector2,
}

impl Icon {
    pub fn rect(self: &Self) -> Rect<i32> {
        Rect {
            x: self.pos.x as i32,
            y: self.pos.y as i32,
            w: self.tex.width,
            h: self.tex.height,
        }
    }
}

impl Icon {
    pub fn render(self: &Self, d: &mut RaylibDrawHandle, tint: Color) {
        d.draw_texture_ex(&self.tex, self.pos, 0f32, 1f32, tint);
    }
}

pub struct MessageSpellIcons {
    blink: Icon
}

impl MessageSpellIcons {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> MessageSpellIcons {
        let start_pos = Vector2::new(PLAY_AREA.w as f32/2f32 - 25f32, PLAY_AREA.h as f32 + 75f32);
        MessageSpellIcons {
            blink: Icon {
                tex: rl.load_texture(&thread, "sc-client/assets/blink.png").unwrap(),
                pos: start_pos,
            }
        }
    }

    pub fn render(self: &Self, d: &mut RaylibDrawHandle, blink_cd: f32) {
        d.draw_text("Z", self.blink.pos.x.round() as i32, self.blink.pos.y.round() as i32, 1, Color::BLACK);
        self.blink.render(d, Color::WHITE);
        d.draw_rectangle_v(self.blink.pos + Vector2::new(0f32, self.blink.tex.height as f32), Vector2::new(blink_cd * self.blink.tex.height as f32, 10f32), Color::BLACK);
    }
}

pub struct ShipSpellIcons([(Icon, char); 2]);

impl ShipSpellIcons {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> ShipSpellIcons {
        let intercept_tex = rl.load_texture(&thread, "sc-client/assets/intercept.png").unwrap();
        let gap = Vector2::new(12f32, 0f32) + Vector2::new(intercept_tex.width as f32, 0f32);
        let start_pos = Vector2::new(PLAY_AREA.w as f32/2f32 - 25f32, PLAY_AREA.h as f32 + 75f32) - gap;
        ShipSpellIcons([
            (Icon {
                tex: rl.load_texture(&thread, "sc-client/assets/message.png").unwrap(),
                pos: start_pos
            }, 'Q'),
            (Icon {
                tex: intercept_tex,
                pos: start_pos + gap
            }, 'W'),
        ])
    }

    pub fn render(self: &Self, d: &mut RaylibDrawHandle) {
        for (icon, c) in &self.0 {
            icon.render(d, Color::WHITE);
            d.draw_text(&c.to_string(), icon.pos.x.round() as i32, icon.pos.y.round() as i32, 1, Color::BLACK);
        }
    }
}