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

use crate::uci::uci_packets::{
    CoreOpCode, DeviceResetCmdBuilder, GetCapsInfoCmdBuilder, GetDeviceInfoCmdBuilder,
    GetDeviceInfoCmdPacket, ResetConfig, SetConfigCmdBuilder, SetConfigRspBuilder,
    SetConfigRspPara, StatusCode, UciCommandPacket, TLV,
};

fn uci_ucif_send_cmd() -> StatusCode {
    let resp = uwb_ucif_check_cmd_queue(GetDeviceInfoCmdBuilder {});
    StatusCode::UciStatusOk
}

fn build_device_info_cmd() -> GetDeviceInfoCmdBuilder {
    GetDeviceInfoCmdBuilder {}
}

fn build_caps_info_cmd() -> GetCapsInfoCmdBuilder {
    GetCapsInfoCmdBuilder {}
}

fn set_config_cmd(tlvs: Vec<TLV>) -> SetConfigCmdBuilder {
    SetConfigCmdBuilder { tlvs }
}

fn build_device_reset_cmd(reset_config: ResetConfig) -> DeviceResetCmdBuilder {
    DeviceResetCmdBuilder { reset_config }
}

fn build_set_config_rsp(status: StatusCode, para: Vec<SetConfigRspPara>) -> SetConfigRspBuilder {
    SetConfigRspBuilder { status, para }
}

fn uwb_ucif_check_cmd_queue(p_message: GetDeviceInfoCmdBuilder) -> StatusCode {
    // TODO : Hook to command queue
    return StatusCode::UciStatusOk;
}