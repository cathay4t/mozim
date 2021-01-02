use crate::DhcpError;
use rand;
use rand::Rng;
use std::net::Ipv4Addr;

const MINIMUM_OPTION_LENGTH: usize = 312;

// RFC 2131
const BOOTREQEST: u8 = 1;
const BOOTREPLY: u8 = 2;
const CHADDR_LEN: usize = 16;
const SNAME_LEN: usize = 64;
const FILE_LEN: usize = 128;

// https://www.iana.org/assignments/arp-parameters/arp-parameters.xhtml
const HW_TYPE_ETHERNET: u8 = 1;

const ETHERNET_HW_ADDR_LEN: u8 = 6;

#[derive(Debug, PartialEq, Clone)]
pub struct DhcpMessage {
    pub op: u8,
    pub htype: u8,
    pub hlen: u8,
    pub hops: u8,
    pub xid: u32,
    pub secs: u16,
    pub flags: u16, // Using of BROADCAST bit is discouraged by RFC 1542
    pub ciaddr: Ipv4Addr,
    pub yiaddr: Ipv4Addr,
    pub siaddr: Ipv4Addr,
    pub giaddr: Ipv4Addr,
    pub chaddr: [u8; CHADDR_LEN],
    pub sname: [u8; SNAME_LEN],
    pub file: [u8; 128],
    pub options: Vec<u8>,
}

impl DhcpMessage {
    pub fn to_u8_vec(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(self.op);
        data.push(self.htype);
        data.push(self.hlen);
        data.push(self.hops);
        data.extend_from_slice(&self.xid.to_be_bytes());
        data.extend_from_slice(&self.secs.to_be_bytes());
        data.extend_from_slice(&self.flags.to_be_bytes());
        data.extend_from_slice(&self.ciaddr.octets());
        data.extend_from_slice(&self.yiaddr.octets());
        data.extend_from_slice(&self.siaddr.octets());
        data.extend_from_slice(&self.giaddr.octets());
        data.extend_from_slice(&self.chaddr);
        data.extend_from_slice(&self.sname);
        data.extend_from_slice(&self.file);
        if self.options.len() < MINIMUM_OPTION_LENGTH {
            let mut options_data = vec![0; MINIMUM_OPTION_LENGTH];
            options_data[..self.options.len()].clone_from_slice(&self.options);
            data.extend_from_slice(&options_data);
        } else {
            data.extend_from_slice(&self.options);
        }
        // BUG: need network package padding
        data
    }

    pub fn new_discover_message(
        hw_addr: &[u8; CHADDR_LEN],
        host_name: String,
    ) -> Result<Self, DhcpError> {
        let mut rng = rand::thread_rng();
        let mut host_name_raw = [0u8; SNAME_LEN];
        if host_name.as_bytes().len() >= SNAME_LEN {
            return Err(DhcpError::invalid_argument(format!(
                "Specified host_name '{}' exceeded the maximum length {}",
                host_name,
                SNAME_LEN - 1
            )));
        }
        host_name_raw[..host_name.as_bytes().len()]
            .clone_from_slice(host_name.as_bytes());

        Ok(Self {
            op: BOOTREQEST,
            htype: HW_TYPE_ETHERNET,
            hlen: ETHERNET_HW_ADDR_LEN,
            hops: 0,
            xid: rng.gen(),
            secs: 0,
            flags: 0,
            ciaddr: Ipv4Addr::UNSPECIFIED,
            yiaddr: Ipv4Addr::UNSPECIFIED,
            siaddr: Ipv4Addr::UNSPECIFIED,
            giaddr: Ipv4Addr::UNSPECIFIED,
            chaddr: hw_addr.clone(),
            sname: host_name_raw,
            file: [0; FILE_LEN],
            options: Vec::new(),
        })
    }
}
