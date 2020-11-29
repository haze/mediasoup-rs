#include "src/lib.h"
#include "mediasoup-sys/src/lib.rs.h"
#include <iostream>
#include <stdio.h>
#include <json.hpp>
#include <fstream>
#include <future>

void print_mediasoup_version() {
  std::cout << mediasoupclient::Version() << std::endl;
}

void initialize() {
  mediasoupclient::Initialize();
}

void debug() {
  auto device = mediasoupclient::Device();
  std::cout << device.IsLoaded() << std::endl;
  /* std::ifstream my_file("/Users/haze/work/mediasoup_client/capabilities.json"); */
  /* nlohmann::json j; */
  /* j << my_file; */
  /* if (!device.IsLoaded()) { */
  /*   device.Load(j); */
  /* } */
  /* std::cout << device.IsLoaded() << std::endl; */
}

rust::String ProxyDevice::get_recv_rtp_capabilities() const {
  auto capabilities = this->device.GetRtpCapabilities();
  return capabilities.dump();
}

rust::String ProxyDevice::get_sctp_capabilities() const {
  auto capabilities = this->device.GetSctpCapabilities();
  return capabilities.dump();
}

void ProxyDevice::CreateFakeSendTransport() const {
}

void ProxyDevice::create_fake_recv_transport(
    const rust::String transportOptionsJsonStr
) {
  auto transportOptions = nlohmann::json::parse(std::string(transportOptionsJsonStr));
  std::cout << transportOptions["sctpParameters"].dump(4) << std::endl;
  this->recvTransport = this->device.CreateRecvTransport(
    this,
    transportOptions["id"],
    transportOptions["iceParameters"],
    transportOptions["iceCandidates"],
    transportOptions["dtlsParameters"],
    transportOptions["sctpParameters"]
  );
}

std::future<void> ProxyDevice::OnConnect(mediasoupclient::Transport* transport, const nlohmann::json& dtlsParameters)
{
  std::cout << "ON CONNECT CALLBACK OR WHATEVVVER" << std::endl;
  std::cout << dtlsParameters.dump(4) << std::endl;
  std::packaged_task<void()> task([]{ 
      std::cout << "on connect!" << std::endl;
      return; 
  }); // wrap the function
  return task.get_future();
}

std::future<void> ProxyDevice::OnConnectRecvTransport(const nlohmann::json& dtlsParameters)
{
    std::cout << "ON CONNECT RECV CALLBACK OR WHATEVVVER" << std::endl;
  std::cout << dtlsParameters.dump(4) << std::endl;
  std::packaged_task<void()> task([]{ 
      std::cout << "on connect recv!" << std::endl;
      return; 
  }); // wrap the function
  return task.get_future();
}

std::future<void> ProxyDevice::OnConnectSendTransport(const nlohmann::json& dtlsParameters)
{
    std::cout << "ON CONNECT SEND CALLBACK OR WHATEVVVER" << std::endl;
  std::cout << dtlsParameters.dump(4) << std::endl;
  std::packaged_task<void()> task([]{ 
      std::cout << "on connect recv!" << std::endl;
      return; 
  }); // wrap the function
  return task.get_future();
}

void ProxyDevice::create_data_consumer(
    const rust::String consumerId,
    const rust::String producerId,
    const rust::String label,
    const rust::String protocol,
    const rust::String appData
) {
  this->dataConsumer = this->recvTransport->ConsumeData(
      this, 
      std::string(consumerId),
      std::string(producerId), 
      std::string(label),
      std::string(protocol),
      nlohmann::json::parse(std::string(appData)));
}

/*
 * Transport::Listener::OnConnectionStateChange.
 */
void ProxyDevice::OnConnectionStateChange(
  mediasoupclient::Transport* /*transport*/, const std::string& connectionState)
{
  std::cout << "Connection state changed to " << connectionState << std::endl;
}

/* Producer::Listener::OnProduce
 *
 * Fired when a producer needs to be created in mediasoup.
 * Retrieve the remote producer ID and feed the caller with it.
 */
std::future<std::string> ProxyDevice::OnProduce(
  mediasoupclient::SendTransport* /*transport*/,
  const std::string& kind,
  nlohmann::json rtpParameters,
  const nlohmann::json& /*appData*/)
{
  std::packaged_task<std::string()> task([]{ 
      std::cout << "onProduce" << std::endl;
      return std::string(); 
  }); // wrap the function
  std::future<std::string> fut = task.get_future();
  return fut;
}

/* Producer::Listener::OnProduceData
 *
 * Fired when a data producer needs to be created in mediasoup.
 * Retrieve the remote producer ID and feed the caller with it.
 */
std::future<std::string> ProxyDevice::OnProduceData(
  mediasoupclient::SendTransport* /*transport*/,
  const nlohmann::json& sctpStreamParameters,
  const std::string& label,
  const std::string& protocol,
  const nlohmann::json& /*appData*/)
{
  std::packaged_task<std::string()> task([]{ 
      std::cout << "onProduceData" << std::endl;
      return std::string(); 
      }); // wrap the function
  return task.get_future();
}

void ProxyDevice::OnTransportClose(mediasoupclient::Producer* /*producer*/)
{
	std::cout << "[INFO] Broadcaster::OnTransportClose()" << std::endl;
}

void ProxyDevice::OnTransportClose(mediasoupclient::DataProducer* /*dataProducer*/)
{
	std::cout << "[INFO] Broadcaster::OnTransportClose()" << std::endl;
}

void ProxyDevice::OnMessage(mediasoupclient::DataConsumer* dataConsumer, const webrtc::DataBuffer& buffer)
{
	std::cout << "[INFO] Broadcaster::OnMessage()" << std::endl;
	if (dataConsumer->GetLabel() == "chat")
	{
		std::string s = std::string(buffer.data.data<char>(), buffer.data.size());
		std::cout << "[INFO] received chat data: " + s << std::endl;
	}
}

void ProxyDevice::OnOpen(mediasoupclient::DataProducer* /*dataProducer*/)
{
	std::cout << "[INFO] ProxyDevice::OnOpen()" << std::endl;
}
void ProxyDevice::OnClose(mediasoupclient::DataProducer* /*dataProducer*/)
{
	std::cout << "[INFO] ProxyDevice::OnClose()" << std::endl;
}
void ProxyDevice::OnBufferedAmountChange(mediasoupclient::DataProducer* /*dataProducer*/, uint64_t /*size*/)
{
	std::cout << "[INFO] ProxyDevice::OnBufferedAmountChange()" << std::endl;
}



void ProxyDevice::load_capabilities_from_string(rust::String capabilities) {
  device.Load(nlohmann::json::parse(std::string(capabilities)));
}

bool ProxyDevice::is_loaded() const {
  return this->device.IsLoaded();
}

std::unique_ptr<ProxyDevice> new_mediasoup_device() {
    return std::unique_ptr<ProxyDevice>(new ProxyDevice());
}
