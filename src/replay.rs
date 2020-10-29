//! Parsing for replay and score files, which are very similar.

use crate::prelude::*;

#[cfg(feature = "compression")]
use xz2::{
    stream::{LzmaOptions, Stream},
    write::{XzDecoder, XzEncoder},
};

///The LZMA compression level (a number between 0 and 9) used to write replay data when it is
///not otherwise specified.
pub const DEFAULT_COMPRESSION_LEVEL: u32 = 5;

///An osu! replay.
///The replay might come from a large `ScoreList` score database, or from an individual standalone
///`.osr` file.
#[cfg_attr(feature = "ser-de", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Replay {
    ///The gamemode the replay was scored in.
    pub mode: Mode,
    ///The `.db` version of the replay file.
    ///If the replay is inside a `scores.db` file, the version should be redundant with it (?).
    pub version: u32,
    ///The MD5 hash of the beatmap played.
    pub beatmap_hash: Option<String>,
    ///The name of the player who scored the replay.
    pub player_name: Option<String>,
    ///The replay-specific MD5 hash.
    pub replay_hash: Option<String>,
    ///The same in all modes: amount of 300 scores.
    pub count_300: u16,
    ///Amount of 100 scores in std and ctb, but 150 in taiko and 200 in mania.
    pub count_100: u16,
    ///Amount of 50 scores in std and mania, but small fruit in ctb.
    pub count_50: u16,
    ///Amount of gekis in std, but MAX scores (rainbow 300s) in mania.
    pub count_geki: u16,
    ///Amount of katsus in std, but 100 scores in mania.
    pub count_katsu: u16,
    ///Amount of misses in all modes.
    pub count_miss: u16,
    ///The numerical score achieved.
    pub score: u32,
    pub max_combo: u16,
    pub perfect_combo: bool,
    ///The mod combination with which the replay was done.
    pub mods: ModSet,
    ///A string representing a graph of how much life bar did the player have along the beatmap.
    ///
    ///It is a comma-separated list of human-readable entries in the form `<offset>|<life>`, where
    ///`<offset>` is the amount of milliseconds since the start of the song and `<life>` is a
    ///number between 0 and 1 representing the amount of life left.
    pub life_graph: Option<String>,
    ///When was the replay scored.
    pub timestamp: DateTime<Utc>,
    ///Compressed replay data.
    ///Only available on standalone `.osr` replays.
    ///
    ///If feature `compression` is enabled, this is automatically decompressed
    ///and parsed into a vector of `Action`s.
    ///Otherwise, the raw compressed bytes are made available, but are typically unusable.
    #[cfg(feature = "compression")]
    pub replay_data: Option<Vec<Action>>,
    #[cfg(not(feature = "compression"))]
    pub replay_data: Option<Vec<u8>>,
    ///Online score id.
    ///Only has a useful value on replays embedded in a `ScoreList`.
    pub online_score_id: u64,
}
impl Replay {
    ///Parse a replay from its raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Replay, Error> {
        Ok(replay(bytes, true).map(|(_rem, replay)| replay)?)
    }

    ///Read a replay from a standalone `.osr` osu! replay file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Replay, Error> {
        Self::from_bytes(&fs::read(path)?)
    }

    ///Write the replay to an arbitrary writer, with the given compression level.
    ///
    ///If the compression level is `None` the arbitrary default
    ///`replay::DEFAULT_COMPRESSION_LEVEL` will be used.
    ///If the `compression` feature is disabled this argument has no effect.
    pub fn to_writer<W: Write>(
        &self,
        mut out: W,
        compression_level: Option<u32>,
    ) -> io::Result<()> {
        self.wr_args(
            &mut out,
            Some(compression_level.unwrap_or(DEFAULT_COMPRESSION_LEVEL)),
        )
    }

    ///Similar to `to_writer` but writes the replay to an `osr` file.
    pub fn save<P: AsRef<Path>>(&self, path: P, compression_level: Option<u32>) -> io::Result<()> {
        self.to_writer(BufWriter::new(File::create(path)?), compression_level)
    }
}

