use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// types used for the `/recv-track` route
pub mod recv_track {
    use super::{Deserialize, RTPCapabilities, Serialize};

    #[derive(Serialize)]
    pub struct Request {
        #[serde(rename = "mediaPeerId")]
        pub media_peer_id: String,
        #[serde(rename = "mediaTag")]
        pub media_tag: String,
        #[serde(rename = "peerId")]
        pub peer_id: String,
        #[serde(rename = "rtpCapabilities")]
        pub rtp_capabilities: RTPCapabilities,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "lowercase")]
    pub enum CodecKind {
        Audio,
        Video,
    }

    // TODO(haze): transform `kind` from String to typed enum
    #[derive(Deserialize, Debug)]
    pub struct Response {
        id: String,
        kind: CodecKind,
        #[serde(rename = "producerId")]
        producer_id: String,
        #[serde(rename = "producerPaused")]
        producer_paused: bool,
        rtp_parameters: RTPParameters,
        #[serde(rename = "type")]
        track_kind: String,
    }

    #[derive(Deserialize, Debug)]
    pub struct RTPParameters {
        codecs: Vec<Codec>,
        encodings: Vec<Encoding>,
        #[serde(rename = "headerExtensions")]
        header_extensions: Vec<HeaderExtension>,
        mid: String,
    }

    #[derive(Deserialize, Debug)]
    pub struct Codec {
        #[serde(rename = "clockRate")]
        clock_rate: usize,
        #[serde(rename = "mimeType")]
        mime_type: String,
        // TODO(haze): figure out what this is
        parameters: serde_json::Value,
        #[serde(rename = "payloadType")]
        payload_type: usize,
        #[serde(rename = "rtcpFeedback")]
        rtcp_feedback: Vec<Feedback>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Feedback {
        #[serde(rename = "type")]
        kind: String,
        parameter: String,
    }

    #[derive(Deserialize, Debug)]
    pub struct Encoding {
        ssrc: usize,
        rtx: Rtx,
    }

    #[derive(Deserialize, Debug)]
    pub struct Rtx {
        ssrc: usize,
    }

    #[derive(Deserialize, Debug)]
    pub struct HeaderExtension {
        encrypt: bool,
        id: usize,
        // TODO(haze): figure out what this is
        parameters: serde_json::Value,
        uri: String,
    }
}

#[derive(Serialize)]
pub struct RawCreateTransport<'peer_id> {
    pub direction: TransportDirection,
    #[serde(rename = "peerId")]
    pub peer_id: &'peer_id str,
}

pub struct CreateTransport {
    pub direction: TransportDirection,
}

impl CreateTransport {
    pub fn into_raw<'peer_id>(self, peer_id: &'peer_id str) -> RawCreateTransport<'peer_id> {
        RawCreateTransport {
            peer_id,
            direction: self.direction,
        }
    }
}

#[derive(Serialize)]
pub enum TransportDirection {
    Send,
}

