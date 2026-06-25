use std::{
    io::{BufRead, BufReader, Error, ErrorKind, Read},
    ops::Index,
};

struct Multipart {
    boundary: String,
}

impl Multipart {
    pub fn new(boundary: String) -> Self {
        return Multipart { boundary };
    }

    pub fn read_item_from_reader(
        &self,
        reader: &mut BufReader<dyn Read>,
    ) -> Result<Vec<MultipartPart>, Error> {
        let mut line = String::new();
        let mut result: Vec<MultipartPart> = Vec::new();

        loop {
            let readed = reader.read_line(&mut line)?;

            if line.starts_with(&self.boundary) {
                if readed == self.boundary.len() + 2 && line.ends_with("--\r\n") {
                    return Ok(result);
                }
                continue;
            }

            if line[0..19].eq_ignore_ascii_case("Content-Disposition") {
                let mut cd_value = line[20..readed].trim().split(';');
                let filename = cd_value.nth(2);
                let name = cd_value.nth(1);
                let data = cd_value.nth(0);

                if 

                if let Some()
            }
        }
    }
}

enum MultipartPart {
    End,
    Field(String, String),
    File(String, String, Box<dyn Read>),
}
