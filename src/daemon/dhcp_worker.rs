// Copyright 2020 Red Hat, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::dhcp_manager::MozimDhcpCmd;
use dhcpc::dhcp_open_raw_socket;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

#[derive(Debug)]
pub(crate) enum MozimDhcpWorkCmd {
    Restart,
    Stop,
}

pub(crate) struct MozimDhcpWorker {
    iface_name: String,
    sender: SyncSender<MozimDhcpCmd>,
    recver: Receiver<MozimDhcpWorkCmd>,
}

impl MozimDhcpWorker {
    pub(crate) fn run(
        iface_name: String,
        to_manager_sender: SyncSender<MozimDhcpCmd>,
        from_manager_recver: Receiver<MozimDhcpWorkCmd>,
    ) {
        let dhcp_socket = match dhcp_open_raw_socket(&iface_name) {
            Err(e) => {
                // TODO: update status to DHCP manager
                eprintln!("Error on open dhcp raw socket: {:?}", e);
                return;
            }
            Ok(s) => s,
        };
        dhcp_socket.try_recv();
        loop {
            // Use a way to select two
            match from_manager_recver.recv() {
                Ok(cmd) => match cmd {
                    MozimDhcpWorkCmd::Stop => {
                        break;
                    }
                    MozimDhcpWorkCmd::Restart => {
                        todo!("restart dhcp");
                    }
                },
                Err(e) => {
                    eprintln!(
                        "DHCP worker {}: Failed to recieve \
                        command from DHCP manager: {} ",
                        &iface_name, e
                    );
                    break;
                }
            }
        }
    }
}
