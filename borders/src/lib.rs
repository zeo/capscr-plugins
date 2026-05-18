// v0.4: capscr's plugin runtime arrives in v0.4. The `runtime` feature flips
// the `Plugin` trait impl + the path dep to `capscr` (Cargo.toml). Standalone
// builds compile as reference code.
#![cfg_attr(not(feature = "runtime"), allow(dead_code))]

#[cfg(feature = "runtime")]
use capscr::plugin::{CaptureType, Plugin, PluginEvent, PluginResponse};
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[cfg(feature = "runtime")]
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderConfig {
    pub enabled: bool,
    pub style: BorderStyle,
    pub size: u32,
    pub color: [u8; 4],
    pub corner_radius: u32,
    pub shadow: Option<ShadowConfig>,
    pub padding: u32,
    pub background_color: Option<[u8; 4]>,
    pub only_modes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    Solid,
    Double,
    Dashed,
    Dotted,
    Groove,
    Ridge,
    Inset,
    Outset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowConfig {
    pub offset_x: i32,
    pub offset_y: i32,
    pub blur_radius: u32,
    pub color: [u8; 4],
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            style: BorderStyle::Solid,
            size: 3,
            color: [60, 60, 60, 255],
            corner_radius: 0,
            shadow: None,
            padding: 0,
            background_color: None,
            only_modes: None,
        }
    }
}

pub struct BordersPlugin {
    config: BorderConfig,
    config_path: PathBuf,
}

impl BordersPlugin {
    pub fn new() -> Self {
        let config_path = Self::default_config_path();
        let config = Self::load_config(&config_path).unwrap_or_default();
        Self { config, config_path }
    }

    pub fn with_config(config: BorderConfig) -> Self {
        Self {
            config,
            config_path: Self::default_config_path(),
        }
    }

    fn default_config_path() -> PathBuf {
        directories::ProjectDirs::from("", "", "capscr")
            .map(|dirs| dirs.config_dir().join("plugins").join("borders.toml"))
            .unwrap_or_else(|| PathBuf::from("borders.toml"))
    }

    fn load_config(path: &PathBuf) -> Option<BorderConfig> {
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }

    fn save_config(&self) {
        if let Some(parent) = self.config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = toml::to_string_pretty(&self.config) {
            let _ = std::fs::write(&self.config_path, content);
        }
    }

    #[cfg(feature = "runtime")]
    fn should_process(&self, mode: &CaptureType) -> bool {
        if !self.config.enabled {
            return false;
        }
        if let Some(ref only_modes) = self.config.only_modes {
            let mode_str = match mode {
                CaptureType::FullScreen => "fullscreen",
                CaptureType::Window => "window",
                CaptureType::Region => "region",
                CaptureType::Gif => "gif",
            };
            return only_modes.iter().any(|m| m.to_lowercase() == mode_str);
        }
        true
    }

    fn apply_border(&self, image: &RgbaImage) -> RgbaImage {
        let (orig_width, orig_height) = image.dimensions();
        let border = self.config.size;
        let padding = self.config.padding;
        let shadow_extra = self.calculate_shadow_extra();

        let new_width = orig_width + (border + padding) * 2 + shadow_extra.0;
        let new_height = orig_height + (border + padding) * 2 + shadow_extra.1;

        let bg_color = self.config.background_color.unwrap_or([0, 0, 0, 0]);
        let mut result = RgbaImage::from_pixel(new_width, new_height, Rgba(bg_color));

        if let Some(ref shadow) = self.config.shadow {
            self.draw_shadow(&mut result, orig_width, orig_height, shadow);
        }

        let content_x = border + padding + shadow_extra.0.saturating_sub(shadow_extra.0 / 2);
        let content_y = border + padding + shadow_extra.1.saturating_sub(shadow_extra.1 / 2);

        self.draw_border(&mut result, content_x, content_y, orig_width, orig_height);

        for y in 0..orig_height {
            for x in 0..orig_width {
                let pixel = image.get_pixel(x, y);
                let dest_x = content_x + x;
                let dest_y = content_y + y;
                if dest_x < new_width && dest_y < new_height {
                    result.put_pixel(dest_x, dest_y, *pixel);
                }
            }
        }

        if self.config.corner_radius > 0 {
            self.apply_corner_mask(&mut result, content_x, content_y, orig_width, orig_height);
        }

        result
    }

