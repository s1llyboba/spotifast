//! An optional wallpaper painted behind every panel.
//!
//! The settings page sets it through [`configure`], once per frame.

use egui::{Color32, Context, Id, LayerId, Rect, TextureHandle, TextureOptions, pos2, vec2};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The longest edge a wallpaper is shrunk to before it is uploaded.
const MAX_EDGE: u32 = 2560;

struct Config {
    path: Option<PathBuf>,
    /// The file exists, checked when the path changes rather than every frame.
    usable: bool,
    strength: f32,
    dim: f32,
}

static CONFIG: Mutex<Config> = Mutex::new(Config {
    path: None,
    usable: false,
    strength: 0.6,
    dim: 0.45,
});

fn config() -> std::sync::MutexGuard<'static, Config> {
    CONFIG.lock().unwrap_or_else(|error| error.into_inner())
}

/// Sets the wallpaper from the settings. Cheap enough to call every frame.
pub fn configure(path: Option<&str>, strength: f32, dim: f32) {
    let path = path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let mut config = config();
    if config.path != path {
        config.usable = match &path {
            Some(path) if path.is_file() => true,
            Some(path) => {
                log::warn!("Background image not found: {}", path.display());
                false
            }
            None => false,
        };
        config.path = path;
    }
    config.strength = strength.clamp(0.0, 1.0);
    config.dim = dim.clamp(0.0, 1.0);
}

/// How much of the wallpaper shows through the panels, or `None` when no
/// wallpaper is set.
pub fn strength() -> Option<f32> {
    let config = config();
    config.usable.then_some(config.strength)
}

fn load(ctx: &Context, path: &Path) -> Option<TextureHandle> {
    let decoded = match image::open(path) {
        Ok(decoded) => decoded,
        Err(error) => {
            log::warn!(
                "Could not load the background image {}: {error}",
                path.display()
            );
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
    let (path, dim) = {
        let config = config();
        match (&config.path, config.usable) {
            (Some(path), true) => (path.clone(), config.dim),
            _ => return,
        }
    };
    let id = Id::new("spotifast-background");
    let cached = ctx.data(|data| data.get_temp::<(PathBuf, Option<TextureHandle>)>(id));
    let texture = match cached {
        Some((cached_path, texture)) if cached_path == path => texture,
        _ => {
            let texture = load(ctx, &path);
            ctx.data_mut(|data| data.insert_temp(id, (path.clone(), texture.clone())));
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
    let painter = ctx.layer_painter(LayerId::background());
    painter.image(texture.id(), screen, uv, Color32::WHITE);

    let alpha = (dim * 255.0) as u8;
    if alpha > 0 {
        painter.rect_filled(screen, 0.0, Color32::from_black_alpha(alpha));
    }
}