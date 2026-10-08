//! A minimal TraCI client: the binary protocol SUMO speaks on `--remote-port`.
//!
//! Wire format (SUMO documentation, "TraCI/Protocol"; the reference client is
//! `tools/traci/connection.py` and `tools/traci/storage.py` shipped with SUMO 1.27):
//!
//! * a message is a big-endian `i32` total length (including those four bytes) followed by
//!   commands;
//! * a command is `u8 length, u8 id, content`, or `0u8, i32 length, u8 id, content` when the
//!   length exceeds 255, the length counting its own bytes;
//! * every command is answered by a status command (`length, id, u8 result, string
//!   description`, result 0 = OK) followed, for a `get`, by the response command;
//! * strings are an `i32` byte count and the bytes; typed values carry a one-byte type tag.
//!
//! Only what the mobility plug-in needs is implemented: version, step, close, variable
//! retrieval (batched in one message per step, so a step costs a fixed number of round
//! trips whatever the fleet size) and the handful of set commands of [`crate::sumo`].

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration as StdDuration;

/// Command identifiers (`tools/traci/constants.py`).
pub mod cmd {
    /// `CMD_GETVERSION`.
    pub const GETVERSION: u8 = 0x00;
    /// `CMD_SIMSTEP`.
    pub const SIMSTEP: u8 = 0x02;
    /// `CMD_CLOSE`.
    pub const CLOSE: u8 = 0x7F;
    /// `CMD_GET_VEHICLE_VARIABLE`.
    pub const GET_VEHICLE: u8 = 0xa4;
    /// `CMD_SET_VEHICLE_VARIABLE`.
    pub const SET_VEHICLE: u8 = 0xc4;
    /// `CMD_GET_TL_VARIABLE`.
    pub const GET_TL: u8 = 0xa2;
    /// `CMD_SET_LANE_VARIABLE`.
    pub const SET_LANE: u8 = 0xc3;
    /// `CMD_SET_ROUTE_VARIABLE`.
    pub const SET_ROUTE: u8 = 0xc6;
    /// `CMD_GET_SIM_VARIABLE`.
    pub const GET_SIM: u8 = 0xab;
}

/// Variable identifiers (`tools/traci/constants.py`).
pub mod var {
    /// `TRACI_ID_LIST`.
    pub const ID_LIST: u8 = 0x00;
    /// `VAR_SPEED` (get) / `VAR_SPEED` (set: hold a speed; −1 releases).
    pub const SPEED: u8 = 0x40;
    /// `VAR_MAXSPEED`.
    pub const MAXSPEED: u8 = 0x41;
    /// `VAR_POSITION`.
    pub const POSITION: u8 = 0x42;
    /// `VAR_ANGLE`: degrees, clockwise from north.
    pub const ANGLE: u8 = 0x43;
    /// `VAR_LENGTH`.
    pub const LENGTH: u8 = 0x44;
    /// `VAR_VEHICLECLASS`.
    pub const VEHICLECLASS: u8 = 0x49;
    /// `VAR_WIDTH`.
    pub const WIDTH: u8 = 0x4d;
    /// `VAR_LANE_ID`.
    pub const LANE_ID: u8 = 0x51;
    /// `VAR_LANEPOSITION`: the front bumper's distance from the lane start.
    pub const LANEPOSITION: u8 = 0x56;
    /// `VAR_ACCELERATION`.
    pub const ACCELERATION: u8 = 0x72;
    /// `CMD_CHANGETARGET`.
    pub const CHANGETARGET: u8 = 0x31;
    /// `LANE_ALLOWED`.
    pub const LANE_ALLOWED: u8 = 0x34;
    /// `LANE_DISALLOWED`.
    pub const LANE_DISALLOWED: u8 = 0x35;
    /// `TL_RED_YELLOW_GREEN_STATE`.
    pub const TL_STATE: u8 = 0x20;
    /// `ADD`.
    pub const ADD: u8 = 0x80;
    /// `REMOVE`.
    pub const REMOVE: u8 = 0x81;
    /// `ADD_FULL`.
    pub const ADD_FULL: u8 = 0x85;
    /// `VAR_TIME`.
    pub const TIME: u8 = 0x66;
}

