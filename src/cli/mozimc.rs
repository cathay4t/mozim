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

use std::env::args;

use mozim::{ipc_connect, ipc_exec};

#[tokio::main]
async fn main() {
    let argv = args().collect::<Vec<String>>();
    if argv.len() == 1 {
        eprintln!(
            r#"Invalid arugment, please use:
 * mozimc ping
 * mozimc start <iface_name>
 * mozimc stop <iface_name>
 * mozimc query <iface_name>
 * mozimc dump
        "#
        );
        std::process::exit(1);
    }
    let args = argv[1..].join(" ");
    let mut connection = ipc_connect().await.unwrap();
    println!(
        "Got reply {}",
        ipc_exec(&mut connection, &args).await.unwrap(),
    );
}
