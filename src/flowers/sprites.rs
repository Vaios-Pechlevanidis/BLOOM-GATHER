//! Painted flower pictures: a species with a picture in
//! `assets/flowers/<species id>.png` is drawn as an upright card showing that
//! picture, turned towards the player (`Billboard`), instead of its
//! procedural model (see `assets/flowers/README.md`
//! for the picture format). An optional `<species id>_glow.png` (black except
//! the parts that glow) lights night flowers up.
//!
//! The picture is embedded at build time (`build.rs`). Here it is measured
//! (where the plant starts and ends, where the stem is) so the plant stands
//! on the ground at its botanical height whatever margins the picture has,
//! and given a mip chain that keeps petal edges clean and petals from
//! thinning out in the distance.

use bevy::asset::RenderAssetUsages;
use bevy::image::{
    CompressedImageFormats, ImageFilterMode, ImageSampler, ImageSamplerDescriptor, ImageType,
};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bloom_core::data::FlowerSpecies;

/// Alpha below this is cut away (`AlphaMode::Mask`).
pub const CUTOFF: f32 = 0.5;

/// Rows per card, so the wind bends the stem instead of tilting a flat board.
const ROWS: usize = 4;

/// A species' picture, ready to upload.
pub struct FlowerSprite {
    pub image: Image,
    pub glow: Option<Image>,
    /// One card facing +Z, rooted at the origin, `species.height` tall.
    pub mesh: Mesh,
}

/// The picture for `species`, if there is one.
pub fn load(species: &FlowerSpecies) -> Option<FlowerSprite> {
    let bytes = find(&species.id)?;
    let pixels = match decode(bytes) {
        Ok(p) => p,
        Err(e) => {
            warn!("Flower picture for {} unreadable: {e}", species.id);
            return None;
        }
    };
    let mesh = card(&pixels, species.height);
    let glow = find(&format!("{}_glow", species.id)).and_then(|b| match decode(b) {
        Ok(p) => Some(texture(p, false)),
        Err(e) => {
            warn!("Glow picture for {} unreadable: {e}", species.id);
            None
        }
    });
    Some(FlowerSprite {
        image: texture(pixels, true),
        glow,
        mesh,
    })
}

fn find(name: &str) -> Option<&'static [u8]> {
    super::embedded::FILES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, b)| *b)
}

/// RGBA8 pixels in sRGB, rows top to bottom.
#[derive(Clone)]
struct Pixels {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Pixels {
    fn alpha(&self, x: u32, y: u32) -> u8 {
        self.rgba[((y * self.width + x) * 4 + 3) as usize]
    }
}

fn decode(bytes: &[u8]) -> Result<Pixels, String> {
    let image = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::MAIN_WORLD,
    )
    .map_err(|e| e.to_string())?;
    let image = if image.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb {
        image
    } else {
        image
            .convert(TextureFormat::Rgba8UnormSrgb)
            .ok_or("unsupported pixel format")?
    };
    let size = image.size();
    let rgba = image.data.ok_or("no pixel data")?;
    Ok(Pixels {
        width: size.x,
        height: size.y,
        rgba,
    })
}

/// Where the plant is in the picture, as fractions of its size.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    /// Top of the bloom (0 = top edge).
    top: f32,
    /// Base of the stem (1 = bottom edge).
    bottom: f32,
    /// Horizontal position of the stem base (0 = left edge).
    stem: f32,
}

fn bounds(p: &Pixels) -> Bounds {
    let cut = (CUTOFF * 255.0) as u8;
    let solid = |y: u32| (0..p.width).any(|x| p.alpha(x, y) >= cut);
    let Some(top) = (0..p.height).find(|&y| solid(y)) else {
        return Bounds {
            top: 0.0,
            bottom: 1.0,
            stem: 0.5,
        };
    };
    let bottom = (0..p.height).rev().find(|&y| solid(y)).unwrap_or(top);
    // The stem: the middle of what is solid in the lowest rows.
    let rows = (p.height / 50).max(1);
    let (mut sum, mut n) = (0u64, 0u64);
    for y in bottom.saturating_sub(rows - 1)..=bottom {
        for x in 0..p.width {
            if p.alpha(x, y) >= cut {
                sum += x as u64;
                n += 1;
            }
        }
    }
    Bounds {
        top: top as f32 / p.height as f32,
        bottom: (bottom + 1) as f32 / p.height as f32,
        stem: (sum as f32 / n.max(1) as f32 + 0.5) / p.width as f32,
    }
}

