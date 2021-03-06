extern crate crc;
extern crate time;

use self::crc::{crc32, Hasher32};
use Communication::Channel::Channel;

#[derive(Debug)]
enum Status {
    on,
    off,
}

#[derive(Debug, PartialEq)]
pub enum device_error {
    could_not_setup_devices,
    could_not_send_setup_command,
    could_not_unserialize_device_packet,
    could_not_recieve_device_packet,
}

#[derive(Debug, PartialEq)]
enum checksum_error {
    mismatch,
}

#[derive(Debug, PartialEq)]
enum data_recieve_error {
    protocol_error,
    could_not_recieve_data,
    possible_corrupted_data,
    empty_buffer,
    mismatch_checksum,
}

#[derive(Debug)]
pub struct Device {
    device_id: u8,
    status: Status,
}

pub fn set_up_devices<'a>(com_channel: &'a Channel) -> Result<Vec<Device>, device_error> {
    match com_channel
        .write_udp_socket
        .connect("255.255.255.255:62344")
    {
        Ok(result) => {
            let current_time = time::now();
            let set_up_command = set_up_command_broadcast();
            match com_channel.write_udp_socket.send(&set_up_command) {
                Ok(_) => retrieve_devices(com_channel),
                Err(_) => Err(device_error::could_not_setup_devices),
            }
        }
        Err(_) => Err(device_error::could_not_send_setup_command),
    }
}

fn retrieve_devices<'a>(com_channel: &'a Channel) -> Result<Vec<Device>, device_error> {
    let mut devices: Vec<Device> = vec![];

    let mut buffer = [0; 256];
    //udp_socket.connect("0.0.0.0:56000").expect("Could not bind to 62344");
    match com_channel.read_udp_socket.recv_from(&mut buffer) {
        Ok(success) => match get_device_from_bytes(&buffer) {
            Ok(device_from_buffer) => {
                devices.push(device_from_buffer);
                match retrieve_devices(com_channel) {
                    Ok(recursive_device) => {
                        devices.extend(recursive_device);
                        Ok(devices)
                    }
                    Err(_) => Ok(devices),
                }
            }
            Err(e) => Err(device_error::could_not_unserialize_device_packet),
        },
        Err(e) => Err(device_error::could_not_recieve_device_packet),
    }
}

fn get_device_from_bytes(buffer: &[u8]) -> Result<Device, data_recieve_error> {
    match buffer.first() {
        Some(first) => {
            if (*first == 165u8) {
                match validate_checksum(buffer) {
                    Ok(validated_buffer) => match create_buffer_from_device(buffer) {
                        Some(result) => Ok(result),
                        None => Err(data_recieve_error::possible_corrupted_data),
                    },
                    Err(_) => Err(data_recieve_error::mismatch_checksum),
                }
            } else {
                Err(data_recieve_error::protocol_error)
            }
        }
        None => Err(data_recieve_error::empty_buffer),
    }
}

fn set_up_command_broadcast() -> [u8; 11] {
    let current_time = time::now();
    let command_without_checksum = [
        165,
        5,
        0,
        0,
        current_time.tm_hour as u8,
        current_time.tm_min as u8,
        current_time.tm_sec as u8,
    ];
    let checksum = get_checksum(&command_without_checksum);
    [
        165,
        9,
        0,
        0,
        current_time.tm_hour as u8,
        current_time.tm_min as u8,
        current_time.tm_sec as u8,
        (checksum >> 24) as u8,
        (checksum >> 16) as u8,
        (checksum >> 8) as u8,
        (checksum | 0x00ff) as u8,
    ]
}

fn get_checksum(bytes: &[u8]) -> u32 {
    crc32::checksum_ieee(bytes)
}

fn validate_checksum(buffer: &[u8]) -> Result<(), checksum_error> {
    if (buffer.len() > 4) {
        let buffer_without_trailing_zeros = buffer.split_at((buffer[1] + 2) as usize).0;
        let buffer_without_checksum =
            buffer_without_trailing_zeros.split_at(buffer_without_trailing_zeros.len() - 4);
        let calculated_read_buffer_checksum = get_checksum(buffer_without_checksum.0);
        let checksum = ((buffer_without_checksum.1[0] as u32) << 24)
            | ((buffer_without_checksum.1[1] as u32) << 16)
            | ((buffer_without_checksum.1[2] as u32) << 8)
            | ((buffer_without_checksum.1[3] as u32));

        if (checksum == calculated_read_buffer_checksum) {
            Ok(())
        } else {
            Err(checksum_error::mismatch)
        }
    } else {
        Err(checksum_error::mismatch)
    }
}

fn create_buffer_from_device(buffer: &[u8]) -> Option<Device> {
    if (buffer.len() > 2) {
        Some(Device {
            device_id: buffer[2],
            status: Status::on,
        })
    } else {
        None
    }
}

#[test]
fn test_validate_checksum() {
    assert_eq!(
        validate_checksum(&[165, 9, 15, 1, 0, 0, 0, 0x95, 0x1c, 0x82, 0xcb]),
        Ok(())
    );
    assert_eq!(
        validate_checksum(&[165, 9, 15, 1, 0, 0, 0, 0x95, 0x1c, 0x82, 0xcb, 0, 0, 0, 0, 23]),
        Ok(())
    );
    assert_eq!(
        validate_checksum(&[165, 9, 19, 1, 0, 0, 0, 0x95, 0x1C, 0x82, 0xCB])
            .expect_err("Matching checksum"),
        checksum_error::mismatch
    );
}

#[test]
fn test_get_device_from_bytes() {
    assert_eq!(
        get_device_from_bytes(&[]).expect_err("should expect protocol error"),
        data_recieve_error::empty_buffer
    );
    assert_eq!(
        get_device_from_bytes(&[00, 9, 15, 1, 0, 0, 0, 0x95, 0x1C, 0x82, 0xCB])
            .expect_err("should expect protocol error"),
        data_recieve_error::protocol_error
    );
    assert_eq!(
        get_device_from_bytes(&[165, 9, 15, 1, 0, 0, 0, 0, 0, 0, 0])
            .expect_err("expected corrupted data"),
        data_recieve_error::mismatch_checksum
    );
    assert_eq!(
        get_device_from_bytes(&[165, 9, 15, 1, 0, 0, 0, 0x95, 0x1C, 0x82, 0xCB])
            .expect("expected device")
            .device_id,
        15
    );
}
