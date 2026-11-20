pub const RULES: &[&str] = &["daily", "weekdays", "weekly", "biweekly", "monthly"];

const DAY: i64 = 86_400;
const MAX_STEPS: usize = 4_000;

pub fn valid(rule: &str) -> bool {
    RULES.contains(&rule)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub(crate) fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        if m <= 2 {
            yoe + era * 400 + 1
        } else {
            yoe + era * 400
        },
        m,
        d,
    )
}

fn days_in_month(y: i64, m: i64) -> i64 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    days_from_civil(ny, nm, 1) - days_from_civil(y, m, 1)
}

fn step(local: i64, rule: &str, day_of_month: i64) -> i64 {
    let days = local.div_euclid(DAY);
    let time = local.rem_euclid(DAY);
    let next = match rule {
        "daily" => days + 1,
        "weekdays" => {
            let mut n = days + 1;
            while matches!((n + 4).rem_euclid(7), 0 | 6) {
                n += 1;
            }
            n
        }
        "weekly" => days + 7,
        "biweekly" => days + 14,
        _ => {
            let (y, m, _) = civil_from_days(days);
            let (y, m) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            days_from_civil(y, m, day_of_month.min(days_in_month(y, m)))
        }
    };
    next * DAY + time
}

pub fn next_due(due: i64, rule: &str, now: i64, offset_minutes: i64) -> i64 {
    let off = offset_minutes * 60;
    let start = due + off;
    let (_, _, dom) = civil_from_days(start.div_euclid(DAY));
    let period = match rule {
        "daily" => DAY,
        "biweekly" => 14 * DAY,
        "monthly" => 0,
        _ => 7 * DAY,
    };
    let behind = now + off - start;
    let skip = if period > 0 && behind > period {
        (behind / period - 1) * period
    } else {
        0
    };
    let mut local = step(start + skip, rule, dom);
    for _ in 0..MAX_STEPS {
        if local - off > now {
            break;
        }
        local = step(local, rule, dom);
    }
    local - off
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i64, m: i64, d: i64, h: i64) -> i64 {
        days_from_civil(y, m, d) * DAY + h * 3600
    }

    #[test]
    fn each_rule_moves_to_the_next_date_after_now() {
        let fri = at(2026, 11, 20, 12);
        let now = fri - 3600;
        assert_eq!(next_due(fri, "daily", now, 0), at(2026, 11, 21, 12));
        assert_eq!(
            next_due(fri, "weekdays", now, 0),
            at(2026, 11, 23, 12),
            "Friday to Monday"
        );
        assert_eq!(next_due(fri, "weekly", now, 0), at(2026, 11, 27, 12));
        assert_eq!(next_due(fri, "biweekly", now, 0), at(2026, 12, 4, 12));
        assert_eq!(next_due(fri, "monthly", now, 0), at(2026, 12, 20, 12));
    }

    #[test]
    fn a_late_card_skips_ahead_past_now() {
        let due = at(2026, 11, 1, 9);
        assert_eq!(
            next_due(due, "weekly", at(2026, 11, 20, 0), 0),
            at(2026, 11, 22, 9)
        );
    }

    #[test]
    fn months_keep_the_day_and_end_of_month_is_clamped() {
        let jan31 = at(2027, 1, 31, 12);
        let feb = next_due(jan31, "monthly", jan31, 0);
        assert_eq!(feb, at(2027, 2, 28, 12));
        assert_eq!(
            next_due(feb, "monthly", feb, 0),
            at(2027, 3, 28, 12),
            "steps from the clamped date"
        );
        assert_eq!(
            next_due(at(2026, 12, 15, 8), "monthly", 0, 0),
            at(2027, 1, 15, 8)
        );
    }

    #[test]
    fn weekdays_follow_the_local_day() {
        let sat_local_morning_utc_friday = at(2026, 11, 20, 20);
        assert_eq!(
            next_due(sat_local_morning_utc_friday, "weekdays", 0, 7 * 60),
            at(2026, 11, 22, 20),
            "Saturday in UTC+7 goes to Monday"
        );
        assert_eq!(
            next_due(sat_local_morning_utc_friday, "weekdays", 0, 0),
            at(2026, 11, 23, 20),
            "Friday in UTC goes to Monday"
        );
        let thu_utc = at(2026, 11, 19, 20);
        assert_eq!(
            next_due(thu_utc, "weekdays", 0, 7 * 60),
            at(2026, 11, 22, 20),
            "Friday in UTC+7 goes to Monday"
        );
        assert_eq!(
            next_due(thu_utc, "weekdays", 0, 0),
            at(2026, 11, 20, 20),
            "Thursday in UTC goes to Friday"
        );
    }

    #[test]
    fn a_card_years_late_still_moves_past_now() {
        let now = at(2026, 11, 20, 10);
        for rule in ["daily", "weekdays", "weekly", "biweekly", "monthly"] {
            let next = next_due(at(2001, 3, 5, 9), rule, now, 0);
            assert!(next > now && next - now <= 32 * DAY, "{rule}: {next}");
            assert_eq!(next.rem_euclid(DAY), 9 * 3600, "{rule} keeps the time");
        }
        let next = next_due(at(2001, 3, 5, 9), "weekdays", now, 0);
        assert!(
            !matches!((next.div_euclid(DAY) + 4).rem_euclid(7), 0 | 6),
            "lands on a weekday"
        );
    }

    #[test]
    fn only_known_rules_are_valid() {
        assert!(valid("weekly"));
        assert!(!valid("yearly"));
        assert!(!valid(""));
    }
}
