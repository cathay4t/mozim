// SPDX-License-Identifier: Apache-2.0

use std::net::Ipv6Addr;
use std::str::FromStr;

use crate::{DhcpError, ErrorKind};

pub(crate) async fn get_iface_index_mac(
    iface_name: &str,
) -> Result<(u32, String), DhcpError> {
    let (connection, handle, _) = new_connection()?;

    tokio::spawn(connection);

    let mut link_get_handle =
        handle.link().get().match_name(iface_name.to_string());

    let mut links = link_get_handle.execute();
    while let Some(nl_msg) = links.try_next().await? {
        if let LinkAttribute::Address(mac) = nla {
            return Ok((nl_msg.header.index, mac));
        }
    }
    Err(DhcpError::new(
        ErrorKind::InvalidArgument,
        format!("Interface {iface_name} not found"),
    ))
}