/// An upright card facing +Z (two-sided, so it casts a shadow whichever side
/// the sun is on), sized so the plant in the picture stands on the ground
/// with its stem at the origin and reaches `height`.
fn card(p: &Pixels, height: f32) -> Mesh {
    let b = bounds(p);
    let card_h = height / (b.bottom - b.top).max(0.05);
    let card_w = card_h * p.width as f32 / p.height as f32;
    let (mut positions, mut normals, mut uvs, mut indices) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::<u32>::new());
    for side in [1.0f32, -1.0] {
        // Lean the normals upwards: lit softly from above like a little
        // bush, not like a flat board.
        let normal = (Vec3::Z * side * 0.45 + Vec3::Y).normalize();
        let base = positions.len() as u32;
        for r in 0..=ROWS {
            let v = r as f32 / ROWS as f32;
            let y = (b.bottom - v) * card_h;
            for u in [0.0f32, 1.0] {
                positions.push([(u - b.stem) * card_w, y, 0.0]);
                normals.push(normal.to_array());
                uvs.push([u, v]);
            }
        }
        for r in 0..ROWS as u32 {
            let (tl, tr) = (base + 2 * r, base + 2 * r + 1);
            let (bl, br) = (tl + 2, tr + 2);
            if side > 0.0 {
                indices.extend([bl, br, tr, bl, tr, tl]);
            } else {
                indices.extend([bl, tr, br, bl, tl, tr]);
            }
        }
    }
    let n = positions.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    // White: the picture keeps its colours; the alpha carries the sway.
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; n])
    .with_inserted_indices(Indices::U32(indices))
}

/// A texture with a full mip chain. For cut-out pictures the colour is
/// spread into the transparent surroundings (no dark or white fringes) and
/// each level keeps the share of solid pixels of the full picture (petals
/// don't thin out with distance).
fn texture(mut p: Pixels, cutout: bool) -> Image {
    let coverage = cutout.then(|| solid_share(&p, 1.0));
    if cutout {
        bleed(&mut p, 2);
    }
    let size = Extent3d {
        width: p.width,
        height: p.height,
        depth_or_array_layers: 1,
    };
    let mut data = p.rgba.clone();
    let mut levels = 1;
    let mut level = p.clone();
    while level.width > 1 || level.height > 1 {
        level = downsample(&level);
        if let Some(target) = coverage {
            keep_coverage(&mut level, target);
            bleed(&mut level, 2);
        }
        data.extend_from_slice(&level.rgba);
        levels += 1;
    }
    let mut image = Image::new(
        size,
        TextureDimension::D2,
        p.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// Half the size, averaging colour in linear light weighted by alpha (so
/// transparent pixels lend no colour).
fn downsample(src: &Pixels) -> Pixels {
    let (w, h) = ((src.width / 2).max(1), (src.height / 2).max(1));
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (mut rgb, mut alpha, mut plain) = ([0.0f32; 3], 0.0f32, [0.0f32; 3]);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let sx = (x * 2 + dx).min(src.width - 1);
                let sy = (y * 2 + dy).min(src.height - 1);
                let i = ((sy * src.width + sx) * 4) as usize;
                let a = src.rgba[i + 3] as f32 / 255.0;
                for k in 0..3 {
                    let c = to_linear(src.rgba[i + k]);
                    rgb[k] += c * a;
                    plain[k] += c;
                }
                alpha += a;
            }
            let o = ((y * w + x) * 4) as usize;
            for k in 0..3 {
                let c = if alpha > 0.0 {
                    rgb[k] / alpha
                } else {
                    plain[k] / 4.0
                };
                rgba[o + k] = to_srgb(c);
            }
            rgba[o + 3] = (alpha / 4.0 * 255.0).round() as u8;
        }
    }
    Pixels {
        width: w,
        height: h,
        rgba,
    }
}

