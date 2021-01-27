use api::{ActiveSpeakerInfo, PeerMap};
use argh::FromArgs;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

mod api;
mod error;
mod mediasoup;

use error::Result;
use reqwest as req; // i typo this way too much

// TODO(haze): Add selector for selecting specific media type for specific peer id (blocking on not
// knowing all media types)
#[derive(FromArgs, Debug)]
#[argh(description = "rust mediasoup client")]
struct Arguments {
    /// optionally set the client peer id for debugging
    #[argh(option)]
    my_peer_id: Option<String>,

    /// remote address for the mediasoup server
    #[argh(option)]
    server: String,

    /// peer_id of peer to connect to when joined
    #[argh(option)]
    peer_id: Option<String>,

    /// client polling sync interval in milliseconds (how often the client should request updates from the server)
    #[argh(option)]
    polling_interval: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Arguments = argh::from_env();
    dbg!(&args);

    // always initialize mediasoup (TODO(haze): Do this lazily at some other point)
    mediasoup_sys::ffi::setup_logging();
    mediasoup_sys::ffi::initialize();

    let my_peer_id = args
        .my_peer_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // default polling interval to 1 second
    let polling_interval = args.polling_interval.unwrap_or_else(|| 1000);

    let mut client = Client::new(
        my_peer_id,
        args.server.clone(),
        args.peer_id,
        polling_interval,
    );
    let join_resp = client.join_room().await?;
    dbg!(&join_resp);
    client.load_device_from_capabilities(&join_resp).await;
    // dbg!(&join_resp);

    // let leave_resp = client.leave_room().await?;
    // dbg!(&leave_resp);
    loop {
        if let Some(ref active_speaker_info) = client.current_active_speaker {
            if let Some(ref active_speaker) = *active_speaker_info.borrow() {
                // dbg!(&active_speaker);
            }
        }
        if let Some(ref peer_info) = client.peer_data {
            if let Some(ref peer_info) = *peer_info.borrow() {
                // dbg!(&peer_info);
            }
        }
    }

    Ok(())
}

type CurrentPeerData = Option<PeerMap>;
type CurrentActiveSpeaker = Option<ActiveSpeakerInfo>;
type CurrentConnectionState = Option<String>;

type SharedProxyDevice = Arc<RwLock<mediasoup_sys::UniquePtr<mediasoup_sys::ffi::ProxyDevice>>>;

struct Client {
    http_client: Arc<req::Client>,
    peer_id: String,
    server_address: String,
    polling_interval_ms: u64,

    /// if we find a specific peer id, try setting up a recv channel for it
    target_peer_id: Option<String>,

    current_active_speaker: Option<watch::Receiver<CurrentActiveSpeaker>>,
    peer_data: Option<watch::Receiver<CurrentPeerData>>,
    connection_state: watch::Receiver<CurrentConnectionState>,

    poll_task_handle: Option<tokio::task::JoinHandle<()>>,
    shutdown_poll_task_sender: Option<tokio::sync::oneshot::Sender<()>>,
    /// our mediasoup device
    /// the device is held within a rwlock to ensure we have exclusive write access, but shared
    /// read access (as pin_mut needs exclusive access as well) (pin_mut is needed for non-const
    /// c++ shadowed functions)
    proxy_device: SharedProxyDevice,
}

// TODO(haze): Extract type out of Response but with errors
// TODO(haze): Extract out types not relevant to client for easy passing along instead of handling
// them one by one
impl Client {
    /// `new` will create
    fn new(
        peer_id: String,
        server_address: String,
        target_peer_id: Option<String>,
        polling_interval_ms: u64,
    ) -> Client {
        let (connection_state_tx, connection_state_rx) = watch::channel(None);
        let mut device = mediasoup_sys::ffi::new_mediasoup_device();
        {
            // pin ref method receiver not implemented, have to re-pin
            device
                .pin_mut()
                .set_on_connect_recv_transport_callback(Client::on_connection_recv_transport);
            device.pin_mut().set_on_connection_state_update_callback(
                Box::new(mediasoup_sys::WatchUpdater(connection_state_tx)),
                |sender, state| {
                    dbg!((&sender, &state));
                },
            );
        }
        Client {
            http_client: Arc::new(req::Client::new()),
            peer_id,
            server_address,
            polling_interval_ms,

            target_peer_id,

            current_active_speaker: None,
            peer_data: None,
            connection_state: connection_state_rx,

            shutdown_poll_task_sender: None,
            poll_task_handle: None,
            // TODO(haze): Does this imply one device per client? Or is one device per computer?
            proxy_device: Arc::new(RwLock::new(device)),
        }
    }

