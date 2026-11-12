//! A small DNS server for the home: answers the one name that sends Steam to the box's
//! cache (`lancache.steamcontent.com`) with the box's own address, and passes every other
//! question, unchanged, to an ordinary resolver.
//!
//! It is off unless the box is started with --dns. Point the router's DNS (or a PC's) at
//! the box and every Steam client in the home downloads through the cache with nothing to
//! set up in Steam itself.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use tokio::net::UdpSocket;

/// Where other questions go when CASTLE_DNS_UPSTREAM does not say.
pub const DEFAULT_UPSTREAM: &str = "1.1.1.1:53";
const ANSWER_TTL: u32 = 60;
const UPSTREAM_WAIT: Duration = Duration::from_secs(4);
const MAX_PACKET: usize = 4096;
const TYPE_A: u16 = 1;
const TYPE_AAAA: u16 = 28;

/// The name a query asks about and its type, or None when it is not a plain query.
pub fn question(packet: &[u8]) -> Option<(String, u16, usize)> {
    if packet.len() < 12 || packet[2] & 0x80 != 0 || u16::from_be_bytes([packet[4], packet[5]]) != 1 {
        return None;
    }
    let mut at = 12;
    let mut labels = Vec::new();
    loop {
        let len = *packet.get(at)? as usize;
        at += 1;
        if len == 0 {
            break;
        }
        if len > 63 {
            return None; // a pointer or something stranger: not in a question
        }
        labels.push(String::from_utf8_lossy(packet.get(at..at + len)?).to_lowercase());
        at += len;
    }
    let kind = u16::from_be_bytes([*packet.get(at)?, *packet.get(at + 1)?]);
    // The question ends after its type and class.
    Some((labels.join("."), kind, at + 4))
}

/// An answer to `query` saying `name` is at `ip` (for an A question), or that there is no
/// other kind of address for it (so a PC falls back to the A answer).
pub fn answer(query: &[u8], question_end: usize, kind: u16, ip: Ipv4Addr) -> Vec<u8> {
    let mut out = query[..question_end].to_vec();
    out[2] = 0x80 | (query[2] & 0x01); // a response; recursion as asked
    out[3] = 0x80; // recursion available, no error
    let answers: u16 = if kind == TYPE_A { 1 } else { 0 };
    out[6..8].copy_from_slice(&answers.to_be_bytes());
    out[8..12].copy_from_slice(&[0, 0, 0, 0]);
    if kind == TYPE_A {
        out.extend([0xC0, 0x0C]); // the name, by pointing at the question
        out.extend(TYPE_A.to_be_bytes());
        out.extend(1u16.to_be_bytes()); // IN
        out.extend(ANSWER_TTL.to_be_bytes());
        out.extend(4u16.to_be_bytes());
        out.extend(ip.octets());
    }
    out
}

/// Answers on `bind` until the box stops. `ip` is the box's address in the home.
pub async fn run(bind: SocketAddr, ip: Ipv4Addr, upstream: SocketAddr) -> std::io::Result<()> {
    let socket = Arc::new(UdpSocket::bind(bind).await?);
    let mut buf = vec![0u8; MAX_PACKET];
    loop {
        let Ok((len, from)) = socket.recv_from(&mut buf).await else { continue };
        let packet = buf[..len].to_vec();
        let socket = socket.clone();
        tokio::spawn(async move {
            if let Some((name, kind, end)) = question(&packet) {
                if name == crate::steam::LANCACHE_NAME && (kind == TYPE_A || kind == TYPE_AAAA) {
                    let _ = socket.send_to(&answer(&packet, end, kind, ip), from).await;
                    return;
                }
            }
            if let Some(reply) = forward(&packet, upstream).await {
                let _ = socket.send_to(&reply, from).await;
            }
        });
    }
}

async fn forward(packet: &[u8], upstream: SocketAddr) -> Option<Vec<u8>> {
    let local: SocketAddr = if upstream.is_ipv4() { ([0, 0, 0, 0], 0).into() } else { "[::]:0".parse().ok()? };
    let socket = UdpSocket::bind(local).await.ok()?;
    socket.send_to(packet, upstream).await.ok()?;
    let mut buf = vec![0u8; MAX_PACKET];
    let (len, _) = tokio::time::timeout(UPSTREAM_WAIT, socket.recv_from(&mut buf)).await.ok()?.ok()?;
    buf.truncate(len);
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(name: &str, kind: u16) -> Vec<u8> {
        let mut q = vec![0x12, 0x34, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
        for label in name.split('.') {
            q.push(label.len() as u8);
            q.extend(label.as_bytes());
        }
        q.push(0);
        q.extend(kind.to_be_bytes());
        q.extend(1u16.to_be_bytes());
        q
    }

    #[test]
    fn the_steam_name_is_answered_with_the_box() {
        let q = query("LANCACHE.steamcontent.com", TYPE_A);
        let (name, kind, end) = question(&q).unwrap();
        assert_eq!((name.as_str(), kind, end), ("lancache.steamcontent.com", TYPE_A, q.len()));
        let a = answer(&q, end, kind, Ipv4Addr::new(192, 168, 1, 50));
        assert_eq!(&a[..2], &[0x12, 0x34], "same id");
        assert_eq!(a[2] & 0x80, 0x80, "a response");
        assert_eq!(u16::from_be_bytes([a[6], a[7]]), 1, "one answer");
        assert_eq!(&a[a.len() - 4..], &[192, 168, 1, 50]);
        // AAAA: no answers, no error, so the A answer is used.
        let q6 = query("lancache.steamcontent.com", TYPE_AAAA);
        let (_, k6, e6) = question(&q6).unwrap();
        let a6 = answer(&q6, e6, k6, Ipv4Addr::LOCALHOST);
        assert_eq!(u16::from_be_bytes([a6[6], a6[7]]), 0);
        assert_eq!(a6[3] & 0x0f, 0);
    }

    #[test]
    fn strange_packets_are_not_questions() {
        assert!(question(&[0u8; 5]).is_none());
        let mut response = query("example.com", TYPE_A);
        response[2] |= 0x80;
        assert!(question(&response).is_none());
    }

    #[tokio::test]
    async fn the_server_answers_over_udp() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = socket.local_addr().unwrap().port();
        drop(socket);
        let bind: SocketAddr = ([127, 0, 0, 1], port).into();
        // The upstream is a closed port: only the Steam name gets an answer.
        tokio::spawn(run(bind, Ipv4Addr::new(10, 0, 0, 7), "127.0.0.1:9".parse().unwrap()));
        tokio::time::sleep(Duration::from_millis(100)).await;
        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        client.send_to(&query("lancache.steamcontent.com", TYPE_A), bind).await.unwrap();
        let mut buf = [0u8; 512];
        let (len, _) = tokio::time::timeout(Duration::from_secs(2), client.recv_from(&mut buf)).await.unwrap().unwrap();
        assert_eq!(&buf[len - 4..len], &[10, 0, 0, 7]);
    }
}