pub(crate) fn replay(bytes: &[u8], standalone: bool) -> IResult<&[u8], Replay> {
    do_parse!(
        bytes,
        mode: map_opt!(byte, Mode::from_raw)
            >> version: int
            >> beatmap_hash: opt_string
            >> player_name: opt_string
            >> replay_hash: opt_string
            >> count_300: short
            >> count_100: short
            >> count_50: short
            >> count_geki: short
            >> count_katsu: short
            >> count_miss: short
            >> score: int
            >> max_combo: short
            >> perfect_combo: boolean
            >> mods: map!(int, ModSet::from_bits)
            >> life_graph: opt_string
            >> timestamp: datetime
            >> replay_data:
                switch!(value!(standalone),
                    false => do_parse!(
                        tag!(&[0xff,0xff,0xff,0xff]) >>
                        (None)
                    ) |
                    true => map!(length_value!(int, |bytes: &[u8]| {
                        expr_res!(&b""[..], replay_data(bytes))
                    }), |rd| Some(rd))
                )
            >> online_score_id: long
            >> (Replay {
                mode,
                version,
                beatmap_hash,
                player_name,
                replay_hash,
                count_300,
                count_100,
                count_50,
                count_geki,
                count_katsu,
                count_miss,
                score,
                max_combo,
                perfect_combo,
                mods,
                life_graph,
                timestamp,
                replay_data,
                online_score_id,
            })
    )
}
writer!(Replay [this,out,compress_data: Option<u32>] {
    this.mode.raw().wr(out)?;
    this.version.wr(out)?;
    this.beatmap_hash.wr(out)?;
    this.player_name.wr(out)?;
    this.replay_hash.wr(out)?;
    this.count_300.wr(out)?;
    this.count_100.wr(out)?;
    this.count_50.wr(out)?;
    this.count_geki.wr(out)?;
    this.count_katsu.wr(out)?;
    this.count_miss.wr(out)?;
    this.score.wr(out)?;
    this.max_combo.wr(out)?;
    this.perfect_combo.wr(out)?;
    this.mods.bits().wr(out)?;
    this.life_graph.wr(out)?;
    this.timestamp.wr(out)?;
    if let Some(compression_level) = compress_data {
        write_replay_data(&this.replay_data,out,compression_level)?;
    }else{
        0xffffffff_u32.wr(out)?;
    }
    this.online_score_id.wr(out)?;
});

///Represents a single action within a replay.
///The meaning of an action depends on the gamemode of the replay, but all actions
///contain:
///
/// - An integral amount of milliseconds elapsed since the last action, `delta`.
/// - 3 pieces of floating-point payload: `x`, `y` and `z`.
#[cfg_attr(feature = "ser-de", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Action {
    ///The amount of milliseconds since the last action.
    pub delta: i64,
    ///First bit of payload in the action.
    ///
    ///In standard:
    ///Represents the `x` coordinate of the mouse, from `0` to `512`.
    ///
    ///In mania:
    ///Represents the bitwise combination of buttons pressed.
    pub x: f32,
    ///Second bit of payload in the action.
    ///
    ///In standard:
    ///Represents the `y` coordinate of the mouse, from `0` to `384`.
    pub y: f32,
    ///Third bit of payload in the action.
    ///
    ///In standard:
    ///Represents the bitwise combination of buttons pressed.
    pub z: f32,
}
impl Action {
    ///Get the pressed osu!standard buttons.
    pub fn std_buttons(&self) -> StandardButtonSet {
        StandardButtonSet::from_bits(self.z as u32)
    }

    ///Get the pressed osu!mania buttons.
    pub fn mania_buttons(&self) -> ManiaButtonSet {
        ManiaButtonSet::from_bits(self.x as u32)
    }
}