/// Share of pixels that survive the cut-off with alpha scaled by `scale`.
fn solid_share(p: &Pixels, scale: f32) -> f32 {
    share_of(&alpha_histogram(p), scale)
}

fn alpha_histogram(p: &Pixels) -> [u32; 256] {
    let mut hist = [0u32; 256];
    for a in p.rgba.iter().skip(3).step_by(4) {
        hist[*a as usize] += 1;
    }
    hist
}

fn share_of(hist: &[u32; 256], scale: f32) -> f32 {
    let cut = CUTOFF * 255.0;
    let total: u32 = hist.iter().sum();
    let solid: u32 = (0..256)
        .filter(|&a| a as f32 * scale >= cut)
        .map(|a| hist[a])
        .sum();
    solid as f32 / total.max(1) as f32
}

/// Scale alpha so the share of solid pixels matches `target`.
fn keep_coverage(p: &mut Pixels, target: f32) {
    let hist = alpha_histogram(p);
    let (mut lo, mut hi) = (0.0f32, 8.0f32);
    for _ in 0..14 {
        let mid = (lo + hi) / 2.0;
        if share_of(&hist, mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    // `hi` is the smallest scale known to reach the target.
    let scale = hi;
    for a in p.rgba.iter_mut().skip(3).step_by(4) {
        *a = (*a as f32 * scale).round().min(255.0) as u8;
    }
}

/// Give fully transparent pixels next to the plant the colour of their
/// visible neighbours, `passes` pixels deep, so filtering at the edges
/// never mixes in the background colour.
fn bleed(p: &mut Pixels, passes: usize) {
    let (w, h) = (p.width as i64, p.height as i64);
    let mut known: Vec<bool> = p.rgba.chunks(4).map(|c| c[3] > 0).collect();
    for _ in 0..passes {
        let mut next = known.clone();
        let mut out = p.rgba.clone();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if known[i] {
                    continue;
                }
                let (mut sum, mut n) = ([0u32; 3], 0u32);
                for (dx, dy) in [
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                    (-1, 0),
                    (1, 0),
                    (-1, 1),
                    (0, 1),
                    (1, 1),
                ] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let j = (ny * w + nx) as usize;
                    if known[j] {
                        for k in 0..3 {
                            sum[k] += p.rgba[j * 4 + k] as u32;
                        }
                        n += 1;
                    }
                }
                for k in 0..3 {
                    if let Some(avg) = sum[k].checked_div(n) {
                        out[i * 4 + k] = avg as u8;
                        next[i] = true;
                    }
                }
            }
        }
        p.rgba = out;
        known = next;
    }
}

