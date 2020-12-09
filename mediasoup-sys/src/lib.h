#pragma once

#include "libmediasoupclient/include/mediasoupclient.hpp"
#include "rust/cxx.h"

// TODO(haze): maybe use absl optional?
#include <optional>

void print_mediasoup_version();
void debug();
void initialize();
void setup_logging();

struct WatchUpdater;

class ProxyDevice : public mediasoupclient::SendTransport::Listener,
                    mediasoupclient::Producer::Listener,
                    mediasoupclient::Consumer::Listener,
                    mediasoupclient::DataProducer::Listener,
                    mediasoupclient::DataConsumer::Listener {
  /* Virtual methods inherited from SendTransport::Listener. */
public:
  std::future<void> OnConnect(mediasoupclient::Transport *transport,
                              const nlohmann::json &dtlsParameters) override;
  void OnConnectionStateChange(mediasoupclient::Transport *transport,
                               const std::string &connectionState) override;
  std::future<std::string>
  OnProduce(mediasoupclient::SendTransport * /*transport*/,
            const std::string &kind, nlohmann::json rtpParameters,
            const nlohmann::json &appData) override;

  std::future<std::string>
  OnProduceData(mediasoupclient::SendTransport *transport,
                const nlohmann::json &sctpStreamParameters,
                const std::string &label, const std::string &protocol,
                const nlohmann::json &appData) override;

  /* Virtual methods inherited from Producer::Listener. */
public:
  void OnTransportClose(mediasoupclient::Producer *producer) override;

  /* Virtual methods inherited from DataConsumer::Listener */
public:
  void OnMessage(mediasoupclient::DataConsumer *dataConsumer,
                 const webrtc::DataBuffer &buffer) override;
  void OnConnecting(mediasoupclient::DataConsumer *dataConsumer) override {}
  void OnClosing(mediasoupclient::DataConsumer *dataConsumer) override {}
  void OnClose(mediasoupclient::DataConsumer *dataConsumer) override {}
  void OnOpen(mediasoupclient::DataConsumer *dataConsumer) override {}
  void OnTransportClose(mediasoupclient::DataConsumer *dataConsumer) override {}

public:
  void OnTransportClose(mediasoupclient::Consumer *dataProducer) override;

  /* Virtual methods inherited from DataProducer::Listener */
public:
  void OnOpen(mediasoupclient::DataProducer *dataProducer) override;
  void OnClose(mediasoupclient::DataProducer *dataProducer) override;
  void OnBufferedAmountChange(mediasoupclient::DataProducer *dataProducer,
                              uint64_t size) override;
  void OnTransportClose(mediasoupclient::DataProducer *dataProducer) override;
  std::future<void>
  OnConnectSendTransport(const nlohmann::json &dtlsParameters);
  std::future<void>
  OnConnectRecvTransport(const nlohmann::json &dtlsParameters);

  bool is_loaded() const;
  rust::String get_recv_rtp_capabilities() const;
  rust::String get_sctp_capabilities() const;
  void load_capabilities_from_string(rust::String);

  void create_data_consumer(const rust::String id,
                            const rust::String producerId,
                            const rust::String label,
                            const rust::String protocol,
                            const rust::String appData);

  void create_consumer(const rust::String id, const rust::String producerId,
                       const rust::String kind,
                       const rust::String rtpParametersStr,
                       const rust::String appDataStr);

  void CreateFakeSendTransport() const;
  void create_fake_recv_transport(const rust::String transportOptionsJsonStr);

  void
  set_on_connect_recv_transport_callback(rust::Fn<void(rust::String)> callback);
  void set_on_connection_state_update_callback(
      rust::Box<WatchUpdater>,
      rust::Fn<void(rust::Box<WatchUpdater>, const std::string &)> callback);

private:
  /* transport connect recv callback */
  std::optional<rust::Fn<void(rust::String)>> onConnectRecvCallback;

  /* connect state change callback */
  std::optional<rust::Fn<void(rust::Box<WatchUpdater>, const std::string &)>>
      onConnectionStateChangedCallback;
  WatchUpdater *onConnectionStateChangedUpdater{nullptr};

  mediasoupclient::Device device;

  mediasoupclient::RecvTransport *recvTransport{nullptr};
  mediasoupclient::SendTransport *sendTransport{nullptr};

  mediasoupclient::DataProducer *dataProducer{nullptr};
  mediasoupclient::DataConsumer *dataConsumer{nullptr};
};

std::unique_ptr<ProxyDevice> new_mediasoup_device();