    // TODO(haze): Return proper error with cxx::Exception
    async fn load_device_from_capabilities(&mut self, capabilities: &api::CapabilitiesResponse) {
        if let Ok(capabilities_str) = serde_json::to_string(&capabilities.router_rtp_capabilities) {
            let mut device_write_lock = self.proxy_device.write().await;
            if let Err(why) = device_write_lock
                .pin_mut()
                .load_capabilities_from_string(capabilities_str)
            {
                eprintln!("Failed to load device: {:?}", &why);
            }
        }
    }

    /// `join_room` will send a `api::Request::JoinRoom` request to the bound server address.
    /// This doesn't do much for us, but registers us as a client on the server end so we can open
    /// transports
    async fn join_room(&mut self) -> Result<api::join::Response> {
        let response = Client::join_room_raw(
            self.http_client.as_ref(),
            &*self.server_address,
            &api::join::Request {
                peer_id: &self.peer_id,
            },
        )
        .await?;
        self.start_sync_polling_task();
        Ok(response)
    }

    async fn join_room_raw<'a>(
        client: &req::Client,
        host: &str,
        request: &api::join::Request<'a>,
    ) -> Result<api::join::Response> {
        let url = format!("{}/signaling/join-as-new-peer", host);
        Ok(client
            .post(&*url)
            .json(request)
            .send()
            .await?
            .json::<api::join::Response>()
            .await?)
    }

    fn start_sync_polling_task(&mut self) {
        let (shutdown_poll_task_sender, mut shutdown_poll_task_receiver) =
            tokio::sync::oneshot::channel();
        let polling_dur = std::time::Duration::from_millis(self.polling_interval_ms);
        let start = tokio::time::Instant::now() + polling_dur;
        let mut interval = tokio::time::interval_at(start, polling_dur);

        self.shutdown_poll_task_sender = Some(shutdown_poll_task_sender);

        let (active_speaker_tx, active_speaker_rx) = watch::channel(None);
        let (peer_data_tx, peer_data_rx) = watch::channel(None);

        let task_peer_peek = peer_data_rx.clone();

        self.peer_data = Some(peer_data_rx);
        self.current_active_speaker = Some(active_speaker_rx);

        let task_host = self.server_address.clone();
        let task_peer_id = self.peer_id.clone();
        let task_server_address = self.server_address.clone();
        let task_target_peer_id = self.target_peer_id.clone();
        let task_device = Arc::clone(&self.proxy_device);
        let task_http_client = Arc::clone(&self.http_client);

        self.poll_task_handle = Some(tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        match Client::sync_raw(task_http_client.as_ref(), &*task_server_address, &*task_peer_id).await {
                            Ok(response) =>
                                        Client::on_sync_update(&peer_data_tx, &task_peer_peek, &active_speaker_tx,
                                            &task_host,
                                            &task_peer_id,
                                            &task_target_peer_id,
                                            &task_device,
                                            task_http_client.as_ref(),
                                            response
                                        ).await,
                            Err(why) =>
                                eprintln!("Got error from sync request: {}", &why),
                        }
                    },
                    _ = &mut shutdown_poll_task_receiver => {
                        // break out of polling loop when we get shutdown signal
                        break;
                    },
                }
            }
        }));
    }

    fn on_connection_recv_transport(dtls_parameters_str: String) {
        println!("hello from rust!");
        println!("params: {}", &dtls_parameters_str);
    }

    /// `sync_update` is called whenever a sync succeeds
    async fn on_sync_update(
        peer_data_tx: &watch::Sender<CurrentPeerData>,
        peer_data_rx: &watch::Receiver<CurrentPeerData>,
        active_speaker_tx: &watch::Sender<CurrentActiveSpeaker>,
        host: &str,
        my_peer_id: &str,
        target_peer_id: &Option<String>,
        device: &SharedProxyDevice,
        client: &req::Client,
        response: api::sync::Response,
    ) {
        // check if we already have seen peers before
        let mut new_peer_found = false;
        {
            if let Some(ref peer_data) = *peer_data_rx.borrow() {
                // three-way optimized match, check key size, then key names, then deep equals
                let key_size_eql = peer_data.keys().len() == response.peers.keys().len();
                let keys_match = peer_data.keys().all(|key| response.peers.contains_key(key));
                let is_new = !key_size_eql
                    || !keys_match
                    || peer_data.iter().all(|(key, value)| {
                        if let Some(new_value) = response.peers.get(key) {
                            value == new_value
                        } else {
                            false
                        }
                    });
                if is_new {
                    println!("new peers below (cache replace):");
                    for peer_id in peer_data.keys() {
                        println!("{}", peer_id);
                    }
                    new_peer_found = true;
                }
            } else {
                println!("new peers below (cache begin):");
                for peer_id in response.peers.keys() {
                    println!("{:?}", peer_id);
                }

                new_peer_found = true;
            }
        }

        if new_peer_found {
            // run code when a new client is found
            if let Some(ref target_peer_id) = target_peer_id {
                // we are looking for a specific client, now is time to do more processing
                if let Some(_target_peer) = response.peers.get(target_peer_id) {
                    if let Err(why) =
                        Client::on_target_found(client, host, my_peer_id, target_peer_id, device)
                            .await
                    {
                        eprintln!("Failed to run on_target_found callback: {:?}", &why);
                    }
                } else {
                    eprintln!("new peers doesn't contain target");
                }
            }
        }

        // finally, broadcast all updates
        if let Err(why) = peer_data_tx.broadcast(Some(response.peers)) {
            eprintln!("Failed to broadcast sync'd peer data: {}", &why);
        }
        if let Err(why) = active_speaker_tx.broadcast(Some(response.active_speaker)) {
            eprintln!("Failed to broadcast sync'd active speaker info: {}", &why);
        }
    }

    async fn on_target_found(
        client: &req::Client,
        host: &str,
        my_peer_id: &str,
        target_peer_id: &str,
        device: &SharedProxyDevice,
    ) -> Result<()> {
        let create_transport_resp = Client::create_transport_raw(
            client,
            host,
            &api::RawCreateTransport {
                direction: api::TransportDirection::Receive,
                peer_id: my_peer_id,
            },
        )
        .await?;
        dbg!(&create_transport_resp);

        // set the recv transport
        if let Ok(transport_options_str) =
            serde_json::to_string(&create_transport_resp.transport_options)
        {
            eprintln!("Creating fake transport...");
            let mut device = device.write().await;
            device
                .pin_mut()
                .create_fake_recv_transport(transport_options_str);
            eprintln!("Created fake transport!");
        }

        // we dropped the write lock, get a read one
        let device_read = device.read().await;
        let rtp_capabilities = serde_json::from_str(&*device_read.get_recv_rtp_capabilities()?)?;
        drop(device_read);

        let recv_track_request = api::recv_track::Request {
            peer_id: my_peer_id.to_string(),
            media_peer_id: target_peer_id.to_string(),
            // TODO(haze): more than just screen-video
            media_tag: String::from("screen-video"),
            rtp_capabilities,
        };

        // create the recv track
        let recv_track_response = Client::recv_track_raw(client, host, &recv_track_request).await?;
        dbg!(&recv_track_response);

        {
            eprintln!("Creating consumer...");
            let mut device = device.write().await;
            // id: String,
            // producer_id: String,
            // kind: String,
            // rtp_parameters_json_str: String,
            // app_data_json_str: String,
            let id = recv_track_response.id.clone();
            let producer_id = recv_track_response.producer_id.clone();
            let kind = recv_track_response.kind.to_string();
            let rtp_parameters = serde_json::to_string_pretty(&recv_track_response.rtp_parameters)?;
            println!("{}", &rtp_parameters);
            device.pin_mut().create_consumer(
                id,
                producer_id,
                kind,
                // rtp_parameters,
                String::from(
                    r#"{"codecs": [
    {
      "clockRate": 90000,
      "mimeType": "video/VP8",
      "parameters": {},
      "payloadType": 101,
      "rtcpFeedback": [
        {
          "type": "transport-cc",
          "parameter": ""
        },
        {
          "type": "ccm",
          "parameter": "fir"
        },
        {
          "type": "nack",
          "parameter": ""
        },
        {
          "type": "nack",
          "parameter": "pli"
        }
      ]
    },
    {
      "clockRate": 90000,
      "mimeType": "video/rtx",
      "parameters": {
        "apt": 101
      },
      "payloadType": 102,
      "rtcpFeedback": []
    }
  ],
  "encodings": [
    {
      "ssrc": 793772131,
      "rtx": {
        "ssrc": 664106021
      }
    }
  ],
  "headerExtensions": [
    {
      "encrypt": false,
      "id": 1,
      "parameters": {},
      "uri": "urn:ietf:params:rtp-hdrext:sdes:mid"
    },
    {
      "encrypt": false,
      "id": 4,
      "parameters": {},
      "uri": "http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time"
    },
    {
      "encrypt": false,
      "id": 5,
      "parameters": {},
      "uri": "http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01"
    },
    {
      "encrypt": false,
      "id": 6,
      "parameters": {},
      "uri": "http://tools.ietf.org/html/draft-ietf-avtext-framemarking-07"
    },
    {
      "encrypt": false,
      "id": 11,
      "parameters": {},
      "uri": "urn:3gpp:video-orientation"
    },
    {
      "encrypt": false,
      "id": 12,
      "parameters": {},
      "uri": "urn:ietf:params:rtp-hdrext:toffset"
    }
  ],
  "rtcp": {
      "cname": "test"
  },
  "mid": "0"
}"#,
                ),
                String::from("{}"),
            );
            eprintln!("Consumer created!");
        }

        // let connect_transport_request = api::connect_transport::Request {
        //     peer_id: my_peer_id.to_string(),
        //     transport_id: recv_track_response.
        // };
        // // ok now connect them together
        // let connect_transport_response = Client::connect_transport_response(host).await?;

        Ok(())
    }

    /// `shutdown_polling_task` will do two things:
    /// 1. Attempt to send a kill signal to the polling task. This will trigger an early break
    ///    (in-between intervals) to the loop and return early.
    /// 2. Attempt to join the task.
    async fn shutdown_polling_task(&mut self) -> Result<()> {
        if let Some(sender) = self.shutdown_poll_task_sender.take() {
            sender
                .send(())
                .map_err(|_| error::Error::ShutdownPollingTaskFailed)?;
        }
        if let Some(handle) = self.poll_task_handle.take() {
            handle.await?;
        }
        Ok(())
    }

    async fn leave_room(&mut self) -> Result<api::leave::Response> {
        let response = Client::leave_room_raw(
            self.http_client.as_ref(),
            &*self.server_address,
            &*self.peer_id,
        )
        .await?;
        if response.left {
            self.shutdown_polling_task().await?;
        }
        Ok(response)
    }

    async fn leave_room_raw(
        client: &req::Client,
        host: &str,
        peer_id: &str,
    ) -> Result<api::leave::Response> {
        let url = format!("{}/signaling/leave", host);
        Ok(client
            .post(&*url)
            .json(&api::leave::Request { peer_id })
            .send()
            .await?
            .json::<api::leave::Response>()
            .await?)
    }

    async fn connect_transport(
        &self,
        request: &api::connect_transport::Request,
    ) -> Result<api::connect_transport::Response> {
        Client::connect_transport_raw(self.http_client.as_ref(), &*self.server_address, request)
            .await
    }

    async fn connect_transport_raw(
        client: &req::Client,
        host: &str,
        request: &api::connect_transport::Request,
    ) -> Result<api::connect_transport::Response> {
        let url = format!("{}/signaling/connect-transport", host);
        Ok(client
            .post(&*url)
            .json(request)
            .send()
            .await?
            .json::<api::connect_transport::Response>()
            .await?)
    }

    async fn sync(&mut self) -> Result<api::sync::Response> {
        Client::sync_raw(
            self.http_client.as_ref(),
            &*self.server_address,
            &*self.peer_id,
        )
        .await
    }

    async fn sync_raw(
        client: &req::Client,
        host: &str,
        peer_id: &str,
    ) -> Result<api::sync::Response> {
        let url = format!("{}/signaling/sync", host);
        Ok(client
            .post(&*url)
            .json(&api::sync::Request { peer_id })
            .send()
            .await?
            .json::<api::sync::Response>()
            .await?)
    }

    async fn recv_track(
        &mut self,
        request: &api::recv_track::Request,
    ) -> Result<api::recv_track::Response> {
        Client::recv_track_raw(self.http_client.as_ref(), &*self.server_address, request).await
    }

    async fn recv_track_raw(
        client: &req::Client,
        host: &str,
        request: &api::recv_track::Request,
    ) -> Result<api::recv_track::Response> {
        let url = format!("{}/signaling/recv-track", host);
        Ok(client
            .post(&*url)
            .json(request)
            .send()
            .await?
            .json::<api::recv_track::Response>()
            .await?)
    }

    async fn create_transport(
        &self,
        params: api::CreateTransport,
    ) -> Result<api::CreateTransportResponse> {
        Client::create_transport_raw(
            self.http_client.as_ref(),
            &*self.server_address,
            &params.into_raw(&*self.peer_id),
        )
        .await
    }

    async fn create_transport_raw<'a>(
        client: &req::Client,
        host: &str,
        request: &api::RawCreateTransport<'a>,
    ) -> Result<api::CreateTransportResponse> {
        let url = format!("{}/signaling/create-transport", host);
        Ok(client
            .post(&*url)
            .json(request)
            .send()
            .await?
            .json::<api::CreateTransportResponse>()
            .await?)
    }
}