/// sRGB to linear light, by table (this runs for every pixel of every
/// mip level).
fn to_linear(c: u8) -> f32 {
    static TABLE: std::sync::LazyLock<[f32; 256]> = std::sync::LazyLock::new(|| {
        std::array::from_fn(|i| {
            let c = i as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
    });
    TABLE[c as usize]
}

/// Linear light to sRGB, by table.
fn to_srgb(c: f32) -> u8 {
    const STEPS: usize = 4096;
    static TABLE: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
        (0..=STEPS)
            .map(|i| {
                let c = i as f32 / STEPS as f32;
                let s = if c <= 0.003_130_8 {
                    c * 12.92
                } else {
                    1.055 * c.powf(1.0 / 2.4) - 0.055
                };
                (s * 255.0).round() as u8
            })
            .collect()
    });
    TABLE[(c.clamp(0.0, 1.0) * STEPS as f32).round() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    /// A `w`x`h` picture with an opaque `color` rectangle (x0..x1, y0..y1).
    fn picture(w: u32, h: u32, rect: (u32, u32, u32, u32), color: [u8; 3]) -> Pixels {
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in rect.2..rect.3 {
            for x in rect.0..rect.1 {
                let i = ((y * w + x) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
        Pixels {
            width: w,
            height: h,
            rgba,
        }
    }

    #[test]
    fn the_plant_is_found_in_the_picture() {
        // Plant from row 16 to 63 (touching the bottom), stem centred at x 40.
        let p = picture(64, 64, (36, 44, 16, 64), [0, 200, 0]);
        let b = bounds(&p);
        assert_eq!(b.top, 0.25);
        assert_eq!(b.bottom, 1.0);
        assert!((b.stem - 40.0 / 64.0).abs() < 1e-6, "{b:?}");
    }

    #[test]
    fn cards_stand_on_the_ground_at_botanical_height() {
        // Margins above and below the plant; stem off centre.
        let p = picture(64, 128, (10, 30, 32, 120), [255, 255, 255]);
        let mesh = card(&p, 0.4);
        let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let b = bounds(&p);
        let card_h = 0.4 / (b.bottom - b.top);
        let lowest = pos.iter().map(|v| v[1]).fold(f32::MAX, f32::min);
        let highest = pos.iter().map(|v| v[1]).fold(f32::MIN, f32::max);
        // The picture's top edge is above the plant, its bottom edge below
        // the ground, by exactly the margins.
        assert!((highest - b.bottom * card_h).abs() < 1e-5);
        assert!((lowest - (b.bottom - 1.0) * card_h).abs() < 1e-5);
        // Where the plant ends: at height 0.4 and on the ground.
        assert!(((b.bottom - b.top) * card_h - 0.4).abs() < 1e-5);
        // The stem stands at the origin.
        let at_ground: Vec<f32> = pos.iter().filter(|v| v[1] < 0.0).map(|v| v[0]).collect();
        let stem_x = at_ground.iter().sum::<f32>() / at_ground.len() as f32;
        let card_w = card_h * 64.0 / 128.0;
        assert!((stem_x - (0.5 - b.stem) * card_w).abs() < 1e-5);
        // A two-sided strip of rows.
        assert_eq!(pos.len(), 2 * (ROWS + 1) * 2);
        assert_eq!(mesh.indices().unwrap().len(), 2 * ROWS * 6);
    }

    #[test]
    fn the_mip_chain_goes_down_to_one_pixel() {
        let p = picture(64, 32, (16, 48, 8, 32), [200, 100, 50]);
        let image = texture(p, true);
        assert_eq!(image.texture_descriptor.mip_level_count, 7);
        let expected: usize = [(64, 32), (32, 16), (16, 8), (8, 4), (4, 2), (2, 1), (1, 1)]
            .iter()
            .map(|(w, h)| w * h * 4)
            .sum();
        assert_eq!(image.data.as_ref().unwrap().len(), expected);
    }

    #[test]
    fn thin_petals_do_not_vanish_in_the_distance() {
        // A one-pixel line on every 4th column: plain averaging would make
        // every far level fully transparent.
        let mut p = picture(64, 64, (0, 0, 0, 0), [0, 0, 0]);
        for y in 0..64 {
            for x in (0..64).step_by(4) {
                let i = ((y * 64 + x) * 4) as usize;
                p.rgba[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        let target = solid_share(&p, 1.0);
        let mut level = downsample(&downsample(&p));
        assert_eq!(solid_share(&level, 1.0), 0.0);
        keep_coverage(&mut level, target);
        assert!(solid_share(&level, 1.0) >= target * 0.9);
    }

    #[test]
    fn edges_take_the_plant_colour_not_the_background() {
        let mut p = picture(16, 16, (6, 10, 6, 10), [250, 120, 30]);
        bleed(&mut p, 2);
        let px = |x: u32, y: u32| &p.rgba[((y * 16 + x) * 4) as usize..][..4];
        // Next to the plant: plant colour, still fully transparent.
        assert_eq!(px(5, 8), &[250, 120, 30, 0]);
        assert_eq!(px(4, 8), &[250, 120, 30, 0]);
        // Far away: untouched.
        assert_eq!(px(0, 0), &[0, 0, 0, 0]);
    }
}