#[derive(Deserialize, Debug)]
pub struct CreateTransportResponse {
    #[serde(rename = "transportOptions")]
    pub transport_options: TransportOptions,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TransportOptions {
    pub id: String,
    #[serde(rename = "iceParameters")]
    pub ice_parameters: IceParameters,
    #[serde(rename = "iceCandidates")]
    pub ice_candidates: Vec<IceCandidate>,
    #[serde(rename = "dtlsParameters")]
    pub dtls_parameters: DTLSParameters,
    #[serde(rename = "sctpParameters")]
    pub sctp_parameteres: Option<SCTPParameters>,
}

type SCTPParameters = serde_json::Value;

#[derive(Deserialize, Serialize, Debug)]
pub struct DTLSParameters {
    role: String,
    fingerprints: Vec<Fingerprint>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Fingerprint {
    algorithm: String,
    value: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct IceCandidate {
    foundation: String,
    ip: String,
    port: usize,
    priority: usize,
    protocol: String,

    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct IceParameters {
    #[serde(rename = "iceLite")]
    ice_lite: bool,
    password: String,
    #[serde(rename = "usernameFragment")]
    username_fragment: String,
}

#[derive(Serialize)]
pub struct PeerId<'peer_id> {
    /// Even though we have `peer_id` conforming to a UUID on our `Client` struct, the server
    /// may not respect this, so we relax the constraint here
    #[serde(rename = "peerId")]
    pub peer_id: &'peer_id str,
}

pub type JoinRoom<'a> = PeerId<'a>;
pub type LeaveRoom<'a> = PeerId<'a>;
pub type SyncRequest<'a> = PeerId<'a>;

#[derive(Deserialize, Debug)]
pub struct LeaveResponse {
    pub left: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CapabilitiesResponse {
    #[serde(rename = "routerRtpCapabilities")]
    router_rtp_capabilities: RTPCapabilities,
}

pub type PeerMap = HashMap<String, PeerInfo>;

#[derive(Deserialize, Debug)]
pub struct SyncResponse {
    #[serde(rename = "activeSpeaker")]
    pub active_speaker: ActiveSpeakerInfo,
    pub peers: PeerMap,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ActiveSpeakerInfo {
    #[serde(rename = "producerId")]
    producer_id: Option<String>,
    #[serde(rename = "peer_id")]
    peer_id: Option<String>,

    // TODO(haze): figure out what this is
    volume: Option<serde_json::Value>,
}

// NOTE(haze): Convert timestamps to proper chrono dates. This was simply left as usizes to keep
// the dependency tree lean. Timestamps are currently not used
// TODO(haze): serde_json::Value's
#[derive(Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct PeerInfo {
    #[serde(rename = "joinTs")]
    join_timestamp: usize,
    #[serde(rename = "lastSeenTs")]
    last_seen_timestamp: usize,
    media: Media,
    #[serde(rename = "consumerLayers")]
    consumer_layers: HashMap<String, ConsumerLayer>,
    stats: HashMap<String, Vec<Stats>>,
}

// TODO(haze): Add extra media types (only experimented with screen sharing for now)
#[derive(Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Media {
    #[serde(rename = "screen-video")]
    screen_video: Option<ScreenVideo>,
}

#[derive(Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct ScreenVideo {
    paused: bool,
    encodings: Vec<Encoding>,
}

#[derive(Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Encoding {
    ssrc: usize,
    dtx: bool,
    rtx: Rtx,
}

#[derive(Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Rtx {
    ssrc: usize,
}

#[derive(Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct ConsumerLayer {
    #[serde(rename = "clientSelectedLayer")]
    client_selected_layer: Option<serde_json::Value>,
    #[serde(rename = "currentLayer")]
    current_layer: Option<serde_json::Value>,
}

// TODO(haze): serde_json::Value's
#[derive(Deserialize, PartialEq, Eq, Clone, Debug)]
pub struct Stats {
    bitrate: usize,
    #[serde(rename = "fractionLost")]
    fraction_lost: serde_json::Value,
    score: usize,
    jitter: Option<usize>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RTPCapabilities {
    codecs: Vec<Codec>,
    #[serde(rename = "headerExtensions")]
    header_extensions: Vec<HeaderExtension>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "kind")]
#[serde(rename_all = "lowercase")]
enum Codec {
    Audio {
        channels: usize,
        #[serde(flatten)]
        attributes: SharedCodecAttributes,
    },
    Video(SharedCodecAttributes),
}

#[derive(Deserialize, Serialize, Debug)]
struct SharedCodecAttributes {
    #[serde(rename = "mimeType")]
    mime: String,
    #[serde(rename = "clockRate")]
    clock_rate: usize,
    #[serde(rename = "preferredPayloadType")]
    preferred_payload_type: usize,
}

// TODO(haze): investigate direction
#[derive(Deserialize, Serialize, Debug)]
struct HeaderExtension {
    kind: HeaderExtensionKind,
    uri: String,
    #[serde(rename = "preferredId")]
    preferred_id: usize,
    #[serde(rename = "preferredEncrypt")]
    preferred_encrypt: bool,
    direction: HeaderExtensionDirection,
}

#[derive(Deserialize, Serialize, Debug)]
enum HeaderExtensionDirection {
    #[serde(rename = "sendonly")]
    SendOnly,
    #[serde(rename = "recvonly")]
    ReceiveOnly,
    #[serde(rename = "sendrecv")]
    SendAndReceive,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "lowercase")]
enum HeaderExtensionKind {
    Audio,
    Video,
}
