//! An optional wallpaper painted behind every panel.
//!
//! The settings page sets it through [`configure`], once per frame.

use egui::{Color32, Context, Id, LayerId, Rect, TextureHandle, TextureOptions, pos2, vec2};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The longest edge a wallpaper is shrunk to before it is uploaded.
const MAX_EDGE: u32 = 2560;

/// How the picture is sized to the window.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Fill the window, cropping what overflows.
    #[default]
    Cover,
    /// Show the whole picture inside the window.
    Contain,
    /// Stretch to the window, ignoring the picture's shape.
    Stretch,
}

/// Everything about how the wallpaper looks.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// How much of the picture shows through the panels, 0.0 to 1.0.
    pub strength: f32,
    /// How much black is laid over the picture, 0.0 to 1.0.
    pub dim: f32,
    pub fit: Fit,
    /// Magnification on top of the fit, 1.0 and up.
    pub zoom: f32,
    /// Where the picture sits, 0.0 (left or top) to 1.0 (right or bottom).
    pub x: f32,
    pub y: f32,
}

struct Config {
    path: Option<PathBuf>,
    /// The file exists, checked when the path changes rather than every frame.
    usable: bool,
    look: Look,
}

static CONFIG: Mutex<Config> = Mutex::new(Config {
    path: None,
    usable: false,
    look: Look {
        strength: 0.6,
        dim: 0.45,
        fit: Fit::Cover,
        zoom: 1.0,
        x: 0.5,
        y: 0.5,
    },
});

fn config() -> std::sync::MutexGuard<'static, Config> {
    CONFIG.lock().unwrap_or_else(|error| error.into_inner())
}

/// Sets the wallpaper from the settings. Cheap enough to call every frame.
pub fn configure(path: Option<&str>, look: Look) {
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
    config.look = Look {
        strength: look.strength.clamp(0.0, 1.0),
        dim: look.dim.clamp(0.0, 1.0),
        fit: look.fit,
        zoom: look.zoom.clamp(1.0, 4.0),
        x: look.x.clamp(0.0, 1.0),
        y: look.y.clamp(0.0, 1.0),
    };
}

/// How much of the wallpaper shows through the panels, or `None` when no
/// wallpaper is set.
pub fn strength() -> Option<f32> {
    let config = config();
    config.usable.then_some(config.look.strength)
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
    let (path, look) = {
        let config = config();
        match (&config.path, config.usable) {
            (Some(path), true) => (path.clone(), config.look),
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

    let screen = ctx.content_rect();
    let image = texture.size_vec2();
    let size = match look.fit {
        Fit::Cover => image * (screen.width() / image.x).max(screen.height() / image.y),
        Fit::Contain => image * (screen.width() / image.x).min(screen.height() / image.y),
        Fit::Stretch => screen.size(),
    } * look.zoom;
    // Where the picture's corner lands: with the picture larger than the
    // window, position 0 shows its left or top edge and 1 its right or
    // bottom; smaller than the window, it slides inside the free space.
    let extra = size - screen.size();
    let min = screen.min - vec2(extra.x * look.x, extra.y * look.y);
    let destination = Rect::from_min_size(min, size);

    let painter = ctx
        .layer_painter(LayerId::background())
        .with_clip_rect(screen);
    painter.image(
        texture.id(),
        destination,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    let alpha = (look.dim * 255.0) as u8;
    if alpha > 0 {
        painter.rect_filled(screen, 0.0, Color32::from_black_alpha(alpha));
    }
}