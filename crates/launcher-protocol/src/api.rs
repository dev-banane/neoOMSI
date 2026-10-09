use prost::Message as _;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Read};

include!(concat!(env!("OUT_DIR"), "/neoomsi.launcher.rs"));
include!(concat!(env!("OUT_DIR"), "/commands.rs"));

pub const VERSION: &str = "2";

pub fn encode(f: &Frame) -> io::Result<Vec<u8>> {
    crate::frame(&f.encode_to_vec())
}

pub fn read_frame(r: &mut impl Read) -> io::Result<Option<Frame>> {
    let Some(data) = crate::read_bytes(r, crate::MAX_FRAME)? else {
        return Ok(None);
    };
    Frame::decode(data.as_slice())
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

impl From<crate::link::GameState> for GameLink {
    fn from(g: crate::link::GameState) -> GameLink {
        let state = match g.state.as_str() {
            "starting" => GameLinkState::Starting,
            "loading" => GameLinkState::Loading,
            "running" => GameLinkState::Running,
            "stopping" => GameLinkState::Stopping,
            "failed" => GameLinkState::Failed,
            _ => GameLinkState::Unspecified,
        };
        GameLink {
            state: state.into(),
            progress: g.progress,
            message: g.message,
            window: g.window,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LanFile {
    role: String,
    name: String,
    code: Option<String>,
    address: Option<String>,
    addresses: Vec<LanFileAddress>,
    trying: Option<Vec<String>>,
    port: Option<u32>,
    session: String,
    tunnel: Option<String>,
    connected: bool,
    rejected: Option<String>,
    warnings: Vec<String>,
    host_name: Option<String>,
    map: String,
    players: Vec<LanFilePlayer>,
    chat: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LanFileAddress {
    label: String,
    address: String,
    kind: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LanFilePlayer {
    id: u32,
    name: String,
    bus: String,
    line: String,
    destination: String,
    passengers: u32,
    #[serde(rename = "where")]
    location: Option<String>,
    drawn: bool,
}

/// A field the game leaves out reads as empty; a file that does not parse, as an empty status.
pub fn lan_status(v: &Value) -> LanStatus {
    let f = LanFile::deserialize(v).unwrap_or_default();
    LanStatus {
        role: match f.role.as_str() {
            "host" => LanRole::Host,
            "client" => LanRole::Client,
            _ => LanRole::Unspecified,
        }
        .into(),
        name: f.name,
        code: f.code,
        address: f.address,
        addresses: f
            .addresses
            .into_iter()
            .map(|a| LanAddress {
                label: a.label,
                address: a.address,
                kind: a.kind,
            })
            .collect(),
        trying: f.trying.unwrap_or_default(),
        port: f.port,
        session: f.session,
        tunnel: f.tunnel,
        connected: f.connected,
        rejected: f.rejected,
        warnings: f.warnings,
        host_name: f.host_name,
        map: f.map,
        players: f
            .players
            .into_iter()
            .map(|p| LanPlayer {
                id: p.id,
                name: p.name,
                bus: p.bus,
                line: p.line,
                destination: p.destination,
                passengers: p.passengers,
                location: p.location,
                drawn: p.drawn,
            })
            .collect(),
        chat: f.chat,
    }
}

/// A value that is not a flag, a number or a text is left out.
pub fn setting_values(v: &Value) -> HashMap<String, SettingValue> {
    v.as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| {
            let value = match v {
                Value::Bool(b) => setting_value::Value::Flag(*b),
                Value::Number(n) => setting_value::Value::Number(n.as_f64()?),
                Value::String(s) => setting_value::Value::Text(s.clone()),
                _ => return None,
            };
            Some((k.clone(), SettingValue { value: Some(value) }))
        })
        .collect()
}

pub fn settings_json(values: &HashMap<String, SettingValue>) -> Value {
    Value::Object(
        values
            .iter()
            .filter_map(|(k, v)| {
                let value = match v.value.as_ref()? {
                    setting_value::Value::Flag(b) => Value::Bool(*b),
                    setting_value::Value::Number(n) => Value::from(*n),
                    setting_value::Value::Text(s) => Value::String(s.clone()),
                };
                Some((k.clone(), value))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frames_round_trip_and_split_anywhere() {
        let a = Frame {
            request_id: "req_1".into(),
            body: Some(frame::Body::Request(Request {
                command: Some(request::Command::Lines(LinesArgs {
                    map: "maps/Spandau/global.cfg".into(),
                    date: String::new(),
                })),
            })),
            ..Default::default()
        };
        let b = Frame {
            request_id: "req_1".into(),
            error: "no OMSI 2 folder".into(),
            body: None,
        };
        let mut bytes = encode(&a).unwrap();
        bytes.extend(encode(&b).unwrap());
        let mut r = io::Cursor::new(bytes);
        assert_eq!(read_frame(&mut r).unwrap(), Some(a));
        assert_eq!(read_frame(&mut r).unwrap(), Some(b));
        assert_eq!(read_frame(&mut r).unwrap(), None);
    }

    #[test]
    fn a_json_frame_of_the_first_protocol_is_refused() {
        let json = crate::frame(br#"{"type":"handshake","payload":{}}"#).unwrap();
        assert!(read_frame(&mut io::Cursor::new(json)).is_err());
    }

    #[test]
    fn the_handshake_and_shutdown_are_not_listed_as_commands() {
        assert!(!COMMANDS.contains(&"handshake") && !COMMANDS.contains(&"shutdown"));
        assert!(COMMANDS.contains(&"launch") && COMMANDS.contains(&"version"));
    }

    #[test]
    fn a_game_link_state_keeps_its_meaning() {
        let link = GameLink::from(crate::link::GameState {
            state: "loading".into(),
            progress: Some(0.25),
            message: "Spandau".into(),
            window: true,
        });
        assert_eq!((link.state(), link.progress), (GameLinkState::Loading, Some(0.25)));
    }

    #[test]
    fn settings_keep_flags_numbers_and_texts() {
        let v = json!({ "vsync": true, "msaa": 4, "ui_opacity": 0.8, "language": "de", "gone": null });
        let values = setting_values(&v);
        assert_eq!(values.len(), 4);
        assert_eq!(
            settings_json(&values),
            json!({ "vsync": true, "msaa": 4.0, "ui_opacity": 0.8, "language": "de" })
        );
    }

    #[test]
    fn the_lan_file_is_read_as_the_game_writes_it() {
        let s = lan_status(&json!({
            "role": "client",
            "code": null,
            "trying": ["10.0.0.2:27015"],
            "port": 27016,
            "connected": false,
            "rejected": "the session is full",
            "players": [{ "id": 3, "name": "Lena", "where": "120 m ahead", "drawn": true }],
            "updated": 1760000000,
        }));
        assert_eq!(s.role(), LanRole::Client);
        assert_eq!((s.code.as_deref(), s.port), (None, Some(27016)));
        assert_eq!(s.rejected.as_deref(), Some("the session is full"));
        assert_eq!(s.players[0].location.as_deref(), Some("120 m ahead"));
        assert_eq!(lan_status(&json!({ "role": 1 })), LanStatus::default());
    }
}
