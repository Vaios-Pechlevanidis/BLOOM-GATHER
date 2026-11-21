//! Procedural 3D flower models, one per species, from 17 silhouette
//! families (`FlowerShape`).
//!
//! Each model is real geometry — stems, leaves, petals with colour gradients —
//! modelled at true size in metres with the origin at the base of the stem.
//! Parts that glow at night (Moonlight Garden) are returned as a separate mesh
//! so only the blooms get the emissive material.
//!
//! These are *temporary art* (see TODO.md): painted glTF models replace them
//! per species via `FlowerSpecies::model` without touching gameplay.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::prelude::*;
use bloom_core::data::{FlowerShape, FlowerSpecies};

use crate::rendering::color;
use crate::rendering::meshes::{hash3, lin, mix, MeshBuilder};

pub struct FlowerModel {
    pub plant: Mesh,
    /// Glowing parts (only for species with `glow > 0`).
    pub glow: Option<Mesh>,
}

/// Reusable unit primitives.
struct Parts {
    /// Unit sphere, smooth (petals, leaves, buds).
    round: Mesh,
    /// Unit icosahedron (tiny florets — cheap).
    tiny: Mesh,
    /// Unit-height, unit-radius cylinder.
    tube: Mesh,
    /// Unit cone (radius 1, height 1, apex +Y).
    cone: Mesh,
}

impl Parts {
    fn new() -> Self {
        Self {
            round: Sphere::new(1.0).mesh().ico(1).expect("valid icosphere"),
            tiny: Sphere::new(1.0).mesh().ico(0).expect("valid icosphere"),
            tube: Mesh::from(Cylinder::new(1.0, 1.0).mesh().resolution(6)),
            cone: Mesh::from(Cone::new(1.0, 1.0).mesh().resolution(10)),
        }
    }
}

/// Builds one plant from parts, in a local frame that can be moved/tilted
/// (e.g. a nodding flower head).
struct Plant<'a> {
    parts: &'a Parts,
    body: MeshBuilder,
    glow: MeshBuilder,
    glows: bool,
    stem: LinearRgba,
    leaf_dark: LinearRgba,
    leaf_light: LinearRgba,
}

impl<'a> Plant<'a> {
    fn new(parts: &'a Parts, glows: bool) -> Self {
        Self {
            parts,
            body: MeshBuilder::default(),
            glow: MeshBuilder::default(),
            glows,
            stem: lin(Color::srgb_u8(104, 146, 86)),
            leaf_dark: lin(Color::srgb_u8(82, 118, 70)),
            leaf_light: lin(Color::srgb_u8(146, 182, 112)),
        }
    }

    fn target(&mut self, bloom: bool) -> &mut MeshBuilder {
        if bloom && self.glows {
            &mut self.glow
        } else {
            &mut self.body
        }
    }

