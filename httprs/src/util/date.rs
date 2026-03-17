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
            month: months as u8 % 12,
            day: days as u8 % 30,
            hour: epoch_hours as u8 % 24,
            minute: epoch_mins as u8 % 60,
            second: epoch_secs as u8 % 60,
            epoch_days,
        }
    }

    const MONTH: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    const MONTH_DATE: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    const MONTH_DATE_LEAP: [u64; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    const WEEK_DAY: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];

    fn is_leap_year(year: u64) -> bool {
        year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
    }

    fn year_and_date(dates: u64) -> (u64, u64, u64) {
        let mut cnt = dates;
        let mut year = 1970;
        let mut month = 0;
        loop {
            let whole_year = if Date::is_leap_year(year) { 366 } else { 365 };

            if cnt < whole_year {
                break;
            }
            cnt -= whole_year;
            year += 1;
        }

        let months = if Date::is_leap_year(year) {
            Date::MONTH_DATE_LEAP
        } else {
            Date::MONTH_DATE
        };

        loop {
            let month_days = months[month as usize];
            if cnt < month_days {
                break;
            }
            cnt -= month_days;
            month += 1;
        }

        (year, month, cnt + 1)
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