/// Type tags.
pub mod ty {
    /// `POSITION_2D`.
    pub const POSITION_2D: u8 = 0x01;
    /// `TYPE_BYTE`.
    pub const BYTE: u8 = 0x08;
    /// `TYPE_INTEGER`.
    pub const INTEGER: u8 = 0x09;
    /// `TYPE_DOUBLE`.
    pub const DOUBLE: u8 = 0x0B;
    /// `TYPE_STRING`.
    pub const STRING: u8 = 0x0C;
    /// `TYPE_STRINGLIST`.
    pub const STRINGLIST: u8 = 0x0E;
    /// `TYPE_COMPOUND`.
    pub const COMPOUND: u8 = 0x0F;
}

/// A value read back from SUMO.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A double.
    Double(f64),
    /// An integer.
    Int(i32),
    /// A string.
    Str(String),
    /// A string list.
    StrList(Vec<String>),
    /// A 2-D position.
    Pos(f64, f64),
    /// Anything else, as raw bytes after its type tag.
    Other(u8),
}

impl Value {
    /// As a double, if it is one.
    pub fn f(&self) -> Option<f64> {
        match self {
            Value::Double(v) => Some(*v),
            Value::Int(v) => Some(f64::from(*v)),
            _ => None,
        }
    }
    /// As a string, if it is one.
    pub fn s(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// A TraCI error.
#[derive(Debug)]
pub enum TraciError {
    /// The socket failed.
    Io(std::io::Error),
    /// SUMO refused a command.
    Refused { cmd: u8, message: String },
    /// The reply did not parse.
    Protocol(String),
}

impl std::fmt::Display for TraciError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TraciError::Io(e) => write!(f, "TraCI socket: {e}"),
            TraciError::Refused { cmd, message } => {
                write!(f, "SUMO refused command 0x{cmd:02x}: {message}")
            }
            TraciError::Protocol(m) => write!(f, "TraCI protocol: {m}"),
        }
    }
}

impl std::error::Error for TraciError {}

impl From<std::io::Error> for TraciError {
    fn from(e: std::io::Error) -> Self {
        TraciError::Io(e)
    }
}

/// Result alias.
pub type TResult<T> = std::result::Result<T, TraciError>;

/// Builds one command's content.
#[derive(Debug, Default, Clone)]
pub struct Content(pub Vec<u8>);

impl Content {
    /// Empty content.
    pub fn new() -> Self {
        Self(Vec::new())
    }
    /// A raw byte.
    pub fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    /// A raw big-endian i32.
    pub fn i32(mut self, v: i32) -> Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    /// A raw big-endian f64.
    pub fn f64(mut self, v: f64) -> Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    /// A raw string (length-prefixed, no tag).
    pub fn raw_str(mut self, s: &str) -> Self {
        self = self.i32(s.len() as i32);
        self.0.extend_from_slice(s.as_bytes());
        self
    }
    /// A tagged string.
    pub fn str(self, s: &str) -> Self {
        self.u8(ty::STRING).raw_str(s)
    }
    /// A tagged double.
    pub fn double(self, v: f64) -> Self {
        self.u8(ty::DOUBLE).f64(v)
    }
    /// A tagged integer.
    pub fn int(self, v: i32) -> Self {
        self.u8(ty::INTEGER).i32(v)
    }
    /// A tagged byte.
    pub fn byte(self, v: u8) -> Self {
        self.u8(ty::BYTE).u8(v)
    }
    /// A tagged string list.
    pub fn str_list(self, items: &[String]) -> Self {
        let mut c = self.u8(ty::STRINGLIST).i32(items.len() as i32);
        for s in items {
            c = c.raw_str(s);
        }
        c
    }
    /// A compound header.
    pub fn compound(self, n: i32) -> Self {
        self.u8(ty::COMPOUND).i32(n)
    }
}

fn frame_command(out: &mut Vec<u8>, id: u8, content: &[u8]) {
    let short = 2 + content.len();
    if short <= 255 {
        out.push(short as u8);
    } else {
        out.push(0);
        out.extend_from_slice(&((6 + content.len()) as i32).to_be_bytes());
    }
    out.push(id);
    out.extend_from_slice(content);
}

