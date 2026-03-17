use std::{
    io::{BufWriter, Write},
    time::SystemTime,
};

use flate2::{Compression, write::GzEncoder};

use crate::http::{
    header::{HttpHeader, HttpResponseHeader, content_length, date},
    request::HttpRequest,
    value::{HttpMethod, HttpResponseCode, HttpVersion},
};

pub struct HttpResponse<'a> {
    version: HttpVersion,
    code: HttpResponseCode,
    header: HttpResponseHeader,
    writer: Box<dyn Write + 'a>,
    buffer: Vec<u8>,
    header_only: bool,
    written: usize,
}

impl<'a> HttpResponse<'a> {
    pub fn new(version: HttpVersion, writer: Box<dyn Write + 'a>) -> Self {
        return Self {
            version: version,
            code: HttpResponseCode::Ok,
            header: HttpResponseHeader::new(),
            writer: writer,
            buffer: vec![],
            header_only: false,
            written: 0,
        };
    }

    pub fn from_request(request: &HttpRequest, writer: Box<dyn Write + 'a>) -> Self {
        return Self {
            version: request.version(),
            code: HttpResponseCode::Ok,
            header: HttpResponseHeader::new(),
            writer: writer,
            buffer: vec![],
            header_only: request.method() == HttpMethod::HEAD,
            written: 0,
        };
    }

    pub fn written(&self) -> usize {
        self.written
    }
}

impl<'a> Write for HttpResponse<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        return Ok(buf.len());
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.header.get("Content-Encoding") {
            Some(v) if v.to_str() == "gzip" => {
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(&self.buffer)?;
                self.buffer = encoder.finish()?;
            }
            _ => {}
        }

        self.set_header(content_length(self.buffer.len()));
        self.set_header(date(SystemTime::now()));

        let mut buf_writer = BufWriter::with_capacity(1 << 15, &mut self.writer);
        let status_line = format!("{} {}{}", self.version, self.code, LINE_END);
        buf_writer.write_all(status_line.as_bytes())?;
        let mut written = status_line.len();

        for i in self.header.into_iter() {
            buf_writer.write_all(i.key_str().as_bytes())?;
            buf_writer.write_all(KV_SEP.as_bytes())?;
            buf_writer.write_all(i.value().to_str().as_bytes())?;
            buf_writer.write_all(LINE_END.as_bytes())?;
            written += i.key_str().len() + KV_SEP.len() + i.value().to_str().len() + LINE_END.len();
        }

        buf_writer.write_all(LINE_END.as_bytes())?;
        written += LINE_END.len();

        if self.header_only {
            buf_writer.flush()?;
            self.written = written;
            return Ok(());
        }

        buf_writer.write_all(&self.buffer)?;
        let body_written = self.buffer.len();

        buf_writer.flush()?;
        self.written = written + body_written;
        Ok(())
    }
}

#[allow(dead_code)]
pub trait HeaderSetter<T> {
    fn set_header(&mut self, header: T);
}

impl<'a> HeaderSetter<HttpHeader> for HttpResponse<'a> {
    fn set_header(&mut self, header: HttpHeader) {
        self.header.add(header);
    }
}

const LINE_END: &str = "\r\n";
const KV_SEP: &str = ": ";

impl HttpResponse<'_> {
    pub fn set_response_code(&mut self, code: HttpResponseCode) {
        self.code = code;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::header::{HttpHeaderValue, content_encoding};
    use flate2::read::GzDecoder;
    use std::io::Read;

    #[test]
    fn test_gzip_encoding_and_decoding() {
        // 테스트용 버퍼 생성
        let mut output_buffer = Vec::new();

        // HttpResponse 생성 및 사용 (별도 스코프로 분리)
        {
            let mut response =
                HttpResponse::new(HttpVersion::default(), Box::new(&mut output_buffer));

            // Content-Encoding 헤더를 gzip으로 설정
            response.set_header(content_encoding(HttpHeaderValue::Str("gzip")));

            // 테스트 데이터 작성
            let test_data = b"Hello, this is a test message for gzip encoding!";
            response.write(test_data).unwrap();

            // flush를 호출하여 실제로 gzip 인코딩 수행
            response.flush().unwrap();
        } // response가 여기서 drop되어 output_buffer의 mutable borrow가 해제됨

        // 출력 버퍼에서 헤더와 바디 분리
        let output_str = String::from_utf8_lossy(&output_buffer);
        let parts: Vec<&str> = output_str.split("\r\n\r\n").collect();

        // 헤더 부분 확인
        assert!(parts[0].contains("Content-Encoding: gzip"));

        // 바디 부분 (gzip으로 인코딩된 데이터) 추출
        let body_start = parts[0].len() + 4; // "\r\n\r\n"의 길이
        let compressed_body = &output_buffer[body_start..];

        // gzip 디코딩
        let mut decoder = GzDecoder::new(compressed_body);
        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();

        // 디코딩된 데이터가 원본과 일치하는지 확인
        let expected = b"Hello, this is a test message for gzip encoding!";
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_no_gzip_encoding() {
        // gzip 없이 일반 응답 테스트
        let mut output_buffer = Vec::new();

        {
            let mut response =
                HttpResponse::new(HttpVersion::default(), Box::new(&mut output_buffer));

            let test_data = b"Hello, this is a plain text message!";
            response.write(test_data).unwrap();
            response.flush().unwrap();
        } // response drop

        // 출력 버퍼에서 헤더와 바디 분리
        let output_str = String::from_utf8_lossy(&output_buffer);
        let parts: Vec<&str> = output_str.split("\r\n\r\n").collect();

        // Content-Encoding 헤더가 없어야 함
        assert!(!parts[0].contains("Content-Encoding"));

        // 바디가 그대로 있어야 함
        let body_start = parts[0].len() + 4;
        let body = &output_buffer[body_start..];

        assert_eq!(body, b"Hello, this is a plain text message!");
    }

    #[test]
    fn test_gzip_with_large_data() {
        // 큰 데이터로 gzip 압축 효율성 테스트
        let mut output_buffer = Vec::new();
        let test_data = "This is a repeating message! ".repeat(100);

        {
            let mut response =
                HttpResponse::new(HttpVersion::default(), Box::new(&mut output_buffer));

            response.set_header(content_encoding(HttpHeaderValue::Str("gzip")));

            // 반복되는 큰 데이터 (압축이 잘 될 것으로 예상)
            response.write(test_data.as_bytes()).unwrap();
            response.flush().unwrap();
        } // response drop

        // 압축된 크기가 원본보다 작은지 확인
        let output_str = String::from_utf8_lossy(&output_buffer);
        let parts: Vec<&str> = output_str.split("\r\n\r\n").collect();
        let body_start = parts[0].len() + 4;
        let compressed_size = output_buffer.len() - body_start;

        println!(
            "Original size: {}, Compressed size: {}",
            test_data.len(),
            compressed_size
        );
        assert!(compressed_size < test_data.len());

        // 디코딩하여 원본과 일치하는지 확인
        let compressed_body = &output_buffer[body_start..];
        let mut decoder = GzDecoder::new(compressed_body);
        let mut decoded = String::new();
        decoder.read_to_string(&mut decoded).unwrap();

        assert_eq!(decoded, test_data);
    }
}
