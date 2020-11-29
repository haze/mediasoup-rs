use api::{ActiveSpeakerInfo, PeerMap};
use argh::FromArgs;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};

mod api;
mod error;
mod mediasoup;

use error::Result;

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

    // always initialize mediasoup (TODO(haze): Do this lazily at some other poitn)
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

type SharedProxyDevice = Arc<RwLock<mediasoup_sys::UniquePtr<mediasoup_sys::ffi::ProxyDevice>>>;

struct Client {
    peer_id: String,
    server_address: String,
    polling_interval_ms: u64,

    /// if we find a specific peer id, try setting up a recv channel for it
    target_peer_id: Option<String>,

    current_active_speaker: Option<watch::Receiver<CurrentActiveSpeaker>>,
    peer_data: Option<watch::Receiver<CurrentPeerData>>,

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
        Client {
            peer_id,
            server_address,
            polling_interval_ms,

            target_peer_id,

            current_active_speaker: None,
            peer_data: None,

            shutdown_poll_task_sender: None,
            poll_task_handle: None,
            // TODO(haze): Does this imply one device per client? Or is one device per computer?
            proxy_device: Arc::new(RwLock::new(mediasoup_sys::ffi::new_mediasoup_device())),
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
    /// This doesn't do much for us, but registeres us as a client on the server end so we can open
    /// transports
    async fn join_room(&mut self) -> Result<api::CapabilitiesResponse> {
        let url = format!("{}/signaling/join-as-new-peer", &self.server_address);
        let response = surf::post(url)
            .body(surf::Body::from_json(&api::JoinRoom {
                peer_id: &self.peer_id,
            })?)
            .recv_json()
            .await?;
        self.start_sync_polling_task();
        return Ok(response);
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

        self.poll_task_handle = Some(tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        match Client::sync_raw(&*task_server_address, &*task_peer_id).await {
                            Ok(response) =>
                                        Client::on_sync_update(&peer_data_tx, &task_peer_peek, &active_speaker_tx,
                                            &task_host,
                                            &task_peer_id,
                                            &task_target_peer_id,
                                            &task_device,
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

    /// `sync_update` is called whenever a sync succeedes
    async fn on_sync_update(
        peer_data_tx: &watch::Sender<CurrentPeerData>,
        peer_data_rx: &watch::Receiver<CurrentPeerData>,
        active_speaker_tx: &watch::Sender<CurrentActiveSpeaker>,
        host: &str,
        my_peer_id: &str,
        target_peer_id: &Option<String>,
        device: &SharedProxyDevice,
        response: api::SyncResponse,
    ) {
        // check if we already have seen peers before
        let mut new_peer_found = false;
        {
            if let Some(ref peer_data) = *peer_data_rx.borrow() {
                // threeway optimized match, check key size, then key names, then deep equals
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
                        Client::on_target_found(host, my_peer_id, target_peer_id, device).await
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
        host: &str,
        my_peer_id: &str,
        target_peer_id: &str,
        device: &SharedProxyDevice,
    ) -> Result<()> {
        let media_tag = String::from("screen-video");
        let create_transport_resp = Client::create_transport_raw(
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
            eprintln!("Created fake transport");
        }

        // we dropped the write lock, get a read one
        let device_read = device.read().await;
        let rtp_capabilities = serde_json::from_str(&*device_read.get_recv_rtp_capabilities()?)?;
        drop(device_read);

        eprintln!("RTP_Capabilities: {:?}", &rtp_capabilities);

        let recv_track_request = api::recv_track::Request {
            peer_id: my_peer_id.to_string(),
            media_peer_id: target_peer_id.to_string(),
            // TODO(haze): more than just screen-video
            media_tag: media_tag.clone(),
            rtp_capabilities,
        };

        // create the recv track
        let recv_track_response = Client::recv_track_raw(host, &recv_track_request).await?;
        dbg!(&recv_track_response);

        // fn create_consumer(
        //     self: Pin<&mut ProxyDevice>,
        //     id: String,
        //     producer_id: String,
        //     kind: String,
        //     rtp_parameters_str: String,
        //     app_data_str: String,
        // );

        {
            let app_data = serde_json::to_string(&CreateConsumerAppData {
                peer_id: target_peer_id.to_string(),
                media_tag: media_tag.clone(),
            })?;
            let rtp_parameters = serde_json::to_string(&recv_track_response.rtp_parameters)?;
            dbg!((&app_data, &rtp_parameters));
            let mut device = device.write().await;
            device.pin_mut().create_consumer(
                recv_track_response.id.clone(),
                recv_track_response.producer_id.clone(),
                recv_track_response.kind.to_string(),
                rtp_parameters,
                app_data,
            );
        }

        // start the consumer

        // let connect_transport_request = api::connect_transport::Request {
        //     peer_id: my_peer_id.to_string(),
        //     transport_id: recv_track_response.transport_options.id.clone(),
        // };
        // // ok now connect them together
        // let connect_transport_response = Client::connect_transport_response(host).await?;

        Ok(())
    }

    /// `shutdown_polling_task` will do two things:
    /// 1. Attempt to send a kill signal to the polling task. This will trigger an early break
    ///    (inbetween intervals) to the loop and return early.
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

    async fn leave_room(&mut self) -> Result<api::LeaveResponse> {
        let response = Client::leave_room_raw(&*self.server_address, &*self.peer_id).await?;
        if response.left {
            self.shutdown_polling_task().await?;
        }
        Ok(response)
    }

    async fn leave_room_raw(host: &str, peer_id: &str) -> Result<api::LeaveResponse> {
        let url = format!("{}/signaling/leave", host);
        surf::post(url)
            .body(surf::Body::from_json(&api::LeaveRoom { peer_id })?)
            .recv_json()
            .await
            .map_err(|e| e.into())
    }

    async fn connect_transport(
        &self,
        request: &api::connect_transport::Request,
    ) -> Result<api::connect_transport::Response> {
        Client::connect_transport_raw(&*self.server_address, request).await
    }

    async fn connect_transport_raw(
        host: &str,
        request: &api::connect_transport::Request,
    ) -> Result<api::connect_transport::Response> {
        let url = format!("{}/signaling/connect-transport", host);
        surf::post(url)
            .body(surf::Body::from_json(request)?)
            .recv_json()
            .await
            .map_err(|e| e.into())
    }

    async fn sync(&mut self) -> Result<api::SyncResponse> {
        Client::sync_raw(&*self.server_address, &*self.peer_id).await
    }

    async fn sync_raw(host: &str, peer_id: &str) -> Result<api::SyncResponse> {
        let url = format!("{}/signaling/sync", host);
        surf::post(url)
            .body(surf::Body::from_json(&api::SyncRequest { peer_id })?)
            .recv_json()
            .await
            .map_err(|e| e.into())
    }

    async fn recv_track(
        &mut self,
        request: &api::recv_track::Request,
    ) -> Result<api::recv_track::Response> {
        Client::recv_track_raw(&*self.server_address, request).await
    }

    async fn recv_track_raw(
        host: &str,
        request: &api::recv_track::Request,
    ) -> Result<api::recv_track::Response> {
        let url = format!("{}/signaling/recv-track", host);
        surf::post(url)
            .body(surf::Body::from_json(request)?)
            .recv_json()
            .await
            .map_err(|e| e.into())
    }

    async fn create_transport(
        &self,
        params: api::CreateTransport,
    ) -> Result<api::CreateTransportResponse> {
        Client::create_transport_raw(&*self.server_address, &params.into_raw(&*self.peer_id)).await
    }

    async fn create_transport_raw<'a>(
        host: &str,
        params: &api::RawCreateTransport<'a>,
    ) -> Result<api::CreateTransportResponse> {
        let url = format!("{}/signaling/create-transport", host);
        surf::post(url)
            .body(surf::Body::from_json(params)?)
            .recv_json()
            .await
            .map_err(|e| e.into())
    }
}

#[derive(serde::Serialize)]
struct CreateConsumerAppData {
    #[serde(rename = "peerId")]
    peer_id: String,
    #[serde(rename = "mediaTag")]
    media_tag: String,
}