/// A read cursor over a reply.
pub struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, at: 0 }
    }
    fn take(&mut self, n: usize) -> TResult<&'a [u8]> {
        if self.at + n > self.buf.len() {
            return Err(TraciError::Protocol(format!(
                "reply truncated: wanted {n} bytes at {} of {}",
                self.at,
                self.buf.len()
            )));
        }
        let s = &self.buf[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }
    fn u8(&mut self) -> TResult<u8> {
        Ok(self.take(1)?[0])
    }
    fn i32(&mut self) -> TResult<i32> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f64(&mut self) -> TResult<f64> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(f64::from_be_bytes(a))
    }
    fn string(&mut self) -> TResult<String> {
        let n = self.i32()?.max(0) as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    /// A command header: returns `(id, end offset)`.
    fn header(&mut self) -> TResult<(u8, usize)> {
        let start = self.at;
        let mut len = usize::from(self.u8()?);
        if len == 0 {
            len = self.i32()?.max(0) as usize;
        }
        let id = self.u8()?;
        Ok((id, start + len))
    }
    fn value(&mut self) -> TResult<Value> {
        let t = self.u8()?;
        Ok(match t {
            ty::DOUBLE => Value::Double(self.f64()?),
            ty::INTEGER => Value::Int(self.i32()?),
            ty::STRING => Value::Str(self.string()?),
            ty::STRINGLIST => {
                let n = self.i32()?.max(0) as usize;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(self.string()?);
                }
                Value::StrList(v)
            }
            ty::POSITION_2D => Value::Pos(self.f64()?, self.f64()?),
            other => Value::Other(other),
        })
    }
    fn status(&mut self, expect: u8) -> TResult<()> {
        let (id, end) = self.header()?;
        let result = self.u8()?;
        let message = self.string()?;
        self.at = end;
        if id != expect {
            return Err(TraciError::Protocol(format!(
                "status for 0x{id:02x}, expected 0x{expect:02x}"
            )));
        }
        if result != 0 {
            return Err(TraciError::Refused { cmd: id, message });
        }
        Ok(())
    }
}

/// One `get` request of a batch.
#[derive(Debug, Clone)]
pub struct Get {
    /// The domain's get command (e.g. [`cmd::GET_VEHICLE`]).
    pub cmd: u8,
    /// The variable.
    pub var: u8,
    /// The object id.
    pub object: String,
}

/// A connection to a running SUMO.
pub struct Traci {
    stream: TcpStream,
    /// The API version SUMO reported.
    pub api_version: i32,
    /// The version string SUMO reported.
    pub sumo_version: String,
}

