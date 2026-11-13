//! `duck-mold edit`: the words of the command line turned into an
//! [`Edit`], which the library carries out. The Studio builds the same
//! `Edit` values from clicks, so whatever it can do, this can.

use clap::{Args, Subcommand};
use duck_mold_core::edit::{Changes, Edit, Free, LimitsTo, Made, Sizes};
use duck_mold_core::recipe::Turns;

/// One change to a recipe. (Each variant's name is the command's, so
/// `set-change` is `SetChange`.)
#[derive(Subcommand)]
#[allow(clippy::enum_variant_names)]
pub enum Change {
    /// Add a part: joined to a socket (--to), or standing free.
    ///
    /// Say what it is made of with one of --shape, --use and --file.
    /// Without --to the part is joined to nothing: it is stood on the
    /// ground, at the recipe's zero or where --stand says, unless --move
    /// says exactly where its middle goes.
    Add {
        /// The new part's name.
        name: String,
        /// The socket it joins, like head.top. Without it the part
        /// stands free.
        #[arg(long, value_name = "PART.SOCKET")]
        to: Option<String>,
        /// A built-in shape: box, cylinder, sphere, cone, wedge, capsule,
        /// torus, tube, pyramid, prism, stairs or arch.
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
        /// For a free part: where its middle goes, x,y,z in metres.
        #[arg(long = "move", value_name = "X,Y,Z", allow_hyphen_values = true)]
        move_to: Option<String>,
        /// For a free part with no --move: where on the ground it is
        /// stood, x,z in metres.
        #[arg(long, value_name = "X,Z", allow_hyphen_values = true)]
        stand: Option<String>,
        /// For a free part: how it is turned, degrees around x, then y,
        /// then z.
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true)]
        rotate: Option<String>,
        /// How much bigger it is: one number, or x,y,z.
        #[arg(long, value_name = "N or X,Y,Z")]
        scale: Option<String>,
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
        /// once. top=none takes the face's own colour away again, so it
        /// has the part's.
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
    /// Rename a socket a part declares. Everything that names it follows.
    RenameSocket {
        /// The socket, like body.spot.
        #[arg(value_name = "PART.SOCKET")]
        socket: String,
        /// Its new name.
        new_name: String,
    },
    /// Remove a socket a part declares.
    ///
    /// Refused if anything uses it, saying what.
    RemoveSocket {
        /// The socket, like body.spot.
        #[arg(value_name = "PART.SOCKET")]
        socket: String,
    },
    /// Remove the sockets the Studio made (spot, spot_2...) that nothing
    /// uses any more.
    TidySockets,
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
    /// Take a colour out of the recipe's palette.
    ///
    /// Refused if anything is painted with it, saying what, unless
    /// --replace-with names the colour those get instead.
    RemoveColor {
        /// Its name.
        name: String,
        /// The palette colour to paint with instead, wherever this one
        /// is used.
        #[arg(long, value_name = "COLOUR")]
        replace_with: Option<String>,
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
    /// Move or turn a socket that add-socket (or the recipe) put on a
    /// side of a part. Whatever is joined to it moves with it.
    SetSocket {
        /// The socket, like body.badge.
        socket: String,
        /// Another side.
        #[arg(long)]
        on: Option<String>,
        /// A new slide across the side: x,y,z.
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true)]
        shift: Option<String>,
        /// Which way it faces: up, down, left, right, front, back, or out
        /// (straight out of its side, as it faces when nothing says).
        #[arg(long, value_name = "WAY")]
        facing: Option<String>,
    },
    /// Pull faces of a box out the way they face, with new walls round
    /// them (a negative --by pushes them in, making a hollow).
    ///
    /// Adds a pull to the part's "changes". Faces that touch are pulled
    /// as one piece. `duck-mold inspect --faces` lists the face names.
    Pull {
        /// The part.
        part: String,
        /// The faces, by name: top, or several with commas, like
        /// top_left,top,top_right.
        faces: String,
        /// How far, in metres. Below 0 pushes the faces in.
        #[arg(long, allow_hyphen_values = true)]
        by: f64,
        /// What the new walls are called: <name>_front, <name>_left...
        /// Without it: <face>_side_front...
        #[arg(long)]
        name: Option<String>,
    },
    /// Make a smaller face inside each face of a box, --by metres in from
    /// its edges, with a ring of faces round it.
    ///
    /// Adds an inset to the part's "changes". The face's old name then
    /// stands for the ring.
    Inset {
        /// The part.
        part: String,
        /// The faces, by name: front, or several with commas.
        faces: String,
        /// How far in from the edges, in metres.
        #[arg(long)]
        by: f64,
        /// What the face in the middle is called. Without it:
        /// <face>_inset.
        #[arg(long)]
        name: Option<String>,
    },
    /// Change the amount of one of a box's changes, counted from 1 as
    /// they are written.
    SetChange {
        /// The part.
        part: String,
        /// Which change: 1 is the first.
        number: usize,
        /// Its new amount: one number (a pull, an inset, a scale alike
        /// each way), or x,y,z (a move, a turn, a scale).
        #[arg(long, allow_hyphen_values = true, value_name = "N or X,Y,Z")]
        by: Option<String>,
        /// A cut's new place: from 0 (the middle) to just under 1 (the
        /// side).
        #[arg(long)]
        at: Option<f64>,
    },
    /// Move edges of a box (where two faces meet: top+front) by x,y,z
    /// metres, in its own directions.
    ///
    /// Right after a move of the same edges, it adds to that one.
    MoveEdge {
        /// The part.
        part: String,
        /// The edges: top+front, or several with commas.
        edges: String,
        /// How far along x, y and z, in metres.
        #[arg(long, allow_hyphen_values = true, value_name = "X,Y,Z")]
        by: String,
    },
    /// Say how far a part may turn about its join when the model is
    /// posed: its "limits". Each of --x (a nod), --y (a turn) and --z (a
    /// tilt) takes two angles in degrees like -30,40, or free (any
    /// amount: a wheel), or none (it doesn't turn that way).
    ///
    /// A part with no limits turns freely every way. Once it has limits,
    /// only the directions they name turn. `duck-mold inspect --joins`
    /// lists every part's.
    SetLimits {
        /// The part.
        part: String,
        /// Around x, left to right: a nod. Positive swings a hanging arm
        /// forward.
        #[arg(long, value_name = "LOW,HIGH", allow_hyphen_values = true)]
        x: Option<String>,
        /// Around y, up: a turn. Positive turns to the part's right.
        #[arg(long, value_name = "LOW,HIGH", allow_hyphen_values = true)]
        y: Option<String>,
        /// Around z, forward: a tilt. Positive leans the top to its left.
        #[arg(long, value_name = "LOW,HIGH", allow_hyphen_values = true)]
        z: Option<String>,
        /// It never turns or moves in a pose: a badge, a window.
        #[arg(long, conflicts_with_all = ["x", "y", "z", "none"])]
        fixed: bool,
        /// Take its limits away: it turns freely again.
        #[arg(long, conflicts_with_all = ["x", "y", "z"])]
        none: bool,
    },
    /// Turn or move a part in a pose. Makes the pose if it isn't there.
    ///
    /// A pose is the model with some parts turned about their joins:
    /// the recipe's [poses.<name>]. --turn is degrees around x (a nod),
    /// then y (a turn), then z (a tilt), about the part's pivot;
    /// everything hanging from the part goes with it. --move slides the
    /// part, in metres. A part inside another is named by its path:
    /// left_arm/elbow. `duck-mold render <recipe> --pose <name>` draws
    /// it; `duck-mold inspect <recipe> --pose <name>` checks it.
    SetPose {
        /// The pose's name.
        pose: String,
        /// The part.
        part: String,
        /// How far it is turned, in degrees: x,y,z.
        #[arg(long, value_name = "X,Y,Z", allow_hyphen_values = true)]
        turn: Option<String>,
        /// How far it is moved from where it stands at rest: x,y,z.
        #[arg(long = "move", value_name = "X,Y,Z", allow_hyphen_values = true)]
        move_by: Option<String>,
        /// Put the part back at rest in this pose.
        #[arg(long, conflicts_with_all = ["turn", "move_by"])]
        rest: bool,
    },
    /// Add a pose: with every part at rest, or a copy of another to
    /// change.
    AddPose {
        /// The new pose's name.
        name: String,
        /// The pose to copy.
        #[arg(long, value_name = "POSE")]
        from: Option<String>,
    },
    /// Rename a pose. Everything that names it follows.
    RenamePose {
        /// The pose.
        pose: String,
        /// Its new name.
        new_name: String,
    },
    /// Remove a pose. Refused while a key of an animation uses it,
    /// unless --with-keys.
    RemovePose {
        /// The pose.
        pose: String,
        /// Take the keys that use the pose away with it.
        #[arg(long)]
        with_keys: bool,
    },
    /// Add an animation, with no keys yet: give it keys with add-key.
    AddAnimation {
        /// The new animation's name.
        name: String,
        /// It starts again after its last key.
        #[arg(long = "loop")]
        looping: bool,
    },
    /// Change whether an animation loops, whether it is smooth, and how
    /// long it lasts.
    SetAnimation {
        /// The animation.
        animation: String,
        /// true: it starts again after its last key. false: it plays
        /// once.
        #[arg(long = "loop", value_name = "true|false")]
        looping: Option<bool>,
        /// true: the motion flows through each pose and starts and ends
        /// gently. false: it goes straight from pose to pose at a
        /// steady speed, like a wheel.
        #[arg(long, value_name = "true|false")]
        smooth: Option<bool>,
        /// How long it lasts, in seconds: every key's time is scaled, so
        /// the whole animation is faster or slower.
        #[arg(long, value_name = "SECONDS")]
        length: Option<f64>,
    },
    /// Rename an animation.
    RenameAnimation {
        /// The animation.
        animation: String,
        /// Its new name.
        new_name: String,
    },
    /// Remove an animation. Its poses stay.
    RemoveAnimation {
        /// The animation.
        animation: String,
    },
    /// Add a key to an animation: a pose, and the time it is reached.
    /// The first key is at 0.
    AddKey {
        /// The animation.
        animation: String,
        /// The pose: one of the recipe's, or rest.
        pose: String,
        /// Its time, in seconds from the animation's start.
        #[arg(long, value_name = "SECONDS")]
        at: f64,
    },
    /// Give a key another time.
    MoveKey {
        /// The animation.
        animation: String,
        /// The key, counted from 1 as inspect --animation prints them.
        key: usize,
        /// Its new time, in seconds.
        #[arg(long, value_name = "SECONDS")]
        at: f64,
    },
    /// Give a key another pose.
    SetKey {
        /// The animation.
        animation: String,
        /// The key, counted from 1.
        key: usize,
        /// Its new pose.
        #[arg(long, value_name = "POSE")]
        pose: String,
    },
    /// Remove a key. If it is the first, the keys after it move earlier
    /// so that the animation still starts at 0.
    RemoveKey {
        /// The animation.
        animation: String,
        /// The key, counted from 1.
        key: usize,
    },
    /// Make the mirror image of a pose, left to right: what left_arm
    /// did, right_arm does, and a part in the middle turns the other
    /// way. Names with "left" and "right" in them are swapped.
    MirrorPose {
        /// The pose.
        pose: String,
        /// The name of the mirror image: a new pose, or one to put it in
        /// place of. Without it, the pose itself is mirrored.
        #[arg(long = "as", value_name = "NAME")]
        as_name: Option<String>,
    },
    /// Make a definition from a part: what it is made of (its shape and
    /// sizes, changes, colours and own sockets) moves to [define.<name>],
    /// and the part uses it where it stands. Add more of it with
    /// `add <name> --use <name>`.
    Define {
        /// The part: one of the model's own list.
        part: String,
        /// The definition's name.
        name: String,
    },
    /// Move corners of a box (where three faces or more meet:
    /// top+front+left) by x,y,z metres, in its own directions.
    ///
    /// Right after a move of the same corners, it adds to that one.
    MoveCorner {
        /// The part.
        part: String,
        /// The corners: top+front+left, or several with commas.
        corners: String,
        /// How far along x, y and z, in metres.
        #[arg(long, allow_hyphen_values = true, value_name = "X,Y,Z")]
        by: String,
    },
    /// Cut all the way round a box, with a flat cut facing a side: every
    /// face it crosses becomes two, to shape further.
    Cut {
        /// The part.
        part: String,
        /// The side the cut faces: top, bottom, left, right, front or
        /// back.
        #[arg(long)]
        toward: String,
        /// How far from the middle (0) to that side (just under 1).
        #[arg(long)]
        at: f64,
        /// What the slices towards the side are called: <face>_<name>.
        /// Without it: <face>_<side>, like top_right.
        #[arg(long)]
        name: Option<String>,
    },
    /// Move faces of a box by x,y,z metres, in its own directions. The
    /// faces round them stretch to follow.
    ///
    /// Right after a move of the same faces, it adds to that one.
    MoveFace {
        /// The part.
        part: String,
        /// The faces, by name: top, or several with commas.
        faces: String,
        /// How far along x, y and z, in metres.
        #[arg(long, allow_hyphen_values = true, value_name = "X,Y,Z")]
        by: String,
    },
    /// Turn faces of a box about their middle: degrees around x, y and z.
    ///
    /// Right after a turn of the same faces, it adds to that one.
    TurnFace {
        /// The part.
        part: String,
        /// The faces, by name.
        faces: String,
        /// Degrees around x, then y, then z.
        #[arg(long, allow_hyphen_values = true, value_name = "X,Y,Z")]
        by: String,
    },
    /// Make faces of a box bigger or smaller about their middle.
    ///
    /// Right after a scale of the same faces, it multiplies that one.
    ScaleFace {
        /// The part.
        part: String,
        /// The faces, by name.
        faces: String,
        /// How much: one number for all three ways, or x,y,z. 1 is as
        /// they are.
        #[arg(long, value_name = "N or X,Y,Z")]
        by: String,
    },
    /// Take one of a box's changes away (or, with --all, every one).
    ///
    /// Refused if a later change, or a colour, names a face only that
    /// change made: the mistakes it would cause are listed.
    RemoveChange {
        /// The part.
        part: String,
        /// Which change: 1 is the first.
        #[arg(required_unless_present = "all")]
        number: Option<usize>,
        /// Take away every change: the part is a plain box again.
        #[arg(long, conflicts_with = "number")]
        all: bool,
    },
}