    fn calculate_shadow_extra(&self) -> (u32, u32) {
        if let Some(ref shadow) = self.config.shadow {
            let blur = shadow.blur_radius;
            let extra_x = (shadow.offset_x.unsigned_abs() + blur * 2) as u32;
            let extra_y = (shadow.offset_y.unsigned_abs() + blur * 2) as u32;
            (extra_x, extra_y)
        } else {
            (0, 0)
        }
    }

    fn draw_shadow(&self, image: &mut RgbaImage, content_w: u32, content_h: u32, shadow: &ShadowConfig) {
        let border = self.config.size;
        let padding = self.config.padding;
        let shadow_extra = self.calculate_shadow_extra();

        let base_x = (border + padding + shadow_extra.0.saturating_sub(shadow_extra.0 / 2)) as i32;
        let base_y = (border + padding + shadow_extra.1.saturating_sub(shadow_extra.1 / 2)) as i32;

        let shadow_x = base_x + shadow.offset_x;
        let shadow_y = base_y + shadow.offset_y;

        let blur = shadow.blur_radius as i32;
        let shadow_color = Rgba(shadow.color);

        for y in -blur..=(content_h as i32 + blur) {
            for x in -blur..=(content_w as i32 + blur) {
                let px = shadow_x + x;
                let py = shadow_y + y;

                if px < 0 || py < 0 || px >= image.width() as i32 || py >= image.height() as i32 {
                    continue;
                }

                let dist_x = if x < 0 {
                    -x
                } else if x >= content_w as i32 {
                    x - content_w as i32 + 1
                } else {
                    0
                };

                let dist_y = if y < 0 {
                    -y
                } else if y >= content_h as i32 {
                    y - content_h as i32 + 1
                } else {
                    0
                };

                let dist = ((dist_x * dist_x + dist_y * dist_y) as f32).sqrt();
                if blur > 0 && dist > blur as f32 {
                    continue;
                }

                let alpha = if blur > 0 {
                    let factor = 1.0 - (dist / blur as f32);
                    (shadow_color.0[3] as f32 * factor * factor) as u8
                } else {
                    shadow_color.0[3]
                };

                let mut pixel = shadow_color;
                pixel.0[3] = alpha;
                self.blend_pixel(image, px as u32, py as u32, pixel);
            }
        }
    }

    fn draw_border(&self, image: &mut RgbaImage, content_x: u32, content_y: u32, content_w: u32, content_h: u32) {
        let border = self.config.size;
        let color = Rgba(self.config.color);

        if border == 0 {
            return;
        }

        match self.config.style {
            BorderStyle::Solid => {
                self.draw_solid_border(image, content_x, content_y, content_w, content_h, border, color);
            }
            BorderStyle::Double => {
                self.draw_double_border(image, content_x, content_y, content_w, content_h, border, color);
            }
            BorderStyle::Dashed => {
                self.draw_dashed_border(image, content_x, content_y, content_w, content_h, border, color, 10, 5);
            }
            BorderStyle::Dotted => {
                self.draw_dashed_border(image, content_x, content_y, content_w, content_h, border, color, 2, 2);
            }
            BorderStyle::Groove => {
                self.draw_3d_border(image, content_x, content_y, content_w, content_h, border, color, true);
            }
            BorderStyle::Ridge => {
                self.draw_3d_border(image, content_x, content_y, content_w, content_h, border, color, false);
            }
            BorderStyle::Inset => {
                self.draw_inset_border(image, content_x, content_y, content_w, content_h, border, color, true);
            }
            BorderStyle::Outset => {
                self.draw_inset_border(image, content_x, content_y, content_w, content_h, border, color, false);
            }
        }
    }

    fn draw_solid_border(&self, image: &mut RgbaImage, cx: u32, cy: u32, cw: u32, ch: u32, border: u32, color: Rgba<u8>) {
        for i in 0..border {
            let x1 = cx.saturating_sub(i + 1);
            let y1 = cy.saturating_sub(i + 1);
            let x2 = cx + cw + i;
            let y2 = cy + ch + i;

            for x in x1..=x2 {
                if x < image.width() {
                    if y1 < image.height() {
                        image.put_pixel(x, y1, color);
                    }
                    if y2 < image.height() {
                        image.put_pixel(x, y2, color);
                    }
                }
            }
            for y in y1..=y2 {
                if y < image.height() {
                    if x1 < image.width() {
                        image.put_pixel(x1, y, color);
                    }
                    if x2 < image.width() {
                        image.put_pixel(x2, y, color);
                    }
                }
            }
        }
    }

