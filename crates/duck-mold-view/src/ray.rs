//! What a ray hits: the first hit shape on it, as a game would find it.
//!
//! Duck Mold's hit shapes are handed to Duck Forge's own test
//! ([`duckforge::hits::HitVolume::ray`]), so a click in the Studio, or
//! `duck-mold inspect --ray`, finds what a game's click finds: the same
//! code, not a copy.

use duck_mold_core::math::{Vec3, vec3};
use duck_mold_core::model::{HitShape, Model};
use duckforge::hits::{HitVolume, Ray};

/// The first hit shape a ray meets.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RayHit {
    /// The part, by path; empty for one shape round the whole model.
    pub part: String,
    /// What the shape is: `box`, `ball`, `capsule`.
    pub shape: &'static str,
    /// Where the ray meets it, in the model.
    pub point: [f64; 3],
    /// How far along the ray, from its start.
    pub distance: f64,
}

fn forge(point: Vec3) -> duckforge::prelude::Vec3 {
    duckforge::prelude::vec3(point.x as f32, point.y as f32, point.z as f32)
}

/// A hit shape as Duck Forge's.
fn volume(shape: &HitShape) -> HitVolume {
    match *shape {
        HitShape::Box { center, axes, half } => HitVolume::Box {
            center: forge(center),
            axes: [
                forge(axes.column(0)),
                forge(axes.column(1)),
                forge(axes.column(2)),
            ],
            half: forge(half),
        },
        HitShape::Sphere { center, radius } => HitVolume::Ball {
            center: forge(center),
            radius: radius as f32,
        },
        HitShape::Capsule { a, b, radius } => HitVolume::Capsule {
            a: forge(a),
            b: forge(b),
            radius: radius as f32,
        },
    }
}

/// The word for a shape: `box`, `ball`, `capsule`.
fn shape_word(shape: &HitShape) -> &'static str {
    match shape {
        HitShape::Box { .. } => "box",
        HitShape::Sphere { .. } => "ball",
        HitShape::Capsule { .. } => "capsule",
    }
}

/// The first of `model`'s hit shapes on the ray from `from` going
/// `toward` (any length but 0), as the model stands (posed, if it is).
/// `None` if it misses every one, or the model has none.
pub fn first_hit(model: &Model, from: Vec3, toward: Vec3) -> Option<RayHit> {
    if toward.length() == 0.0 {
        return None;
    }
    let ray = Ray::new(forge(from), forge(toward));
    model
        .colliders
        .iter()
        .filter_map(|collider| {
            let (distance, _) = volume(&collider.shape).ray(&ray)?;
            Some((distance, collider))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(distance, collider)| {
            let at = ray.at(distance);
            RayHit {
                part: collider.part.clone(),
                shape: shape_word(&collider.shape),
                point: [tidy(at.x), tidy(at.y), tidy(at.z)],
                distance: tidy(distance),
            }
        })
}

/// A number worked out in Duck Forge's single precision, to the tenth of
/// a millimetre: what it can be trusted to.
fn tidy(number: f32) -> f64 {
    (f64::from(number) * 10_000.0).round() / 10_000.0
}

/// `x,y,z` as a point: for `--ray` and `--toward`.
pub fn parse_point(text: &str) -> Option<Vec3> {
    let numbers: Vec<f64> = text
        .split(',')
        .map(|number| number.trim().parse().ok())
        .collect::<Option<Vec<f64>>>()?;
    match numbers.as_slice() {
        [x, y, z] => Some(vec3(*x, *y, *z)),
        _ => None,
    }
}

/// What `duck-mold inspect --ray` reports.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RayReport {
    /// True: the recipe built.
    pub ok: bool,
    /// The file.
    pub source: String,
    /// Where the ray starts, in the model.
    pub from: [f64; 3],
    /// Which way it goes.
    pub toward: [f64; 3],
    /// The pose the model stands in, or `rest`; for an animation, its
    /// name and the time.
    pub stands: String,
    /// The first hit shape on it, if any.
    pub hit: Option<RayHit>,
    /// How many hit shapes the model has.
    pub hit_shapes: usize,
}

impl RayReport {
    /// The report as a few lines of words.
    pub fn to_text(&self) -> String {
        let point = |p: [f64; 3]| format!("{:.3}, {:.3}, {:.3}", p[0], p[1], p[2]);
        let mut text = format!(
            "a ray from ({}) toward ({}), the model {}  ({})\n",
            point(self.from),
            point(self.toward),
            self.stands,
            self.source
        );
        match &self.hit {
            Some(hit) => text.push_str(&format!(
                "  hits         {} (its {}), {:.3} m away, at ({})\n",
                if hit.part.is_empty() { "the whole model" } else { &hit.part },
                hit.shape,
                hit.distance,
                point(hit.point)
            )),
            None if self.hit_shapes == 0 => text.push_str(
                "  hits         nothing: the model has no hit shapes (colliders = \"parts\" gives every part one)\n",
            ),
            None => text.push_str("  hits         nothing\n"),
        }
        text
    }
}

/// `duck-mold inspect <recipe> --ray x,y,z --toward x,y,z`: the first hit
/// shape on the ray, with the model at rest, in `pose`, or at time `at`
/// of `animation`.
pub fn ray_file(
    path: &std::path::Path,
    from: Vec3,
    toward: Vec3,
    pose: Option<&str>,
    animation: Option<(&str, f64)>,
) -> Result<RayReport, duck_mold_core::tools::Failure> {
    use duck_mold_core::motion::{Motion, posed};
    use duck_mold_core::tools;
    let built = tools::build(path)?;
    let (model, stands) = match (pose, animation) {
        (_, Some((name, at))) => {
            let played = tools::animation_named(&built.model, name, &built.source)?;
            let joints = Motion::of(&built.model, played).joints_at(at);
            (
                posed(&built.model, &joints),
                format!("in \"{name}\" at {at} s"),
            )
        }
        (Some(name), None) => {
            let pose = tools::pose_named(&built.model, name, &built.source)?;
            (
                posed(&built.model, &pose.joints),
                format!("in the pose \"{name}\""),
            )
        }
        (None, None) => (built.model.clone(), "at rest".to_owned()),
    };
    Ok(RayReport {
        ok: true,
        source: built.source.clone(),
        from: [from.x, from.y, from.z],
        toward: [toward.x, toward.y, toward.z],
        stands,
        hit: first_hit(&model, from, toward),
        hit_shapes: model.colliders.len(),
    })
}
