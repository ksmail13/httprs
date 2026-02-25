use std::fmt::Write;
use std::rc::Rc;
use std::time::SystemTime;

use crate::http::value::WeightedValue;
use crate::util::date::Date;

pub trait ToString: std::fmt::Debug {
    fn to_string(&self) -> Rc<String>;
}

#[allow(dead_code)]
pub enum HttpHeaderValue {
    String(String),
    Str(&'static str),
}

impl HttpHeaderValue {
    pub fn to_value(&self) -> Rc<dyn ToString> {
        match self {
            HttpHeaderValue::String(string) => Rc::new(HeaderValueString {
                string: Rc::new(string.clone()),
            }),
            HttpHeaderValue::Str(str) => Rc::new(HeaderValueStr { str: str }),
        }
    }
}

#[derive(Debug)]
struct HeaderValueStr {
    str: &'static str,
}

impl ToString for HeaderValueStr {
    fn to_string(&self) -> Rc<String> {
        Rc::new(self.str.to_string())
    }
}

#[derive(Debug)]
struct HeaderValueString {
    string: Rc<String>,
}

impl ToString for HeaderValueString {
    fn to_string(&self) -> Rc<String> {
        self.string.clone()
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct HeaderValueWeighted {
    weighted: Vec<WeightedValue>,
}

impl ToString for HeaderValueWeighted {
    fn to_string(&self) -> Rc<String> {
        let mut val = self.weighted.iter().fold(String::new(), |mut s, w| {
            s.push_str(&w.value());
            if let Some(w) = w.weight() {
                let _ = write!(s, ";q={:.2}", w).map_err(|e| e.to_string());
            }
            s
        });
        val.remove(val.len() - 1);
        return Rc::new(val);
    }
}

#[derive(Debug)]
struct HeaderValueTime {
    time: Date,
}

impl HeaderValueTime {
    fn time_to_header_string(&self) -> String {
        self.time.to_rfc1123()
    }

    pub fn from_system_time(time: SystemTime) -> Self {
        Self {
            time: Date::from(time),
        }
    }
}

impl ToString for HeaderValueTime {
    fn to_string(&self) -> Rc<String> {
        Rc::new(self.time_to_header_string())
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct HttpHeader {
    key_str: Option<&'static str>,
    key_string: Option<Rc<String>>,
    value: Rc<dyn ToString>,
}

impl HttpHeader {
    pub fn key_str(&self) -> Option<&'static str> {
        self.key_str
    }

    pub fn key_string(&self) -> Option<Rc<String>> {
        self.key_string.clone()
    }

    pub fn value(&self) -> &Rc<dyn ToString> {
        &self.value
    }
}

fn from_str_key(key: &'static str, value: Rc<dyn ToString>) -> HttpHeader {
    HttpHeader {
        key_str: Some(key),
        key_string: None,
        value: value,
    }
}

#[allow(dead_code)]
fn from_string_key(key: String, value: Rc<dyn ToString>) -> HttpHeader {
    HttpHeader {
        key_str: None,
        key_string: Some(Rc::new(key)),
        value: value,
    }
}

// common
pub fn date(time: std::time::SystemTime) -> HttpHeader {
    return from_str_key("Date", Rc::new(HeaderValueTime::from_system_time(time)));
}

// entity
#[allow(dead_code)]
pub fn allow(values: Vec<WeightedValue>) -> HttpHeader {
    from_str_key("Allow", Rc::new(HeaderValueWeighted { weighted: values }))
}

#[allow(dead_code)]
pub fn content_encoding(value: HttpHeaderValue) -> HttpHeader {
    from_str_key("Content-Encoding", value.to_value())
}

#[allow(dead_code)]
pub fn content_length(value: usize) -> HttpHeader {
    from_str_key(
        "Content-Length",
        Rc::new(HeaderValueString {
            string: Rc::new(value.to_string()),
        }),
    )
}

// entity
#[allow(dead_code)]
pub fn content_type(value: HttpHeaderValue) -> HttpHeader {
    from_str_key("Content-Type", value.to_value())
}

#[allow(dead_code)]
pub fn expires(time: std::time::SystemTime) -> HttpHeader {
    from_str_key("Expires", Rc::new(HeaderValueTime::from_system_time(time)))
}

#[allow(dead_code)]
pub fn last_modified(time: std::time::SystemTime) -> HttpHeader {
    from_str_key(
        "Last-Modified",
        Rc::new(HeaderValueTime::from_system_time(time)),
    )
}

#[allow(dead_code)]
pub fn header(key: &'static str, value: HttpHeaderValue) -> HttpHeader {
    from_str_key(key, value.to_value())
}

#[allow(dead_code)]
pub fn location(value: HttpHeaderValue) -> HttpHeader {
    from_str_key("Location", value.to_value())
}

#[allow(dead_code)]
pub fn server(value: HttpHeaderValue) -> HttpHeader {
    from_str_key("Server", value.to_value())
}

#[allow(dead_code)]
pub fn www_authenticate(value: HttpHeaderValue) -> HttpHeader {
    from_str_key("WWW-Authenticate", value.to_value())
}

#[cfg(test)]
mod test {
    use std::time::SystemTime;

    use crate::http::header::{HeaderValueTime, ToString};

    #[test]
    fn test_time_to_header_string() {
        assert_eq!(
            HeaderValueTime::from_system_time(SystemTime::UNIX_EPOCH)
                .to_string()
                .as_ref(),
            "Thu, 01 Jan 1970 00:00:00 GMT"
        );

        println!(
            "{}",
            HeaderValueTime::from_system_time(SystemTime::now()).to_string()
        )
    }
}
