use std::collections::HashSet;

use raylib::prelude::*;
use sc_types::*;
use sc_types::shapes::*;

pub struct Icon {
    tex: Texture2D,
    pos: Vector2,
    size: Vector2,
}

impl Icon {
    pub fn rect(self: &Self) -> Rect<i32> {
        Rect {
            x: self.pos.x as i32,
            y: self.pos.y as i32,
            w: self.size.x as i32,
            h: self.size.y as i32,
        }
    }
}

impl Icon {
    pub fn render(self: &Self, d: &mut RaylibDrawHandle, cooldown: f32, tint: Color) {
        d.draw_texture_ex(&self.tex, self.pos, 0f32, 1f32, tint);
        d.draw_rectangle_v(self.pos + Vector2::new(0f32, self.size.y), Vector2::new(cooldown * self.size.x, 10f32), Color::BLACK);
    }
}

pub struct MessageSpellIcons {
    fast: Icon,
    slow: Icon,
    blink: Icon,
    invuln: Icon,
}

impl MessageSpellIcons {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> MessageSpellIcons {
        let icon_size = Vector2::new(50f32, 50f32);
        let start_pos = Vector2::new(394f32, 843f32);
        let gap = Vector2::new(icon_size.x + 12f32, 0f32);
        MessageSpellIcons {
            fast: Icon {
                tex: rl.load_texture(&thread, "sc-client/assets/fast.png").unwrap(),
                pos: start_pos,
                size: icon_size
            },
            slow: Icon {
                tex: rl.load_texture(&thread, "sc-client/assets/slow.png").unwrap(),
                pos: start_pos + gap,
                size: icon_size
            },
            blink: Icon {
                tex: rl.load_texture(&thread, "sc-client/assets/blink.png").unwrap(),
                pos: start_pos + gap.scale_by(2f32),
                size: icon_size
            },
            invuln: Icon {
                tex: rl.load_texture(&thread, "sc-client/assets/invuln.png").unwrap(),
                pos: start_pos + gap.scale_by(3f32),
                size: icon_size
            },
        }
    }

    pub fn render(self: &Self, d: &mut RaylibDrawHandle, blink_cd: f32) {
        self.fast.render(d, 0f32, Color::WHITE);
        self.slow.render(d, 0f32, Color::WHITE);
        self.blink.render(d, blink_cd, Color::WHITE);
        self.invuln.render(d, 0f32, Color::WHITE);
    }
}

pub struct ShipSpellIcons {
    intercept: Icon,
}

impl ShipSpellIcons {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> ShipSpellIcons {
        let icon_size = Vector2::new(50f32, 50f32);
        let start_pos = Vector2::new(394f32, 843f32);
        ShipSpellIcons {
            intercept: Icon {
                tex: rl.load_texture(&thread, "sc-client/assets/intercept.png").unwrap(),
                pos: start_pos,
                size: icon_size
            }
        }
    }

    pub fn render(self: &Self, d: &mut RaylibDrawHandle, intercept_cd: f32) {
        self.intercept.render(d, intercept_cd, Color::WHITE);
    }
}

pub fn text_icon(rl: &mut RaylibHandle, thread: &RaylibThread, s: &str, pos: Vector2) -> Result<Icon, String> {
    let img = Image::image_text(s, 20, Color::BLACK);
    let tex = rl.load_texture_from_image(thread, &img)?;
    Ok(Icon {
        tex: tex,
        pos: pos,
        size: Vector2 { x: img.width as f32, y: img.height as f32 }
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
        self.blink.render(d, 0f32, if gold >= Item::Blink.cost() { col } else { nogold });
        if !upgrades.contains(&Upgrade::InterceptSpeed) {
            self.intercept_speed.render(d, 0f32, if gold >= Upgrade::InterceptSpeed.cost() { col } else { nogold });
        }
        if !upgrades.contains(&Upgrade::InterceptRange) {
            self.intercept_range.render(d, 0f32, if gold >= Upgrade::InterceptRange.cost() { col } else { nogold });
        }
    }
}