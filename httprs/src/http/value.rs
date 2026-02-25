use std::{fmt::Display, hash::Hash, net::SocketAddr};

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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

#[allow(dead_code)]
pub enum HttpResponseCode {
    Ok,
    Created,
    Accepted,
    NoContent,
    MovedPermanetly,
    MovedTemporarily,
    NotModified,
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    InternalServerError,
    NotImplemented,
    BadGateway,
    ServiceUnavailable,
}

impl HttpResponseCode {
    pub fn code(&self) -> i32 {
        return match self {
            HttpResponseCode::Ok => 200,
            HttpResponseCode::Created => 201,
            HttpResponseCode::Accepted => 202,
            HttpResponseCode::NoContent => 204,
            HttpResponseCode::MovedPermanetly => 301,
            HttpResponseCode::MovedTemporarily => 302,
            HttpResponseCode::NotModified => 304,
            HttpResponseCode::BadRequest => 400,
            HttpResponseCode::Unauthorized => 401,
            HttpResponseCode::Forbidden => 403,
            HttpResponseCode::NotFound => 404,
            HttpResponseCode::InternalServerError => 500,
            HttpResponseCode::NotImplemented => 501,
            HttpResponseCode::BadGateway => 502,
            HttpResponseCode::ServiceUnavailable => 503,
        };
    }

    pub fn reason(&self) -> &str {
        return match self {
            HttpResponseCode::Ok => "OK",
            HttpResponseCode::Created => "Created",
            HttpResponseCode::Accepted => "Accepted",
            HttpResponseCode::NoContent => "No Content",
            HttpResponseCode::MovedPermanetly => "Moved Permanently",
            HttpResponseCode::MovedTemporarily => "Moved Temporarily",
            HttpResponseCode::NotModified => "Not Modified",
            HttpResponseCode::BadRequest => "Bad Request",
            HttpResponseCode::Unauthorized => "Unauthorized",
            HttpResponseCode::Forbidden => "Forbidden",
            HttpResponseCode::NotFound => "NotFound",
            HttpResponseCode::InternalServerError => "Interna Server Error",
            HttpResponseCode::NotImplemented => "Not Implemented",
            HttpResponseCode::BadGateway => "Bad Gateway",
            HttpResponseCode::ServiceUnavailable => "Service Unavailable",
        };
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Error {
    ParseFail(String),
    ReadFail(String),
    WriteFail(String),
    BadRequest(SocketAddr, &'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
