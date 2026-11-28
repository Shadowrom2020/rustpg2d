use crate::gpu::Gpu;
use crate::texture::{FilterMode, Texture2D};
use crate::{color::Color, math::Rect, texture::Image};

use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct Sprite {
    pub rect: Rect,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum SpriteKey {
    Id(u64),
}
pub struct Atlas {
    texture: Texture2D,
    image: Image,
    pub sprites: HashMap<SpriteKey, Sprite>,
    cursor_x: u16,
    cursor_y: u16,
    max_line_height: u16,

    pub dirty: bool,

    filter: FilterMode,

    unique_id: u64,
}

impl Atlas {
    // pixel gap between glyphs in the atlas
    const GAP: u16 = 2;
    // well..
    const UNIQUENESS_OFFSET: u64 = 100000;

    pub fn new(gpu: &Gpu, filter: FilterMode) -> Atlas {
        let image = Image::gen_image_color(512, 512, Color::new(0.0, 0.0, 0.0, 0.0));
        // (As macroquad: the first texture is sampled nearest whatever `filter` says; a bigger one
        // made later takes `filter`.)
        let texture = Texture2D::create(gpu, image.width as u32, image.height as u32, false, FilterMode::Nearest);
        texture.write(gpu, 0, 0, image.width as u32, image.height as u32, &image.bytes);

        Atlas {
            image,
            texture,
            cursor_x: 0,
            cursor_y: 0,
            dirty: false,
            max_line_height: 0,
            sprites: HashMap::new(),
            filter,
            unique_id: Self::UNIQUENESS_OFFSET,
        }
    }

    pub fn new_unique_id(&mut self) -> SpriteKey {
        self.unique_id += 1;

        SpriteKey::Id(self.unique_id)
    }

    pub fn set_filter(&mut self, filter_mode: FilterMode) {
        self.filter = filter_mode;
        self.texture.set_filter(filter_mode);
    }

    pub fn get(&self, key: SpriteKey) -> Option<Sprite> {
        self.sprites.get(&key).cloned()
    }



    /// The atlas texture, brought up to date with the glyphs cached since the last call.
    pub fn texture(&mut self) -> Texture2D {
        if self.dirty {
            self.dirty = false;
            if self.texture.width() as u16 != self.image.width || self.texture.height() as u16 != self.image.height {
                // (A new texture: draw calls still waiting keep the old one, with the old places.)
                self.texture = Texture2D::from_rgba8(self.image.width, self.image.height, &self.image.bytes);
                self.texture.set_filter(self.filter);
            } else {
                // (Straight to the GPU, without sending the waiting draw calls first: glyphs are
                // only ever added, so what they show stays.)
                self.texture.write(&crate::get_context().gpu, 0, 0, self.image.width as u32, self.image.height as u32, &self.image.bytes);
            }
        }
        self.texture.clone()
    }


    pub fn cache_sprite(&mut self, key: SpriteKey, sprite: Image) {
        let (width, height) = (sprite.width as usize, sprite.height as usize);

        let x = if self.cursor_x + (width as u16) < self.image.width {
            if height as u16 > self.max_line_height {
                self.max_line_height = height as u16;
            }
            let res = self.cursor_x + Self::GAP;
            self.cursor_x += width as u16 + Self::GAP * 2;
            res
        } else {
            self.cursor_y += self.max_line_height + Self::GAP * 2;
            self.cursor_x = width as u16 + Self::GAP;
            self.max_line_height = height as u16;
            Self::GAP
        };
        let y = self.cursor_y;

        // texture bounds exceeded
        if y + sprite.height > self.image.height || x + sprite.width > self.image.width {
            // reset glyph cache state
            let sprites = self.sprites.drain().collect::<Vec<_>>();
            self.cursor_x = 0;
            self.cursor_y = 0;
            self.max_line_height = 0;

            let old_image = self.image.clone();

            // increase font texture size
            // note: if we tried to fit gigantic texture into a small atlas,
            // new_width will still be not enough. But its fine, it will
            // be regenerated on the recursion call.
            let new_width = self.image.width * 2;
            let new_height = self.image.height * 2;

            self.image =
                Image::gen_image_color(new_width, new_height, Color::new(0.0, 0.0, 0.0, 0.0));

            // recache all previously cached symbols
            for (key, sprite) in sprites {
                let image = old_image.sub_image(sprite.rect);
                self.cache_sprite(key, image);
            }

            // cache the new sprite
            self.cache_sprite(key, sprite);
        } else {
            self.dirty = true;

            for j in 0..height {
                for i in 0..width {
                    self.image.set_pixel(
                        x as u32 + i as u32,
                        y as u32 + j as u32,
                        sprite.get_pixel(i as u32, j as u32),
                    );
                }
            }

            self.sprites.insert(
                key,
                Sprite {
                    rect: Rect::new(x as f32, y as f32, width as f32, height as f32),
                },
            );
        }
    }
}