    /// A tube between two points.
    fn tube(&mut self, frame: Transform, from: Vec3, to: Vec3, radius: f32, col: LinearRgba) {
        let d = to - from;
        let len = d.length();
        if len < 1e-5 {
            return;
        }
        let t = Transform::from_translation((from + to) / 2.0)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, d / len))
            .with_scale(Vec3::new(radius, len, radius));
        let mesh = &self.parts.tube;
        self.body.add(mesh, frame.mul_transform(t), move |p| {
            mix(col * 0.85, col, p.y + 0.5)
        });
    }

    /// A gently curved stem from the origin to `top` (3 segments).
    fn stem(&mut self, frame: Transform, top: Vec3, radius: f32) {
        // Botanical stems vanish at game scale; draw them a little sturdier.
        let radius = radius * 1.4;
        let mid = Vec3::new(top.x * 0.25, top.y * 0.5, top.z * 0.25);
        let low = Vec3::new(top.x * 0.06, top.y * 0.22, top.z * 0.06);
        let c = self.stem;
        self.tube(frame, Vec3::ZERO, low, radius * 1.15, c);
        self.tube(frame, low, mid, radius, c);
        self.tube(frame, mid, top, radius * 0.85, c);
    }

    /// A flattened ellipsoid "blade" starting at `base`, pointing along
    /// `yaw` (around Y) and lifted by `tilt` (radians above horizontal).
    #[allow(clippy::too_many_arguments)]
    fn blade(
        &mut self,
        frame: Transform,
        base: Vec3,
        yaw: f32,
        tilt: f32,
        len: f32,
        width: f32,
        thick: f32,
        base_col: LinearRgba,
        tip_col: LinearRgba,
        bloom: bool,
    ) {
        let rot = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-tilt);
        let t = Transform::from_translation(base + rot * Vec3::new(0.0, 0.0, len / 2.0))
            .with_rotation(rot)
            .with_scale(Vec3::new(width / 2.0, thick / 2.0, len / 2.0));
        let mesh = &self.parts.round;
        self.target(bloom)
            .add(mesh, frame.mul_transform(t), move |p| {
                mix(base_col, tip_col, p.z * 0.5 + 0.5)
            });
    }

    fn leaf(&mut self, frame: Transform, base: Vec3, yaw: f32, tilt: f32, len: f32, width: f32) {
        let (d, l) = (self.leaf_dark, self.leaf_light);
        self.blade(
            frame,
            base,
            yaw,
            tilt,
            len,
            width,
            width * 0.12,
            d,
            l,
            false,
        );
    }

    /// `n` petals around `center` in the frame's XZ plane.
    #[allow(clippy::too_many_arguments)]
    fn ring(
        &mut self,
        frame: Transform,
        center: Vec3,
        n: usize,
        offset: f32,
        tilt: f32,
        len: f32,
        width: f32,
        base_col: LinearRgba,
        tip_col: LinearRgba,
    ) {
        for i in 0..n {
            let yaw = offset + i as f32 / n as f32 * TAU;
            self.blade(
                frame,
                center,
                yaw,
                tilt,
                len,
                width,
                width * 0.18,
                base_col,
                tip_col,
                true,
            );
        }
    }

    fn ball(
        &mut self,
        frame: Transform,
        center: Vec3,
        scale: Vec3,
        col: LinearRgba,
        bloom: bool,
        tiny: bool,
    ) {
        let mesh = if tiny {
            &self.parts.tiny
        } else {
            &self.parts.round
        };
        let t = Transform::from_translation(center).with_scale(scale);
        self.target(bloom)
            .add(mesh, frame.mul_transform(t), move |p| {
                mix(col * 0.82, col, p.y * 0.5 + 0.5)
            });
    }

    /// A cone with its tip at `tip`, opening along `dir`.
    fn bell(
        &mut self,
        frame: Transform,
        tip: Vec3,
        dir: Vec3,
        radius: f32,
        len: f32,
        col: LinearRgba,
    ) {
        let d = dir.normalize();
        // Cone apex is +Y; flip so the opening faces `dir`.
        let t = Transform::from_translation(tip + d * len / 2.0)
            .with_rotation(Quat::from_rotation_arc(Vec3::NEG_Y, d))
            .with_scale(Vec3::new(radius, len, radius));
        let mesh = &self.parts.cone;
        self.target(true)
            .add(mesh, frame.mul_transform(t), move |p| {
                mix(col, col * 0.75, p.y + 0.5)
            });
    }

    fn finish(self) -> FlowerModel {
        let glow = self.glows.then(|| self.glow.build());
        FlowerModel {
            plant: self.body.build(),
            glow,
        }
    }
}

