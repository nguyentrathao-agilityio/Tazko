use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{ArrayVec, DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::NextTimeout;

fn private_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || o[0] == 0
        || (o[0] == 100 && (64..128).contains(&o[1]))
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
        || (o[0] == 198 && (18..20).contains(&o[1]))
        || o[0] >= 240
}

fn private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => private_v4(v4),
        IpAddr::V6(v6) => private_v6(v6),
    }
}

fn private_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return private_v4(v4);
    }

    let s = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (s[0] & 0xfe00) == 0xfc00
        || (s[0] & 0xffc0) == 0xfe80
        || (s[0] & 0xffc0) == 0xfec0
        || s[0] == 0x2001 && s[1] == 0x0db8
        || (s[0] == 0x64 && s[1] == 0xff9b)
        || s[0] == 0x2002
        || (s[0] == 0x2001 && s[1] == 0)
        || s[..6].iter().all(|x| *x == 0)
}

fn loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v| v.is_loopback()),
    }
}

pub fn allowed(ip: IpAddr, allow_local: bool) -> bool {
    !private_ip(ip) || (allow_local && loopback(ip))
}

const ENTER_ADDRESS: &str = "enter a web address such as https://example.com/hooks/tazko";

pub fn check_scheme(raw: &str, allow_local: bool) -> Result<(String, u16), String> {
    let raw = raw.trim();
    if raw.len() > 500 || raw.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(ENTER_ADDRESS.into());
    }

    let uri: Uri = raw.parse().map_err(|_| ENTER_ADDRESS.to_string())?;
    let scheme = uri.scheme_str().unwrap_or("");
    if scheme != "https" && !(scheme == "http" && allow_local) {
        return Err("the address must start with https://".into());
    }

    let authority = uri.authority().ok_or(ENTER_ADDRESS)?;
    if authority.as_str().contains('@') {
        return Err("the address cannot contain a user name or password".into());
    }

    let host = authority
        .host()
        .trim_start_matches('[')
        .trim_end_matches(']');
    if host.is_empty() {
        return Err(ENTER_ADDRESS.into());
    }

    let port = authority
        .port_u16()
        .unwrap_or(if scheme == "https" { 443 } else { 80 });
    Ok((host.to_string(), port))
}

pub fn check_url(raw: &str, allow_local: bool) -> Result<(), String> {
    let (host, port) = check_scheme(raw, allow_local)?;
    let addrs: Vec<SocketAddr> = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|_| "that address could not be found".to_string())?
        .collect();
    if addrs.is_empty() {
        return Err("that address could not be found".into());
    }

    if addrs.iter().any(|a| !allowed(a.ip(), allow_local)) {
        return Err("webhooks cannot be sent to private or local network addresses".into());
    }

    Ok(())
}

#[derive(Debug)]
pub struct Guarded {
    pub allow_local: bool,
}

impl Resolver for Guarded {
    fn resolve(
        &self,
        uri: &Uri,
        config: &Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let found = DefaultResolver::default().resolve(uri, config, timeout)?;
        let mut out: ResolvedSocketAddrs =
            ArrayVec::from_fn(|_| SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));
        for a in found.iter() {
            if allowed(a.ip(), self.allow_local) {
                out.push(*a);
            }
        }
        if out.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_and_local_addresses_are_refused() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "fd00::1",
            "::7f00:1",
            "::a00:1",
            "2001:0:4136:e378::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "224.0.0.1",
        ] {
            assert!(!allowed(ip.parse().unwrap(), false), "{ip}");
        }
        for ip in ["93.184.216.34", "1.1.1.1", "2606:4700::1111"] {
            assert!(allowed(ip.parse().unwrap(), false), "{ip}");
        }
    }

    #[test]
    fn the_dev_flag_opens_loopback_only() {
        assert!(allowed("127.0.0.1".parse().unwrap(), true));
        assert!(allowed("::1".parse().unwrap(), true));
        assert!(!allowed("10.0.0.1".parse().unwrap(), true));
        assert!(!allowed("169.254.169.254".parse().unwrap(), true));
    }

    #[test]
    fn addresses_are_checked_before_saving() {
        assert!(check_url("https://127.0.0.1/hook", false).is_err());
        assert!(check_url("http://localhost:9/hook", false).is_err());
        assert!(check_url("https://user:pw@1.1.1.1/hook", false).is_err());
        assert!(check_url("ftp://1.1.1.1/hook", false).is_err());
        assert!(check_url("not a url", false).is_err());
        assert!(check_url("https://1.1.1.1/hook", false).is_ok());
        assert!(check_url("http://127.0.0.1:9/hook", true).is_ok());
    }
}
