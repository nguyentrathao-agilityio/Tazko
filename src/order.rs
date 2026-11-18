const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: usize = 62;

fn value(c: u8) -> Option<usize> {
    DIGITS.iter().position(|d| *d == c)
}

pub fn valid(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 200
        && key.bytes().all(|c| value(c).is_some())
        && !key.ends_with('0')
}

fn midpoint(a: &[u8], b: Option<&[u8]>) -> Vec<u8> {
    if let Some(b) = b {
        let mut n = 0;
        while n < b.len() && a.get(n).copied().unwrap_or(b'0') == b[n] {
            n += 1;
        }
        if n > 0 {
            let mut out = b[..n].to_vec();
            let rest_a = if n < a.len() { &a[n..] } else { &[][..] };
            out.extend(midpoint(rest_a, Some(&b[n..])));
            return out;
        }
    }

    let da = a.first().and_then(|c| value(*c)).unwrap_or(0);
    let db = b.map_or(BASE, |b| value(b[0]).unwrap_or(BASE));
    if db - da > 1 {
        return vec![DIGITS[(da + db) / 2]];
    }

    if let Some(b) = b.filter(|b| b.len() > 1) {
        return vec![b[0]];
    }

    let mut out = vec![DIGITS[da]];
    let rest = if a.len() > 1 { &a[1..] } else { &[][..] };
    out.extend(midpoint(rest, None));
    out
}

pub fn between(a: Option<&str>, b: Option<&str>) -> String {
    let a = a.unwrap_or("").as_bytes();
    let b = b.map(str::as_bytes);
    if let Some(b) = b {
        debug_assert!(a < b, "keys out of order");
    }

    String::from_utf8(midpoint(a, b)).expect("ascii digits")
}

pub fn after(last: Option<&str>) -> String {
    between(last, None)
}

pub fn spread(count: usize) -> Vec<String> {
    let mut keys = Vec::with_capacity(count);
    let mut last: Option<String> = None;
    for _ in 0..count {
        let next = after(last.as_deref());
        keys.push(next.clone());
        last = Some(next);
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_sort_between_their_neighbours() {
        let a = between(None, None);
        let b = after(Some(&a));
        let m = between(Some(&a), Some(&b));
        assert!(a < m && m < b, "{a} {m} {b}");
        let first = between(None, Some(&a));
        assert!(first < a);
    }

    #[test]
    fn repeated_inserts_at_the_front_middle_and_end_stay_ordered() {
        let mut keys: Vec<String> = spread(3);
        for i in 0..2000 {
            let at = match i % 3 {
                0 => 0,
                1 => keys.len() / 2,
                _ => keys.len(),
            };
            let before = if at == 0 {
                None
            } else {
                Some(keys[at - 1].as_str())
            };
            let after = keys.get(at).map(String::as_str);
            let k = between(before, after);
            assert!(valid(&k), "{k}");
            if let Some(b) = before {
                assert!(b < k.as_str());
            }
            if let Some(a) = after {
                assert!(k.as_str() < a);
            }
            keys.insert(at, k);
        }
        let mut sorted = keys.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted, keys);
    }

    #[test]
    fn invalid_keys_are_refused() {
        for k in ["", "a0", "a-b", "é"] {
            assert!(!valid(k), "{k}");
        }
        assert!(valid("V"));
    }
}
