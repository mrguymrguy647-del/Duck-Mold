//! `duck-mold edit`: the words of the command line turned into an
//! [`Edit`], which the library carries out. The Studio builds the same
//! `Edit` values from clicks, so whatever it can do, this can.

use clap::{Args, Subcommand};
use duck_mold_core::edit::{Changes, Edit, Made, Sizes};

/// One change to a recipe.
#[derive(Subcommand)]
pub enum Change {
    /// Add a part, joined to a socket.
    ///
    /// Say what it is made of with one of --shape, --use and --file.
    Add {
        /// The new part's name.
        name: String,
        /// The socket it joins, like head.top.
        #[arg(long, value_name = "PART.SOCKET")]
        to: String,
        /// One of the five shapes: box, cylinder, sphere, cone, wedge.
        #[arg(long, group = "made")]
        shape: Option<String>,
        /// A part the recipe defines.
        #[arg(long = "use", value_name = "DEFINED", group = "made")]
        uses: Option<String>,
        /// A recipe or a mesh file (.obj, .gltf, .glb), from the recipe's
        /// folder.
        #[arg(long, group = "made")]
        file: Option<String>,
        /// Its own socket to join by. Without it: the side opposite the
        /// socket it joins (its bottom on a top), otherwise its bottom.
        #[arg(long, value_name = "SOCKET")]
        attach: Option<String>,
        #[command(flatten)]
        sizes: SizeOptions,
        /// Its colour, by palette name.
        #[arg(long)]
        color: Option<String>,
        /// Degrees to twist it around the join.
        #[arg(long, allow_negative_numbers = true)]
        twist: Option<f64>,
    },
    /// Remove a part.
    ///
    /// A part that others hang from is not removed, and they are listed,
    /// unless --with-attached is given.
    Remove {
        /// The part, as inspect prints it: head, left_arm/hand, or #2.
        part: String,
        /// Remove the parts that hang from it too.
        #[arg(long)]
        with_attached: bool,
    },
    /// Rename a part, and everything in the recipe that refers to it.
    Rename {
        /// The part.
        part: String,
        /// Its new name.
        new_name: String,
    },
    /// Move a part's join to another socket.
    Join {
        /// The part.
        part: String,
        /// The socket to join, like body.neck.
        #[arg(long, value_name = "PART.SOCKET")]
        to: String,
        /// Its own socket to join by, if that changes too.
        #[arg(long, value_name = "SOCKET")]
        attach: Option<String>,
    },
    /// Change a part's shape, sizes, colours or twist. Several at once
    /// are one edit.
    Set {
        /// The part.
        part: String,
        /// A new shape. Sizes the new shape doesn't have are dropped.
        #[arg(long)]
        shape: Option<String>,
        #[command(flatten)]
        sizes: SizeOptions,
        /// A new colour for the whole part, by palette name.
        #[arg(long)]
        color: Option<String>,
        /// A colour for one face, like top=plank. Can be given more than
        /// once.
        #[arg(long = "face-color", value_name = "FACE=COLOUR")]
        face_colors: Vec<String>,
        /// Degrees to twist it around its join.
        #[arg(long, allow_negative_numbers = true)]
        twist: Option<f64>,
        /// Where a free part's middle goes: x,y,z, in metres. (A joined
        /// part goes where its socket is.)
        #[arg(long = "move", value_name = "X,Y,Z", allow_hyphen_values = true)]
        move_to: Option<String>,
        /// How a free part is turned: degrees around x, then y, then z.
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true)]
        rotate: Option<String>,
        /// How much bigger the part is: one number, or x,y,z to stretch
        /// it.
        #[arg(long, value_name = "N or X,Y,Z")]
        scale: Option<String>,
    },
    /// Make a joined part free, exactly where it stands: its join becomes
    /// a `move` and a `rotate`.
    Unjoin {
        /// The part.
        part: String,
    },
    /// Copy a part, comments and all, straight after it.
    Duplicate {
        /// The part.
        part: String,
        /// The copy's name. Without it: the part's name with a number, or,
        /// with --mirror, with left and right swapped.
        #[arg(long)]
        name: Option<String>,
        /// The socket the copy joins. Without it: the same socket, or,
        /// with --mirror, that socket with left and right swapped.
        #[arg(long, value_name = "PART.SOCKET")]
        to: Option<String>,
        /// Make the copy a mirror image: a right arm from a left arm.
        #[arg(long)]
        mirror: bool,
    },
    /// Add a colour to the recipe's palette.
    AddColor {
        /// Its name.
        name: String,
        /// The colour, as #RRGGBB.
        hex: String,
    },
    /// Change a colour that is in the recipe's palette.
    SetColor {
        /// Its name.
        name: String,
        /// The new colour, as #RRGGBB.
        hex: String,
    },
    /// Declare a socket on one side of a part. It sits on the surface.
    AddSocket {
        /// The part.
        part: String,
        /// The socket's name.
        name: String,
        /// The side: top, bottom, left, right, front or back.
        #[arg(long)]
        on: String,
        /// How far it is slid across the side: x,y,z, each from -1 (the
        /// left, bottom or back edge) to 1. The number for the direction
        /// the side faces stays 0.
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true)]
        shift: Option<String>,
    },
    /// Move a socket that add-socket (or the recipe) put on a side of a
    /// part. Whatever is joined to it moves with it.
    SetSocket {
        /// The socket, like body.badge.
        socket: String,
        /// Another side.
        #[arg(long)]
        on: Option<String>,
        /// A new slide across the side: x,y,z.
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true)]
        shift: Option<String>,
    },
}

