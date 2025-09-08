// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use rand::Rng;

use super::{
    event::DhcpV4Event,
    time::{gen_dhcp_request_delay, gen_renew_rebind_times},
};
use crate::{
    socket::{DhcpRawSocket, DhcpSocket, DhcpUdpSocket},
    DhcpError, DhcpV4Config, DhcpV4Lease, DhcpV4Message, DhcpV4MessageType,
    ErrorKind,
};

// RFC 2131 suggests four times(60 seconds) retry before fallback to
// discovery state
const MAX_REQUEST_RETRY_COUNT: u32 = 4;

const NOT_RETRY: bool = false;
const IS_RETRY: bool = true;

/// DHCPv4 Client State
/// RFC 2131 Table 4: Client messages from different states
#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash, PartialOrd, Ord, Default)]
pub enum DhcpV4State {
    /// DHCP lease acquired, waiting T1/T2 to refresh the lease
    Done,
    /// Sending broadcast DHCPDISCOVER to server and waiting DHCPOFFER
    #[default]
    InitReboot,
    /// Sending broadcast DHCPREQUEST to server and waiting DHCPACK
    Selecting,
    /// T1 expired, sending unicast DHCPREQUEST and waiting DHCPACK
    Renewing,
    /// T2 expired, sending broadcast DHCPREQUEST and waiting DHCPACK
    Rebinding,
    /// Failed on acquiring DHCP, DHCP client session has been terminated.
    /// Please run [DhcpV4Client::reinit()] and [DhcpV4Config::run()] to start
    /// the process again.
    Failed,
}

impl std::fmt::Display for DhcpV4State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Done => "done",
                Self::InitReboot => "init_reboot",
                Self::Selecting => "selecting",
                Self::Renewing => "renewing",
                Self::Rebinding => "rebinding",
                Self::Failed => "failed",
            }
        )
    }
}

/// DHCPv4 Client
/// Example code:
/// ```no_run
/// const TIMEOUT: u32 = 30;
/// let mut config = DhcpV4Config::new(iface_name);
/// config.set_host_name("foo.example.org");
/// config.use_host_name_as_client_id();
/// config.set_timeout_sec(TIMEOUT);
/// let mut cli = DhcpV4Client::init(config, None).unwrap();
///
/// loop {
///     match cli.run() {
///         Ok(lease) => {
///             // apply_dhcp_lease(iface_name, &lease);
///         }
///         Err(_) => {
///             // purge_dhcp_lease(iface_name);
///             break;
///         }
///     }
/// }
/// ```
#[derive(Debug, Default)]
pub struct DhcpV4Client {
    config: DhcpV4Config,
    lease: Option<DhcpV4Lease>,
    pending_lease: Option<DhcpV4Lease>,
    state: DhcpV4State,
    raw_socket: Option<DhcpRawSocket>,
    udp_socket: Option<DhcpUdpSocket>,
    retry_count: u32,
    xid: u32,
    t1_timer: Option<DhcpTimerFd>,
    t2_timer: Option<DhcpTimerFd>,
    lease_timer: Option<DhcpTimerFd>,
    error: Option<DhcpError>,
}

impl DhcpV4Client {
    pub async fn init(
        mut config: DhcpV4Config,
        lease: Option<DhcpV4Lease>,
    ) -> Result<Self, DhcpError> {
        if config.need_resolve() {
            config.resolve_iface_index_and_mac().await?;
        }

        let state = if lease.is_some() {
            DhcpV4State::Selecting
        } else {
            DhcpV4State::InitReboot
        };

        let xid = rand::thread_rng().gen();

        Ok(Self {
            config,
            lease,
            state,
            xid,
            ..Default::default()
        })
    }

    pub fn reinit(&mut self) -> Result<(), DhcpError> {
        *self = Self::init(self.config, None)?;
        Ok(())
    }

    /// Please run this function in a loop so it could refresh the lease with
    /// DHCP server.
    /// Return whenever state change, lease acquired or error(including
    /// timeout).
    pub async fn run(
        &mut self,
        timeout_sec: u32,
    ) -> Result<(DhcpV4State, Option<DhcpV4Lease>), DhcpError> {
        let result = match self.state {
            DhcpV4State::InitReboot => self.discovery().await,
            DhcpV4State::Selecting => self.request().await,
            DhcpV4State::Renewing => self.renew().await,
            DhcpV4State::Rebinding => self.rebind().await,
            DhcpV4State::Done => self.wait_lease_timers().await,
            DhcpV4State::Failed => {
                // Prevent infinite loop when user run this function in
                // loop without error handling
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                self.error.clone().unwrap_or_else(|| {
                    DhcpError::new(
                        ErrorKind::Bug,
                        format!(
                            "DhcpV4State::Failed with empty error: {self:?}"
                        ),
                    )
                })
            }
        };
        if let Err(e) = result {
            self.error = Some(e.clone());
            Err(e)
        } else {
            Ok((self.state, self.lease.clone()))
        }
    }

    async fn get_udp_socket_or_init(
        &mut self,
    ) -> Result<&mut DhcpUdpSocket, DhcpError> {
        todo!()
    }

    async fn get_raw_socket_or_init(
        &mut self,
    ) -> Result<&mut DhcpRawSocket, DhcpError> {
        todo!()
    }

    async fn request(&mut self) -> Result<(), DhcpError> {
        todo!()
    }

    // Unicast DHCPREQUEST to DHCP server
    async fn renew(&mut self) -> Result<(), DhcpError> {
        todo!()
    }

    // Broadcast DHCPREQUEST
    async fn rebind(&mut self) -> Result<(), DhcpError> {
        todo!()
    }

    async fn wait_lease_timers(&mut self) -> Result<(), DhcpError> {
        if let Some(lease_timer) = self.lease_timer.as_ref() {
            if lease_timer.is_expired() {
                self.state = DhcpV4State::InitReboot;
                return Ok(());
            }
        }

        if let Some(t2_timer) = self.t2_timer.as_ref() {
            if t2_timer.is_expired() {
                self.state = DhcpV4State::Rebinding;
                return Ok(());
            }
        }
        if let Some(t1_timer) = self.t1_timer.as_ref() {
            if t1_timer.is_expired() {
                self.state = DhcpV4State::Rebinding;
                return Ok(());
            }
        }

        if let Some(t1_timer) = self.t1_timer.take() {
            t1_timer.wait().await?;
        } else if let Some(t2_timer) = self.t2_timer.take() {
            t2_timer.wait().await?;
        } else if let Some(lease_timer) = self.lease_timer.take() {
            lease_timer.wait().await?;
        } else {
            self.state = DhcpV4State::InitReboot;
            return DhcpV4State::InitReboot;
        }
        self.wait_lease_timers().await
    }
}
