//! Points, turns and boxes: the small amount of maths Duck Mold needs.
//!
//! Directions are Duck Forge's: x is right, y is up, z is forward.
//!
//! Everything here is built from plain arithmetic (`+ - * /`) and square
//! roots, which every computer works out to exactly the same digits. The
//! sine and cosine that the operating system supplies can differ in the
//! last digit between Windows, Linux and Mac, so this file has its own
//! ([`sin_cos_degrees`]). That is what lets the same recipe give the same
//! bytes everywhere.

use std::ops::{Add, Mul, Neg, Sub};

/// A point or a direction: x (right), y (up), z (forward).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    /// Right.
    pub x: f64,
    /// Up.
    pub y: f64,
    /// Forward.
    pub z: f64,
}

/// Short for `Vec3 { x, y, z }`.
pub const fn vec3(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3 { x, y, z }
}

impl Vec3 {
    /// `(0, 0, 0)`.
    pub const ZERO: Vec3 = vec3(0.0, 0.0, 0.0);
    /// Up: `(0, 1, 0)`.
    pub const UP: Vec3 = vec3(0.0, 1.0, 0.0);
    /// Forward: `(0, 0, 1)`.
    pub const FORWARD: Vec3 = vec3(0.0, 0.0, 1.0);

    /// The three numbers as a list.
    pub fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    /// How much two directions point the same way: 1 for the same, 0 for
    /// square to each other, -1 for opposite (when both are 1 long).
    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// A direction square to both. For a triangle's corners `a`, `b`, `c`
    /// in Duck Forge's order, `(b - a).cross(c - a)` points out of its
    /// front.
    pub fn cross(self, other: Vec3) -> Vec3 {
        vec3(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// How long it is.
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// The same direction, 1 long. `None` for a direction with no length.
    pub fn normalized(self) -> Option<Vec3> {
        let length = self.length();
        (length > 1e-12).then(|| self * (1.0 / length))
    }

    /// Each number multiplied by the other's: `(x * x, y * y, z * z)`.
    pub fn times(self, other: Vec3) -> Vec3 {
        vec3(self.x * other.x, self.y * other.y, self.z * other.z)
    }

    /// The smaller of each pair of numbers.
    pub fn min(self, other: Vec3) -> Vec3 {
        vec3(
            self.x.min(other.x),
            self.y.min(other.y),
            self.z.min(other.z),
        )
    }

    /// The bigger of each pair of numbers.
    pub fn max(self, other: Vec3) -> Vec3 {
        vec3(
            self.x.max(other.x),
            self.y.max(other.y),
            self.z.max(other.z),
        )
    }

    /// The point with each number tidied (see [`tidy`]).
    pub fn tidied(self) -> Vec3 {
        vec3(tidy(self.x), tidy(self.y), tidy(self.z))
    }
}

impl From<[f64; 3]> for Vec3 {
    fn from([x, y, z]: [f64; 3]) -> Vec3 {
        vec3(x, y, z)
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, other: Vec3) -> Vec3 {
        vec3(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, other: Vec3) -> Vec3 {
        vec3(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, by: f64) -> Vec3 {
        vec3(self.x * by, self.y * by, self.z * by)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        vec3(-self.x, -self.y, -self.z)
    }
}

/// Rounds to a thousandth of a millimetre and never gives `-0`.
///
/// Sums of decimal numbers pick up noise in their last digits (`0.1 + 0.2`
/// is `0.30000000000000004`). Rounding every finished position removes
/// it, so a model that should stand at `0` does, and reports are clean.
pub fn tidy(value: f64) -> f64 {
    // Adding 0 turns -0 into 0.
    (value * 1e6).round() / 1e6 + 0.0
}

/// The sine and cosine of an angle in degrees, the same on every
/// computer, and exact for quarter turns (the sine of 90 is exactly 1,
/// the cosine exactly 0).
pub fn sin_cos_degrees(degrees: f64) -> (f64, f64) {
    // A turn the other way has the same cosine and the opposite sine, so
    // only turns one way are worked out. Then bring the angle into one
    // full turn; `%` on decimals is exact.
    let turn = degrees.abs() % 360.0;
    // Which quarter of the turn, and how far into it.
    let quarter = (turn / 90.0).floor();
    let into = turn - quarter * 90.0;
    // Within a quarter, only angles up to 45 degrees are worked out; the
    // rest follows from "the sine of an angle is the cosine of what is
    // left of the quarter".
    let (sin, cos) = if into == 0.0 {
        (0.0, 1.0)
    } else if into == 45.0 {
        // Exactly halfway, the two are equal.
        (
            std::f64::consts::FRAC_1_SQRT_2,
            std::f64::consts::FRAC_1_SQRT_2,
        )
    } else if into < 45.0 {
        let radians = into * (std::f64::consts::PI / 180.0);
        (small_sin(radians), small_cos(radians))
    } else {
        let radians = (90.0 - into) * (std::f64::consts::PI / 180.0);
        (small_cos(radians), small_sin(radians))
    };
    // Each further quarter swaps the two and flips a sign.
    let (sin, cos) = match quarter as u8 {
        0 => (sin, cos),
        1 => (cos, -sin),
        2 => (-sin, -cos),
        _ => (-cos, sin),
    };
    // Adding 0 turns -0 into 0.
    if degrees < 0.0 {
        (-sin + 0.0, cos + 0.0)
    } else {
        (sin + 0.0, cos + 0.0)
    }
}

/// The sine of an angle from 0 to a little over 45 degrees, in radians:
/// the series `x - x^3/3! + x^5/5! - ...`, to the 17th power, which is
/// more than a decimal number can hold at this size.
fn small_sin(x: f64) -> f64 {
    let x2 = x * x;
    // Written inside out, so each step is one multiply and one add.
    let mut sum = 1.0 / 355_687_428_096_000.0; // 1 / 17!
    for inverse in [
        -1.0 / 1_307_674_368_000.0, // 15!
        1.0 / 6_227_020_800.0,      // 13!
        -1.0 / 39_916_800.0,        // 11!
        1.0 / 362_880.0,            // 9!
        -1.0 / 5040.0,              // 7!
        1.0 / 120.0,                // 5!
        -1.0 / 6.0,                 // 3!
        1.0,
    ] {
        sum = sum * x2 + inverse;
    }
    sum * x
}

/// The cosine of an angle from 0 to a little over 45 degrees, in
/// radians: the series `1 - x^2/2! + x^4/4! - ...`, to the 18th power.
fn small_cos(x: f64) -> f64 {
    let x2 = x * x;
    let mut sum = -1.0 / 6_402_373_705_728_000.0; // 1 / 18!
    for inverse in [
        1.0 / 20_922_789_888_000.0, // 16!
        -1.0 / 87_178_291_200.0,    // 14!
        1.0 / 479_001_600.0,        // 12!
        -1.0 / 3_628_800.0,         // 10!
        1.0 / 40_320.0,             // 8!
        -1.0 / 720.0,               // 6!
        1.0 / 24.0,                 // 4!
        -1.0 / 2.0,                 // 2!
        1.0,
    ] {
        sum = sum * x2 + inverse;
    }
    sum
}

/// A turn, a stretch, a flip, or any mix of them: three rows of three
/// numbers. Multiplying a point by it gives the turned (stretched,
/// flipped) point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat3 {
    /// The three rows.
    pub rows: [[f64; 3]; 3],
}

impl Mat3 {
    /// Changes nothing.
    pub const IDENTITY: Mat3 = Mat3 {
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    /// Stretches by `scale.x` along x, `scale.y` along y, `scale.z` along
    /// z. A negative number flips that direction.
    pub fn stretch(scale: Vec3) -> Mat3 {
        Mat3 {
            rows: [
                [scale.x, 0.0, 0.0],
                [0.0, scale.y, 0.0],
                [0.0, 0.0, scale.z],
            ],
        }
    }

    /// A turn around x, in Duck Forge's direction: a positive angle turns
    /// forward (+z) towards up (+y).
    pub fn around_x(degrees: f64) -> Mat3 {
        let (s, c) = sin_cos_degrees(degrees);
        Mat3 {
            rows: [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]],
        }
    }

    /// A turn around y, in Duck Forge's direction: a positive angle turns
    /// forward (+z) towards right (+x).
    pub fn around_y(degrees: f64) -> Mat3 {
        let (s, c) = sin_cos_degrees(degrees);
        Mat3 {
            rows: [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        }
    }

    /// A turn around z, in Duck Forge's direction: a positive angle turns
    /// right (+x) towards up (+y).
    pub fn around_z(degrees: f64) -> Mat3 {
        let (s, c) = sin_cos_degrees(degrees);
        Mat3 {
            rows: [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// A recipe's `rotate = [x, y, z]`: around x, then y, then z, as Duck
    /// Forge's `Transform` does.
    pub fn from_rotate(degrees: [f64; 3]) -> Mat3 {
        Mat3::around_z(degrees[2]) * Mat3::around_y(degrees[1]) * Mat3::around_x(degrees[0])
    }

    /// The other way round from [`Mat3::from_rotate`]: the `rotate = [x,
    /// y, z]` that gives this turn, in degrees, each from -180 to 180.
    /// The matrix must be a plain turn (no stretch, no flip). When the
    /// turn around y is a quarter turn, x and z can't be told apart, and
    /// all of it is given to x.
    pub fn to_rotate(&self) -> [f64; 3] {
        let m = self.rows;
        let sin_y = (-m[2][0]).clamp(-1.0, 1.0);
        let y = sin_y.asin().to_degrees();
        if sin_y.abs() > 0.999_999 {
            let x = m[1][2].atan2(m[1][1]).to_degrees();
            return [x, y, 0.0];
        }
        let x = (-m[2][1]).atan2(m[2][2]).to_degrees();
        let z = m[1][0].atan2(m[0][0]).to_degrees();
        [x, y, z]
    }

    /// One of its three columns: where it sends the direction x (0), y
    /// (1) or z (2).
    pub fn column(&self, which: usize) -> Vec3 {
        vec3(
            self.rows[0][which],
            self.rows[1][which],
            self.rows[2][which],
        )
    }

    /// A turn around any direction (`axis` must be 1 long). Seen from
    /// where `axis` points to, looking back, a positive angle is
    /// clockwise in Duck Forge's view. Around "up" it is the same turn
    /// as [`Mat3::around_y`].
    pub fn around(axis: Vec3, degrees: f64) -> Mat3 {
        let (s, c) = sin_cos_degrees(degrees);
        let Vec3 { x, y, z } = axis;
        let t = 1.0 - c;
        Mat3 {
            rows: [
                [c + t * x * x, t * x * y - s * z, t * x * z + s * y],
                [t * x * y + s * z, c + t * y * y, t * y * z - s * x],
                [t * x * z - s * y, t * y * z + s * x, c + t * z * z],
            ],
        }
    }

    /// The smallest turn that makes the direction `from` point along
    /// `to` (both 1 long).
    ///
    /// When they are exactly opposite, there are endless half turns to
    /// choose from, so there is a fixed rule: turn around the upright
    /// axis, so things stay the right way up; if the directions are
    /// themselves upright, roll over sideways (around the front-to-back
    /// axis), so the front stays in front.
    pub fn turning(from: Vec3, to: Vec3) -> Mat3 {
        let cos = from.dot(to);
        if cos > 1.0 - 1e-12 {
            return Mat3::IDENTITY;
        }
        if cos < -1.0 + 1e-9 {
            // A half turn around a direction square to `from`.
            let prefer = if from.y.abs() < 0.999 {
                Vec3::UP
            } else {
                Vec3::FORWARD
            };
            let axis = (prefer - from * prefer.dot(from))
                .normalized()
                .unwrap_or(Vec3::UP);
            let Vec3 { x, y, z } = axis;
            return Mat3 {
                rows: [
                    [2.0 * x * x - 1.0, 2.0 * x * y, 2.0 * x * z],
                    [2.0 * x * y, 2.0 * y * y - 1.0, 2.0 * y * z],
                    [2.0 * x * z, 2.0 * y * z, 2.0 * z * z - 1.0],
                ],
            };
        }
        // No angle is worked out: the turn follows from the two
        // directions with plain arithmetic.
        let w = from.cross(to);
        let k = 1.0 / (1.0 + cos);
        Mat3 {
            rows: [
                [
                    cos + k * w.x * w.x,
                    k * w.x * w.y - w.z,
                    k * w.x * w.z + w.y,
                ],
                [
                    k * w.x * w.y + w.z,
                    cos + k * w.y * w.y,
                    k * w.y * w.z - w.x,
                ],
                [
                    k * w.x * w.z - w.y,
                    k * w.y * w.z + w.x,
                    cos + k * w.z * w.z,
                ],
            ],
        }
    }

    /// The same numbers with rows and columns swapped. For a plain turn
    /// this is the turn back.
    pub fn transposed(&self) -> Mat3 {
        let r = self.rows;
        Mat3 {
            rows: [
                [r[0][0], r[1][0], r[2][0]],
                [r[0][1], r[1][1], r[2][1]],
                [r[0][2], r[1][2], r[2][2]],
            ],
        }
    }

    /// What undoes it: `m.inverse() * (m * point)` is `point` again.
    /// `None` if it squashes everything flat, so that nothing can undo
    /// it.
    pub fn inverse(&self) -> Option<Mat3> {
        let determinant = self.determinant();
        if determinant.abs() < 1e-18 {
            return None;
        }
        let [a, b, c] = self.rows;
        let rows = [
            [
                b[1] * c[2] - b[2] * c[1],
                a[2] * c[1] - a[1] * c[2],
                a[1] * b[2] - a[2] * b[1],
            ],
            [
                b[2] * c[0] - b[0] * c[2],
                a[0] * c[2] - a[2] * c[0],
                a[2] * b[0] - a[0] * b[2],
            ],
            [
                b[0] * c[1] - b[1] * c[0],
                a[1] * c[0] - a[0] * c[1],
                a[0] * b[1] - a[1] * b[0],
            ],
        ];
        Some(Mat3 {
            rows: rows.map(|row| row.map(|cell| cell / determinant)),
        })
    }

    /// Whether it flips things over (a mirror). Then a triangle's corners
    /// end up in the opposite order and have to be reversed.
    pub fn determinant(&self) -> f64 {
        let [a, b, c] = self.rows;
        a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0])
    }

    /// What it does to a direction that says which way a surface faces,
    /// such as a socket's. A plain turn turns it; a stretch must not
    /// tilt it the wrong way, which is why this isn't simply `self *
    /// direction`. The result is 1 long.
    pub fn turn_facing(&self, facing: Vec3) -> Vec3 {
        let [a, b, c] = self.rows.map(Vec3::from);
        // The rows of the "cofactor" form: each is square to the other
        // two rows.
        let rows = [b.cross(c), c.cross(a), a.cross(b)];
        let sign = if self.determinant() < 0.0 { -1.0 } else { 1.0 };
        let turned = vec3(
            rows[0].dot(facing),
            rows[1].dot(facing),
            rows[2].dot(facing),
        ) * sign;
        turned.normalized().unwrap_or(facing)
    }
}

impl Mul<Vec3> for Mat3 {
    type Output = Vec3;
    fn mul(self, point: Vec3) -> Vec3 {
        let [a, b, c] = self.rows.map(Vec3::from);
        vec3(a.dot(point), b.dot(point), c.dot(point))
    }
}

/// `a * b` does `b` first, then `a`.
impl Mul<Mat3> for Mat3 {
    type Output = Mat3;
    fn mul(self, other: Mat3) -> Mat3 {
        let mut rows = [[0.0; 3]; 3];
        for (row, out) in rows.iter_mut().enumerate() {
            for (column, cell) in out.iter_mut().enumerate() {
                *cell = (0..3)
                    .map(|k| self.rows[row][k] * other.rows[k][column])
                    .sum();
            }
        }
        Mat3 { rows }
    }
}

/// Where a part is: turned (stretched, flipped) by `turn`, then moved by
/// `shift`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// The turn, stretch and flip.
    pub turn: Mat3,
    /// The move, done last.
    pub shift: Vec3,
}

impl Transform {
    /// Changes nothing.
    pub const IDENTITY: Transform = Transform {
        turn: Mat3::IDENTITY,
        shift: Vec3::ZERO,
    };

    /// Only a move.
    pub fn moving(shift: Vec3) -> Transform {
        Transform {
            turn: Mat3::IDENTITY,
            shift,
        }
    }

    /// Where a point ends up.
    pub fn apply(&self, point: Vec3) -> Vec3 {
        self.turn * point + self.shift
    }

    /// Which way a facing direction ends up pointing (see
    /// [`Mat3::turn_facing`]).
    pub fn facing(&self, facing: Vec3) -> Vec3 {
        self.turn.turn_facing(facing)
    }

    /// This transform followed by `outer`: a part's place inside its
    /// assembly, then the assembly's place in the model.
    pub fn then(&self, outer: &Transform) -> Transform {
        Transform {
            turn: outer.turn * self.turn,
            shift: outer.apply(self.shift),
        }
    }

    /// True if it flips things over, so that triangle corners must be
    /// reversed to keep facing outwards.
    pub fn is_mirrored(&self) -> bool {
        self.turn.determinant() < 0.0
    }
}

/// The box around some points, with its sides along x, y and z.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// The corner with the smallest x, y and z.
    pub min: Vec3,
    /// The corner with the biggest x, y and z.
    pub max: Vec3,
}

impl Bounds {
    /// The box around these points. `None` if there are none.
    pub fn around(points: impl IntoIterator<Item = Vec3>) -> Option<Bounds> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let mut bounds = Bounds {
            min: first,
            max: first,
        };
        for point in points {
            bounds.min = bounds.min.min(point);
            bounds.max = bounds.max.max(point);
        }
        Some(bounds)
    }

    /// The box around this box and another.
    pub fn with(&self, other: &Bounds) -> Bounds {
        Bounds {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// The middle of the box.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// From the middle to each side.
    pub fn half(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    /// Width (x), height (y) and depth (z).
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// The gap between two boxes: 0 if they touch or overlap.
    pub fn gap_to(&self, other: &Bounds) -> f64 {
        let apart = |min_a: f64, max_a: f64, min_b: f64, max_b: f64| {
            (min_b - max_a).max(min_a - max_b).max(0.0)
        };
        vec3(
            apart(self.min.x, self.max.x, other.min.x, other.max.x),
            apart(self.min.y, self.max.y, other.min.y, other.max.y),
            apart(self.min.z, self.max.z, other.min.z, other.max.z),
        )
        .length()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-12
    }

    #[test]
    fn quarter_turns_are_exact() {
        assert_eq!(sin_cos_degrees(0.0), (0.0, 1.0));
        assert_eq!(sin_cos_degrees(90.0), (1.0, 0.0));
        assert_eq!(sin_cos_degrees(180.0), (0.0, -1.0));
        assert_eq!(sin_cos_degrees(270.0), (-1.0, 0.0));
        assert_eq!(sin_cos_degrees(360.0), (0.0, 1.0));
        assert_eq!(sin_cos_degrees(-90.0), (-1.0, 0.0));
        assert_eq!(sin_cos_degrees(-180.0), (0.0, -1.0));
        // Never a negative zero.
        assert!(sin_cos_degrees(-180.0).0.is_sign_positive());
        assert!(sin_cos_degrees(180.0).0.is_sign_positive());
        let half = std::f64::consts::FRAC_1_SQRT_2;
        assert_eq!(sin_cos_degrees(45.0), (half, half));
        assert_eq!(sin_cos_degrees(135.0), (half, -half));
        assert_eq!(sin_cos_degrees(-45.0), (-half, half));
        assert_eq!(sin_cos_degrees(720.0 + 90.0), (1.0, 0.0));
    }

    #[test]
    fn sine_and_cosine_agree_with_the_usual_ones() {
        // The system's own sine and cosine may differ from ours in the
        // last digits (mostly because turning big angles into radians
        // loses a little); no more.
        let mut degrees = -720.0;
        while degrees <= 720.0 {
            let (sin, cos) = sin_cos_degrees(degrees);
            let radians = degrees.to_radians();
            assert!((sin - radians.sin()).abs() < 1e-14, "sin {degrees}");
            assert!((cos - radians.cos()).abs() < 1e-14, "cos {degrees}");
            // Always a point on the circle.
            assert!((sin * sin + cos * cos - 1.0).abs() < 1e-15, "{degrees}");
            degrees += 0.37;
        }
    }

    #[test]
    fn half_and_whole_angles_mirror_each_other() {
        for degrees in [1.0, 12.5, 30.0, 44.0, 45.0, 60.0, 89.0, 0.37, 123.456] {
            let (sin, cos) = sin_cos_degrees(degrees);
            assert_eq!(sin_cos_degrees(-degrees), (-sin, cos));
        }
        for degrees in [1.0, 12.5, 30.0, 44.0, 45.0, 60.0, 89.0] {
            let (sin, cos) = sin_cos_degrees(degrees);
            assert_eq!(sin_cos_degrees(-degrees), (-sin, cos));
            assert_eq!(sin_cos_degrees(180.0 - degrees), (sin, -cos));
            assert_eq!(sin_cos_degrees(90.0 - degrees), (cos, sin));
        }
    }

    #[test]
    fn a_turn_goes_back_to_its_rotate_numbers() {
        // Every mix of turns, a quarter turn around y among them (where x
        // and z can't be told apart): the numbers that come back give
        // the same turn.
        let angles = [
            -170.0, -90.0, -45.0, -15.0, 0.0, 15.0, 30.0, 90.0, 135.0, 180.0,
        ];
        for x in angles {
            for y in angles {
                for z in angles {
                    let turn = Mat3::from_rotate([x, y, z]);
                    let back = Mat3::from_rotate(turn.to_rotate());
                    for row in 0..3 {
                        for column in 0..3 {
                            let off = (turn.rows[row][column] - back.rows[row][column]).abs();
                            assert!(
                                off < 1e-9,
                                "[{x}, {y}, {z}] came back as {:?}",
                                turn.to_rotate()
                            );
                        }
                    }
                }
            }
        }
        // A turn around one direction comes back as itself.
        let back = Mat3::from_rotate([0.0, 45.0, 0.0]).to_rotate();
        assert!((back[1] - 45.0).abs() < 1e-9 && back[0].abs() < 1e-9 && back[2].abs() < 1e-9);
        assert_eq!(Mat3::IDENTITY.to_rotate(), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_matrix_is_undone_by_its_inverse() {
        let m = Mat3::from_rotate([20.0, 40.0, 10.0]) * Mat3::stretch(vec3(1.0, 2.0, -0.5));
        let point = vec3(0.3, -1.2, 2.5);
        let back = m.inverse().unwrap() * (m * point);
        assert!((back - point).length() < 1e-12);
        assert!(Mat3::stretch(vec3(1.0, 0.0, 1.0)).inverse().is_none());
        assert_eq!(m.column(1), m * vec3(0.0, 1.0, 0.0));
    }

    #[test]
    fn tidy_removes_noise_and_negative_zero() {
        assert_eq!(tidy(0.1 + 0.2), 0.3);
        assert_eq!(tidy(-0.000_000_01), 0.0);
        assert!(tidy(-0.000_000_01).is_sign_positive());
        assert_eq!(tidy(1.234_567_89), 1.234_568);
        assert_eq!(tidy(-2.5), -2.5);
    }

    #[test]
    fn turns_go_duck_forges_way() {
        let (right, up, forward) = (vec3(1.0, 0.0, 0.0), Vec3::UP, Vec3::FORWARD);
        // Duck Forge: around x, forward turns towards up.
        assert_eq!(Mat3::around_x(90.0) * forward, up);
        // Around y, forward turns towards right.
        assert_eq!(Mat3::around_y(90.0) * forward, right);
        // Around z, right turns towards up.
        assert_eq!(Mat3::around_z(90.0) * right, up);
    }

    #[test]
    fn rotate_turns_around_x_then_y_then_z() {
        // Duck Forge's `Transform::apply`, written out.
        let forge = |p: Vec3, [x, y, z]: [f64; 3]| {
            let (s, c) = sin_cos_degrees(x);
            let p = vec3(p.x, p.y * c + p.z * s, -p.y * s + p.z * c);
            let (s, c) = sin_cos_degrees(y);
            let p = vec3(p.x * c + p.z * s, p.y, -p.x * s + p.z * c);
            let (s, c) = sin_cos_degrees(z);
            vec3(p.x * c - p.y * s, p.x * s + p.y * c, p.z)
        };
        let point = vec3(0.3, -1.2, 2.0);
        for rotate in [[90.0, 0.0, 0.0], [10.0, 20.0, 30.0], [-45.0, 135.0, 270.0]] {
            assert!(
                close(Mat3::from_rotate(rotate) * point, forge(point, rotate)),
                "{rotate:?}"
            );
        }
    }

    #[test]
    fn turning_around_up_is_the_y_turn() {
        for degrees in [90.0, 30.0, -120.0] {
            let point = vec3(0.5, 2.0, -1.0);
            assert!(close(
                Mat3::around(Vec3::UP, degrees) * point,
                Mat3::around_y(degrees) * point
            ));
        }
        // Around "forward" it is the z turn.
        assert_eq!(
            Mat3::around(Vec3::FORWARD, 90.0) * vec3(1.0, 0.0, 0.0),
            Vec3::UP
        );
    }

    #[test]
    fn the_smallest_turn_between_two_directions() {
        let (right, up, forward) = (vec3(1.0, 0.0, 0.0), Vec3::UP, Vec3::FORWARD);
        // Already there: nothing turns.
        assert_eq!(Mat3::turning(up, up), Mat3::IDENTITY);
        // Up to right: a quarter turn that leaves the front in front.
        let turn = Mat3::turning(up, right);
        assert_eq!(turn * up, right);
        assert_eq!(turn * forward, forward);
        // Any two directions.
        let from = vec3(1.0, 2.0, -0.5).normalized().unwrap();
        let to = vec3(-0.3, 0.1, 1.0).normalized().unwrap();
        let turn = Mat3::turning(from, to);
        assert!(close(turn * from, to));
        assert!((turn.determinant() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn opposite_directions_follow_the_fixed_rule() {
        let (right, up, forward) = (vec3(1.0, 0.0, 0.0), Vec3::UP, Vec3::FORWARD);
        // Sideways: turn around the upright axis, staying the right way up.
        let turn = Mat3::turning(right, -right);
        assert_eq!(turn * right, -right);
        assert_eq!(turn * up, up);
        let turn = Mat3::turning(forward, -forward);
        assert_eq!(turn * forward, -forward);
        assert_eq!(turn * up, up);
        // Upright: roll over sideways, the front staying in front.
        let turn = Mat3::turning(up, -up);
        assert_eq!(turn * up, -up);
        assert_eq!(turn * forward, forward);
        // A slanted direction still ends up exactly opposite.
        let slanted = vec3(1.0, 1.0, 0.0).normalized().unwrap();
        assert!(close(Mat3::turning(slanted, -slanted) * slanted, -slanted));
    }

    #[test]
    fn a_facing_direction_survives_stretching_and_flipping() {
        let stretch = Mat3::stretch(vec3(1.0, 4.0, 1.0));
        // A side that faces up still faces up when the part is made taller.
        assert_eq!(stretch.turn_facing(Vec3::UP), Vec3::UP);
        // A flip across x turns "facing right" into "facing left".
        let flip = Mat3::stretch(vec3(-1.0, 1.0, 1.0));
        assert_eq!(flip.turn_facing(vec3(1.0, 0.0, 0.0)), vec3(-1.0, 0.0, 0.0));
        assert_eq!(flip.turn_facing(Vec3::UP), Vec3::UP);
        // A turn turns it.
        assert_eq!(
            Mat3::around_y(90.0).turn_facing(Vec3::FORWARD),
            vec3(1.0, 0.0, 0.0)
        );
        assert!(flip.determinant() < 0.0);
    }

    #[test]
    fn transforms_chain_inside_out() {
        let inner = Transform {
            turn: Mat3::around_y(90.0),
            shift: vec3(1.0, 0.0, 0.0),
        };
        let outer = Transform::moving(vec3(0.0, 5.0, 0.0));
        let both = inner.then(&outer);
        let point = Vec3::FORWARD;
        assert_eq!(both.apply(point), outer.apply(inner.apply(point)));
        assert_eq!(both.apply(point), vec3(2.0, 5.0, 0.0));
        assert!(!both.is_mirrored());
    }

    #[test]
    fn bounds_measure_boxes() {
        let bounds = Bounds::around([vec3(-1.0, 0.0, 2.0), vec3(3.0, -2.0, 4.0)]).unwrap();
        assert_eq!(bounds.min, vec3(-1.0, -2.0, 2.0));
        assert_eq!(bounds.size(), vec3(4.0, 2.0, 2.0));
        assert_eq!(bounds.center(), vec3(1.0, -1.0, 3.0));
        assert!(Bounds::around([]).is_none());
        let far = Bounds::around([vec3(6.0, 0.0, 2.0), vec3(7.0, 1.0, 3.0)]).unwrap();
        assert_eq!(bounds.gap_to(&far), 3.0);
        assert_eq!(bounds.gap_to(&bounds), 0.0);
    }
}
