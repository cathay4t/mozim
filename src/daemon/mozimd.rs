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

mod threads_manager;
mod thread_manager;
mod dhcp_manager;
mod dhcp_worker;

use crate::threads_manager::MozimThreadsManager;
use mozim::{
    ipc_bind, ipc_recv, ipc_send, DhcpStatus, ErrorKind, MozimError,
    MozimResult,
};
use serde_json;
use std::convert::TryFrom;
use std::convert::TryInto;
use tokio::net::UnixStream;

#[derive(Debug, Clone)]
enum MozimAction {
    Ping,
    Start,
    Query,
    Stop,
    Dump,
}

impl std::fmt::Display for MozimAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl TryFrom<&str> for MozimAction {
    type Error = MozimError;
    fn try_from(s: &str) -> Result<MozimAction, MozimError> {
        match s {
            "ping" => Ok(MozimAction::Ping),
            "start" => Ok(MozimAction::Start),
            "query" => Ok(MozimAction::Query),
            "stop" => Ok(MozimAction::Stop),
            "dump" => Ok(MozimAction::Dump),
            _ => Err(MozimError {
                kind: ErrorKind::InvalidIpcCommand,
                msg: format!("Invalid command '{}'", s),
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct MozimCmd {
    action: MozimAction,
    arguments: String,
}

impl std::fmt::Display for MozimCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.action, self.arguments)
    }
}

#[tokio::main]
async fn main() {
    let listener = ipc_bind().unwrap();
    let mut threads_mgr = MozimThreadsManager::new();

    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                process_socket_connection(&mut threads_mgr, &mut stream).await;
                if let Err(e) = stream.shutdown(std::net::Shutdown::Both) {
                    eprintln!("Faield to shutdown stream {}", e);
                }
            }
            Err(e) => eprintln!("IPC error {}", e),
        }
    }
}

async fn process_socket_connection(
    threads_mgr: &mut MozimThreadsManager,
    stream: &mut UnixStream,
) {
    match ipc_recv(stream).await {
        Ok(cmd_str) => match parse_ipc_cmd(&cmd_str) {
            Ok(cmd) => process_cmd(&cmd, threads_mgr, stream).await,
            Err(e) => reply_ipc_cmd(stream, Err(e)).await,
        },
        Err(e) => eprintln!("IPC error {}", e),
    }
}

fn parse_ipc_cmd(cmd_str: &str) -> Result<MozimCmd, MozimError> {
    let v: Vec<&str> = cmd_str.splitn(2, " ").collect();
    if v.len() == 1 {
        Ok(MozimCmd {
            action: v[0].try_into()?,
            arguments: "".into(),
        })
    } else if v.len() > 1 {
        Ok(MozimCmd {
            action: v[0].try_into()?,
            arguments: v[1].into(),
        })
    } else {
        Err(MozimError {
            kind: ErrorKind::InvalidIpcCommand,
            msg: format!("Invalid command '{}'", cmd_str),
        })
    }
}

async fn process_cmd(
    cmd: &MozimCmd,
    threads_mgr: &mut MozimThreadsManager,
    stream: &mut UnixStream,
) {
    let result = match cmd.action {
        MozimAction::Ping => process_cmd_ping(),
        MozimAction::Start => process_cmd_start(cmd, threads_mgr),
        MozimAction::Query => process_cmd_query(cmd, threads_mgr),
        MozimAction::Stop => process_cmd_stop(cmd, threads_mgr),
        MozimAction::Dump => process_cmd_dump(threads_mgr),
    };
    reply_ipc_cmd(stream, result).await;
}

fn process_cmd_ping() -> Result<MozimResult, MozimError> {
    Ok(MozimResult::data("pong".to_string()))
}

fn process_cmd_start(
    cmd: &MozimCmd,
    threads_mgr: &mut MozimThreadsManager,
) -> Result<MozimResult, MozimError> {
    let iface_name = &cmd.arguments;
    if iface_name.len() == 0 {
        Err(MozimError {
            kind: ErrorKind::InvalidIpcCommand,
            msg: format!("start command missing interface name"),
        })
    } else {
        dhcp_status_to_mozim_result(&threads_mgr.start_dhcp(iface_name)?)
    }
}

fn process_cmd_query(
    cmd: &MozimCmd,
    threads_mgr: &mut MozimThreadsManager,
) -> Result<MozimResult, MozimError> {
    let iface_name = &cmd.arguments;
    dhcp_status_to_mozim_result(&threads_mgr.query_dhcp(iface_name)?)
}

fn process_cmd_stop(
    cmd: &MozimCmd,
    threads_mgr: &mut MozimThreadsManager,
) -> Result<MozimResult, MozimError> {
    let iface_name = &cmd.arguments;
    dhcp_status_to_mozim_result(&threads_mgr.stop_dhcp(iface_name)?)
}

fn process_cmd_dump(
    threads_mgr: &mut MozimThreadsManager,
) -> Result<MozimResult, MozimError> {
    match serde_json::to_string(&threads_mgr.query_all()?) {
        Ok(s) => Ok(MozimResult::data(s)),
        Err(e) => Err(MozimError::bug(format!(
            "process_cmd_dump(): serde_json::to_string() error: {}",
            e
        ))),
    }
}

async fn reply_ipc_cmd(
    stream: &mut UnixStream,
    result: Result<MozimResult, MozimError>,
) {
    let result = match result {
        Ok(r) => r,
        Err(e) => MozimResult::error(e),
    };
    if let Err(e) = match serde_json::to_string(&result) {
        Ok(result_json) => ipc_send(stream, &result_json).await,
        Err(e) => ipc_send(stream, &format!(r#""Mozim BUG {}""#, e)).await,
    } {
        eprintln!("IPC failure {}", e);
    }
}

fn dhcp_status_to_mozim_result(
    dhcp_status: &DhcpStatus,
) -> Result<MozimResult, MozimError> {
    match serde_json::to_string(dhcp_status) {
        Ok(s) => Ok(MozimResult::data(s)),
        Err(e) => Err(MozimError::bug(format!(
            "process_cmd_query(): serde_json::to_string(): {}",
            e
        ))),
    }
}
