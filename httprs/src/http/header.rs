use std::fmt::{Debug, Write};
use std::rc::Rc;
use std::time::SystemTime;

use crate::http::value::WeightedValue;
use crate::util::date::Date;

pub trait ToStr: std::fmt::Debug {
    fn to_str(&self) -> &str;
}

#[allow(dead_code)]
pub enum HttpHeaderValue {
    String(String),
    Str(&'static str),
}

impl HttpHeaderValue {
    pub fn to_value(&self) -> Rc<dyn ToStr> {
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

impl ToStr for HeaderValueStr {
    fn to_str(&self) -> &str {
        self.str
    }
}

#[derive(Debug)]
struct HeaderValueString {
    string: Rc<String>,
}

impl ToStr for HeaderValueString {
    fn to_str(&self) -> &str {
        self.string.as_ref()
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct HeaderValueWeighted {
    weighted: Vec<WeightedValue>,
    string: String,
}

impl HeaderValueWeighted {
    pub fn from(weighted: Vec<WeightedValue>) -> Self {
        let mut string = weighted.iter().fold(String::new(), |mut s, w| {
            s.push_str(&w.value());
            if let Some(w) = w.weight() {
                let _ = write!(s, ";q={:.2}", w).map_err(|e| e.to_string());
            }
            s
        });
        string.pop();
        Self { weighted, string }
    }
}

impl ToStr for HeaderValueWeighted {
    fn to_str(&self) -> &str {
        self.string.as_str()
    }
}

#[derive(Debug)]
struct HeaderValueTime {
    string: String,
}

impl HeaderValueTime {
    pub fn from_system_time(time: SystemTime) -> Self {
        let date = Date::from(time);
        let date_string = date.to_rfc1123();
        Self {
            string: date_string,
        }
    }
}

impl ToStr for HeaderValueTime {
    fn to_str(&self) -> &str {
        self.string.as_str()
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum HttpHeader {
    StrKey(&'static str, Rc<dyn ToStr>),
    StringKey(Rc<String>, Rc<dyn ToStr>),
}

impl HttpHeader {
    pub fn key_str(&self) -> &str {
        match self {
            HttpHeader::StrKey(key, _) => key,
            HttpHeader::StringKey(key, _) => key.as_str(),
        }
    }

    pub fn value(&self) -> &Rc<dyn ToStr> {
        match self {
            HttpHeader::StrKey(_, value) => value,
            HttpHeader::StringKey(_, value) => value,
        }
    }
}

fn from_str_key(key: &'static str, value: Rc<dyn ToStr>) -> HttpHeader {
    HttpHeader::StrKey(key, value)
}

#[allow(dead_code)]
fn from_string_key(key: String, value: Rc<dyn ToStr>) -> HttpHeader {
    HttpHeader::StringKey(Rc::new(key), value)
}

// common
pub fn date(time: std::time::SystemTime) -> HttpHeader {
    return from_str_key("Date", Rc::new(HeaderValueTime::from_system_time(time)));
}

// entity
#[allow(dead_code)]
pub fn allow(values: Vec<WeightedValue>) -> HttpHeader {
    from_str_key("Allow", Rc::new(HeaderValueWeighted::from(values)))
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

#[derive(Debug)]
pub struct HttpResponseHeader {
    defined: [Option<HttpHeader>; Self::HEADER_SIZE],
    undefined: Vec<HttpHeader>,
}

impl HttpResponseHeader {
    const HEADER_SIZE: usize = 16;
    const HEADER_DEFINED_SIZE: usize = 10;
    const HEADER_LIST: [&'static str; Self::HEADER_SIZE] = [
        "Allow",
        "Content-Encoding",
        "Content-Length",
        "Content-Type",
        "Date",
        "Expires",
        "Last-Modified",
        "Location",
        "Server",
        "WWW-Authenticate", // http1.0
        "",
        "",
        "",
        "",
        "",
        "",
    ];

    pub fn new() -> Self {
        Self {
            defined: [const { None }; Self::HEADER_SIZE],
            undefined: Vec::new(),
        }
    }

    pub fn add(&mut self, header: HttpHeader) {
        if let Some(index) = Self::HEADER_LIST
            .iter()
            .position(|&k| k.eq_ignore_ascii_case(header.key_str()))
        {
            self.defined[index] = Some(header);
            return;
        }

        if let Some(idx) = self
            .undefined
            .iter()
            .position(|h| h.key_str().eq_ignore_ascii_case(header.key_str()))
        {
            self.undefined[idx] = header;
            return;
        }

        self.undefined.push(header);
    }

    pub fn set(&mut self, key: &'static str, value: Rc<dyn ToStr>) {
        if let Some(index) = Self::HEADER_LIST
            .iter()
            .position(|&k| k.eq_ignore_ascii_case(key))
        {
            self.defined[index] = Some(HttpHeader::StrKey(key, value));
            return;
        }

        if let Some(idx) = self
            .undefined
            .iter()
            .position(|h| h.key_str().eq_ignore_ascii_case(key))
        {
            self.undefined[idx] = HttpHeader::StrKey(key, value);
            return;
        }

        self.undefined.push(HttpHeader::StrKey(key, value));
    }

    pub fn get(&self, key: &'static str) -> Option<Rc<dyn ToStr>> {
        if let Some(index) = Self::HEADER_LIST
            .iter()
            .position(|&k| k.eq_ignore_ascii_case(key))
        {
            return self.defined[index].as_ref().map(|h| h.value().clone());
        }

        if let Some(idx) = self
            .undefined
            .iter()
            .position(|h| h.key_str().eq_ignore_ascii_case(key))
        {
            return Some(self.undefined[idx].value().clone());
        }

        None
    }
}

pub struct HttpResponseHeaderIterator<'a> {
    headers: &'a HttpResponseHeader,
    index: usize,
}

impl<'a> Iterator for HttpResponseHeaderIterator<'a> {
    type Item = &'a HttpHeader;

    fn next(&mut self) -> Option<&'a HttpHeader> {
        while self.index < HttpResponseHeader::HEADER_DEFINED_SIZE {
            let value = &self.headers.defined[self.index];
            self.index += 1;
            let peek = value.as_ref();
            if peek.is_some() {
                return peek;
            }
        }

        let header = self
            .headers
            .undefined
            .get(self.index - HttpResponseHeader::HEADER_DEFINED_SIZE);
        if header.is_some() {
            self.index += 1;
        }
        return header;
    }
}

impl<'a> IntoIterator for &'a HttpResponseHeader {
    type Item = &'a HttpHeader;
    type IntoIter = HttpResponseHeaderIterator<'a>;

    fn into_iter(self) -> HttpResponseHeaderIterator<'a> {
        HttpResponseHeaderIterator {
            headers: self,
            index: 0,
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::SystemTime;

    use crate::http::header::{HeaderValueTime, ToStr};

    #[test]
    fn test_time_to_header_string() {
        assert_eq!(
            HeaderValueTime::from_system_time(SystemTime::UNIX_EPOCH).to_str(),
            "Thu, 01 Jan 1970 00:00:00 GMT"
        );

        println!(
            "{}",
            HeaderValueTime::from_system_time(SystemTime::now()).to_str()
        )
    }

    #[test]
    fn test_http_response_header() {
        use super::{HttpHeaderValue, HttpResponseHeader};
        let mut headers = HttpResponseHeader::new();

        // 1. 정의된 헤더 테스트 (Predefined)
        let content_type = HttpHeaderValue::Str("text/plain").to_value();
        headers.set("Content-Type", content_type.clone());
        assert_eq!(headers.get("Content-Type").unwrap().to_str(), "text/plain");

        // 대소문자 구분 없음 확인
        assert_eq!(headers.get("content-type").unwrap().to_str(), "text/plain");

        // 2. 정의되지 않은 커스텀 헤더 테스트 (Undefined)
        let x_custom = HttpHeaderValue::Str("custom-value").to_value();
        headers.set("X-Custom-Header", x_custom.clone());
        assert_eq!(
            headers.get("X-Custom-Header").unwrap().to_str(),
            "custom-value"
        );

        // 커스텀 헤더 대소문자 구분 없음 확인
        assert_eq!(
            headers.get("x-custom-header").unwrap().to_str(),
            "custom-value"
        );

        // 3. 덮어쓰기 테스트
        let new_content_type = HttpHeaderValue::Str("application/json").to_value();
        headers.set("CONTENT-TYPE", new_content_type.clone());
        assert_eq!(
            headers.get("Content-Type").unwrap().to_str(),
            "application/json"
        );

        // 4. 존재하지 않는 헤더
        assert!(headers.get("Non-Existent").is_none());
    }
    #[test]
    fn test_http_response_header_iterator() {
        use super::{HttpHeaderValue, HttpResponseHeader};
        let mut headers = HttpResponseHeader::new();

        // 정의된 헤더 2개 추가
        headers.set(
            "Content-Type",
            HttpHeaderValue::Str("application/json").to_value(),
        );
        headers.set("Server", HttpHeaderValue::Str("httprs/1.0").to_value());

        // 정의되지 않은 헤더 2개 추가
        headers.set("X-Custom-1", HttpHeaderValue::Str("val1").to_value());
        headers.set("X-Custom-2", HttpHeaderValue::Str("val2").to_value());

        let result: Vec<_> = headers.into_iter().collect();

        // 총 4개의 헤더가 있어야 함
        assert_eq!(result.len(), 4);

        // 결과 확인 (순서는 Defined -> Undefined 순)
        assert_eq!(result[0].key_str(), "Content-Type");
        assert_eq!(result[1].key_str(), "Server");
        assert_eq!(result[2].key_str(), "X-Custom-1");
        assert_eq!(result[3].key_str(), "X-Custom-2");
    }
}