/// Build the model for one species.
pub fn build(species: &FlowerSpecies) -> FlowerModel {
    let parts = Parts::new();
    let mut p = Plant::new(&parts, species.glow > 0.0);
    let h = species.height;
    let petal = lin(color(species.petal_color));
    let center = lin(color(species.center_color));
    let pale = mix(petal, LinearRgba::WHITE, 0.35);
    let deep = mix(petal, center, 0.5);
    let id = Transform::IDENTITY;
    // Every species gets a slightly different natural lean.
    let seed = species.id.len() as f32 * 1.7;
    let lean = Vec3::new(seed.sin() * 0.04, 0.0, seed.cos() * 0.04);

    match species.shape {
        FlowerShape::Daisy => {
            let top = Vec3::new(lean.x, h, lean.z);
            p.stem(id, top, 0.0045);
            for i in 0..3 {
                p.leaf(id, Vec3::Y * 0.01, i as f32 * 2.2, 0.35, 0.09, 0.022);
            }
            let head = Transform::from_translation(top).with_rotation(Quat::from_rotation_x(-0.25));
            p.ring(head, Vec3::ZERO, 16, 0.0, 0.12, 0.07, 0.021, pale, petal);
            p.ball(
                head,
                Vec3::Y * 0.006,
                Vec3::new(0.024, 0.014, 0.024),
                center,
                true,
                false,
            );
        }
        FlowerShape::Trumpet => {
            let top = Vec3::new(0.0, h, 0.03);
            p.stem(id, top, 0.005);
            for i in 0..3 {
                p.leaf(id, Vec3::ZERO, i as f32 * 2.1 + 0.4, 1.25, h * 0.75, 0.022);
            }
            // Nodding head: faces sideways and a little down.
            let head = Transform::from_translation(top)
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2 + 0.35));
            p.ring(head, Vec3::ZERO, 6, 0.3, 0.05, 0.055, 0.036, petal, pale);
            p.bell(head, Vec3::Y * 0.048, Vec3::NEG_Y, 0.024, 0.045, center);
            p.ball(
                head,
                Vec3::Y * 0.004,
                Vec3::splat(0.012),
                center,
                true,
                false,
            );
        }
        FlowerShape::Cup => {
            let top = Vec3::new(lean.x * 2.0, h, lean.z * 2.0);
            p.stem(id, top, 0.0035);
            p.leaf(id, Vec3::Y * 0.02, 0.5, 0.4, 0.1, 0.035);
            p.leaf(id, Vec3::Y * 0.02, 3.4, 0.45, 0.09, 0.03);
            let head = Transform::from_translation(top);
            p.ring(head, Vec3::ZERO, 4, 0.0, 0.95, 0.05, 0.058, deep, petal);
            p.ring(
                head,
                Vec3::Y * 0.004,
                4,
                PI / 4.0,
                0.75,
                0.046,
                0.052,
                deep,
                mix(petal, pale, 0.4),
            );
            p.ball(
                head,
                Vec3::Y * 0.012,
                Vec3::new(0.012, 0.01, 0.012),
                center,
                true,
                false,
            );
        }
        FlowerShape::Bells => {
            // Arching stem; bells hang along the arch.
            let top = Vec3::new(0.09, h, 0.0);
            p.stem(id, top, 0.0028);
            for i in 0..3 {
                p.leaf(id, Vec3::ZERO, i as f32 * 2.3 + 1.0, 1.1, h * 0.9, 0.012);
            }
            for i in 0..5 {
                let t = 0.55 + i as f32 * 0.1;
                let at = Vec3::new(top.x * t * t, h * (0.6 + t * 0.4) - (t - 0.5) * 0.06, 0.0);
                let side = if i % 2 == 0 { 0.012 } else { -0.012 };
                let tip = at + Vec3::new(0.0, -0.012, side);
                p.tube(id, at, tip, 0.0012, p.stem);
                p.bell(
                    id,
                    tip,
                    Vec3::new(0.15, -1.0, side * 10.0),
                    0.01,
                    0.02,
                    petal,
                );
            }
        }
        FlowerShape::Pompom => {
            let top = Vec3::new(lean.x * 0.5, h, lean.z * 0.5);
            p.stem(id, top, 0.0025);
            // Trefoil leaves.
            for k in 0..3 {
                let base = Vec3::new(
                    (k as f32 * 2.1).cos() * 0.03,
                    0.03,
                    (k as f32 * 2.1).sin() * 0.03,
                );
                for l in 0..3 {
                    p.leaf(id, base, l as f32 * TAU / 3.0 + k as f32, 0.15, 0.022, 0.02);
                }
            }
            p.ball(
                id,
                top + Vec3::Y * 0.014,
                Vec3::splat(0.017),
                deep,
                true,
                false,
            );
            for i in 0..22 {
                let y = 1.0 - (i as f32 + 0.5) / 22.0 * 1.6;
                let r = (1.0 - y * y).max(0.0).sqrt();
                let a = i as f32 * 2.4;
                let dir = Vec3::new(a.cos() * r, y, a.sin() * r);
                let c = if y > 0.0 { petal } else { pale };
                p.ball(
                    id,
                    top + Vec3::Y * 0.014 + dir * 0.019,
                    Vec3::splat(0.0065),
                    c,
                    true,
                    true,
                );
            }
        }
        FlowerShape::Spike => {
            for s in 0..5 {
                let a = s as f32 * 1.26 + 0.3;
                let spread = if s == 0 { 0.0 } else { 0.07 };
                let top = Vec3::new(
                    a.cos() * spread,
                    h * (0.85 + (s as f32 * 0.37).sin().abs() * 0.15),
                    a.sin() * spread,
                );
                p.stem(id, top, 0.0022);
                for b in 0..9 {
                    let t = 1.0 - b as f32 * 0.035;
                    let at = top * t;
                    let r = 0.006 + b as f32 * 0.0006;
                    let c = mix(petal, center, b as f32 / 9.0);
                    p.ball(id, at, Vec3::new(r, r * 1.4, r), c, true, true);
                }
            }
            for i in 0..6 {
                p.leaf(id, Vec3::ZERO, i as f32 * 1.05, 1.0, 0.11, 0.008);
            }
        }
        FlowerShape::Tulip => {
            let top = Vec3::new(lean.x, h, lean.z);
            p.stem(id, top, 0.0045);
            p.leaf(id, Vec3::Y * 0.01, 0.3, 1.05, h * 0.55, 0.05);
            p.leaf(id, Vec3::Y * 0.02, 3.5, 1.15, h * 0.5, 0.045);
            let head = Transform::from_translation(top);
            p.ring(head, Vec3::ZERO, 3, 0.0, 1.28, 0.065, 0.04, deep, petal);
            p.ring(
                head,
                Vec3::ZERO,
                3,
                PI / 3.0,
                1.22,
                0.062,
                0.042,
                deep,
                mix(petal, pale, 0.3),
            );
        }
        FlowerShape::Sunflower => {
            let top = Vec3::new(0.0, h, 0.06);
            p.stem(id, top, 0.012);
            for i in 0..4 {
                let y = 0.25 + i as f32 * 0.2;
                p.leaf(id, Vec3::Y * h * y, i as f32 * PI + 0.4, 0.15, 0.16, 0.11);
            }
            // Face looks out sideways.
            let head = Transform::from_translation(top)
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2 - 0.15));
            p.ring(head, Vec3::ZERO, 18, 0.0, 0.08, 0.085, 0.03, deep, petal);
            p.ring(
                head,
                Vec3::Y * -0.004,
                18,
                PI / 18.0,
                0.02,
                0.075,
                0.03,
                deep,
                petal,
            );
            p.ball(
                head,
                Vec3::Y * 0.006,
                Vec3::new(0.07, 0.018, 0.07),
                center,
                true,
                false,
            );
        }
        FlowerShape::Rose => {
            let top = Vec3::new(lean.x, h, lean.z);
            p.stem(id, top, 0.0042);
            for i in 0..3 {
                let base = Vec3::Y * h * (0.35 + i as f32 * 0.17);
                let yaw = i as f32 * 2.3;
                for l in 0..3 {
                    p.leaf(id, base, yaw + (l as f32 - 1.0) * 0.5, 0.25, 0.04, 0.024);
                }
            }
            let head = Transform::from_translation(top);
            p.ball(
                head,
                Vec3::Y * 0.02,
                Vec3::new(0.014, 0.024, 0.014),
                center,
                true,
                false,
            );
            for (ring, (n, tilt, len)) in [(3, 1.35, 0.03), (5, 1.05, 0.036), (6, 0.65, 0.042)]
                .into_iter()
                .enumerate()
            {
                let offset = ring as f32 * 0.7;
                p.ring(
                    head,
                    Vec3::Y * 0.004,
                    n,
                    offset,
                    tilt,
                    len,
                    0.034,
                    center,
                    mix(petal, pale, ring as f32 * 0.15),
                );
            }
        }
        FlowerShape::Ruffle => {
            let top = Vec3::new(lean.x, h, lean.z);
            p.stem(id, top, 0.005);
            for i in 0..3 {
                p.leaf(id, Vec3::Y * h * 0.4, i as f32 * 2.1, 0.3, 0.11, 0.045);
            }
            let head = Transform::from_translation(top);
            for ring in 0..4 {
                let n = 6 + ring * 2;
                let tilt = 1.3 - ring as f32 * 0.28;
                let jitter = hash3(Vec3::splat(ring as f32)) * 0.4;
                let c = mix(deep, pale, ring as f32 * 0.25);
                p.ring(
                    head,
                    Vec3::Y * 0.006,
                    n,
                    jitter,
                    tilt,
                    0.04 + ring as f32 * 0.006,
                    0.042,
                    deep,
                    c,
                );
            }
        }
        FlowerShape::Mophead => {
            for s in 0..3 {
                let a = s as f32 * 2.1 + 0.2;
                let top = Vec3::new(a.cos() * 0.09, h * (0.85 + s as f32 * 0.07), a.sin() * 0.09);
                p.stem(id, top, 0.005);
                p.leaf(id, top * 0.55, a, 0.2, 0.12, 0.07);
                p.leaf(id, top * 0.35, a + PI, 0.25, 0.11, 0.065);
                for i in 0..28 {
                    let y = 1.0 - (i as f32 + 0.5) / 28.0 * 1.4;
                    let r = (1.0 - y * y).max(0.0).sqrt();
                    let ang = i as f32 * 2.4 + s as f32;
                    let dir = Vec3::new(ang.cos() * r, y, ang.sin() * r);
                    let at = top + Vec3::Y * 0.035 + dir * 0.065;
                    let tint = mix(petal, center, hash3(at * 40.0) * 0.5 + 0.5);
                    let frame = Transform::from_translation(at)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir));
                    for k in 0..4 {
                        p.blade(
                            frame,
                            Vec3::ZERO,
                            k as f32 * FRAC_PI_2,
                            0.1,
                            0.016,
                            0.014,
                            0.003,
                            tint,
                            mix(tint, LinearRgba::WHITE, 0.2),
                            true,
                        );
                    }
                }
            }
        }
        FlowerShape::Foxglove => {
            let top = Vec3::new(lean.x * 2.0, h, lean.z * 2.0);
            p.stem(id, top, 0.006);
            for i in 0..7 {
                p.leaf(id, Vec3::Y * 0.01, i as f32 * 0.9, 0.18, 0.17, 0.06);
            }
            for b in 0..11 {
                let t = 0.45 + b as f32 * 0.05;
                let at = top * t;
                let a = b as f32 * 0.55 - 0.6; // bells mostly on one side
                let out = Vec3::new(a.cos(), -0.9, a.sin());
                let size = 1.0 - b as f32 * 0.05;
                p.bell(id, at + out * 0.004, out, 0.016 * size, 0.042 * size, petal);
            }
            p.ball(id, top, Vec3::new(0.006, 0.012, 0.006), deep, true, false);
        }
        FlowerShape::Orchid => {
            let top = Vec3::new(0.12, h, 0.02);
            p.stem(id, top, 0.0025);
            p.leaf(id, Vec3::Y * 0.01, 0.2, 0.12, 0.16, 0.05);
            p.leaf(id, Vec3::Y * 0.01, 3.0, 0.1, 0.15, 0.048);
            for i in 0..3 {
                let t = 0.65 + i as f32 * 0.17;
                let at = Vec3::new(top.x * t * t, h * t, top.z * t);
                let head =
                    Transform::from_translation(at).with_rotation(Quat::from_rotation_z(-1.2));
                // Three narrow sepals, two broad petals, one large lip.
                p.ring(head, Vec3::ZERO, 3, 0.0, 0.15, 0.032, 0.012, petal, pale);
                p.blade(
                    head,
                    Vec3::ZERO,
                    PI * 0.4,
                    0.25,
                    0.028,
                    0.026,
                    0.004,
                    petal,
                    pale,
                    true,
                );
                p.blade(
                    head,
                    Vec3::ZERO,
                    -PI * 0.4,
                    0.25,
                    0.028,
                    0.026,
                    0.004,
                    petal,
                    pale,
                    true,
                );
                p.blade(
                    head,
                    Vec3::ZERO,
                    PI,
                    -0.3,
                    0.032,
                    0.03,
                    0.005,
                    center,
                    deep,
                    true,
                );
            }
        }
        FlowerShape::Lily => {
            let top = Vec3::new(lean.x, h, lean.z);
            p.stem(id, top, 0.005);
            for i in 0..8 {
                p.leaf(
                    id,
                    Vec3::Y * h * (0.2 + i as f32 * 0.07),
                    i as f32 * 2.4,
                    0.5,
                    0.09,
                    0.012,
                );
            }
            let head = Transform::from_translation(top).with_rotation(Quat::from_rotation_x(-0.6));
            for i in 0..6 {
                let yaw = i as f32 * TAU / 6.0;
                // Two segments: rising, then curling back.
                let rot = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-0.9);
                let mid = rot * Vec3::new(0.0, 0.0, 0.045);
                p.blade(
                    head,
                    Vec3::ZERO,
                    yaw,
                    0.9,
                    0.05,
                    0.026,
                    0.004,
                    center,
                    petal,
                    true,
                );
                p.blade(head, mid, yaw, 0.15, 0.04, 0.022, 0.004, petal, pale, true);
            }
            for i in 0..6 {
                let a = i as f32 * TAU / 6.0 + 0.5;
                let tip = Vec3::new(a.cos() * 0.016, 0.05, a.sin() * 0.016);
                p.tube(head, Vec3::ZERO, tip, 0.0009, center);
                p.ball(
                    head,
                    tip,
                    Vec3::new(0.003, 0.0025, 0.006),
                    lin(Color::srgb_u8(190, 120, 70)),
                    true,
                    true,
                );
            }
        }
        FlowerShape::StarCluster => {
            for b in 0..7 {
                let a = b as f32 * 0.9;
                let tip = Vec3::new(
                    a.cos() * 0.14,
                    h * (0.7 + (b as f32 * 0.7).sin().abs() * 0.3),
                    a.sin() * 0.14,
                );
                p.stem(id, tip, 0.0025);
                for l in 0..3 {
                    p.leaf(
                        id,
                        tip * (0.4 + l as f32 * 0.18),
                        a + l as f32 * 2.0,
                        0.3,
                        0.035,
                        0.016,
                    );
                }
                for f in 0..2 {
                    let at = tip
                        + Vec3::new(
                            (f as f32 * 2.0 + a).cos() * 0.02,
                            0.004 * f as f32,
                            (f as f32 * 2.0 + a).sin() * 0.02,
                        );
                    let head = Transform::from_translation(at);
                    p.ring(head, Vec3::ZERO, 5, a, 0.2, 0.014, 0.007, center, petal);
                }
            }
        }
        FlowerShape::Moonbloom => {
            let top = Vec3::new(lean.x * 2.0, h, lean.z * 2.0);
            p.stem(id, top, 0.004);
            for i in 0..3 {
                p.ball(
                    id,
                    Vec3::new(
                        (i as f32 * 2.1).cos() * 0.06,
                        h * (0.3 + i as f32 * 0.15),
                        (i as f32 * 2.1).sin() * 0.06,
                    ),
                    Vec3::new(0.035, 0.006, 0.03),
                    p.leaf_dark,
                    false,
                    false,
                );
            }
            let head = Transform::from_translation(top);
            p.bell(head, Vec3::ZERO, Vec3::Y, 0.06, 0.05, petal);
            p.ring(
                head,
                Vec3::Y * 0.048,
                5,
                0.0,
                0.25,
                0.03,
                0.035,
                center,
                pale,
            );
            p.ball(
                head,
                Vec3::Y * 0.02,
                Vec3::splat(0.008),
                center,
                true,
                false,
            );
        }
        FlowerShape::Star => {
            let top = Vec3::new(lean.x, h, lean.z);
            p.stem(id, top, 0.0035);
            p.leaf(id, Vec3::Y * 0.02, 0.4, 0.5, 0.08, 0.018);
            p.leaf(id, Vec3::Y * 0.03, 3.6, 0.55, 0.07, 0.016);
            let head = Transform::from_translation(top).with_rotation(Quat::from_rotation_x(-0.2));
            p.ring(head, Vec3::ZERO, 5, 0.0, 0.18, 0.06, 0.02, center, petal);
            p.ball(
                head,
                Vec3::Y * 0.006,
                Vec3::splat(0.012),
                center,
                true,
                false,
            );
        }
    }
    p.finish()
}

/// Small flat petal used for harvest particles (unit size).
pub fn petal_particle() -> Mesh {
    Sphere::new(1.0).mesh().ico(0).expect("valid icosphere")
}