#[cfg_attr(feature = "ser-de", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum StandardButton {
    MousePrimary,
    MouseSecondary,
    KeyPrimary,
    KeySecondary,
}
impl StandardButton {
    pub fn raw(&self) -> u32 {
        *self as u32
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct StandardButtonSet(pub u32);
impl StandardButtonSet {
    pub fn bits(self) -> u32 {
        self.0
    }
    pub fn from_bits(bits: u32) -> StandardButtonSet {
        StandardButtonSet(bits)
    }

    ///Create a new button combination with no buttons pressed.
    pub fn none() -> StandardButtonSet {
        StandardButtonSet::from_bits(0)
    }

    ///Check whether the combination lists the button as pressed.
    pub fn is_down(&self, button: StandardButton) -> bool {
        self.bits().bit(button.raw() as usize)
    }

    ///Set the pressed status of the given button.
    pub fn set_down(&self, button: StandardButton, is_down: bool) -> StandardButtonSet {
        let mut bits = self.bits();
        bits.set_bit(button.raw() as usize, is_down);
        StandardButtonSet::from_bits(bits)
    }
    ///Set the pressed status of a button to `true`.
    pub fn press(&self, button: StandardButton) -> StandardButtonSet {
        self.set_down(button, true)
    }
    ///Set the pressed status of a button to `false`.
    pub fn release(&self, button: StandardButton) -> StandardButtonSet {
        self.set_down(button, false)
    }
}

///Any combination of mania buttons being pressed.
///
///Button indices start from `0`, and go left-to-right.
///Button indices outside the replay key count should never be down.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ManiaButtonSet(pub u32);
impl ManiaButtonSet {
    pub fn bits(&self) -> u32 {
        self.0
    }
    pub fn from_bits(bits: u32) -> ManiaButtonSet {
        ManiaButtonSet(bits)
    }

    ///Create a new key combination with no keys pressed.
    pub fn none() -> ManiaButtonSet {
        ManiaButtonSet::from_bits(0)
    }

    ///Check whether a certain key is pressed.
    pub fn is_down(&self, button: usize) -> bool {
        self.bits().bit(button)
    }

    ///Set the pressed status of a key.
    pub fn set_down(&self, button: usize, is_down: bool) -> ManiaButtonSet {
        let mut bits = self.bits();
        bits.set_bit(button, is_down);
        ManiaButtonSet::from_bits(bits)
    }
    ///Set the pressed status of a key to `true`.
    pub fn press(&self, button: usize) -> ManiaButtonSet {
        self.set_down(button, true)
    }
    ///Set the pressed status of a key to `false`.
    pub fn release(&self, button: usize) -> ManiaButtonSet {
        self.set_down(button, false)
    }
}

#[cfg(feature = "compression")]
fn replay_data(raw: &[u8]) -> Result<Vec<Action>, Error> {
    let mut decoder =
        XzDecoder::new_stream(Vec::new(), Stream::new_lzma_decoder(u64::max_value())?);
    decoder.write_all(raw)?;
    let data = decoder.finish()?;
    let actions = actions(&data)?.1;
    Ok(actions)
}
#[cfg(not(feature = "compression"))]
fn replay_data(data: &[u8]) -> Result<Vec<u8>, Error> {
    Ok(data.to_vec())
}

#[cfg(feature = "compression")]
fn write_replay_data<W: Write>(
    actions: &Option<Vec<Action>>,
    out: &mut W,
    compression_level: u32,
) -> io::Result<()> {
    //Generate compressed replay data
    let raw = {
        let mut encoder = XzEncoder::new_stream(
            Vec::new(),
            Stream::new_lzma_encoder(&LzmaOptions::new_preset(compression_level)?)?,
        );
        if let Some(actions) = actions {
            for action in actions.iter() {
                action.wr(&mut encoder)?;
            }
        }
        encoder.finish()?
    };
    write_raw_replay_data(&raw, out, compression_level)
}
#[cfg(not(feature = "compression"))]
use self::write_raw_replay_data as write_replay_data;

fn write_raw_replay_data<W: Write>(
    raw: &[u8],
    out: &mut W,
    _compression_level: u32,
) -> io::Result<()> {
    //Prefix the data with its length
    (raw.len() as u32).wr(out)?;
    out.write_all(&raw)?;
    Ok(())
}

// Parse the plaintext list of actions.
named!(actions<&[u8], Vec<Action>>,
    many0!(complete!(do_parse!(
        delta: number >>
        tag!(b"|") >>
        x: number >>
        tag!(b"|") >>
        y: number >>
        tag!(b"|") >>
        z: number >>
        tag!(b",") >>
        (Action {
            delta: delta as i64,
            x: x as f32,
            y: y as f32,
            z: z as f32,
        })
    )))
);
writer!(Action [this,out] {
    write!(out, "{}|{}|{}|{},", this.delta,this.x,this.y,this.z)?;
});

// Parse a textually encoded decimal number.
named!(number<&[u8], f64>, do_parse!(
    sign: opt!(tag!(b"-")) >>
    whole: take_while1!(|b: u8| b.is_ascii_digit()) >>
    decimal: opt!(do_parse!(
        tag!(b".") >>
        decimal: take_while1!(|b: u8| b.is_ascii_digit()) >>
        (decimal)
    )) >>
    ({
        let mut num=0.0;
        for byte in whole {
            num*=10.0;
            num+=(*byte - b'0') as f64;
        }
        if let Some(decimal) = decimal {
            let mut value=1.0;
            for byte in decimal {
                value/=10.0;
                num+=(*byte - b'0') as f64 * value;
            }
        }
        if sign.is_some() {num*=-1.0}
        num
    })
));
