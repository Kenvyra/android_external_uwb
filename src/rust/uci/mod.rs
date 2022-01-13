/*
 * Copyright (C) 2021 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

pub mod state_machine;
pub mod uci_hmsgs;
pub mod uci_hrcv;

use crate::adaptation::UwbAdaptation;
use crate::error::UwbErr;
use crate::uci::uci_hrcv::UciResponse;
use log::{error, info};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{mpsc, oneshot};
use tokio::{select, task};
use uwb_uci_packets::Packet;

pub type Result<T> = std::result::Result<T, UwbErr>;
pub type UciResponseHandle = oneshot::Sender<UciResponse>;

// TODO: Use real values for these enums.

// Commands sent from JNI.
#[derive(Debug)]
pub enum JNICommand {
    UwaEnable,
    UwaDisable(bool),
    UwaSessionDeinit(u32),
    UwaSessionGetCount,
    UwaStartRange(u32),
    UwaStopRange(u32),
    UwaGetSessionState(u32),
    UwaSessionUpdateMulticastList {
        session_id: u32,
        action: u8,
        no_of_controlee: u8,
        address_list: Vec<u8>,
        sub_session_id_list: Vec<i32>,
    },
    UwaSetCountryCode {
        code: Vec<u8>,
    },
    Exit,
}

// Commands sent from JNI, which blocks until it gets a response.
#[derive(Debug)]
pub enum BlockingJNICommand {
    GetDeviceInfo,
    UwaSessionInit(u32, u8),
}

// Responses from the HAL.
#[derive(Debug)]
pub enum HALResponse {
    // TODO: Can we combine HALResponse and UciResponse and "inline" this?
    Uci(uci_hrcv::UciResponse),
    A,
    B,
}

// Commands we send to the HAL.
#[derive(Debug)]
enum HALCommand {
    A,
    B,
}

struct Driver {
    adaptation: UwbAdaptation,
    cmd_receiver: mpsc::UnboundedReceiver<JNICommand>,
    blocking_cmd_receiver: mpsc::UnboundedReceiver<(BlockingJNICommand, UciResponseHandle)>,
    rsp_receiver: mpsc::UnboundedReceiver<HALResponse>,
    response_channel: Option<UciResponseHandle>,
}

// Creates a future that handles messages from JNI and the HAL.
async fn drive(
    adaptation: UwbAdaptation,
    cmd_receiver: mpsc::UnboundedReceiver<JNICommand>,
    blocking_cmd_receiver: mpsc::UnboundedReceiver<(BlockingJNICommand, UciResponseHandle)>,
    rsp_receiver: mpsc::UnboundedReceiver<HALResponse>,
) -> Result<()> {
    Driver::new(adaptation, cmd_receiver, blocking_cmd_receiver, rsp_receiver).drive().await
}

impl Driver {
    fn new(
        adaptation: UwbAdaptation,
        cmd_receiver: mpsc::UnboundedReceiver<JNICommand>,
        blocking_cmd_receiver: mpsc::UnboundedReceiver<(BlockingJNICommand, UciResponseHandle)>,
        rsp_receiver: mpsc::UnboundedReceiver<HALResponse>,
    ) -> Self {
        Self {
            adaptation,
            cmd_receiver,
            blocking_cmd_receiver,
            rsp_receiver,
            response_channel: None,
        }
    }

    // Continually handles messages.
    async fn drive(mut self) -> Result<()> {
        loop {
            self.drive_once().await?
        }
    }

    // Handles a single message from JNI or the HAL.
    async fn drive_once(&mut self) -> Result<()> {
        // TODO: Handle messages for real instead of just logging them.
        select! {
            Some(cmd) = self.cmd_receiver.recv() => {
                match cmd {
                    JNICommand::UwaEnable => {
                        log::info!("{:?}", cmd);
                        self.adaptation.initialize();
                        self.adaptation.hal_open();
                        self.adaptation.core_initialization()?;
                    },
                    JNICommand::UwaDisable(graceful) => log::info!("{:?}", cmd),
                    JNICommand::UwaSessionDeinit(session_id) => log::info!("{:?}", cmd),
                    JNICommand::UwaSessionGetCount => log::info!("{:?}", cmd),
                    JNICommand::UwaStartRange(session_id) => log::info!("{:?}", cmd),
                    JNICommand::UwaStopRange(session_id) => log::info!("{:?}", cmd),
                    JNICommand::UwaGetSessionState(session_id) => log::info!("{:?}", cmd),
                    JNICommand::UwaSessionUpdateMulticastList{session_id, action, no_of_controlee, ref address_list, ref sub_session_id_list} => log::info!("{:?}", cmd),
                    JNICommand::UwaSetCountryCode{ref code} => log::info!("{:?}", cmd),
                    JNICommand::Exit => return Err(UwbErr::Exit),
                }
            }
            Some((cmd, tx)) = self.blocking_cmd_receiver.recv(), if self.response_channel.is_none() => {
                // TODO: If we do something similar to communication to the HAL (using a channel
                // to hide the asynchrony, we can remove the field and make this straightline code.
                self.response_channel = Some(tx);
                match cmd {
                    BlockingJNICommand::GetDeviceInfo => {
                        log::info!("BlockingJNICommand::GetDeviceInfo");
                        let bytes = uci_hmsgs::build_device_info_cmd().build().to_vec();
                        self.adaptation.send_uci_message(&bytes);
                    },
                    BlockingJNICommand::UwaSessionInit(session_id, session_type) => {
                        log::info!("{:?}", cmd);
                        let bytes = uci_hmsgs::build_session_init_cmd(session_id, session_type).build().to_vec();
                        self.adaptation.send_uci_message(&bytes);
                    }
                }
            }
            Some(rsp) = self.rsp_receiver.recv() => {
                match rsp {
                    HALResponse::A => log::error!("HALResponse::A"),
                    HALResponse::B => log::error!("HALResponse::B"),
                    HALResponse::Uci(response) => {
                        self.response_channel.take().expect("the response channel does not exist").send(response);
                    },
                }
            }
        }
        Ok(())
    }
}

// Controller for sending tasks for the native thread to handle.
pub struct Dispatcher {
    cmd_sender: mpsc::UnboundedSender<JNICommand>,
    blocking_cmd_sender: mpsc::UnboundedSender<(BlockingJNICommand, UciResponseHandle)>,
    join_handle: task::JoinHandle<Result<()>>,
    runtime: Runtime,
}

impl Dispatcher {
    pub fn new() -> Result<Dispatcher> {
        info!("initializing dispatcher");
        let (cmd_sender, cmd_receiver) = mpsc::unbounded_channel::<JNICommand>();
        let (blocking_cmd_sender, blocking_cmd_receiver) =
            mpsc::unbounded_channel::<(BlockingJNICommand, UciResponseHandle)>();
        let (rsp_sender, rsp_receiver) = mpsc::unbounded_channel::<HALResponse>();
        let adaptation = UwbAdaptation::new(None, rsp_sender);
        // We create a new thread here both to avoid reusing the Java service thread and because
        // binder threads will call into this.
        let runtime =
            Builder::new_multi_thread().worker_threads(1).thread_name("uwb-uci-handler").build()?;
        let join_handle =
            runtime.spawn(drive(adaptation, cmd_receiver, blocking_cmd_receiver, rsp_receiver));
        Ok(Dispatcher { cmd_sender, blocking_cmd_sender, join_handle, runtime })
    }

    pub fn send_jni_command(&self, cmd: JNICommand) -> Result<()> {
        self.cmd_sender.send(cmd)?;
        Ok(())
    }

    // TODO: Consider implementing these separate for different commands so we can have more
    // specific return types.
    pub fn block_on_jni_command(&self, cmd: BlockingJNICommand) -> Result<UciResponse> {
        let (tx, rx) = oneshot::channel();
        self.blocking_cmd_sender.send((cmd, tx))?;
        Ok(self.runtime.block_on(rx)?)
    }

    fn exit(&mut self) -> Result<()> {
        self.send_jni_command(JNICommand::Exit)?;
        let _ = self.runtime.block_on(&mut self.join_handle);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver() -> Result<()> {
        // TODO: Remove this once we call it somewhere real.
        logger::init(
            logger::Config::default().with_tag_on_device("uwb").with_min_level(log::Level::Error),
        );
        let mut dispatcher = Dispatcher::new()?;
        dispatcher.send_jni_command(JNICommand::UwaEnable)?;
        dispatcher.exit()?;
        Ok(())
    }
}
