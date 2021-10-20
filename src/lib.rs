
pub mod types;
pub mod mlst;

#[cfg(any(feature = "async", feature = "async-secure"))]
mod async_client;
#[cfg(any(test, not(any(feature = "async", feature = "async-secure"))))]
mod sync_client;


// -- export async
#[cfg(any(feature = "async", feature = "async-secure"))]
pub use async_client::FtpClient;
// -- export sync
#[cfg(not(any(feature = "async", feature = "async-secure")))]
pub use sync_client::FtpClient;
