use std::{
    collections::HashMap,
    io::{BufWriter, IoSlice, Write},
    rc::Rc,
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
    buffer: Vec<Vec<u8>>,
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
        self.buffer.push(buf.to_vec());

        return Ok(buf.len());
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.set_header(content_length(
            self.buffer.iter().map(|b| b.len()).sum::<usize>(),
        ));
        self.set_header(date(SystemTime::now()));

        let mut buf_writer = BufWriter::with_capacity(1 << 15, &mut self.writer);

        let mut header_lines: Vec<String> = vec![];
        let status_line = format!("{} {}", self.version.clone(), self.code);
        header_lines.push(status_line);

        self.header.into_iter().for_each(|i| {
            let k = i.key_str();
            let v = i.value().to_string();
            header_lines.push(format!("{}: {}", k, v));
        });

        let header_slices: Vec<IoSlice<'_>> = header_lines
            .iter()
            .flat_map(|h| [IoSlice::new(h.as_bytes()), IoSlice::new(LINE_END)])
            .collect();
        let written = buf_writer.write_vectored(&header_slices)?;
        buf_writer.write(LINE_END)?;

        if self.header_only {
            buf_writer.flush()?;
            self.written = written;
            return Ok(());
        }

        let data: Vec<IoSlice<'_>> = self.buffer.iter().map(|b| IoSlice::new(&b)).collect();

        let mut writer: Box<dyn Write> = match self.header.get("Content-Encoding") {
            Some(v) if *v.to_string() == "gzip" => {
                Box::new(GzEncoder::new(buf_writer, Compression::default()))
            }
            _ => Box::new(buf_writer),
        };
        let body_written = writer.write_vectored(&data)?;

        writer.flush()?;
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

const LINE_END: &[u8] = "\r\n".as_bytes();
const KV_SEP: &[u8] = ": ".as_bytes();

impl HttpResponse<'_> {
    pub fn set_response_code(&mut self, code: HttpResponseCode) {
        self.code = code;
    }

    pub fn write_header(
        header: &HashMap<&'static str, Rc<dyn crate::http::header::ToString>>,
        header_str: &HashMap<Rc<String>, Rc<dyn crate::http::header::ToString>>,
        writer: &mut BufWriter<&mut Box<dyn Write + '_>>,
    ) -> std::io::Result<usize> {
        let mut written = 0;
        if !header.is_empty() {
            for (key, value) in header.clone().into_iter() {
                written += Self::write_header_value(
                    writer,
                    &key.as_bytes(),
                    value.to_string().as_bytes(),
                )?;
            }
        }

        if !header_str.is_empty() {
            for (key, value) in header_str.clone().into_iter() {
                written += Self::write_header_value(
                    writer,
                    &key.as_bytes(),
                    value.to_string().as_bytes(),
                )?;
            }
        }

        written += writer.write(LINE_END)?;

        return Ok(written);
    }

    fn write_header_value(
        writer: &mut BufWriter<&mut Box<dyn Write + '_>>,
        k: &[u8],
        v: &[u8],
    ) -> std::io::Result<usize> {
        let mut written = writer.write(k)?;
        written += writer.write(KV_SEP)?;
        written += writer.write(v)?;
        written += writer.write(LINE_END)?;

        return Ok(written);
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
