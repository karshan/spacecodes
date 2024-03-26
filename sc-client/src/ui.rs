use raylib::prelude::*;
pub struct SpellIcon {
    tex: Texture2D,
    pos: Vector2,
    size: Vector2,
}

impl SpellIcon {
    pub fn render(self: &Self, d: &mut RaylibDrawHandle, cooldown: f32) {
        d.draw_texture_ex(&self.tex, self.pos, 0f32, 1f32, Color::WHITE);
        d.draw_rectangle_v(self.pos + Vector2::new(0f32, self.size.y), Vector2::new(cooldown * self.size.x, 10f32), Color::BLACK);
    }
}

pub struct MessageSpellIcons {
    fast: SpellIcon,
    slow: SpellIcon,
    blink: SpellIcon,
    invuln: SpellIcon,
}

impl MessageSpellIcons {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread) -> MessageSpellIcons {
        let icon_size = Vector2::new(50f32, 50f32);
        let start_pos = Vector2::new(394f32, 843f32);
        let gap = Vector2::new(icon_size.x + 12f32, 0f32);
        MessageSpellIcons {
            fast: SpellIcon {
                tex: rl.load_texture(&thread, "sc-client/assets/fast.png").unwrap(),
                pos: start_pos,
                size: icon_size
            },
            slow: SpellIcon {
                tex: rl.load_texture(&thread, "sc-client/assets/slow.png").unwrap(),
                pos: start_pos + gap,
                size: icon_size
            },
            blink: SpellIcon {
                tex: rl.load_texture(&thread, "sc-client/assets/blink.png").unwrap(),
                pos: start_pos + gap.scale_by(2f32),
                size: icon_size
            },
            invuln: SpellIcon {
                tex: rl.load_texture(&thread, "sc-client/assets/invuln.png").unwrap(),
                pos: start_pos + gap.scale_by(3f32),
                size: icon_size
            },
        }
    }

    pub fn render(self: &Self, d: &mut RaylibDrawHandle, blink_cd: f32) {
        self.fast.render(d, 0f32);
        self.slow.render(d, 0f32);
        self.blink.render(d, blink_cd);
        self.invuln.render(d, 0f32);
    }
}