    fn draw_double_border(&self, image: &mut RgbaImage, cx: u32, cy: u32, cw: u32, ch: u32, border: u32, color: Rgba<u8>) {
        let outer_width = border / 3;
        let inner_width = border / 3;
        let gap = border - outer_width - inner_width;

        self.draw_solid_border(image, cx, cy, cw, ch, inner_width, color);

        let outer_cx = cx.saturating_sub(inner_width + gap);
        let outer_cy = cy.saturating_sub(inner_width + gap);
        let outer_cw = cw + (inner_width + gap) * 2;
        let outer_ch = ch + (inner_width + gap) * 2;
        self.draw_solid_border(image, outer_cx, outer_cy, outer_cw, outer_ch, outer_width, color);
    }

    fn draw_dashed_border(&self, image: &mut RgbaImage, cx: u32, cy: u32, cw: u32, ch: u32, border: u32, color: Rgba<u8>, dash_len: u32, gap_len: u32) {
        for i in 0..border {
            let x1 = cx.saturating_sub(i + 1);
            let y1 = cy.saturating_sub(i + 1);
            let x2 = cx + cw + i;
            let y2 = cy + ch + i;

            let mut pos = 0u32;
            for x in x1..=x2 {
                if x < image.width() && (pos % (dash_len + gap_len)) < dash_len {
                    if y1 < image.height() {
                        image.put_pixel(x, y1, color);
                    }
                    if y2 < image.height() {
                        image.put_pixel(x, y2, color);
                    }
                }
                pos += 1;
            }

            pos = 0;
            for y in y1..=y2 {
                if y < image.height() && (pos % (dash_len + gap_len)) < dash_len {
                    if x1 < image.width() {
                        image.put_pixel(x1, y, color);
                    }
                    if x2 < image.width() {
                        image.put_pixel(x2, y, color);
                    }
                }
                pos += 1;
            }
        }
    }

    fn draw_3d_border(&self, image: &mut RgbaImage, cx: u32, cy: u32, cw: u32, ch: u32, border: u32, color: Rgba<u8>, groove: bool) {
        let half = border / 2;
        let (light, dark) = self.calculate_3d_colors(color);
        let (outer_top, outer_bottom, inner_top, inner_bottom) = if groove {
            (dark, light, light, dark)
        } else {
            (light, dark, dark, light)
        };

        for i in 0..half {
            let x1 = cx.saturating_sub(half + i + 1);
            let y1 = cy.saturating_sub(half + i + 1);
            let x2 = cx + cw + half + i;
            let y2 = cy + ch + half + i;

            for x in x1..=x2 {
                if x < image.width() {
                    if y1 < image.height() {
                        image.put_pixel(x, y1, outer_top);
                    }
                    if y2 < image.height() {
                        image.put_pixel(x, y2, outer_bottom);
                    }
                }
            }
            for y in y1..=y2 {
                if y < image.height() {
                    if x1 < image.width() {
                        image.put_pixel(x1, y, outer_top);
                    }
                    if x2 < image.width() {
                        image.put_pixel(x2, y, outer_bottom);
                    }
                }
            }
        }

        for i in 0..half {
            let x1 = cx.saturating_sub(i + 1);
            let y1 = cy.saturating_sub(i + 1);
            let x2 = cx + cw + i;
            let y2 = cy + ch + i;

            for x in x1..=x2 {
                if x < image.width() {
                    if y1 < image.height() {
                        image.put_pixel(x, y1, inner_top);
                    }
                    if y2 < image.height() {
                        image.put_pixel(x, y2, inner_bottom);
                    }
                }
            }
            for y in y1..=y2 {
                if y < image.height() {
                    if x1 < image.width() {
                        image.put_pixel(x1, y, inner_top);
                    }
                    if x2 < image.width() {
                        image.put_pixel(x2, y, inner_bottom);
                    }
                }
            }
        }
    }

