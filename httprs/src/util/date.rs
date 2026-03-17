use std::{
    fmt::Display,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub struct Date {
    year: u32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,

    epoch_days: u64,
}

impl From<SystemTime> for Date {
    fn from(value: SystemTime) -> Self {
        Date::from_system_time(value)
    }
}

impl Date {
    pub fn from_system_time(time: SystemTime) -> Self {
        let epoch_secs = time
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let epoch_mins = epoch_secs / 60;
        let epoch_hours = epoch_mins / 60;
        let epoch_days = epoch_hours / 24;

        let (years, months, days) = Date::year_and_date(epoch_days);

        Date {
            year: years as u32,
            month: months as u8,
            day: days as u8,
            hour: (epoch_hours % 24) as u8,
            minute: (epoch_mins % 60) as u8,
            second: (epoch_secs % 60) as u8,
            epoch_days,
        }
    }

    const MONTH: [&str; 13] = [
        "", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    const WEEK_DAY: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];

    fn year_and_date(dates: u64) -> (u64, u64, u64) {
        // Shift epoch from 1970-01-01 to 0000-03-01 layout
        let z = dates + 719468;
        let era = z / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let year = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = year + if m <= 2 { 1 } else { 0 };
        (year, m, d)
    }

    fn write_to_buf(&self, buf: &mut [u8; 29]) {
        buf[0..3].copy_from_slice(Date::WEEK_DAY[(self.epoch_days % 7) as usize].as_bytes());
        buf[3..5].copy_from_slice(b", ");

        buf[5] = b'0' + (self.day / 10);
        buf[6] = b'0' + (self.day % 10);

        buf[8..11].copy_from_slice(Date::MONTH[self.month as usize].as_bytes());

        let mut y = self.year;
        buf[15] = b'0' + (y % 10) as u8;
        y /= 10;
        buf[14] = b'0' + (y % 10) as u8;
        y /= 10;
        buf[13] = b'0' + (y % 10) as u8;
        y /= 10;
        buf[12] = b'0' + (y % 10) as u8;

        buf[17] = b'0' + (self.hour / 10);
        buf[18] = b'0' + (self.hour % 10);
        buf[19] = b':';

        buf[20] = b'0' + (self.minute / 10);
        buf[21] = b'0' + (self.minute % 10);
        buf[22] = b':';

        buf[23] = b'0' + (self.second / 10);
        buf[24] = b'0' + (self.second % 10);

        buf[25..29].copy_from_slice(b" GMT");
    }

    // make SystemTime to rfc1123-date
    // Mon, 22 Nov 1990 00:00:00 GMT
    pub fn to_rfc1123(&self) -> String {
        let mut buf = [b' '; 29];
        self.write_to_buf(&mut buf);
        unsafe { String::from_utf8_unchecked(buf.to_vec()) }
    }
}

impl Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = [b' '; 29];
        self.write_to_buf(&mut buf);
        let s = unsafe { std::str::from_utf8_unchecked(&buf) };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_year_and_date_epoch() {
        let (y, m, d) = Date::year_and_date(0);
        assert_eq!(y, 1970);
        assert_eq!(m, 1); // Jan
        assert_eq!(d, 1);
    }

    #[test]
    fn test_date_from_system_time() {
        let d = Date::from_system_time(UNIX_EPOCH);
        assert_eq!(d.year, 1970);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
        assert_eq!(d.hour, 0);
        assert_eq!(d.minute, 0);
        assert_eq!(d.second, 0);

        // test to_rfc1123
        assert_eq!(d.to_rfc1123(), "Thu, 01 Jan 1970 00:00:00 GMT");
        assert_eq!(d.to_string(), "Thu, 01 Jan 1970 00:00:00 GMT");
    }

    #[test]
    fn test_rfc1123_format() {
        // 2024-02-28T12:00:00Z -> epoch diff is 19781 days + 12 hours
        let time = UNIX_EPOCH + Duration::from_secs(1709121600);
        let d = Date::from_system_time(time);
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, 2); // Feb = 1
        assert_eq!(d.day, 28);
        assert_eq!(d.hour, 12);
        assert_eq!(d.minute, 0);
        assert_eq!(d.second, 0);
        assert_eq!(d.to_rfc1123(), "Wed, 28 Feb 2024 12:00:00 GMT");

        // leap year: 2024-02-29T00:00:00Z
        let time2 = UNIX_EPOCH + Duration::from_secs(1709164800);
        let d2 = Date::from_system_time(time2);
        assert_eq!(d2.year, 2024);
        assert_eq!(d2.month, 2); // Feb
        assert_eq!(d2.day, 29);
        assert_eq!(d2.to_rfc1123(), "Thu, 29 Feb 2024 00:00:00 GMT");
    }
}