impl Traci {
    /// Connects to `127.0.0.1:port`, retrying for up to `wait` while SUMO starts.
    ///
    /// # Errors
    /// The last connection error, or a failed version handshake.
    pub fn connect(port: u16, wait: StdDuration) -> TResult<Self> {
        let deadline = std::time::Instant::now() + wait;
        let stream = loop {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(s) => break s,
                Err(e) if std::time::Instant::now() < deadline => {
                    let _ = e;
                    std::thread::sleep(StdDuration::from_millis(50));
                }
                Err(e) => return Err(e.into()),
            }
        };
        stream.set_nodelay(true)?;
        let mut t = Self {
            stream,
            api_version: 0,
            sumo_version: String::new(),
        };
        let reply = t.exchange(&[(cmd::GETVERSION, Vec::new())])?;
        let mut r = Reader::new(&reply);
        r.status(cmd::GETVERSION)?;
        let (_, end) = r.header()?;
        t.api_version = r.i32()?;
        t.sumo_version = r.string()?;
        let _ = end;
        Ok(t)
    }

    fn exchange(&mut self, commands: &[(u8, Vec<u8>)]) -> TResult<Vec<u8>> {
        let mut body = Vec::new();
        for (id, content) in commands {
            frame_command(&mut body, *id, content);
        }
        let mut msg = Vec::with_capacity(body.len() + 4);
        msg.extend_from_slice(&((body.len() + 4) as i32).to_be_bytes());
        msg.extend_from_slice(&body);
        self.stream.write_all(&msg)?;
        let mut len = [0u8; 4];
        self.stream.read_exact(&mut len)?;
        let n = (i32::from_be_bytes(len) as usize).saturating_sub(4);
        let mut reply = vec![0u8; n];
        self.stream.read_exact(&mut reply)?;
        Ok(reply)
    }

    /// Advances SUMO to `t_s` seconds (or one step when 0). Subscription results, which
    /// this client never asks for, are skipped.
    ///
    /// # Errors
    /// A refused step or a broken socket.
    pub fn step_to(&mut self, t_s: f64) -> TResult<()> {
        let reply = self.exchange(&[(cmd::SIMSTEP, Content::new().f64(t_s).0)])?;
        let mut r = Reader::new(&reply);
        r.status(cmd::SIMSTEP)
    }

    /// Sends a batch of `get`s in one message and returns the values in request order.
    ///
    /// # Errors
    /// The first refused request, or a broken reply.
    pub fn get_batch(&mut self, gets: &[Get]) -> TResult<Vec<Value>> {
        if gets.is_empty() {
            return Ok(Vec::new());
        }
        let commands: Vec<(u8, Vec<u8>)> = gets
            .iter()
            .map(|g| (g.cmd, Content::new().u8(g.var).raw_str(&g.object).0))
            .collect();
        let reply = self.exchange(&commands)?;
        let mut r = Reader::new(&reply);
        let mut out = Vec::with_capacity(gets.len());
        for g in gets {
            r.status(g.cmd)?;
            let (_, end) = r.header()?;
            let _var = r.u8()?;
            let _obj = r.string()?;
            out.push(r.value()?);
            r.at = end;
        }
        Ok(out)
    }

    /// One `get`.
    ///
    /// # Errors
    /// As [`Traci::get_batch`].
    pub fn get(&mut self, cmd: u8, var: u8, object: &str) -> TResult<Value> {
        let mut v = self.get_batch(&[Get {
            cmd,
            var,
            object: object.to_string(),
        }])?;
        v.pop()
            .ok_or_else(|| TraciError::Protocol("empty reply".to_string()))
    }

    /// Sends a batch of `set`s `(domain set command, variable, object, typed value)` in
    /// one message. Returns one result per command: a refusal of one does not hide the
    /// others' outcome.
    ///
    /// # Errors
    /// A broken socket or reply.
    pub fn set_batch(&mut self, sets: &[(u8, u8, String, Content)]) -> TResult<Vec<Result<(), String>>> {
        if sets.is_empty() {
            return Ok(Vec::new());
        }
        let commands: Vec<(u8, Vec<u8>)> = sets
            .iter()
            .map(|(c, v, o, val)| {
                let mut content = Content::new().u8(*v).raw_str(o).0;
                content.extend_from_slice(&val.0);
                (*c, content)
            })
            .collect();
        let reply = self.exchange(&commands)?;
        let mut r = Reader::new(&reply);
        let mut out = Vec::with_capacity(sets.len());
        for (c, ..) in sets {
            match r.status(*c) {
                Ok(()) => out.push(Ok(())),
                Err(TraciError::Refused { message, .. }) => out.push(Err(message)),
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Asks SUMO to close and waits for its acknowledgement.
    ///
    /// # Errors
    /// A broken socket.
    pub fn close(&mut self) -> TResult<()> {
        let reply = self.exchange(&[(cmd::CLOSE, Vec::new())])?;
        let mut r = Reader::new(&reply);
        r.status(cmd::CLOSE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_commands_use_the_extended_length() {
        let mut out = Vec::new();
        frame_command(&mut out, 0xa4, &vec![7u8; 300]);
        assert_eq!(out[0], 0);
        assert_eq!(i32::from_be_bytes([out[1], out[2], out[3], out[4]]), 306);
        assert_eq!(out[5], 0xa4);
        let mut short = Vec::new();
        frame_command(&mut short, 0x02, &[1, 2, 3]);
        assert_eq!(short, vec![5, 0x02, 1, 2, 3]);
    }

    #[test]
    fn a_refused_status_is_an_error() {
        let mut buf = Vec::new();
        let content = Content::new().u8(0xff).raw_str("no such vehicle").0;
        frame_command(&mut buf, 0xa4, &content);
        let mut r = Reader::new(&buf);
        assert!(matches!(r.status(0xa4), Err(TraciError::Refused { .. })));
    }
}
