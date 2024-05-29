// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::os::fd::{AsRawFd, RawFd};
use std::time::Duration;

use nix::sys::epoll::{
    epoll_create, epoll_ctl, epoll_wait, EpollEvent, EpollFlags, EpollOp,
};

use crate::{time::DhcpTimerFd, DhcpError, ErrorKind};

const EVENT_BUFFER_COUNT: usize = 64;

pub(crate) trait DhcpEvent:
    std::fmt::Display
    + Into<u64>
    + Eq
    + std::hash::Hash
    + TryFrom<u64, Error = DhcpError>
    + Copy
{
}

#[derive(Debug)]
pub(crate) struct DhcpEpoll {
    pub(crate) fd: RawFd,
}

impl AsRawFd for DhcpEpoll {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl DhcpEpoll {
    pub(crate) fn new() -> Result<Self, DhcpError> {
        Ok(Self {
            fd: epoll_create().map_err(|e| {
                let e = DhcpError::new(
                    ErrorKind::Bug,
                    format!("Failed to epoll_create(): {e}"),
                );
                log::error!("{}", e);
                e
            })?,
        })
    }

    pub(crate) fn add_fd<T>(&self, fd: RawFd, event: T) -> Result<(), DhcpError>
    where
        T: DhcpEvent,
    {
        log::debug!("Adding event {} to epoll {}", event, self.fd);
        let event = EpollEvent::new(EpollFlags::EPOLLIN, event.into());
        epoll_ctl(self.fd, EpollOp::EpollCtlAdd, fd, &mut Some(event)).map_err(
            |e| {
                let e = DhcpError::new(
                    ErrorKind::Bug,
                    format!(
                        "Failed to epoll_ctl({}, {:?}, {}, {:?}): {}",
                        self.fd,
                        EpollOp::EpollCtlAdd,
                        fd,
                        event,
                        e
                    ),
                );
                log::error!("{}", e);
                e
            },
        )
    }

    pub(crate) fn del_fd<T>(&self, fd: RawFd, event: T) -> Result<(), DhcpError>
    where
        T: DhcpEvent,
    {
        log::debug!(
            "Removing fd {} from Epoll {}, event {}",
            fd,
            self.fd,
            event
        );
        let event = EpollEvent::new(EpollFlags::EPOLLIN, event.into());
        epoll_ctl(self.fd, EpollOp::EpollCtlDel, fd, &mut Some(event)).map_err(
            |e| {
                let e = DhcpError::new(
                    ErrorKind::Bug,
                    format!(
                        "Failed to epoll_ctl({}, {:?}, {}, {:?}): {}",
                        self.fd,
                        EpollOp::EpollCtlDel,
                        fd,
                        event,
                        e
                    ),
                );
                log::error!("{}", e);
                e
            },
        )
    }

    pub(crate) fn poll<T>(&self, wait_time: isize) -> Result<Vec<T>, DhcpError>
    where
        T: DhcpEvent,
    {
        let mut events: [EpollEvent; EVENT_BUFFER_COUNT] =
            [EpollEvent::empty(); EVENT_BUFFER_COUNT];

        loop {
            match epoll_wait(self.fd, &mut events, 1000 * wait_time) {
                Ok(c) => {
                    let mut ret = Vec::new();
                    for i in &events[..c] {
                        ret.push(T::try_from(i.data())?);
                    }
                    return Ok(ret);
                }
                Err(e) => match e {
                    nix::errno::Errno::EINTR | nix::errno::Errno::EAGAIN => {
                        // retry
                        continue;
                    }
                    _ => {
                        let e = DhcpError::new(
                            ErrorKind::Bug,
                            format!("Failed on epoll_wait(): {e}"),
                        );
                        return Err(e);
                    }
                },
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct DhcpEventPool<T: DhcpEvent> {
    timer_fds: HashMap<T, DhcpTimerFd>,
    socket_fds: HashMap<T, RawFd>,
    pub(crate) epoll: DhcpEpoll,
}

impl<T: DhcpEvent> Drop for DhcpEventPool<T> {
    fn drop(&mut self) {
        self.remove_all_event();
        if self.epoll.fd >= 0 {
            unsafe {
                libc::close(self.epoll.fd as libc::c_int);
            }
        }
    }
}

impl<T: DhcpEvent> DhcpEventPool<T> {
    pub(crate) fn remove_all_event(&mut self) {
        for (event, timer_fd) in self.timer_fds.drain() {
            self.epoll.del_fd(timer_fd.as_raw_fd(), event).ok();
        }
        for (event, fd) in self.socket_fds.drain() {
            self.epoll.del_fd(fd, event).ok();
        }
    }

    pub(crate) fn new() -> Result<Self, DhcpError> {
        Ok(Self {
            timer_fds: HashMap::new(),
            socket_fds: HashMap::new(),
            epoll: DhcpEpoll::new()?,
        })
    }

    pub(crate) fn add_socket(
        &mut self,
        fd: RawFd,
        event: T,
    ) -> Result<(), DhcpError> {
        log::debug!("Adding socket {} with event {} to event pool", fd, event);
        self.socket_fds.insert(event, fd);
        self.epoll.add_fd(fd, event)
    }

    pub(crate) fn del_socket(&mut self, event: T) -> Result<(), DhcpError> {
        if let Some(fd) = self.socket_fds.remove(&event) {
            self.epoll.del_fd(fd, event)?;
        }
        Ok(())
    }

    pub(crate) fn add_timer(
        &mut self,
        timeout: Duration,
        event: T,
    ) -> Result<(), DhcpError> {
        log::debug!(
            "Adding timer {} milliseconds with event {} to event pool",
            timeout.as_millis(),
            event
        );
        let timer_fd = DhcpTimerFd::new(timeout)?;
        self.epoll.add_fd(timer_fd.as_raw_fd(), event)?;
        self.timer_fds.insert(event, timer_fd);
        Ok(())
    }

    pub(crate) fn del_timer(&mut self, event: T) -> Result<(), DhcpError> {
        if let Some(timer_fd) = self.timer_fds.remove(&event) {
            self.epoll.del_fd(timer_fd.as_raw_fd(), event)?;
        }
        Ok(())
    }

    pub(crate) fn poll(&self, wait_time: u32) -> Result<Vec<T>, DhcpError> {
        match isize::try_from(wait_time) {
            Ok(i) => self.epoll.poll(i),
            Err(_) => Err(DhcpError::new(
                ErrorKind::InvalidArgument,
                format!(
                    "Invalid timeout, should be in the range of 0 - {}",
                    isize::MAX
                ),
            )),
        }
    }
}
