//! A point on a part's surface, written the way a recipe declares a
//! socket: which side of the part it is on, and how far it is slid
//! across that side.
//!
//! This is what lets a click in the Studio become an `add-socket` edit:
//! the click gives a point of the model, and a recipe wants `on =
//! "front", shift = [0.4, -0.2, 0]`.

use crate::math::{Bounds, Vec3};
use crate::model::{Model, Part};
use crate::recipe::Side;

/// Where a socket goes, as `add-socket` and `set-socket` take it.
#[derive(Debug, Clone, PartialEq)]
pub struct Spot {
    /// The side of the part: `top`, `bottom`, `left`, `right`, `front` or
    /// `back`.
    pub on: String,
    /// How far it is slid across that side, -1 to 1 each way.
    pub shift: [f64; 3],
}

/// Works out the side and the slide for a socket at `point` on `part`,
/// where the surface faces `normal` (both as they are in the model).
///
/// A socket declared with them sits on the box around the part, slid
/// across one side, and then moves straight in until it touches the
/// part. So the side is the one the surface faces most, and the slide is
/// where the point is across that side. `None` for a part with no faces,
/// or one squashed flat.
pub fn spot(model: &Model, part: &Part, point: Vec3, normal: Vec3) -> Option<Spot> {
    // Back from the model to the part's own space, where its sockets are
    // declared: before it is mirrored, scaled, turned and moved.
    let back = part.place.turn.inverse()?;
    let own = |at: Vec3| back * (at - part.place.shift);
    let bounds = Bounds::around(
        model.faces[part.faces.clone()]
            .iter()
            .flat_map(|face| face.corners.iter().map(|&corner| own(corner))),
    )?;
    // Which way a surface faces goes back by the turn's rows and columns
    // swapped (a stretch would tilt it otherwise).
    let facing = (part.place.turn.transposed() * normal).normalized()?;
    let side = Side::ALL
        .into_iter()
        .filter(|side| *side != Side::Center)
        .max_by(|a, b| {
            let (a, b) = (
                facing.dot(Vec3::from(a.facing())),
                facing.dot(Vec3::from(b.facing())),
            );
            a.total_cmp(&b)
        })?;
    let (center, half, at) = (bounds.center(), bounds.half(), own(point));
    let across = |at: f64, center: f64, half: f64, along: f64| {
        if along != 0.0 || half <= 0.0 {
            // Along the side's own direction there is no slide.
            0.0
        } else {
            let slid = ((at - center) / half).clamp(-1.0, 1.0);
            (slid * 1000.0).round() / 1000.0 + 0.0
        }
    };
    let along = side.facing();
    Some(Spot {
        on: side.name().to_owned(),
        shift: [
            across(at.x, center.x, half.x, along[0]),
            across(at.y, center.y, half.y, along[1]),
            across(at.z, center.z, half.z, along[2]),
        ],
    })
}
