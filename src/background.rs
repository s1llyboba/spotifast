//! An optional wallpaper painted behind every panel.
//!
//! Set `SPOTIFAST_BACKGROUND` to an image file to turn it on, and
//! `SPOTIFAST_BACKGROUND_STRENGTH` (0.0 to 1.0, default 0.3) to choose how
//! much of it shows through the panels.

use egui::{Color32, Context, Id, LayerId, Rect, TextureHandle, TextureOptions, pos2, vec2};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The longest edge a wallpaper is shrunk to before it is uploaded.
const MAX_EDGE: u32 = 2560;

fn config() -> Option<&'static (PathBuf, f32)> {
    static CONFIG: OnceLock<Option<(PathBuf, f32)>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let path = PathBuf::from(
                std::env::var_os("SPOTIFAST_BACKGROUND").filter(|value| !value.is_empty())?,
            );
            if !path.is_file() {
                log::warn!("Background image not found: {}", path.display());
                return None;
            }
            let strength = std::env::var("SPOTIFAST_BACKGROUND_STRENGTH")
                .ok()
                .and_then(|value| value.parse::<f32>().ok())
                .unwrap_or(0.3)
                .clamp(0.0, 1.0);
            Some((path, strength))
        })
        .as_ref()
}

/// How much of the wallpaper shows through the panels, or `None` when no
/// wallpaper is set.
pub fn strength() -> Option<f32> {
    config().map(|(_, strength)| *strength)
}
/// How much black is laid over the wallpaper (0.0 to 1.0).
fn dim() -> f32 {
    static DIM: OnceLock<f32> = OnceLock::new();
    *DIM.get_or_init(|| {
        std::env::var("SPOTIFAST_BACKGROUND_DIM")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or(0.35)
            .clamp(0.0, 1.0)
    })
}

fn load(ctx: &Context, path: &Path) -> Option<TextureHandle> {
    let decoded = match image::open(path) {
        Ok(decoded) => decoded,
        Err(error) => {
            log::warn!("Could not load the background image {}: {error}", path.display());
            return None;
        }
    };
    let rgba = decoded.thumbnail(MAX_EDGE, MAX_EDGE).to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture("spotifast-background", color, TextureOptions::LINEAR))
}

/// Paints the wallpaper over the whole window, below every panel. Call it
/// once per frame, before the panels are drawn.
pub fn paint(ctx: &Context) {
    let Some((path, _)) = config() else {
        return;
    };
    let id = Id::new("spotifast-background");
    let cached = ctx.data(|data| data.get_temp::<Option<TextureHandle>>(id));
    let texture = match cached {
        Some(texture) => texture,
        None => {
            let texture = load(ctx, path);
            ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        }
    };
    let Some(texture) = texture else {
        return;
    };

    // "Cover" fit: fill the window and crop what overflows.
    let screen = ctx.content_rect();
    let image = texture.size_vec2();
    let scale = (screen.width() / image.x).max(screen.height() / image.y);
    let visible = vec2(
        screen.width() / (image.x * scale),
        screen.height() / (image.y * scale),
    );
    let uv = Rect::from_center_size(pos2(0.5, 0.5), visible);
    ctx.layer_painter(LayerId::background())
        .image(texture.id(), screen, uv, Color32::WHITE);
    
    let alpha = (dim() * 255.0) as u8;
    if alpha > 0 {
        ctx.layer_painter(LayerId::background())
            .rect_filled(screen, 0.0, Color32::from_black_alpha(alpha));
    }
}