    fn draw_inset_border(&self, image: &mut RgbaImage, cx: u32, cy: u32, cw: u32, ch: u32, border: u32, color: Rgba<u8>, inset: bool) {
        let (light, dark) = self.calculate_3d_colors(color);
        let (top_color, bottom_color) = if inset { (dark, light) } else { (light, dark) };

        for i in 0..border {
            let x1 = cx.saturating_sub(i + 1);
            let y1 = cy.saturating_sub(i + 1);
            let x2 = cx + cw + i;
            let y2 = cy + ch + i;

            for x in x1..=x2 {
                if x < image.width() {
                    if y1 < image.height() {
                        image.put_pixel(x, y1, top_color);
                    }
                    if y2 < image.height() {
                        image.put_pixel(x, y2, bottom_color);
                    }
                }
            }
            for y in y1..=y2 {
                if y < image.height() {
                    if x1 < image.width() {
                        image.put_pixel(x1, y, top_color);
                    }
                    if x2 < image.width() {
                        image.put_pixel(x2, y, bottom_color);
                    }
                }
            }
        }
    }

    fn calculate_3d_colors(&self, base: Rgba<u8>) -> (Rgba<u8>, Rgba<u8>) {
        let lighten = |v: u8| -> u8 { v.saturating_add(60) };
        let darken = |v: u8| -> u8 { v.saturating_sub(60) };

        let light = Rgba([lighten(base.0[0]), lighten(base.0[1]), lighten(base.0[2]), base.0[3]]);
        let dark = Rgba([darken(base.0[0]), darken(base.0[1]), darken(base.0[2]), base.0[3]]);
        (light, dark)
    }

    fn apply_corner_mask(&self, image: &mut RgbaImage, cx: u32, cy: u32, cw: u32, ch: u32) {
        let radius = self.config.corner_radius;
        let border = self.config.size;

        let corners = [
            (cx.saturating_sub(border), cy.saturating_sub(border), false, false),
            (cx + cw + border - radius, cy.saturating_sub(border), true, false),
            (cx.saturating_sub(border), cy + ch + border - radius, false, true),
            (cx + cw + border - radius, cy + ch + border - radius, true, true),
        ];

        for (corner_x, corner_y, flip_x, flip_y) in corners {
            for dy in 0..radius {
                for dx in 0..radius {
                    let px = if flip_x { corner_x + radius - 1 - dx } else { corner_x + dx };
                    let py = if flip_y { corner_y + radius - 1 - dy } else { corner_y + dy };

                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist > radius as f32 {
                        if px < image.width() && py < image.height() {
                            image.put_pixel(px, py, Rgba([0, 0, 0, 0]));
                        }
                    }
                }
            }
        }
    }

    fn blend_pixel(&self, image: &mut RgbaImage, x: u32, y: u32, src: Rgba<u8>) {
        if x >= image.width() || y >= image.height() {
            return;
        }

        let dst = image.get_pixel(x, y);
        let src_a = src.0[3] as f32 / 255.0;
        let dst_a = dst.0[3] as f32 / 255.0;

        let out_a = src_a + dst_a * (1.0 - src_a);
        if out_a == 0.0 {
            return;
        }

        let blend = |s: u8, d: u8| -> u8 {
            ((s as f32 * src_a + d as f32 * dst_a * (1.0 - src_a)) / out_a) as u8
        };

        image.put_pixel(x, y, Rgba([
            blend(src.0[0], dst.0[0]),
            blend(src.0[1], dst.0[1]),
            blend(src.0[2], dst.0[2]),
            (out_a * 255.0) as u8,
        ]));
    }
}

impl Default for BordersPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "runtime")]
impl Plugin for BordersPlugin {
    fn name(&self) -> &str {
        "Borders"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Add customizable borders to captured images"
    }

    fn on_event(&mut self, event: &PluginEvent) -> PluginResponse {
        match event {
            PluginEvent::PostCapture { image, mode } => {
                if !self.should_process(mode) {
                    return PluginResponse::Continue;
                }
                let bordered = self.apply_border(image);
                PluginResponse::ModifiedImage(Arc::new(bordered))
            }
            _ => PluginResponse::Continue,
        }
    }

    fn on_load(&mut self) {
        if !self.config_path.exists() {
            self.save_config();
        }
    }

    fn on_unload(&mut self) {
        self.save_config();
    }
}
