#[cxx::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("src/lib.h");
        pub fn print_mediasoup_version();
        pub fn debug() -> Result<()>;
        pub fn initialize();

        type ProxyDevice;

        fn new_mediasoup_device() -> UniquePtr<ProxyDevice>;
        fn is_loaded(&self) -> bool;
        fn get_recv_rtp_capabilities(&self) -> Result<String>;
        fn load_capabilities_from_string(
            self: Pin<&mut ProxyDevice>,
            capabilities: String,
        ) -> Result<()>;

        fn create_fake_recv_transport(self: Pin<&mut ProxyDevice>, transport_options_str: String);

        fn create_data_consumer(
            self: Pin<&mut ProxyDevice>,
            id: String,
            producer_id: String,
            label: String,
            protocol: String,
            app_data: String,
        );
    }
}

pub use cxx::{Exception, UniquePtr};

// TODO(haze): Vet this
unsafe impl Send for ffi::ProxyDevice {}
unsafe impl Sync for ffi::ProxyDevice {}

#[cfg(test)]
mod test {
    use super::ffi;

    #[test]
    fn it_works() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let capabilities_str =
            std::fs::read_to_string("/Users/haze/work/mediasoup_client/capabilities.json")?;
        println!("Using media soup version:");
        ffi::print_mediasoup_version();
        let mut device = ffi::new_mediasoup_device();
        device
            .pin_mut()
            .load_capabilities_from_string(capabilities_str.clone())?;
        let capabilities = device.get_recv_rtp_capabilities()?;
        dbg!(device.is_loaded());
        println!("{}", capabilities);
        Ok(())
    }
}
