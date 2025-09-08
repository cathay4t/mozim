// SPDX-License-Identifier: Apache-2.0

use crate::{DhcpError, DhcpV4Client, ErrorKind};

impl DhcpV4Client {
    pub(crate) async fn discovery(&mut self) -> Result<(), DhcpError> {
        self.state = DhcpV4State::InitReboot;
        self.lease = None;
        let mut raw_socket = self.get_raw_socket_or_init().await?;
        todo!()
    }
}