/// `top_left,top` as a list of names.
fn face_list(written: &str) -> Vec<String> {
    written
        .split(',')
        .map(|face| face.trim().to_owned())
        .filter(|face| !face.is_empty())
        .collect()
}

/// The sizes of a shape.
#[derive(Args)]
pub struct SizeOptions {
    /// Width, height and depth of a box or a wedge: x,y,z.
    #[arg(long, value_name = "X,Y,Z")]
    size: Option<String>,
    /// The radius of a round shape.
    #[arg(long)]
    radius: Option<f64>,
    /// How thick a torus's ring is: from its middle to its surface.
    #[arg(long)]
    tube: Option<f64>,
    /// How thick the wall of a tube or an arch is.
    #[arg(long)]
    wall: Option<f64>,
    /// The height of a cylinder, a cone, a capsule or a tube.
    #[arg(long)]
    height: Option<f64>,
    /// How many flat sides a round shape has.
    #[arg(long)]
    segments: Option<u32>,
    /// How many steps stairs have.
    #[arg(long)]
    steps: Option<u32>,
    /// How much to cut off the edges of a box or a cylinder, in metres.
    /// 0 makes them sharp again.
    #[arg(long)]
    bevel: Option<f64>,
    /// How many flat cuts go round each cut edge: 1 to 4.
    #[arg(long, value_name = "COUNT")]
    bevel_steps: Option<u32>,
    /// How wide the top of a box, a cylinder or a tube is, as a share of
    /// its bottom: one number, or x,z. 1 makes the sides straight again.
    #[arg(long, value_name = "N or X,Z")]
    taper: Option<String>,
}

