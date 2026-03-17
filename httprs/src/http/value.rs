use std::{
    fmt::{Display, Formatter},
    hash::Hash,
    net::SocketAddr,
};

pub enum HttpVersion {
    Http10,
    Http11,
}

#[allow(dead_code)]
impl HttpVersion {
    pub fn parse(str: &str) -> Option<Self> {
        return if str.eq_ignore_ascii_case("http/1.0") {
            Some(HttpVersion::Http10)
        } else if str.eq_ignore_ascii_case("http/1.1") {
            Some(HttpVersion::Http11)
        } else {
            None
        };
    }
}

impl Default for HttpVersion {
    fn default() -> Self {
        return Self::Http10;
    }
}

impl Display for HttpVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return f.write_str(match self {
            HttpVersion::Http10 => "HTTP/1.0",
            HttpVersion::Http11 => "HTTP/1.1",
        });
    }
}

impl Clone for HttpVersion {
    fn clone(&self) -> Self {
        match self {
            Self::Http10 => Self::Http10,
            Self::Http11 => Self::Http11,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    POST,
    HEAD, // HTTP 1.0
    UNDEFINED(String),
}

impl HttpMethod {
    pub fn parse(str: &str) -> Self {
        return match str.to_uppercase().as_str() {
            "GET" => HttpMethod::GET,
            "POST" => HttpMethod::POST,
            "HEAD" => HttpMethod::HEAD,
            _ => HttpMethod::UNDEFINED(str.to_string()),
        };
    }
}

impl Display for HttpMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return f.write_fmt(format_args!(
            "{}",
            match self {
                HttpMethod::GET => "GET",
                HttpMethod::POST => "POST",
                HttpMethod::HEAD => "HEAD",
                HttpMethod::UNDEFINED(v) => v,
            }
        ));
    }
}

macro_rules! define_http_response_code {
    (
        $(
            $name:ident = $code:expr, $reason:expr
        ),* $(,)?
    ) => {
        #[allow(dead_code)]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum HttpResponseCode {
            $( $name ),*
        }

        impl HttpResponseCode {
            pub fn code(&self) -> i32 {
                match self {
                    $( HttpResponseCode::$name => $code ),*
                }
            }

            pub fn reason(&self) -> &'static str {
                match self {
                    $( HttpResponseCode::$name => $reason ),*
                }
            }

            pub fn as_str(&self) -> &'static str {
                match self {
                    $( HttpResponseCode::$name => concat!(stringify!($code), " ", $reason) ),*
                }
            }
        }

        impl Display for HttpResponseCode {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    }
}

define_http_response_code! {
    Ok = 200, "OK",
    Created = 201, "Created",
    Accepted = 202, "Accepted",
    NoContent = 204, "No Content",
    MovedPermanetly = 301, "Moved Permanently",
    MovedTemporarily = 302, "Moved Temporarily",
    NotModified = 304, "Not Modified",
    BadRequest = 400, "Bad Request",
    Unauthorized = 401, "Unauthorized",
    Forbidden = 403, "Forbidden",
    NotFound = 404, "Not Found",
    InternalServerError = 500, "Internal Server Error",
    NotImplemented = 501, "Not Implemented",
    BadGateway = 502, "Bad Gateway",
    ServiceUnavailable = 503, "Service Unavailable",
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Error {
    ParseFail(String),
    ReadFail(String),
    WriteFail(String),
    BadRequest(SocketAddr, &'static str),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Error::ParseFail(m) => ("parse fail", m),
            Error::ReadFail(m) => ("read fail", m),
            Error::WriteFail(m) => ("write fail", m),
            Error::BadRequest(remote, msg) => ("bad request", &format!("{} {}", remote, msg)),
        };

        return f.write_fmt(format_args!("HttpError: [{}] {}", name.0, name.1));
    }
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Clone)]
pub struct WeightedValue {
    value: String,
    weight: Option<f64>,
}

impl Eq for WeightedValue {}

impl Hash for WeightedValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
        if let Some(weight) = self.weight {
            weight.to_bits().hash(state);
        }
    }
}

#[allow(dead_code)]
impl WeightedValue {
    pub fn value(&self) -> &String {
        &self.value
    }

    pub fn weight(&self) -> Option<f64> {
        self.weight
    }
}
