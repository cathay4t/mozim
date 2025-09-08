// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use nix::sys::time::TimeSpec;
use nix::sys::timerfd::{
    ClockId::CLOCK_BOOTTIME, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags,
};
use nix::time::clock_gettime;
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, ReadBuf};

use crate::{DhcpError, ErrorKind};

#[derive(Debug)]
pub(crate) struct DhcpTimer {
    pub(crate) end: TimeSpec,
    pub(crate) fd: AsyncFd<TimerFd>,
}

impl DhcpTimer {
    pub(crate) fn new(time: Duration) -> Result<Self, DhcpError> {
        let fd = TimerFd::new(CLOCK_BOOTTIME, TimerFlags::TFD_NONBLOCK)
            .map_err(|e| {
                let e = DhcpError::new(
                    ErrorKind::Bug,
                    format!("Failed to create timerfd {e}"),
                );
                log::error!("{e}");
                e
            })?;

        let end = boot_time_now()? + TimeSpec::from_duration(time);

        fd.set(
            Expiration::OneShot(TimeSpec::from_duration(time)),
            TimerSetTimeFlags::empty(),
        )
        .map_err(|e| {
            let e = DhcpError::new(
                ErrorKind::Bug,
                format!("Failed to set timerfd {e}"),
            );
            log::error!("{e}");
            e
        })?;
        log::debug!(
            "TimerFd created {:?} with {} milliseconds",
            fd,
            time.as_millis()
        );
        Ok(Self {
            end,
            fd: AsyncFd::new(fd)?,
        })
    }

    pub(crate) async fn wait(&self) -> Result<(), DhcpError> {
        if self.end > boot_time_now()? {
            self.fd.readable().await?;
        }
        Ok(())
    }

    pub(crate) fn is_expired(&self) -> Result<bool, DhcpError> {
        self.end <= boot_time_now()?
    }
}

fn boot_time_now() -> Result<TimeSpec, DhcpError> {
    clock_gettime(CLOCK_BOOTTIME).map_err(|e| {
        DhcpError::new(
            ErrorKind::Bug,
            format!("Failed to retrieve CLOCK_BOOTTIME: {e}"),
        )
    })
}