/// How far a part may turn around one direction: `-30,40`, `free` or
/// `none`.
fn turns(option: &str, written: &str) -> Result<Turns, String> {
    match written.trim() {
        "free" => Ok(Turns::Any),
        "none" => Ok(Turns::No),
        angles => {
            let wrong = || {
                format!(
                    "{option} needs two angles in degrees, like -30,40, or free, or none, not \"{written}\""
                )
            };
            let (low, high) = angles.split_once(',').ok_or_else(wrong)?;
            let low: f64 = low.trim().parse().map_err(|_| wrong())?;
            let high: f64 = high.trim().parse().map_err(|_| wrong())?;
            Ok(Turns::Between(low, high))
        }
    }
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
            tube: self.tube,
            wall: self.wall,
            height: self.height,
            segments: self.segments,
            steps: self.steps,
            bevel: self.bevel,
            bevel_steps: self.bevel_steps,
            taper: self
                .taper
                .as_deref()
                .map(|written| {
                    let numbers: Vec<f64> = written
                        .split(',')
                        .filter_map(|number| number.trim().parse().ok())
                        .collect();
                    match numbers.as_slice() {
                        [all] => Ok([*all; 2]),
                        [across, deep] => Ok([*across, *deep]),
                        _ => Err(format!(
                            "--taper needs one number, like 0.5, or two, like 0.5,1, not \"{written}\""
                        )),
                    }
                })
                .transpose()?,
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
                move_to,
                stand,
                rotate,
                scale,
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
                free: Free {
                    move_to: move_to
                        .as_deref()
                        .map(|written| three("--move", written))
                        .transpose()?,
                    stand: stand
                        .as_deref()
                        .map(|written| {
                            let numbers: Vec<f64> = written
                                .split(',')
                                .filter_map(|number| number.trim().parse().ok())
                                .collect();
                            <[f64; 2]>::try_from(numbers).map_err(|_| {
                                format!("--stand needs two numbers, like 1,-0.5, not \"{written}\"")
                            })
                        })
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
            Change::RenameSocket { socket, new_name } => Edit::RenameSocket {
                socket: socket.clone(),
                new_name: new_name.clone(),
            },
            Change::RemoveSocket { socket } => Edit::RemoveSocket {
                socket: socket.clone(),
            },
            Change::TidySockets => Edit::TidySockets,
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
            Change::RemoveColor { name, replace_with } => Edit::RemoveColor {
                name: name.clone(),
                replace_with: replace_with.clone(),
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
            Change::Pull {
                part,
                faces,
                by,
                name,
            } => Edit::Pull {
                part: part.clone(),
                faces: face_list(faces),
                by: *by,
                name: name.clone(),
            },
            Change::Inset {
                part,
                faces,
                by,
                name,
            } => Edit::Inset {
                part: part.clone(),
                faces: face_list(faces),
                by: *by,
                name: name.clone(),
            },
            Change::SetChange {
                part,
                number,
                by,
                at,
            } => {
                let (one, xyz) = match by.as_deref() {
                    None => (None, None),
                    Some(by) if by.contains(',') => (None, Some(three("--by", by)?)),
                    Some(by) => (
                        Some(by.trim().parse::<f64>().map_err(|_| {
                            format!("--by needs a number, like 0.5, or three, like 0,0.2,0, not \"{by}\"")
                        })?),
                        None,
                    ),
                };
                Edit::SetChange {
                    part: part.clone(),
                    number: *number,
                    by: one,
                    by_xyz: xyz,
                    at: *at,
                }
            }
            Change::MoveEdge { part, edges, by } => Edit::MoveEdge {
                part: part.clone(),
                edges: face_list(edges),
                by: three("--by", by)?,
            },
            Change::Define { part, name } => Edit::Define {
                part: part.clone(),
                name: name.clone(),
            },
            Change::SetPose {
                pose,
                part,
                turn,
                move_by,
                rest,
            } => Edit::SetPose {
                pose: pose.clone(),
                part: part.clone(),
                turn: turn
                    .as_deref()
                    .map(|turn| three("--turn", turn))
                    .transpose()?,
                move_by: move_by
                    .as_deref()
                    .map(|move_by| three("--move", move_by))
                    .transpose()?,
                rest: *rest,
            },
            Change::AddPose { name, from } => Edit::AddPose {
                name: name.clone(),
                from: from.clone(),
            },
            Change::RenamePose { pose, new_name } => Edit::RenamePose {
                pose: pose.clone(),
                new_name: new_name.clone(),
            },
            Change::RemovePose { pose, with_keys } => Edit::RemovePose {
                pose: pose.clone(),
                with_keys: *with_keys,
            },
            Change::AddAnimation { name, looping } => Edit::AddAnimation {
                name: name.clone(),
                looping: *looping,
            },
            Change::SetAnimation {
                animation,
                looping,
                smooth,
                length,
            } => Edit::SetAnimation {
                animation: animation.clone(),
                looping: *looping,
                smooth: *smooth,
                length: *length,
            },
            Change::RenameAnimation {
                animation,
                new_name,
            } => Edit::RenameAnimation {
                animation: animation.clone(),
                new_name: new_name.clone(),
            },
            Change::RemoveAnimation { animation } => Edit::RemoveAnimation {
                animation: animation.clone(),
            },
            Change::AddKey {
                animation,
                pose,
                at,
            } => Edit::AddKey {
                animation: animation.clone(),
                pose: pose.clone(),
                at: *at,
            },
            Change::MoveKey { animation, key, at } => Edit::MoveKey {
                animation: animation.clone(),
                key: *key,
                at: *at,
            },
            Change::SetKey {
                animation,
                key,
                pose,
            } => Edit::SetKey {
                animation: animation.clone(),
                key: *key,
                pose: pose.clone(),
            },
            Change::RemoveKey { animation, key } => Edit::RemoveKey {
                animation: animation.clone(),
                key: *key,
            },
            Change::MirrorPose { pose, as_name } => Edit::MirrorPose {
                pose: pose.clone(),
                as_name: as_name.clone(),
            },
            Change::SetLimits {
                part,
                x,
                y,
                z,
                fixed,
                none,
            } => Edit::SetLimits {
                part: part.clone(),
                limits: if *fixed {
                    LimitsTo::Fixed
                } else if *none {
                    LimitsTo::None
                } else {
                    LimitsTo::Turns([
                        x.as_deref().map(|x| turns("--x", x)).transpose()?,
                        y.as_deref().map(|y| turns("--y", y)).transpose()?,
                        z.as_deref().map(|z| turns("--z", z)).transpose()?,
                    ])
                },
            },
            Change::MoveCorner { part, corners, by } => Edit::MoveCorner {
                part: part.clone(),
                corners: face_list(corners),
                by: three("--by", by)?,
            },
            Change::Cut {
                part,
                toward,
                at,
                name,
            } => Edit::Cut {
                part: part.clone(),
                toward: toward.clone(),
                at: *at,
                name: name.clone(),
            },
            Change::MoveFace { part, faces, by } => Edit::MoveFace {
                part: part.clone(),
                faces: face_list(faces),
                by: three("--by", by)?,
            },
            Change::TurnFace { part, faces, by } => Edit::TurnFace {
                part: part.clone(),
                faces: face_list(faces),
                by: three("--by", by)?,
            },
            Change::ScaleFace { part, faces, by } => Edit::ScaleFace {
                part: part.clone(),
                faces: face_list(faces),
                by: if by.contains(',') {
                    three("--by", by)?
                } else {
                    [by.trim().parse::<f64>().map_err(|_| {
                        format!("--by needs a number, like 0.5, or three, like 1,0.5,1, not \"{by}\"")
                    })?; 3]
                },
            },
            Change::RemoveChange { part, number, .. } => Edit::RemoveChange {
                part: part.clone(),
                number: *number,
            },
            Change::SetSocket {
                socket,
                on,
                shift,
                facing,
            } => Edit::SetSocket {
                socket: socket.clone(),
                on: on.clone(),
                shift: shift
                    .as_deref()
                    .map(|shift| three("--shift", shift))
                    .transpose()?,
                facing: facing.clone(),
            },
        })
    }
}