/// The sizes of a shape.
#[derive(Args)]
pub struct SizeOptions {
    /// Width, height and depth of a box or a wedge: x,y,z.
    #[arg(long, value_name = "X,Y,Z")]
    size: Option<String>,
    /// The radius of a cylinder, a sphere or a cone.
    #[arg(long)]
    radius: Option<f64>,
    /// The height of a cylinder or a cone.
    #[arg(long)]
    height: Option<f64>,
    /// How many flat sides a round shape has.
    #[arg(long)]
    segments: Option<u32>,
}

/// `1,2,0.5` as three numbers.
fn three(option: &str, written: &str) -> Result<[f64; 3], String> {
    let numbers: Vec<f64> = written
        .split(',')
        .map(|number| number.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("{option} needs three numbers, like 1,2,0.5, not \"{written}\""))?;
    <[f64; 3]>::try_from(numbers)
        .map_err(|_| format!("{option} needs three numbers, like 1,2,0.5, not \"{written}\""))
}

impl SizeOptions {
    fn sizes(&self) -> Result<Sizes, String> {
        Ok(Sizes {
            size: self
                .size
                .as_deref()
                .map(|size| three("--size", size))
                .transpose()?,
            radius: self.radius,
            height: self.height,
            segments: self.segments,
        })
    }
}

impl Change {
    /// The edit these words ask for, or a sentence saying what is wrong
    /// with them.
    pub fn to_edit(&self) -> Result<Edit, String> {
        Ok(match self {
            Change::Add {
                name,
                to,
                shape,
                uses,
                file,
                attach,
                sizes,
                color,
                twist,
            } => Edit::Add {
                name: name.clone(),
                made: match (shape, uses, file) {
                    (Some(shape), _, _) => Made::Shape(shape.clone()),
                    (None, Some(defined), _) => Made::Use(defined.clone()),
                    (None, None, Some(file)) => Made::File(file.clone()),
                    (None, None, None) => {
                        return Err(
                            "say what the new part is made of: --shape box (or cylinder, sphere, cone, wedge), --use <defined part> or --file <file>"
                                .to_owned(),
                        );
                    }
                },
                to: to.clone(),
                attach: attach.clone(),
                sizes: sizes.sizes()?,
                color: color.clone(),
                twist: *twist,
            },
            Change::Remove {
                part,
                with_attached,
            } => Edit::Remove {
                part: part.clone(),
                with_attached: *with_attached,
            },
            Change::Rename { part, new_name } => Edit::Rename {
                part: part.clone(),
                new_name: new_name.clone(),
            },
            Change::Join { part, to, attach } => Edit::Join {
                part: part.clone(),
                to: to.clone(),
                attach: attach.clone(),
            },
            Change::Set {
                part,
                shape,
                sizes,
                color,
                face_colors,
                twist,
                move_to,
                rotate,
                scale,
            } => Edit::Set {
                part: part.clone(),
                changes: Changes {
                shape: shape.clone(),
                sizes: sizes.sizes()?,
                color: color.clone(),
                face_colors: face_colors
                    .iter()
                    .map(|written| match written.split_once('=') {
                        Some((face, color)) if !face.is_empty() && !color.is_empty() => {
                            Ok((face.to_owned(), color.to_owned()))
                        }
                        _ => Err(format!(
                            "--face-color needs a face and a colour, like top=plank, not \"{written}\""
                        )),
                    })
                    .collect::<Result<_, _>>()?,
                twist: *twist,
                move_to: move_to
                    .as_deref()
                    .map(|written| three("--move", written))
                    .transpose()?,
                rotate: rotate
                    .as_deref()
                    .map(|written| three("--rotate", written))
                    .transpose()?,
                scale: scale
                    .as_deref()
                    .map(|written| match written.trim().parse::<f64>() {
                        Ok(all) => Ok([all; 3]),
                        Err(_) => three("--scale", written),
                    })
                    .transpose()?,
                },
            },
            Change::Unjoin { part } => Edit::Unjoin { part: part.clone() },
            Change::Duplicate {
                part,
                name,
                to,
                mirror,
            } => Edit::Duplicate {
                part: part.clone(),
                name: name.clone(),
                to: to.clone(),
                mirror: *mirror,
            },
            Change::AddColor { name, hex } => Edit::AddColor {
                name: name.clone(),
                hex: hex.clone(),
            },
            Change::SetColor { name, hex } => Edit::SetColor {
                name: name.clone(),
                hex: hex.clone(),
            },
            Change::AddSocket {
                part,
                name,
                on,
                shift,
            } => Edit::AddSocket {
                part: part.clone(),
                name: name.clone(),
                on: on.clone(),
                shift: shift
                    .as_deref()
                    .map(|shift| three("--shift", shift))
                    .transpose()?
                    .unwrap_or([0.0; 3]),
            },
            Change::SetSocket { socket, on, shift } => Edit::SetSocket {
                socket: socket.clone(),
                on: on.clone(),
                shift: shift
                    .as_deref()
                    .map(|shift| three("--shift", shift))
                    .transpose()?,
            },
        })
    }
}
