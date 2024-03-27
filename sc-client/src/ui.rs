use std::collections::HashSet;

use raylib::prelude::*;
use sc_types::*;
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
        let start_pos = Vector2::new(394f32, 843f32);
        MessageSpellIcons {
            blink: Icon {
                tex: rl.load_texture(&thread, "sc-client/assets/blink.png").unwrap(),
                pos: start_pos,
            }
        }
    }

    pub fn render(self: &Self, d: &mut RaylibDrawHandle, blink_cd: f32) {
        self.blink.render(d, Color::WHITE);
        d.draw_rectangle_v(self.blink.pos + Vector2::new(0f32, self.blink.tex.height as f32), Vector2::new(blink_cd * self.blink.tex.height as f32, 10f32), Color::BLACK);
    }
}

pub struct ShipSpellIcons([(Icon, char); 3]);

impl ShipSpellIcons {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> ShipSpellIcons {
        let start_pos = Vector2::new(394f32, 843f32);
        let intercept_tex = rl.load_texture(&thread, "sc-client/assets/intercept.png").unwrap();
        let gap = Vector2::new(12f32, 0f32) + Vector2::new(intercept_tex.width as f32, 0f32);
        ShipSpellIcons([
            (Icon {
                tex: intercept_tex,
                pos: start_pos
            }, 'I'),
            (Icon {
                tex: rl.load_texture(&thread, "sc-client/assets/message.png").unwrap(),
                pos: start_pos + gap
            }, 'M'),
            (Icon {
                tex: rl.load_texture(&thread, "sc-client/assets/blinking_message.png").unwrap(),
                pos: start_pos + gap.scale_by(2f32)
            }, 'B')
        ])
    }

    pub fn render(self: &Self, d: &mut RaylibDrawHandle) {
        for (icon, c) in &self.0 {
            icon.render(d, Color::WHITE);
            d.draw_text(&c.to_string(), icon.pos.x.round() as i32, icon.pos.y.round() as i32, 1, Color::BLACK);
        }
    }
}

pub fn text_icon(rl: &mut RaylibHandle, thread: &RaylibThread, s: &str, pos: Vector2) -> Result<Icon, String> {
    let img = Image::image_text(s, 20, Color::BLACK);
    let tex = rl.load_texture_from_image(thread, &img)?;
    Ok(Icon {
        tex: tex,
        pos: pos
    })
}

pub struct Shop {
    blink: Icon,
    intercept_speed: Icon,
    intercept_range: Icon,
}

impl Shop {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> Result<Shop, String> {
        let first_pos = Vector2::new(863f32, 768f32);
        let gap = Vector2::new(0f32, 20f32);
        Ok(Shop {
            blink: text_icon(rl, thread, "Blink", first_pos)?,
            intercept_speed: text_icon(rl, thread, "I Speed", first_pos + gap)?,
            intercept_range: text_icon(rl, thread, "I Range", first_pos + gap.scale_by(2f32))?,
        })
    }

    pub fn click(self: &Self, mouse_position: Vector2) -> Option<ShopItem> {
        if contains_point(&self.blink.rect(), &mouse_position) {
            Some(ShopItem::Item(Item::Blink))
        } else if contains_point(&self.intercept_speed.rect(), &mouse_position) {
            Some(ShopItem::Upgrade(Upgrade::InterceptSpeed))
        } else if contains_point(&self.intercept_range.rect(), &mouse_position) {
            Some(ShopItem::Upgrade(Upgrade::InterceptRange))
        } else {
            None
        }
    }

    pub fn render(self: &Self, d: &mut RaylibDrawHandle, upgrades: &HashSet<Upgrade>, gold: f32) {
        d.draw_line(863, 768, 863, 967, Color::BLACK);
        let col = Color::WHITE;
        let nogold = rcolor(255, 255, 255, 100);
        self.blink.render(d, if gold >= Item::Blink.cost() { col } else { nogold });
        if !upgrades.contains(&Upgrade::InterceptSpeed) {
            self.intercept_speed.render(d, if gold >= Upgrade::InterceptSpeed.cost() { col } else { nogold });
        }
        if !upgrades.contains(&Upgrade::InterceptRange) {
            self.intercept_range.render(d, if gold >= Upgrade::InterceptRange.cost() { col } else { nogold });
        }
    }
}