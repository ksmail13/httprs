use flate2::read::GzDecoder;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, SystemTime},
};

use crate::{
    http::{
        handler::Handler,
        header::{HttpHeaderValue, content_type, date, server},
        request::HttpRequest,
        response::{HeaderSetter, HttpResponse},
        value::{Error, HttpMethod, HttpResponseCode, HttpVersion},
    },
    process::{self, Process},
};

pub struct Http1<T: Handler> {
    max_header_length: usize,
    handler: T,
}

impl<T> Process for Http1<T>
where
    T: Handler,
{
    fn process(
        &self,
        stream: TcpStream,
        client_addr: &std::net::SocketAddr,
    ) -> Result<(usize, usize), process::Error> {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));

        log::trace!("Read timeout: {:?}", stream.read_timeout());
        log::trace!("Write timeout: {:?}", stream.write_timeout());

        let mut reader: BufReader<Box<dyn Read>> = BufReader::new(Box::new(&stream));

        let (header_readed, headers) =
            self.read_header(client_addr, &mut reader).map_err(|err| {
                self.error_response_for_invalid_request(&stream);
                process::Error::IoFail(format!("Read header failed: ({})", err))
            })?;

        let mut request = self
            .init_request(client_addr, &headers, Box::new(&stream))
            .map_err(|e| {
                self.error_response_for_invalid_request(&stream);
                process::Error::ParseFail(e.to_string())
            })?;
        let mut response = HttpResponse::from_request(&request, Box::new(&stream));
        response.set_header(&server(HttpHeaderValue::Str("server_rs")));

        self.handler.handle(&mut request, &mut response);

        response
            .flush()
            .map_err(|e| process::Error::IoFail(e.to_string()))?;

        Ok((header_readed, response.written()))
    }

    fn name(&self) -> String {
        return "http".to_string();
    }
}

impl<T> Http1<T>
where
    T: Handler,
{
    pub fn new(max_header_length: usize, handler: T) -> Self {
        return Http1 {
            max_header_length,
            handler,
        };
    }

    fn read_header<'a>(
        &self,
        client_addr: &SocketAddr,
        reader: &mut BufReader<Box<dyn Read + 'a>>,
    ) -> Result<(usize, Vec<String>), Error> {
        let mut res = vec![];
        let mut readed = 0;
        loop {
            let mut buf = String::new();
            let result = reader.read_line(&mut buf);
            if let Err(err) = result {
                return Err(Error::ReadFail(format!("{}", err)));
            }
            readed += result.unwrap();

            if readed > self.max_header_length {
                return Err(Error::BadRequest(
                    client_addr.clone(),
                    "header size limit exceed",
                ));
            }

            while buf
                .chars()
                .nth(0)
                .map(|v| v.is_whitespace())
                .unwrap_or(false)
            {
                buf.remove(0);
            }

            // head end
            if buf.is_empty() {
                break;
            }

            if !buf.ends_with("\r\n") {
                return Err(Error::ParseFail(format!("Invalid heaader {}", buf)));
            }

            buf.remove(buf.len() - 1); // delete \n
            buf.remove(buf.len() - 1); // delete \r

            log::trace!("<< {}", buf);

            res.push(buf);
        }

        if readed == 0 {
            return Err(Error::ReadFail(format!("EOF")));
        }

        return Ok((readed, res));
    }

    /**
     * Read header part of HTTP request
     */
    fn init_request<'a>(
        &self,
        client_addr: &'a std::net::SocketAddr,
        raw_header: &'a Vec<String>,
        reader: Box<dyn Read + 'a>,
    ) -> Result<HttpRequest<'a>, Error> {
        let buf = &raw_header[0];

        let mut req_line = buf.split(" ");

        let method = req_line
            .next()
            .ok_or_else(|| Error::ParseFail(format!("invalid request line: {}", buf)))?;
        let path_query = req_line
            .next()
            .ok_or_else(|| Error::ParseFail(format!("invalid request line: {}", buf)))?;

        // 1.1 구현 후 반영
        let version = HttpVersion::default();

        let (path, param) = parse_url(path_query);

        let header = self.init_header(raw_header);
        let content_encoding = *header
            .get("Content-Encoding")
            .map_or(None, |v| v.first())
            .unwrap_or(&"");

        let body_reader = if content_encoding == "gzip" {
            Box::new(GzDecoder::new(reader))
        } else {
            reader
        };

        return Ok(HttpRequest::new(
            client_addr,
            HttpMethod::parse(method),
            version,
            path,
            header,
            param,
            body_reader,
        ));
    }

    fn init_header<'a>(&self, reader: &'a Vec<String>) -> HashMap<&'a str, Vec<&'a str>> {
        let mut header_map: HashMap<&str, Vec<&str>> = HashMap::new();
        for i in 1..reader.len() {
            let buf = &reader[i];

            let div_idx = match buf.find(':') {
                Some(idx) => idx,
                None => continue,
            };

            let (key, value) = buf.split_at(div_idx);

            put_data_to_hashmap(&mut header_map, key.trim(), value[1..].trim());
        }

        return header_map;
    }

    fn error_response_for_invalid_request(&self, stream: &TcpStream) {
        let mut response = HttpResponse::new(HttpVersion::default(), Box::new(stream));

        response.set_response_code(HttpResponseCode::BadRequest);
        response.set_header(&server(HttpHeaderValue::Str("server_rs")));
        response.set_header(&content_type(HttpHeaderValue::Str("text/plain")));
        response.set_header(&date(SystemTime::now()));
        let _ = response.write("Invalid request".as_bytes());
        let _ = response.flush();
    }
}

fn parse_url(query: &str) -> (String, HashMap<&str, Vec<&str>>) {
    let path_param: Vec<&str> = query.split("?").collect();

    if path_param.len() < 2 {
        return (path_param[0].to_string(), HashMap::new());
    }

    return (
        path_param[0].to_string(),
        path_param[1]
            .split("&")
            .filter(|p| !p.is_empty())
            .map(|s| match s.find('=') {
                Some(idx) => s.split_at(idx),
                None => (s, "=true"),
            })
            .fold(HashMap::new(), |mut m, p| {
                put_data_to_hashmap(&mut m, p.0, &p.1[1..]);
                return m;
            }),
    );
}

fn put_data_to_hashmap<'a>(map: &mut HashMap<&'a str, Vec<&'a str>>, key: &'a str, value: &'a str) {
    map.entry(key).or_default().push(value);
}

#[cfg(test)]
mod test {
    use crate::http::http::parse_url;

    #[test]
    fn test_parse_url() {
        let (path, param) = parse_url("/test?asdf=asdf&asdf=fdsa");

        assert_eq!(path, "/test");
        assert_eq!(param.get("asdf"), Some(&vec!["asdf", "fdsa"]));
    }

    #[test]
    fn test_no_param() {
        let (path, param) = parse_url("/test");

        assert_eq!(path, "/test");
        assert!(param.is_empty());
    }
}
